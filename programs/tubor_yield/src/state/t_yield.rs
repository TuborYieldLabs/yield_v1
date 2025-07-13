//! Tubor Yield Protocol State and Security Controls
//!
//! This module defines the core on-chain state for the Tubor Yield protocol, including the
//! `TYield` account, security controls, and utility methods for protocol operations.
//!
//! # Features
//!
//! - **Protocol State**: The `TYield` account holds global protocol parameters, tax rates, balances, and security flags.
//! - **Security Controls**: Circuit breaker, rate limiter, and parameter bounds for robust protocol safety.
//! - **Permission System**: Fine-grained permissions for agent deployment, trading, and withdrawals.
//! - **Token/NFT Operations**: Utility methods for minting, transferring, and burning protocol tokens and NFTs.
//! - **Emergency Controls**: Emergency pause and circuit breaker for rapid response to critical events.
//!
//! # Example
//!
//! ```rust
//! use tubor_yield::state::t_yield::TYield;
//! let mut t_yield = TYield::default();
//! t_yield.paused = true; // Emergency pause
//! ```
//!
//! # Security
//!
//! This module implements comprehensive security validation, including:
//! - Parameter validation (tax, protocol balance)
//! - Rate limiting for critical updates
//! - Circuit breaker for emergency shutdown
//! - Emergency pause functionality
//!
//! All methods return `TYieldResult` for robust error handling.

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

/// Permissions for protocol operations.
///
/// Controls which actions are allowed for agents and users.
#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct Permissions {
    /// Allow deploying new agents
    pub allow_agent_deploy: bool,
    /// Allow buying agents
    pub allow_agent_buy: bool,
    /// Allow selling agents
    pub allow_agent_sell: bool,
    /// Allow withdrawing protocol yield
    pub allow_withdraw_yield: bool,
}

/// Circuit breaker for emergency protocol controls.
///
/// Used to halt trading and protocol operations in critical situations.
#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct CircuitBreaker {
    /// Whether the circuit breaker is currently triggered
    pub is_triggered: bool, // 1 byte
    /// Timestamp when the circuit breaker was triggered
    pub trigger_time: i64, // 8 bytes
    /// Reason code for the trigger
    pub trigger_reason: u8, // 1 byte
    /// Price threshold that triggered the breaker
    pub price_threshold: u64, // 8 bytes
    /// Volume threshold that triggered the breaker
    pub volume_threshold: u64, // 8 bytes
    /// Cooldown period (seconds) before protocol can resume
    pub cooldown_period_sec: u32, // 4 bytes
    /// Reserved for future use
    pub _padding: [u8; 6], // 6 bytes padding
}

/// Rate limiter for critical parameter updates.
///
/// Prevents excessive or rapid changes to protocol parameters.
#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct RateLimiter {
    /// Last time a critical update occurred
    pub last_update_time: i64, // 8 bytes
    /// Minimum interval (seconds) between updates
    pub min_interval_sec: u32, // 4 bytes
    /// Maximum number of updates allowed per day
    pub max_updates_per_day: u32, // 4 bytes
    /// Number of updates performed today
    pub daily_update_count: u32, // 4 bytes
    /// Start of the last reset day (unix timestamp)
    pub last_reset_day: i64, // 8 bytes
    /// Reserved for future use
    pub _padding: [u8; 4], // 4 bytes padding
}

/// Parameter bounds for protocol safety.
///
/// Defines maximum and minimum values for critical protocol parameters.
#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct ParameterBounds {
    /// Maximum allowed tax percentage (basis points, e.g. 1000 = 10%)
    pub max_tax_percentage: u64, // 8 bytes
    /// Maximum allowed price deviation
    pub max_price_deviation: u64, // 8 bytes
    /// Maximum allowed protocol balance
    pub max_protocol_balance: u64, // 8 bytes
    /// Minimum interval (seconds) between parameter updates
    pub min_update_interval: u32, // 4 bytes
    /// Reserved for future use
    pub _padding: [u8; 4], // 4 bytes padding
}

