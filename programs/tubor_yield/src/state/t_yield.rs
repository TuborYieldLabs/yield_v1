use crate::program::Tuboryield;

use mpl_token_metadata::{
    instructions::{CreateV1CpiBuilder, MintV1CpiBuilder, TransferCpiBuilder},
    types::{
        Collection, Creator, PrintSupply, TokenStandard as MetaplexTokenStandard, TransferArgs,
    },
};

use {
    crate::{
        error::{ErrorCode, TYieldResult},
        instructions::{MintAgent, MintAgentParams, MintMasterAgent, MintMasterAgentParams},
        math::SafeMath,
        state::{OracleParams, Size},
        try_from,
    },
    anchor_lang::prelude::*,
};

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct Permissions {
    pub allow_agent_deploy: bool,
    pub allow_agent_buy: bool,
    pub allow_agent_sell: bool,
    pub allow_withdraw_yield: bool,
}

#[account]
#[derive(Default, PartialEq, Debug)]
pub struct TYield {
    // 8-byte aligned fields (largest first)
    pub oracle_param: OracleParams, // 109 bytes

    pub y_mint: Pubkey,
    /// PRECISION PERCENTAGE_PRECISION
    pub buy_tax: u64,
    /// PRECISION PERCENTAGE_PRECISION
    pub sell_tax: u64,
    /// PRECISION PERCENTAGE_PRECISION
    pub max_tax_percentage: u64,
    /// PRECISION PERCENTAGE_PRECISION
    pub ref_earn_percentage: u64,
    /// PRECISION PERCENTAGE_PRECISION
    pub max_agent_price_new: u64,

    /// PRECISION QUOTE_PRECISION
    pub protocol_current_holding: u64,

    /// PRECISION QUOTE_PRECISION
    pub protcol_total_fees: u64,

    /// PRECISION QUOTE_PRECISION
    pub protocol_total_earnings: u64,

    /// PRECISION QUOTE_PRECISION
    pub protocol_total_balance_usd: u64,

    // 4-byte aligned fields
    pub inception_time: i64, // 4 bytes

    // 1-byte aligned fields (smallest last)
    pub permissions: Permissions,    // 4 bytes
    pub transfer_authority_bump: u8, // 1 byte
    pub t_yield_bump: u8,            // 1 byte
    pub paused: bool,                // 1 byte - protocol paused flag
    // Padding for future-proofing and alignment
    pub _padding: [u8; 5], // 5 bytes to align to 8-byte boundary (was 6)
}

#[event]
pub struct InitProtocolEvent {
    pub inception_time: i64,
    pub paused: bool,
    pub permissions: Permissions,
}
#[event]
pub struct UpdateProtocolEvent {}

impl TYield {
    pub fn get_time(&self) -> TYieldResult<i64> {
        let clock = anchor_lang::solana_program::sysvar::clock::Clock::get()
            .map_err(|_| ErrorCode::MathError)?;
        let time = clock.unix_timestamp;
        if time > 0 {
            Ok(time)
        } else {
            Err(ErrorCode::MathError)
        }
    }

