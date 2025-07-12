use anchor_lang::{prelude::*, solana_program::hash::hashv};

use crate::{
    error::{ErrorCode, TYieldResult},
    math::{SafeMath, MAX_SIGNERS},
    msg,
    state::Size,
};

#[repr(C, packed)]
#[account(zero_copy)]
#[derive(Default, PartialEq, Debug)]
pub struct Multisig {
    // Large arrays first (32-byte alignment)
    pub signers: [Pubkey; MAX_SIGNERS], // Array of authorized signer public keys (192 bytes)

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
    pub signed: [u8; MAX_SIGNERS], // Bitmap tracking which signers have signed (0/1)

    // Padding for future proofing
    pub _padding: [u8; 3], // Padding for future extensibility
}

pub enum AdminInstruction {
    UpdatePrice,
    DeployAgent,
    AddAgent,
    BanUser,
    PermManager,
    WithdrawFees,
    OpenTrade,
}

impl Size for Multisig {
    const SIZE: usize = 224;
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
            _padding: [0; 3],
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
        if instruction_hash[..8] != self.instruction_hash.to_le_bytes()
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

    // Helper function to create mock account infos
    fn create_mock_account_info(key: Pubkey, is_signer: bool) -> AccountInfo<'static> {
        // Each call gets its own heap allocations to avoid test state leakage
        let key_ref = Box::leak(Box::new(key));
        let lamports = Box::leak(Box::new(0u64));
        let data = Box::leak(Box::new(Vec::<u8>::new()));
        let owner = Box::leak(Box::new(Pubkey::default()));
        AccountInfo::new(key_ref, is_signer, false, lamports, data, owner, false, 0)
    }

    #[test]
    fn test_multisig_size() {
        // On-chain size includes 8 bytes for Anchor discriminator
        assert_eq!(8 + std::mem::size_of::<Multisig>(), Multisig::SIZE);
        println!("Multisig on-chain size: {} bytes", Multisig::SIZE);

        // Verify the manual calculation matches the actual size
        let expected_size = 8 + // discriminator
                           32 * MAX_SIGNERS + // signers
                           8 + // instruction_hash
                           2 + // instruction_data_len
                           1 + // num_signers
                           1 + // num_signed
                           1 + // min_signatures
                           1 + // bump
                           1 + // instruction_accounts_len
                           MAX_SIGNERS + // signed
                           3; // padding
        assert_eq!(expected_size, Multisig::SIZE);
    }

    #[test]
    fn test_multisig_memory_layout() {
        // Test that Multisig struct can be created and serialized
        let _multisig = Multisig::default();
        assert_eq!(Multisig::SIZE, 224);
        println!("Multisig on-chain size: {} bytes", Multisig::SIZE);
    }

    #[test]
    fn test_get_instruction_hash() {
        let key1 = Pubkey::new_unique();
        let key2 = Pubkey::new_unique();
        let accounts = vec![
            create_mock_account_info(key1, false),
            create_mock_account_info(key2, false),
        ];
        let data = vec![1, 2, 3, 4];

        let hash = Multisig::get_instruction_hash(&accounts, &data);
        assert_eq!(hash.len(), 32);

        // Same inputs should produce same hash
        let hash2 = Multisig::get_instruction_hash(&accounts, &data);
        assert_eq!(hash, hash2);

        // Different data should produce different hash
        let data2 = vec![1, 2, 3, 5];
        let hash3 = Multisig::get_instruction_hash(&accounts, &data2);
        assert_ne!(hash, hash3);
    }

    #[test]
    fn test_get_instruction_hash_empty_data() {
        let key1 = Pubkey::new_unique();
        let accounts = vec![create_mock_account_info(key1, false)];
        let data = vec![];

        let hash = Multisig::get_instruction_hash(&accounts, &data);
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_get_signer_index() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let signer2 = Pubkey::new_unique();
        let signer3 = Pubkey::new_unique();

        // Set up multisig with 3 signers
        let accounts = vec![
            create_mock_account_info(signer1, true),
            create_mock_account_info(signer2, true),
            create_mock_account_info(signer3, true),
        ];
        multisig.set_signers(&accounts, 2).unwrap();

        // Test finding existing signers
        assert_eq!(multisig.get_signer_index(&signer1).unwrap(), 0);
        assert_eq!(multisig.get_signer_index(&signer2).unwrap(), 1);
        assert_eq!(multisig.get_signer_index(&signer3).unwrap(), 2);

        // Test finding non-existent signer
        let non_signer = Pubkey::new_unique();
        assert!(multisig.get_signer_index(&non_signer).is_err());
    }

    #[test]
    fn test_set_signers() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let signer2 = Pubkey::new_unique();

        let accounts = vec![
            create_mock_account_info(signer1, true),
            create_mock_account_info(signer2, true),
        ];

        // Test successful setup
        let result = multisig.set_signers(&accounts, 2);
        assert!(result.is_ok());
        assert_eq!(multisig.num_signers, 2);
        assert_eq!(multisig.min_signatures, 2);
        assert_eq!(multisig.num_signed, 0);
        assert_eq!(multisig.signers[0], signer1);
        assert_eq!(multisig.signers[1], signer2);
    }

    #[test]
    fn test_set_signers_empty_signers() {
        let mut multisig = Multisig::default();
        let accounts = vec![];

        let result = multisig.set_signers(&accounts, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_signers_zero_min_signatures() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let accounts = vec![create_mock_account_info(signer1, true)];

        let result = multisig.set_signers(&accounts, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_signers_min_signatures_exceeds_signers() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let accounts = vec![create_mock_account_info(signer1, true)];

        let result = multisig.set_signers(&accounts, 2);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_signers_duplicate_signers() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let accounts = vec![
            create_mock_account_info(signer1, true),
            create_mock_account_info(signer1, true), // Duplicate
        ];

        let result = multisig.set_signers(&accounts, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_sign_multisig_single_signer() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let accounts = vec![create_mock_account_info(signer1, true)];

        multisig.set_signers(&accounts, 1).unwrap();

        let signer_account = create_mock_account_info(signer1, true);
        let instruction_accounts = vec![];
        let instruction_data = vec![1, 2, 3];

        let result =
            multisig.sign_multisig(&signer_account, &instruction_accounts, &instruction_data);
        assert_eq!(result.unwrap(), 0); // No more signatures needed
    }

    #[test]
    fn test_sign_multisig_not_signer() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let accounts = vec![create_mock_account_info(signer1, true)];

        multisig.set_signers(&accounts, 1).unwrap();

        let non_signer = Pubkey::new_unique();
        let signer_account = create_mock_account_info(non_signer, true);
        let instruction_accounts = vec![];
        let instruction_data = vec![1, 2, 3];

        let result =
            multisig.sign_multisig(&signer_account, &instruction_accounts, &instruction_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_sign_multisig_not_signed() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let signer2 = Pubkey::new_unique();
        let accounts = vec![
            create_mock_account_info(signer1, true),
            create_mock_account_info(signer2, true),
        ];

        multisig.set_signers(&accounts, 2).unwrap();

        let signer_account = create_mock_account_info(signer1, false); // Not signed
        let instruction_accounts = vec![];
        let instruction_data = vec![1, 2, 3];

        let result =
            multisig.sign_multisig(&signer_account, &instruction_accounts, &instruction_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_sign_multisig_multi_signer_flow() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let signer2 = Pubkey::new_unique();
        let signer3 = Pubkey::new_unique();

        // Store AccountInfo objects in a Vec<AccountInfo>
        let accounts = vec![
            create_mock_account_info(signer1, true),
            create_mock_account_info(signer2, true),
            create_mock_account_info(signer3, true),
        ];

        multisig.set_signers(&accounts, 2).unwrap();

        let instruction_accounts: Vec<AccountInfo> = vec![];
        let instruction_data = vec![1, 2, 3];

        // First signature
        let result = multisig.sign_multisig(&accounts[0], &instruction_accounts, &instruction_data);
        assert_eq!(result.unwrap(), 1); // 1 more signature needed

        // Second signature
        let result = multisig.sign_multisig(&accounts[1], &instruction_accounts, &instruction_data);
        assert_eq!(result.unwrap(), 0); // No more signatures needed

        // Third signature (should fail, already executed)
        let result = multisig.sign_multisig(&accounts[2], &instruction_accounts, &instruction_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_sign_multisig_already_signed() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let accounts = vec![create_mock_account_info(signer1, true)];

        multisig.set_signers(&accounts, 1).unwrap();

        let signer_account = create_mock_account_info(signer1, true);
        let instruction_accounts = vec![];
        let instruction_data = vec![1, 2, 3];

        // First signature
        let result =
            multisig.sign_multisig(&signer_account, &instruction_accounts, &instruction_data);
        assert_eq!(result.unwrap(), 0);

        // Try to sign again (should reset because single signer always resets)
        let result =
            multisig.sign_multisig(&signer_account, &instruction_accounts, &instruction_data);
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_unsign_multisig() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let signer2 = Pubkey::new_unique();
        let accounts = vec![
            create_mock_account_info(signer1, true),
            create_mock_account_info(signer2, true),
        ];

        multisig.set_signers(&accounts, 2).unwrap();

        let signer1_account = create_mock_account_info(signer1, true);
        let signer2_account = create_mock_account_info(signer2, true);
        let instruction_accounts = vec![];
        let instruction_data = vec![1, 2, 3];

        // Sign with both signers
        multisig
            .sign_multisig(&signer1_account, &instruction_accounts, &instruction_data)
            .unwrap();
        multisig
            .sign_multisig(&signer2_account, &instruction_accounts, &instruction_data)
            .unwrap();

        // Unsign one (should not decrement after execution)
        let result = multisig.unsign_multisig(&signer1_account);
        assert!(result.is_ok());
        // Only the bitmap should change
        assert_eq!(multisig.signed[0], 0);
        assert_eq!(multisig.signed[1], 1);
    }

    #[test]
    fn test_unsign_multisig_not_signer() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let accounts = vec![create_mock_account_info(signer1, true)];

        multisig.set_signers(&accounts, 1).unwrap();

        let non_signer = Pubkey::new_unique();
        let non_signer_account = create_mock_account_info(non_signer, false);
        let result = multisig.unsign_multisig(&non_signer_account);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_signer() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let signer2 = Pubkey::new_unique();
        let accounts = vec![
            create_mock_account_info(signer1, true),
            create_mock_account_info(signer2, true),
        ];

        multisig.set_signers(&accounts, 2).unwrap();

        assert!(multisig.is_signer(&signer1).unwrap());
        assert!(multisig.is_signer(&signer2).unwrap());

        let non_signer = Pubkey::new_unique();
        assert!(!multisig.is_signer(&non_signer).unwrap());
    }

    #[test]
    fn test_get_instruction_data() {
        #[derive(AnchorSerialize)]
        struct TestParams {
            value: u64,
            flag: bool,
        }

        let params = TestParams {
            value: 12345,
            flag: true,
        };

        let result = Multisig::get_instruction_data(AdminInstruction::UpdatePrice, &params);
        assert!(result.is_ok());

        let data = result.unwrap();
        assert!(!data.is_empty());

        // The last byte should be the instruction type
        assert_eq!(data[data.len() - 1], AdminInstruction::UpdatePrice as u8);
    }

    #[test]
    fn test_admin_instruction_values() {
        // Test that all AdminInstruction variants have unique values
        let mut values = std::collections::HashSet::new();

        values.insert(AdminInstruction::UpdatePrice as u8);
        values.insert(AdminInstruction::DeployAgent as u8);
        values.insert(AdminInstruction::AddAgent as u8);
        values.insert(AdminInstruction::BanUser as u8);
        values.insert(AdminInstruction::PermManager as u8);
        values.insert(AdminInstruction::WithdrawFees as u8);
        values.insert(AdminInstruction::OpenTrade as u8);

        // All values should be unique
        assert_eq!(values.len(), 7);
    }

    #[test]
    fn test_multisig_default_state() {
        let multisig = Multisig::default();

        // Test that default creates a valid multisig
        assert_eq!(multisig.num_signers, 0);
        assert_eq!(multisig.num_signed, 0);
        assert_eq!(multisig.min_signatures, 0);
        assert_eq!(multisig.bump, 0);
        assert_eq!(multisig.instruction_accounts_len, 0);

        // Check that arrays are properly initialized
        for i in 0..MAX_SIGNERS {
            assert_eq!(multisig.signers[i], Pubkey::default());
            assert_eq!(multisig.signed[i], 0);
        }
    }

    #[test]
    fn test_multisig_partial_eq() {
        let mut multisig1 = Multisig::default();
        let mut multisig2 = Multisig::default();

        // Should be equal when both are default
        assert_eq!(multisig1, multisig2);

        // Should not be equal after modification
        multisig1.num_signers = 1;
        assert_ne!(multisig1, multisig2);

        // Should be equal again after same modification
        multisig2.num_signers = 1;
        assert_eq!(multisig1, multisig2);
    }
}
