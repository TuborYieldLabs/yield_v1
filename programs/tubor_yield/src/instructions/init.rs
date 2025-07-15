//! Instruction: Initialize Protocol
//!
//! Sets up the protocol's global state, multisig, and authority accounts. This instruction must be called once by the upgrade authority to initialize the protocol.
//!
//! Accounts:
//! - Upgrade authority (signer, payer)
//! - Multisig PDA (protocol admin control)
//! - Protocol state PDA (t_yield)
//! - Transfer authority PDA (protocol token authority)
//! - ProgramData (for upgrade authority validation)
//! - Program (for upgrade authority validation)
//! - Supported mint (SPL token used by protocol)
//! - Supported mint token account (protocol's token account for supported mint)
//! - System, Token, Associated Token programs
//! - Remaining: 1 to Multisig::MAX_SIGNERS admin signers (read-only, unsigned)

use crate::{error::TYieldResult, program::Tuboryield, state::InitProtocolEvent};
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Token, TokenAccount},
};

use {
    crate::state::{Multisig, Size, TYield},
    anchor_lang::prelude::*,
};

/// Parameters for protocol initialization.
#[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone, Default)]
pub struct InitParams {
    /// Minimum number of signatures required for multisig actions.
    pub min_signatures: u8,
    /// Whether agents can be deployed.
    pub allow_agent_deploy: bool,
    /// Whether agents can be bought.
    pub allow_agent_buy: bool,
    /// Whether agents can be sold.
    pub allow_agent_sell: bool,
    /// Whether yield withdrawals are allowed.
    pub allow_withdraw_yield: bool,
    /// Buy tax (percentage, fixed-point).
    pub buy_tax: u64,
    /// Sell tax (percentage, fixed-point).
    pub sell_tax: u64,
    /// Maximum allowed tax percentage.
    pub max_tax_percentage: u64,
    /// Referral earnings percentage.
    pub ref_earn_percentage: u64,
    /// Supported SPL token mint for protocol operations.
    pub supported_mint: Pubkey,
    /// Whether protocol is paused at initialization (optional, default false)
    pub paused: Option<bool>,
    /// Initial circuit breaker state (optional)
    pub circuit_breaker: Option<crate::state::t_yield::CircuitBreaker>,
    /// Initial rate limiter state (optional)
    pub rate_limiter: Option<crate::state::t_yield::RateLimiter>,
    /// Initial parameter bounds (optional)
    pub parameter_bounds: Option<crate::state::t_yield::ParameterBounds>,
}

/// Accounts required for protocol initialization.
///
/// This instruction must be called by the upgrade authority. It creates and initializes the multisig and t_yield state PDAs, sets up the protocol's transfer authority, and creates the protocol's token account for the supported mint.
#[derive(Accounts)]
#[instruction(params:InitParams)]
pub struct Init<'info> {
    /// Upgrade authority for the program. Pays for account creation. Must sign.
    #[account(mut)]
    pub upgrade_authority: Signer<'info>,

    // --- Protocol State Accounts ---
    /// Multisig PDA for protocol admin control.
    /// Seeds: [b"multisig"]
    /// Space: Multisig::SIZE
    #[account(
        init,
        payer = upgrade_authority,
        space = Multisig::SIZE,
        seeds = [b"multisig"],
        bump
    )]
    pub multisig: AccountLoader<'info, Multisig>,

    /// Protocol global state PDA.
    /// Seeds: [b"t_yield"]
    /// Space: TYield::SIZE
    #[account(
        init,
        payer = upgrade_authority,
        space = TYield::SIZE,
        seeds = [b"t_yield"],
        bump
    )]
    pub t_yield: Box<Account<'info, TYield>>,

    /// Transfer authority PDA (protocol authority for token operations).
    /// Seeds: [b"transfer_authority"]
    /// Space: 0 (PDA, no data)
    /// CHECK: This is a PDA controlled by the program, only used as authority.
    #[account(
        init,
        payer = upgrade_authority,
        space = 0,
        seeds = [b"transfer_authority"],
        bump
    )]
    pub transfer_authority: AccountInfo<'info>,

    // --- Program Validation Accounts ---
    /// ProgramData account (for upgrade authority validation).
    /// CHECK: Used for upgrade authority validation only.
    #[account()]
    pub t_yield_program_data: AccountInfo<'info>,

    /// The program itself (for upgrade authority validation).
    pub t_yield_program: Program<'info, Tuboryield>,

    // --- Mint & Token Accounts ---
    /// Supported SPL token mint for protocol operations.
    /// CHECK: This is a PDA controlled by the program, only used as authority.
    #[account(
        address = params.supported_mint
    )]
    pub supported_mint: AccountInfo<'info>,

    /// Protocol's token account for the supported mint.
    /// Associated token account owned by transfer_authority.
    #[account(
        init_if_needed,
        payer = upgrade_authority,
        associated_token::mint = supported_mint,
        associated_token::authority = transfer_authority,
    )]
    pub supported_mint_token_account: Box<Account<'info, TokenAccount>>,

    // --- System & Program Accounts ---
    /// Solana system program.
    pub system_program: Program<'info, System>,
    /// SPL token program.
    pub token_program: Program<'info, Token>,
    /// SPL associated token program.
    pub associated_token_program: Program<'info, AssociatedToken>,
    // --- Misc ---
    /// CHECK: Event authority for CPI event logs (used for event emission; not written to).
    #[account(
        seeds = [b"__event_authority"],
        bump,
    )]
    pub event_authority: AccountInfo<'info>,
    // --- Remaining Accounts ---
    // 1 to Multisig::MAX_SIGNERS admin signers (read-only, unsigned)
}