    pub fn get_user_pda(authority: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"user", authority.as_ref()], &crate::ID)
    }

    pub fn get_referral_registry_pda(referrer: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"referral_registry", referrer.as_ref()], &crate::ID)
    }

    pub fn validate_upgrade_authority(
        expected_upgrade_authority: Pubkey,
        program_data: &AccountInfo,
        program: &Program<Tuboryield>,
    ) -> Result<()> {
        if let Some(programdata_address) = program.programdata_address()? {
            require_keys_eq!(
                programdata_address,
                program_data.key(),
                ErrorCode::InvalidProgramExecutable
            );
            let program_data = try_from!(Account::<ProgramData>, program_data)?;
            if let Some(current_upgrade_authority) = program_data.upgrade_authority_address {
                if current_upgrade_authority != Pubkey::default() {
                    require_keys_eq!(
                        current_upgrade_authority,
                        expected_upgrade_authority,
                        ErrorCode::ConstraintOwner
                    );
                }
            }
        } // otherwise not upgradeable

        Ok(())
    }

    pub fn is_empty_account(account_info: &AccountInfo) -> TYieldResult<bool> {
        Ok(account_info.data_is_empty() || account_info.lamports() == 0)
    }

    pub fn close_token_account<'info>(
        receiver: AccountInfo<'info>,
        token_account: AccountInfo<'info>,
        token_program: AccountInfo<'info>,
        authority: AccountInfo<'info>,
        seeds: &[&[&[u8]]],
    ) -> Result<()> {
        let cpi_accounts = anchor_spl::token_2022::CloseAccount {
            account: token_account,
            destination: receiver,
            authority,
        };
        let cpi_context = anchor_lang::context::CpiContext::new(token_program, cpi_accounts);

        anchor_spl::token_2022::close_account(cpi_context.with_signer(seeds))
    }

    pub fn transfer_sol<'a>(
        source_account: AccountInfo<'a>,
        destination_account: AccountInfo<'a>,
        system_program: AccountInfo<'a>,
        amount: u64,
    ) -> Result<()> {
        let cpi_accounts = anchor_lang::system_program::Transfer {
            from: source_account,
            to: destination_account,
        };
        let cpi_context = anchor_lang::context::CpiContext::new(system_program, cpi_accounts);

        anchor_lang::system_program::transfer(cpi_context, amount)
    }

    pub fn transfer_sol_from_owned<'a>(
        program_owned_source_account: AccountInfo<'a>,
        destination_account: AccountInfo<'a>,
        amount: u64,
    ) -> TYieldResult<()> {
        **destination_account
            .try_borrow_mut_lamports()
            .map_err(|_| ErrorCode::InvalidAccount)? =
            destination_account.lamports().safe_add(amount)?;

        let source_balance = program_owned_source_account.lamports();
        **program_owned_source_account
            .try_borrow_mut_lamports()
            .map_err(|_| ErrorCode::InvalidAccount)? = source_balance.safe_sub(amount)?;

        Ok(())
    }

    pub fn burn_tokens<'info>(
        &self,
        mint: AccountInfo<'info>,
        from: AccountInfo<'info>,
        authority: AccountInfo<'info>,
        token_program: AccountInfo<'info>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        let cpi_accounts = anchor_spl::token_2022::BurnChecked {
            mint,
            from,
            authority,
        };
        let cpi_context = anchor_lang::context::CpiContext::new(token_program, cpi_accounts);

        anchor_spl::token_2022::burn_checked(cpi_context, amount, decimals)
    }

    pub fn transfer_tokens<'info>(
        from: AccountInfo<'info>,
        mint: AccountInfo<'info>,
        to: AccountInfo<'info>,
        authority: AccountInfo<'info>,
        token_program: AccountInfo<'info>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        let cpi_accounts = anchor_spl::token_2022::TransferChecked {
            from,
            mint,
            to,
            authority,
        };
        let cpi_context = anchor_lang::context::CpiContext::new(token_program, cpi_accounts);
        anchor_spl::token_2022::transfer_checked(cpi_context, amount, decimals)
    }

    pub fn mint_tokens<'info>(
        mint: AccountInfo<'info>,
        to: AccountInfo<'info>,
        authority: AccountInfo<'info>,
        token_program: AccountInfo<'info>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        let cpi_accounts = anchor_spl::token_2022::MintToChecked {
            mint,
            to,
            authority,
        };
        let cpi_context = anchor_lang::context::CpiContext::new(token_program, cpi_accounts);
        anchor_spl::token_2022::mint_to_checked(cpi_context, amount, decimals)
    }

    pub fn realloc<'info>(
        funding_account: AccountInfo<'info>,
        target_account: AccountInfo<'info>,
        system_program: AccountInfo<'info>,
        new_len: usize,
        zero_init: bool,
    ) -> Result<()> {
        let new_minimum_balance = Rent::get()?.minimum_balance(new_len);
        let lamports_diff = new_minimum_balance.safe_sub(target_account.try_lamports()?)?;

        TYield::transfer_sol(
            funding_account,
            target_account.clone(),
            system_program,
            lamports_diff,
        )?;

        target_account
            .realloc(new_len, zero_init)
            .map_err(|_| ProgramError::InvalidRealloc.into())
    }

    pub fn mint_master_agent(
        &self,
        ctx: &Context<MintMasterAgent>,
        params: MintMasterAgentParams,
    ) -> Result<()> {
        let authority_seeds: &[&[&[u8]]] =
            &[&[b"transfer_authority", &[self.transfer_authority_bump]]];

        let creators = vec![Creator {
            address: ctx.accounts.authority.key(),
            verified: true,
            share: 100,
        }];

        CreateV1CpiBuilder::new(&ctx.accounts.metadata_program)
            .metadata(&ctx.accounts.metadata)
            .mint(&ctx.accounts.mint.to_account_info(), true)
            .authority(&ctx.accounts.authority)
            .payer(&ctx.accounts.payer)
            .update_authority(&ctx.accounts.authority, true)
            .master_edition(Some(&ctx.accounts.master_edition))
            .name(params.name)
            .symbol(params.symbol)
            .uri(params.uri)
            .seller_fee_basis_points(params.seller_fee_basis_points)
            .creators(creators)
            .token_standard(MetaplexTokenStandard::NonFungible)
            .decimals(0)
            .print_supply(PrintSupply::Zero)
            .is_mutable(true)
            .system_program(&ctx.accounts.system_program)
            .spl_token_program(Some(&ctx.accounts.token_program))
            .sysvar_instructions(&ctx.accounts.sysvar_instructions)
            .invoke_signed(authority_seeds)?;

        MintV1CpiBuilder::new(&ctx.accounts.metadata_program)
            .token(&ctx.accounts.token_account.to_account_info())
            .token_owner(Some(&ctx.accounts.authority))
            .metadata(&ctx.accounts.metadata)
            .master_edition(Some(&ctx.accounts.master_edition))
            .mint(&ctx.accounts.mint.to_account_info())
            .authority(&ctx.accounts.authority)
            .payer(&ctx.accounts.payer)
            .amount(1)
            .system_program(&ctx.accounts.system_program)
            .spl_token_program(&ctx.accounts.token_program)
            .spl_ata_program(&ctx.accounts.associated_token_program)
            .sysvar_instructions(&ctx.accounts.sysvar_instructions)
            .invoke_signed(authority_seeds)?;

        Ok(())
    }

    pub fn mint_agent(&self, ctx: &Context<MintAgent>, params: MintAgentParams) -> Result<()> {
        let authority_seeds: &[&[&[u8]]] =
            &[&[b"transfer_authority", &[self.transfer_authority_bump]]];

        let creators = vec![Creator {
            address: ctx.accounts.authority.key(),
            verified: true,
            share: 100,
        }];

        CreateV1CpiBuilder::new(&ctx.accounts.metadata_program)
            .metadata(&ctx.accounts.metadata)
            .mint(&ctx.accounts.mint.to_account_info(), true)
            .authority(&ctx.accounts.authority)
            .payer(&ctx.accounts.payer)
            .update_authority(&ctx.accounts.authority, true)
            .master_edition(Some(&ctx.accounts.master_edition))
            .name(params.name)
            .symbol(params.symbol)
            .uri(params.uri)
            .seller_fee_basis_points(params.seller_fee_basis_points)
            .creators(creators)
            .token_standard(MetaplexTokenStandard::NonFungible)
            .print_supply(PrintSupply::Zero)
            .spl_token_program(Some(&ctx.accounts.token_program))
            .system_program(&ctx.accounts.system_program)
            .sysvar_instructions(&ctx.accounts.sysvar_instructions)
            .collection(Collection {
                key: ctx.accounts.master_agent_mint.key(),
                verified: false,
            })
            .decimals(0)
            .is_mutable(true)
            .invoke_signed(authority_seeds)?;

        // // Mint the NFT
        MintV1CpiBuilder::new(&ctx.accounts.metadata_program)
            .token(&ctx.accounts.token_account.to_account_info())
            .token_owner(Some(&ctx.accounts.authority))
            .master_edition(Some(&ctx.accounts.master_edition))
            .mint(&ctx.accounts.mint.to_account_info())
            .authority(&ctx.accounts.authority)
            .payer(&ctx.accounts.payer)
            .amount(1)
            .system_program(&ctx.accounts.system_program)
            .spl_token_program(&ctx.accounts.token_program)
            .spl_ata_program(&ctx.accounts.associated_token_program)
            .sysvar_instructions(&ctx.accounts.sysvar_instructions)
            .invoke_signed(authority_seeds)?;
        Ok(())
    }

    pub fn transfer_agent(&self, params: TransferAgentParams) -> Result<()> {
        let authority_seeds: &[&[&[u8]]] =
            &[&[b"transfer_authority", &[self.transfer_authority_bump]]];

        TransferCpiBuilder::new(&params.metadata_program)
            .token(&params.sender_nft_token_account)
            .token_owner(&params.authority)
            .destination_token(&params.receiver_token_account)
            .destination_owner(&params.receiver)
            .mint(&params.mint)
            .metadata(&params.metadata)
            .authority(&params.authority)
            .payer(&params.payer)
            .token_record(None)
            .destination_token_record(None)
            .authorization_rules_program(None)
            .authorization_rules(None)
            .system_program(&params.system_program)
            .spl_ata_program(&params.associated_token_program)
            .spl_token_program(&params.token_program)
            .sysvar_instructions(&params.sysvar_instructions)
            .transfer_args(TransferArgs::V1 {
                amount: 1,
                authorization_data: None,
            })
            .invoke_signed(authority_seeds)?;

        Ok(())
    }
}