/// Global protocol state for Tubor Yield.
///
/// The `TYield` account stores all global parameters, balances, and security controls for the protocol.
///
/// # Fields
/// - `oracle_param`: Oracle configuration and price feed parameters.
/// - `y_mint`: The protocol's yield token mint address.
/// - `buy_tax`, `sell_tax`, `max_tax_percentage`, `ref_earn_percentage`, `max_agent_price_new`: Tax and referral parameters.
/// - `protocol_current_holding`, `protocol_total_fees`, `protocol_total_earnings`, `protocol_total_balance_usd`: Protocol financials.
/// - `inception_time`: Protocol start timestamp.
/// - `permissions`: Fine-grained permissions for protocol actions.
/// - `paused`: Emergency pause flag.
/// - `circuit_breaker`: Emergency circuit breaker for halting protocol operations.
/// - `rate_limiter`: Rate limiting for critical parameter updates.
/// - `parameter_bounds`: Bounds for safe parameter values.
/// - `_padding`: Reserved for future upgrades and alignment.
///
/// # Security
/// This struct is designed for robust protocol safety, with built-in validation, emergency controls, and upgradeability.
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
    pub protocol_total_fees: u64,

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

    // CRITICAL FIX: Add security controls
    pub circuit_breaker: CircuitBreaker,   // 36 bytes
    pub rate_limiter: RateLimiter,         // 32 bytes
    pub parameter_bounds: ParameterBounds, // 32 bytes

    // Padding for future-proofing and alignment
    pub _padding: [u8; 3], // 3 bytes to align to 8-byte boundary
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
    /// Validate that buy and sell tax parameters are within allowed bounds.
    ///
    /// # Arguments
    /// * `buy_tax` - Proposed buy tax (basis points)
    /// * `sell_tax` - Proposed sell tax (basis points)
    ///
    /// # Returns
    /// * `Ok(())` if valid, `Err(ErrorCode)` if out of bounds
    pub fn validate_tax_parameters(&self, buy_tax: u64, sell_tax: u64) -> TYieldResult<()> {
        // Check against maximum tax percentage
        if buy_tax > self.parameter_bounds.max_tax_percentage {
            msg!(
                "Buy tax {} exceeds maximum {}",
                buy_tax,
                self.parameter_bounds.max_tax_percentage
            );
            return Err(ErrorCode::MathError);
        }
        if sell_tax > self.parameter_bounds.max_tax_percentage {
            msg!(
                "Sell tax {} exceeds maximum {}",
                sell_tax,
                self.parameter_bounds.max_tax_percentage
            );
            return Err(ErrorCode::MathError);
        }

        // Check absolute maximum (100%)
        if buy_tax > 10000 || sell_tax > 10000 {
            msg!("Tax percentage cannot exceed 100%");
            return Err(ErrorCode::MathError);
        }

        Ok(())
    }

    pub fn validate_protocol_balance(&self, new_balance: u64) -> TYieldResult<()> {
        if new_balance > self.parameter_bounds.max_protocol_balance {
            msg!(
                "Protocol balance {} exceeds maximum {}",
                new_balance,
                self.parameter_bounds.max_protocol_balance
            );
            return Err(ErrorCode::MathError);
        }
        Ok(())
    }

    pub fn update_protocol_balance(
        &mut self,
        new_balance: u64,
        current_time: i64,
    ) -> TYieldResult<()> {
        self.validate_protocol_balance(new_balance)?;
        self.protocol_total_balance_usd = new_balance;
        self.rate_limiter.last_update_time = current_time;
        Ok(())
    }

    pub fn update_protocol_fees(&mut self, new_fees: u64, current_time: i64) -> TYieldResult<()> {
        // Validate that fees don't exceed protocol balance
        if new_fees > self.protocol_total_balance_usd {
            msg!(
                "Protocol fees {} cannot exceed balance {}",
                new_fees,
                self.protocol_total_balance_usd
            );
            return Err(ErrorCode::MathError);
        }
        self.protocol_total_fees = new_fees;
        self.rate_limiter.last_update_time = current_time;
        Ok(())
    }

    pub fn check_rate_limit(&self, current_time: i64) -> TYieldResult<()> {
        // Check minimum interval between updates
        let time_since_last = current_time.safe_sub(self.rate_limiter.last_update_time)?;
        if time_since_last < self.rate_limiter.min_interval_sec as i64 {
            msg!("Rate limit: too soon since last update");
            return Err(ErrorCode::RateLimitExceeded);
        }

        // Check daily update limit
        let current_day = current_time - (current_time % 86400); // Start of day
        if current_day > self.rate_limiter.last_reset_day {
            // Reset daily counter
            return Ok(());
        }

        if self.rate_limiter.daily_update_count >= self.rate_limiter.max_updates_per_day {
            msg!("Rate limit: daily update limit exceeded");
            return Err(ErrorCode::RateLimitExceeded);
        }

        Ok(())
    }

    pub fn check_circuit_breaker(&self, current_time: i64) -> TYieldResult<()> {
        if self.circuit_breaker.is_triggered {
            let time_since_trigger = current_time.safe_sub(self.circuit_breaker.trigger_time)?;
            if time_since_trigger < self.circuit_breaker.cooldown_period_sec as i64 {
                msg!("Circuit breaker active: trading suspended");
                return Err(ErrorCode::CircuitBreakerTriggered);
            }
        }
        Ok(())
    }

    pub fn trigger_circuit_breaker(&mut self, reason: u8, current_time: i64) -> TYieldResult<()> {
        self.circuit_breaker.is_triggered = true;
        self.circuit_breaker.trigger_time = current_time;
        self.circuit_breaker.trigger_reason = reason;
        msg!("Circuit breaker triggered: reason {}", reason);
        Ok(())
    }

    pub fn reset_circuit_breaker(&mut self) -> TYieldResult<()> {
        self.circuit_breaker.is_triggered = false;
        self.circuit_breaker.trigger_time = 0;
        self.circuit_breaker.trigger_reason = 0;
        msg!("Circuit breaker reset");
        Ok(())
    }

    /// Comprehensive security validation of the protocol state.
    ///
    /// # Arguments
    /// * `current_time` - The current Unix timestamp.
    ///
    /// # Returns
    /// * `Ok(())` if all checks pass, `Err(ErrorCode)` if any check fails.
    pub fn validate_security_state(&self, current_time: i64) -> TYieldResult<()> {
        // Check if protocol is paused
        if self.paused {
            msg!("Protocol is paused");
            return Err(ErrorCode::CannotPerformAction);
        }

        // Check circuit breaker
        self.check_circuit_breaker(current_time)?;

        // Validate tax parameters
        self.validate_tax_parameters(self.buy_tax, self.sell_tax)?;

        // Validate protocol balance
        self.validate_protocol_balance(self.protocol_total_balance_usd)?;

        // Check rate limiting
        self.check_rate_limit(current_time)?;

        Ok(())
    }

    /// Emergency pause functionality.
    ///
    /// Halts all protocol operations and sets the circuit breaker.
    ///
    /// # Arguments
    /// * `current_time` - The current Unix timestamp.
    ///
    /// # Returns
    /// * `Ok(())` on success, `Err(ErrorCode)` on failure.
    pub fn emergency_pause(&mut self, current_time: i64) -> TYieldResult<()> {
        self.paused = true;
        self.circuit_breaker.is_triggered = true;
        self.circuit_breaker.trigger_time = current_time;
        self.circuit_breaker.trigger_reason = 255; // Emergency pause reason
        msg!("EMERGENCY PAUSE ACTIVATED");
        Ok(())
    }

    /// Retrieves the current Unix timestamp.
    ///
    /// # Returns
    /// * `Ok(i64)` on success, `Err(ErrorCode)` on failure.
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

    /// Retrieves the PDA (Program Derived Address) for a user.
    ///
    /// # Arguments
    /// * `authority` - The authority's Pubkey.
    ///
    /// # Returns
    /// * `(Pubkey, u8)` - The PDA and bump.
    pub fn get_user_pda(authority: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"user", authority.as_ref()], &crate::ID)
    }

    /// Retrieves the PDA (Program Derived Address) for the referral registry.
    ///
    /// # Arguments
    /// * `referrer` - The referrer's Pubkey.
    ///
    /// # Returns
    /// * `(Pubkey, u8)` - The PDA and bump.
    pub fn get_referral_registry_pda(referrer: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"referral_registry", referrer.as_ref()], &crate::ID)
    }

    /// Validates the upgrade authority of the program data account.
    ///
    /// # Arguments
    /// * `expected_upgrade_authority` - The expected upgrade authority Pubkey.
    /// * `program_data` - The program data account.
    /// * `program` - The program instance.
    ///
    /// # Returns
    /// * `Result<()>` - Ok if valid, Err if invalid.
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

    /// Checks if an account is empty or has zero lamports.
    ///
    /// # Arguments
    /// * `account_info` - The account to check.
    ///
    /// # Returns
    /// * `TYieldResult<bool>` - Ok(true) if empty/zero, Ok(false) otherwise.
    pub fn is_empty_account(account_info: &AccountInfo) -> TYieldResult<bool> {
        Ok(account_info.data_is_empty() || account_info.lamports() == 0)
    }

    /// Closes a token account.
    ///
    /// # Arguments
    /// * `receiver` - The account to receive the token.
    /// * `token_account` - The account to close.
    /// * `token_program` - The token program.
    /// * `authority` - The authority to close the account.
    /// * `seeds` - The seeds for the authority.
    ///
    /// # Returns
    /// * `Result<()>` - Ok on success, Err on failure.
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

    /// Transfers SOL from a source account to a destination account.
    ///
    /// # Arguments
    /// * `source_account` - The account to transfer from.
    /// * `destination_account` - The account to transfer to.
    /// * `system_program` - The system program.
    /// * `amount` - The amount of SOL to transfer.
    ///
    /// # Returns
    /// * `Result<()>` - Ok on success, Err on failure.
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

    /// Transfers SOL from a program-owned source account to a destination account.
    ///
    /// # Arguments
    /// * `program_owned_source_account` - The account owned by the program.
    /// * `destination_account` - The account to transfer to.
    /// * `amount` - The amount of SOL to transfer.
    ///
    /// # Returns
    /// * `TYieldResult<()>` - Ok on success, Err on failure.
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

    /// Burns tokens from an account.
    ///
    /// # Arguments
    /// * `mint` - The mint account.
    /// * `from` - The account to burn from.
    /// * `authority` - The authority to burn.
    /// * `token_program` - The token program.
    /// * `amount` - The amount of tokens to burn.
    /// * `decimals` - The number of decimals for the mint.
    ///
    /// # Returns
    /// * `Result<()>` - Ok on success, Err on failure.
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

    /// Transfers tokens from one account to another.
    ///
    /// # Arguments
    /// * `from` - The account to transfer from.
    /// * `mint` - The mint account.
    /// * `to` - The account to transfer to.
    /// * `authority` - The authority to transfer.
    /// * `token_program` - The token program.
    /// * `amount` - The amount of tokens to transfer.
    /// * `decimals` - The number of decimals for the mint.
    ///
    /// # Returns
    /// * `Result<()>` - Ok on success, Err on failure.
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

    /// Mints tokens to an account.
    ///
    /// # Arguments
    /// * `mint` - The mint account.
    /// * `to` - The account to mint to.
    /// * `authority` - The authority to mint.
    /// * `token_program` - The token program.
    /// * `amount` - The amount of tokens to mint.
    /// * `decimals` - The number of decimals for the mint.
    ///
    /// # Returns
    /// * `Result<()>` - Ok on success, Err on failure.
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

    /// Reallocates an account's data.
    ///
    /// # Arguments
    /// * `funding_account` - The account to fund the reallocation.
    /// * `target_account` - The account to reallocate.
    /// * `system_program` - The system program.
    /// * `new_len` - The new length of the account.
    /// * `zero_init` - Whether to zero-initialize the new space.
    ///
    /// # Returns
    /// * `Result<()>` - Ok on success, Err on failure.
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

    /// Mints a new master agent NFT.
    ///
    /// # Arguments
    /// * `ctx` - The context for the mint operation.
    /// * `params` - The parameters for minting the master agent.
    ///
    /// # Returns
    /// * `Result<()>` - Ok on success, Err on failure.
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

    /// Mints a new agent NFT.
    ///
    /// # Arguments
    /// * `ctx` - The context for the mint operation.
    /// * `params` - The parameters for minting the agent.
    ///
    /// # Returns
    /// * `Result<()>` - Ok on success, Err on failure.
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

    /// Transfers an agent NFT from one account to another.
    ///
    /// # Arguments
    /// * `params` - The parameters for the transfer operation.
    ///
    /// # Returns
    /// * `Result<()>` - Ok on success, Err on failure.
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