/// Handler for protocol initialization.
///
/// Validates the upgrade authority, initializes the multisig and protocol state, sets up permissions and tax parameters, and records bumps.
///
/// # Arguments
/// * `ctx` - Context with the required accounts.
/// * `params` - Initialization parameters.
///
/// # Errors
/// Returns an error if authority validation or account initialization fails.
pub fn init(ctx: Context<Init>, params: &InitParams) -> TYieldResult<()> {
    TYield::validate_upgrade_authority(
        ctx.accounts.upgrade_authority.key(),
        &ctx.accounts.t_yield_program_data.to_account_info(),
        &ctx.accounts.t_yield_program,
    )
    .map_err(|_| crate::error::ErrorCode::InvalidAuthority)?;

    // initialize multisig, this will fail if account is already initialized
    let mut multisig = ctx
        .accounts
        .multisig
        .load_init()
        .map_err(|_| crate::error::ErrorCode::InvalidBump)?;

    multisig
        .set_signers(ctx.remaining_accounts, params.min_signatures)
        .map_err(|_| crate::error::ErrorCode::InvalidBump)?;

    // record multisig PDA bump
    multisig.bump = ctx.bumps.multisig;

    // record protocol state
    let t_yield = ctx.accounts.t_yield.as_mut();

    t_yield.permissions.allow_agent_deploy = params.allow_agent_deploy;
    t_yield.permissions.allow_agent_buy = params.allow_agent_buy;
    t_yield.permissions.allow_agent_sell = params.allow_agent_sell;
    t_yield.permissions.allow_withdraw_yield = params.allow_withdraw_yield;

    t_yield.transfer_authority_bump = ctx.bumps.transfer_authority;
    t_yield.t_yield_bump = ctx.bumps.t_yield;

    t_yield.y_mint = ctx.accounts.supported_mint.key();

    t_yield.buy_tax = params.buy_tax;
    t_yield.sell_tax = params.sell_tax;
    t_yield.max_tax_percentage = params.max_tax_percentage;
    t_yield.ref_earn_percentage = params.ref_earn_percentage;

    t_yield.inception_time = t_yield.get_time()?;

    // Set additional protocol state fields if provided
    t_yield.paused = params.paused.unwrap_or(false);
    if let Some(cb) = params.circuit_breaker {
        t_yield.circuit_breaker = cb;
    }
    if let Some(rl) = params.rate_limiter {
        t_yield.rate_limiter = rl;
    }
    if let Some(pb) = params.parameter_bounds {
        t_yield.parameter_bounds = pb;
    }

    emit_cpi!(InitProtocolEvent {
        inception_time: t_yield.inception_time,
        paused: t_yield.paused,
        permissions: t_yield.permissions,
    });

    Ok(())
}