pub struct TransferAgentParams<'info> {
    pub payer: AccountInfo<'info>,
    pub sender_nft_token_account: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
    pub receiver_token_account: AccountInfo<'info>,
    pub receiver: AccountInfo<'info>,
    pub mint: AccountInfo<'info>,
    pub metadata: AccountInfo<'info>,
    pub metadata_program: AccountInfo<'info>,
    pub system_program: AccountInfo<'info>,
    pub associated_token_program: AccountInfo<'info>,
    pub token_program: AccountInfo<'info>,
    pub sysvar_instructions: AccountInfo<'info>,
}

impl Size for TYield {
    const SIZE: usize = 216;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_t_yield_size() {
        // On-chain size includes 8 bytes for Anchor discriminator
        assert_eq!(8 + std::mem::size_of::<TYield>(), TYield::SIZE);
        println!("TYield on-chain size: {} bytes", TYield::SIZE);
    }

    #[test]
    fn test_t_yield_memory_layout() {
        // Test that TYield struct can be created and serialized
        let t_yield = TYield::default();
        assert_eq!(t_yield.y_mint, Pubkey::default());
        assert_eq!(t_yield.buy_tax, 0);
        assert_eq!(t_yield.sell_tax, 0);
        assert_eq!(t_yield.inception_time, 0);
        assert_eq!(t_yield.transfer_authority_bump, 0);
        assert_eq!(t_yield.t_yield_bump, 0);
        assert_eq!(t_yield.paused, false);
        assert_eq!(t_yield._padding, [0; 5]);
    }
}
