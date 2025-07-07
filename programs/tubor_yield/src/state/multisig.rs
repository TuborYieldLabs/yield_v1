use anchor_lang::{prelude::*, solana_program::hash::hashv};

use crate::{
    error::{ErrorCode, TYieldResult},
    math::{SafeMath, MAX_SIGNERS},
    msg,
    state::Size,
};

#[repr(C, packed)]
#[account(zero_copy)]
#[derive(Default, PartialEq)]
pub struct Multisig {
    // Large arrays first (32-byte alignment)
    pub signers: [Pubkey; 6], // Array of authorized signer public keys (192 bytes)

    // 64-bit fields
    pub instruction_hash: u64, // Hash of the instruction being signed

    // 16-bit fields
    pub instruction_data_len: u16, // Length of instruction data

    // 8-bit fields grouped together
    pub num_signers: u8,              // Total number of authorized signers
    pub num_signed: u8,               // Number of signatures collected so far
    pub min_signatures: u8,           // Minimum signatures required for execution
    pub bump: u8,                     // PDA bump seed
    pub instruction_accounts_len: u8, // Number of accounts in the instruction

    // Small arrays
    pub signed: [u8; 6], // Bitmap tracking which signers have signed (0/1)

    // Padding for future proofing
    pub _padding: [u8; 32], // Padding for future extensibility
}

pub enum AdminInstruction {
    CreateUser,
    DeployAgent,
    AddAgent,
    BanUser,
    PermManager,
    WithdrawFees,
}

impl Size for Multisig {
    const SIZE: usize = 8 + // discriminator (Anchor handles this)
                       32 * 6 + // signers (6 * Pubkey = 192 bytes)
                       8 + // instruction_hash (u64)
                       2 + // instruction_data_len (u16)
                       1 + // num_signers (u8)
                       1 + // num_signed (u8)
                       1 + // min_signatures (u8)
                       1 + // bump (u8)
                       1 + // instruction_accounts_len (u8)
                       6 + // signed (6 * u8)
                       32; // padding
}

impl Multisig {
    pub fn get_instruction_hash(
        instruction_accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> [u8; 32] {
        let mut data_to_hash = Vec::new();

        for account in instruction_accounts {
            data_to_hash.extend_from_slice(account.key.as_ref());
        }

        if !instruction_data.is_empty() {
            data_to_hash.extend_from_slice(instruction_data);
        }

        hashv(&[&data_to_hash]).to_bytes()
    }

    pub fn get_account_infos<'info, T: ToAccountInfos<'info> + anchor_lang::Bumps>(
        ctx: &Context<'_, '_, '_, 'info, T>,
    ) -> Vec<AccountInfo<'info>> {
        let mut infos = ctx.accounts.to_account_infos();
        infos.extend_from_slice(ctx.remaining_accounts);
        infos
    }

    pub fn get_instruction_data<T: AnchorSerialize>(
        instruction_type: AdminInstruction,
        params: &T,
    ) -> Result<Vec<u8>> {
        let mut res = vec![];
        AnchorSerialize::serialize(&params, &mut res)?;
        res.push(instruction_type as u8);
        Ok(res)
    }

    pub fn get_signer_index(&self, signer: &Pubkey) -> TYieldResult<usize> {
        for i in 0..self.num_signers as usize {
            if &self.signers[i] == signer {
                return Ok(i);
            }
        }
        Err(ErrorCode::MultisigAccountNotAuthorized)
    }

    pub fn set_signers(&mut self, admin_signers: &[AccountInfo], min_signatures: u8) -> Result<()> {
        if admin_signers.is_empty() || min_signatures == 0 {
            msg!("Error: At least one signer is required");
            return Err(ProgramError::MissingRequiredSignature.into());
        }
        if (min_signatures as usize) > admin_signers.len() {
            msg!(
                "Error: Number of min signatures ({}) exceeded number of signers ({})",
                min_signatures,
                admin_signers.len(),
            );
            return Err(ProgramError::InvalidArgument.into());
        }
        if admin_signers.len() > MAX_SIGNERS {
            msg!(
                "Error: Number of signers ({}) exceeded max ({})",
                admin_signers.len(),
                MAX_SIGNERS
            );
            return Err(ProgramError::InvalidArgument.into());
        }

        let mut signers: [Pubkey; MAX_SIGNERS] = Default::default();
        let mut signed: [u8; MAX_SIGNERS] = Default::default();

        for idx in 0..admin_signers.len() {
            if signers.contains(admin_signers[idx].key) {
                msg!("Error: Duplicate signer {}", admin_signers[idx].key);
                return Err(ProgramError::InvalidArgument.into());
            }
            signers[idx] = *admin_signers[idx].key;
            signed[idx] = 0;
        }

        *self = Multisig {
            num_signers: admin_signers.len() as u8,
            num_signed: 0,
            min_signatures,
            bump: self.bump,
            instruction_accounts_len: 0,
            instruction_data_len: 0,
            instruction_hash: 0,
            signers,
            signed,
            _padding: [0; 32],
        };

        Ok(())
    }