/// Parameters for transferring an agent NFT.
///
/// Contains all accounts and programs required for a secure agent transfer.
pub struct TransferAgentParams<'info> {
    /// The payer for the transaction
    pub payer: AccountInfo<'info>,
    /// The sender's NFT token account
    pub sender_nft_token_account: AccountInfo<'info>,
    /// The authority (owner) of the NFT
    pub authority: AccountInfo<'info>,
    /// The receiver's token account
    pub receiver_token_account: AccountInfo<'info>,
    /// The receiver's main account
    pub receiver: AccountInfo<'info>,
    /// The NFT mint account
    pub mint: AccountInfo<'info>,
    /// The NFT metadata account
    pub metadata: AccountInfo<'info>,
    /// The metadata program
    pub metadata_program: AccountInfo<'info>,
    /// The system program
    pub system_program: AccountInfo<'info>,
    /// The associated token program
    pub associated_token_program: AccountInfo<'info>,
    /// The token program
    pub token_program: AccountInfo<'info>,
    /// The sysvar instructions account
    pub sysvar_instructions: AccountInfo<'info>,
}

/// Implements the Size trait for TYield, specifying the on-chain account size.
impl Size for TYield {
    const SIZE: usize = 328;
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
        let t_yield = TYield::default();
        assert_eq!(t_yield.y_mint, Pubkey::default());
        assert_eq!(t_yield.buy_tax, 0);
        assert_eq!(t_yield.sell_tax, 0);
        assert_eq!(t_yield.inception_time, 0);
        assert_eq!(t_yield.transfer_authority_bump, 0);
        assert_eq!(t_yield.t_yield_bump, 0);
        assert_eq!(t_yield.paused, false);
        assert_eq!(t_yield._padding, [0; 3]);
        assert_eq!(t_yield.circuit_breaker.is_triggered, false);
        assert_eq!(t_yield.rate_limiter.last_update_time, 0);
        assert_eq!(t_yield.parameter_bounds.max_tax_percentage, 0);
    }

    #[test]
    fn test_validate_tax_parameters() {
        let mut t_yield = TYield::default();
        t_yield.parameter_bounds.max_tax_percentage = 1000; // 10%
        assert!(t_yield.validate_tax_parameters(500, 800).is_ok());
        assert!(t_yield.validate_tax_parameters(1500, 800).is_err());
        assert!(t_yield.validate_tax_parameters(500, 1200).is_err());
        assert!(t_yield.validate_tax_parameters(10001, 0).is_err());
    }

    #[test]
    fn test_validate_protocol_balance() {
        let mut t_yield = TYield::default();
        t_yield.parameter_bounds.max_protocol_balance = 1_000_000;
        assert!(t_yield.validate_protocol_balance(500_000).is_ok());
        assert!(t_yield.validate_protocol_balance(1_000_001).is_err());
    }

    #[test]
    fn test_rate_limiter() {
        let mut t_yield = TYield::default();
        t_yield.rate_limiter.last_update_time = 100;
        t_yield.rate_limiter.min_interval_sec = 10;
        t_yield.rate_limiter.max_updates_per_day = 2;
        t_yield.rate_limiter.daily_update_count = 1;
        t_yield.rate_limiter.last_reset_day = 0;
        // Should fail if too soon
        assert!(t_yield.check_rate_limit(105).is_err());
        // Should pass if enough time has passed
        assert!(t_yield.check_rate_limit(111).is_ok());
        // Should fail if daily update count exceeded
        t_yield.rate_limiter.daily_update_count = 2;
        t_yield.rate_limiter.last_reset_day = 0;
        assert!(t_yield.check_rate_limit(200).is_err());
        // Should reset daily count if new day
        t_yield.rate_limiter.last_reset_day = 0;
        assert!(t_yield.check_rate_limit(86400).is_ok()); // 86400 = start of next day
    }

    #[test]
    fn test_circuit_breaker() {
        let mut t_yield = TYield::default();
        t_yield.circuit_breaker.is_triggered = false;
        t_yield.circuit_breaker.cooldown_period_sec = 10;
        // Not triggered
        assert!(t_yield.check_circuit_breaker(100).is_ok());
        // Triggered and within cooldown
        t_yield.circuit_breaker.is_triggered = true;
        t_yield.circuit_breaker.trigger_time = 100;
        assert!(t_yield.check_circuit_breaker(105).is_err());
        // After cooldown
        assert!(t_yield.check_circuit_breaker(120).is_ok());
    }

    #[test]
    fn test_comprehensive_security_validation() {
        let mut t_yield = TYield::default();
        t_yield.parameter_bounds.max_tax_percentage = 1000;
        t_yield.parameter_bounds.max_protocol_balance = 1_000_000;
        t_yield.buy_tax = 500;
        t_yield.sell_tax = 500;
        t_yield.protocol_total_balance_usd = 500_000;
        t_yield.rate_limiter.last_update_time = 0;
        t_yield.rate_limiter.min_interval_sec = 1;
        t_yield.rate_limiter.max_updates_per_day = 10;
        t_yield.rate_limiter.daily_update_count = 0;
        t_yield.rate_limiter.last_reset_day = 0;
        t_yield.circuit_breaker.cooldown_period_sec = 10;
        // All valid
        assert!(t_yield.validate_security_state(100).is_ok());
        // Paused
        t_yield.paused = true;
        assert!(t_yield.validate_security_state(100).is_err());
        t_yield.paused = false;
        // Circuit breaker
        t_yield.circuit_breaker.is_triggered = true;
        t_yield.circuit_breaker.trigger_time = 100;
        assert!(t_yield.validate_security_state(105).is_err());
        t_yield.circuit_breaker.is_triggered = false;
        // Tax too high
        t_yield.buy_tax = 2000;
        assert!(t_yield.validate_security_state(100).is_err());
        t_yield.buy_tax = 500;
        // Balance too high
        t_yield.protocol_total_balance_usd = 2_000_000;
        assert!(t_yield.validate_security_state(100).is_err());
    }

    #[test]
    fn test_emergency_pause() {
        let mut t_yield = TYield::default();
        t_yield.paused = false;
        t_yield.circuit_breaker.is_triggered = false;
        t_yield.emergency_pause(123).unwrap();
        assert!(t_yield.paused);
        assert!(t_yield.circuit_breaker.is_triggered);
        assert_eq!(t_yield.circuit_breaker.trigger_time, 123);
        assert_eq!(t_yield.circuit_breaker.trigger_reason, 255);
    }
}