    pub fn sign_multisig(
        &mut self,
        signer_account: &AccountInfo,
        instruction_accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> TYieldResult<u8> {
        // return early if not a signer
        if !signer_account.is_signer {
            return Err(ErrorCode::MissingRequiredSignature);
        }

        // find index of current signer or return error if not found
        let signer_idx = if let Ok(idx) = self.get_signer_index(signer_account.key) {
            idx
        } else {
            return Err(ErrorCode::MultisigAccountNotAuthorized);
        };

        // if single signer return Ok to continue
        if self.num_signers <= 1 {
            return Ok(0);
        }

        let instruction_hash =
            Multisig::get_instruction_hash(instruction_accounts, instruction_data);
        if instruction_hash[..] != self.instruction_hash.to_le_bytes()
            || instruction_accounts.len() != self.instruction_accounts_len as usize
            || instruction_data.len() != self.instruction_data_len as usize
        {
            let bytes: [u8; 8] = instruction_hash
                .get(..8)
                .ok_or(ErrorCode::InvalidInstructionHash)?
                .try_into()
                .map_err(|_| ErrorCode::InvalidInstructionHash)?;

            // if this is a new instruction reset the data
            self.num_signed = 1;
            self.instruction_accounts_len = instruction_accounts.len() as u8;
            self.instruction_data_len = instruction_data.len() as u16;
            // self.instruction_hash = instruction_hash;

            self.instruction_hash = u64::from_le_bytes(bytes);
            self.signed.fill(0);
            self.signed[signer_idx] = 1;
            //multisig.pack(*multisig_account.try_borrow_mut_data()?)?;

            // Return remaining signatures needed (min_signatures - 1 since we just signed)
            let remaining = self.min_signatures.safe_sub(1)?;
            Ok(remaining)
        } else if self.signed[signer_idx] == 1 {
            Err(ErrorCode::MultisigAlreadySigned)
        } else if self.num_signed < self.min_signatures {
            // count the signature in
            self.num_signed = self.num_signed.safe_add(1)?;

            self.signed[signer_idx] = 1;

            if self.num_signed == self.min_signatures {
                Ok(0) // No more signatures needed
            } else {
                // Return remaining signatures needed
                let remaining = self.min_signatures.safe_sub(self.num_signed)?;
                Ok(remaining)
            }
        } else {
            Err(ErrorCode::MultisigAlreadyExecuted)
        }
    }

    pub fn unsign_multisig(&mut self, signer_account: &AccountInfo) -> Result<()> {
        // return early if not a signer
        if !signer_account.is_signer {
            return Err(ProgramError::MissingRequiredSignature.into());
        }

        // if single signer return
        if self.num_signers <= 1 || self.num_signed == 0 {
            return Ok(());
        }

        // find index of current signer or return error if not found
        let signer_idx = if let Ok(idx) = self.get_signer_index(signer_account.key) {
            idx
        } else {
            return err!(ErrorCode::MultisigAccountNotAuthorized);
        };

        // if not signed by this account return
        if self.signed[signer_idx] == 0 {
            return Ok(());
        }

        // remove signature
        self.num_signed = self.num_signed.safe_sub(1)?;
        self.signed[signer_idx] = 0;

        Ok(())
    }

    pub fn is_signer(&self, key: &Pubkey) -> Result<bool> {
        Ok(self.get_signer_index(key).is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multisig_size() {
        // On-chain size includes 8 bytes for Anchor discriminator
        assert_eq!(8 + std::mem::size_of::<Multisig>(), Multisig::SIZE);
        println!("Multisig on-chain size: {} bytes", Multisig::SIZE);
    }

    #[test]
    fn test_multisig_memory_layout() {
        // Test that Multisig struct can be created
        // Note: Cannot directly access packed struct fields due to alignment issues
        let _multisig = Multisig::default();
        // Just verify the struct can be created without panicking
        assert_eq!(std::mem::size_of::<Multisig>(), Multisig::SIZE - 8);
    }
}
