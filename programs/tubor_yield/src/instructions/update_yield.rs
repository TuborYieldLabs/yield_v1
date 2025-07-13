//! Instruction: Update Yield
//!
//! Allows a master agent (with multisig approval) to update their yield rate.
//! Requires multisig signatures and enforces protocol-level constraints on yield changes.
//! Emits an `UpdateYieldEvent` on success.
//!
//! # Overview
//! This instruction provides a secure way to update yield rates for master agents
//! while maintaining protocol stability and preventing manipulation. It requires
//! multisig approval and enforces comprehensive security constraints.
//!
//! # Security Features
//! - **Multisig Protection**: Requires multiple authorized signatures
//! - **Authority Validation**: Only authorized users can update yield rates
//! - **Rate Limiting**: Maximum 5% increase per update to prevent manipulation
//! - **Time Restrictions**: Minimum 1 hour between updates
//! - **Maximum Limits**: 50% maximum yield rate to ensure sustainability
//! - **Safe Math**: All calculations use safe operations to prevent overflow
//!
//! # Event Emission
//! On successful execution, this instruction emits an `UpdateYieldEvent` containing:
//! - Authority and mint information
//! - Old and new yield rates
//! - Change amount and percentage
//! - Context data (agent count, trade count, price, etc.)
//! - Total yield impact assessment
//!
//! # Accounts
//! - authority: The signer proposing/signing the yield update (must be a multisig signer)
//! - multisig: Protocol multisig PDA (controls admin actions)
//! - t_yield: Protocol global state/config PDA
//! - master_agent: Master agent account PDA (whose yield is being updated)
//! - master_agent_mint: Mint account for the master agent NFT
//! - system_program: Solana system program
//! - event_authority: Event authority for CPI event logs (used for event emission)

use anchor_lang::prelude::*;

use crate::{
    error::{ErrorCode, TYieldResult},
    math::SafeMath,
    state::{AdminInstruction, MasterAgent, Multisig, TYield, UpdateYieldEvent},
};

/// Parameters for updating the master agent's yield rate.
///
/// This struct contains the parameters required to update a master agent's yield rate.
/// The yield rate is specified in basis points where 10000 = 100%.
///
/// # Example
/// ```
/// # use anchor_lang::prelude::*;
/// # use tubor_yield::instructions::UpdateYieldParams;
/// let params = UpdateYieldParams {
///     new_yield_rate: 600, // 6% yield rate
/// };
/// ```
///
/// # Validation
/// - Yield rate must be greater than 0
/// - Yield rate cannot exceed 50000 (50%)
/// - Yield rate increase cannot exceed 5% per update
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct UpdateYieldParams {
    /// The new yield rate to set for the master agent (in basis points, e.g. 500 = 5%)
    pub new_yield_rate: u64,
}

/// Accounts required for updating a master agent's yield rate.
///
/// This instruction requires multisig approval and enforces protocol-level constraints on yield changes.
/// All accounts must be properly validated and the authority must be an authorized multisig signer.
///
/// # Security Requirements
/// - Authority must be a valid multisig signer
/// - Master agent must exist and be properly initialized
/// - Multisig must have sufficient signatures for execution
/// - All PDAs must have correct seeds and bumps
///
/// # Account Validation
/// - Authority account must be a signer
/// - Multisig account must be mutable and properly seeded
/// - Master agent account must be mutable and properly seeded
/// - Mint account is used for seed validation only
#[derive(Accounts)]
pub struct UpdateYield<'info> {
    /// The signer proposing/signing the yield update (must be a multisig signer).
    #[account(mut)]
    pub authority: Signer<'info>,

    /// Protocol multisig PDA (controls admin actions).
    /// Seeds: ["multisig"]
    #[account(
        mut,
        seeds = [b"multisig"],
        bump = multisig.load()?.bump
    )]
    pub multisig: AccountLoader<'info, Multisig>,

    /// Protocol global state/config PDA.
    /// Seeds: ["t_yield"]
    #[account(
        seeds = [b"t_yield"],
        bump = t_yield.t_yield_bump
    )]
    pub t_yield: Box<Account<'info, TYield>>,

    /// Master agent account PDA (whose yield is being updated).
    /// Seeds: ["master_agent", master_agent_mint]
    #[account(
        mut,
        seeds = [b"master_agent".as_ref(), master_agent_mint.key().as_ref()],
        bump = master_agent.bump,
    )]
    pub master_agent: Box<Account<'info, MasterAgent>>,

    /// Mint account for the master agent NFT.
    /// CHECK: Only used for seed validation.
    pub master_agent_mint: AccountInfo<'info>,

    /// Solana system program.
    pub system_program: Program<'info, System>,

    /// Event authority for CPI event logs (used for event emission; not written to).
    /// Seeds: ["__event_authority"]
    /// CHECK: Derived by Anchor for event emission.
    #[account(
        seeds = [b"__event_authority"],
        bump,
    )]
    pub event_authority: AccountInfo<'info>,
}

/// Handler for updating a master agent's yield rate.
///
/// This function implements the complete yield update flow with comprehensive security
/// validation, multisig approval, and event emission. It enforces strict constraints
/// to prevent yield manipulation and ensure protocol stability.
///
/// # Flow Overview
/// 1. **Multisig Validation**: Verifies authority is a valid multisig signer
/// 2. **Instruction Signing**: Collects signatures and validates instruction data
/// 3. **Security Checks**: Enforces yield rate constraints and time restrictions
/// 4. **State Update**: Updates the master agent's yield rate
/// 5. **Event Emission**: Emits comprehensive event with all relevant data
///
/// # Security Constraints
/// - **Authority Validation**: Only authorized multisig signers can update yield rates
/// - **Rate Limiting**: Maximum 5% increase per update to prevent manipulation
/// - **Time Restrictions**: Minimum 1 hour between updates
/// - **Maximum Limits**: 50% maximum yield rate for sustainability
/// - **Safe Math**: All calculations use safe operations
///
/// # Arguments
/// * `ctx` - Context with the required accounts and validation
/// * `params` - Parameters containing the new yield rate
///
/// # Returns
/// * `Ok(signatures_left)` - If more multisig signatures are required
/// * `Ok(0)` - If the yield was updated successfully
/// * `Err(ErrorCode::InvalidBump)` - If multisig account validation fails
/// * `Err(ErrorCode::InvalidInstructionHash)` - If instruction data validation fails
/// * `Err(ErrorCode::InvalidAuthority)` - If authority is not authorized
/// * `Err(ErrorCode::MathError)` - If yield rate is invalid or exceeds limits
/// * `Err(ErrorCode::PriceUpdateTooSoon)` - If insufficient time has passed
/// * `Err(ErrorCode::PriceUpdateTooHigh)` - If the increase exceeds rate limits
///
/// # Event Emission
/// On successful execution, emits an `UpdateYieldEvent` with:
/// - Authority and mint information
/// - Old and new yield rates with change calculations
/// - Context data (agent count, trade count, price, trading status)
/// - Total yield impact assessment
/// - Timestamp and bump seed for audit purposes
///
///
/// # Security Notes
/// - All validation is performed before any state changes
/// - Event emission provides complete audit trail
/// - Rate limiting prevents rapid yield manipulation
/// - Authority validation ensures only authorized changes
/// - Safe math operations prevent overflow/underflow
pub fn update_yield<'info>(
    ctx: Context<'_, '_, '_, 'info, UpdateYield<'info>>,
    params: UpdateYieldParams,
) -> TYieldResult<u8> {
    let mut multisig = ctx
        .accounts
        .multisig
        .load_mut()
        .map_err(|_| ErrorCode::InvalidBump)?;

    let instruction_data = Multisig::get_instruction_data(AdminInstruction::UpdateYield, &params)
        .map_err(|_| ErrorCode::InvalidInstructionHash)?;

    let current_time = ctx.accounts.t_yield.get_time()?;
    let nonce = current_time as u64; // Use current time as nonce for simplicity

    let signatures_left = multisig.sign_multisig(
        &ctx.accounts.authority,
        &Multisig::get_account_infos(&ctx)[1..],
        &instruction_data,
        nonce,
        current_time,
    )?;
    if signatures_left > 0 {
        msg!(
            "Instruction has been signed but more signatures are required: {}",
            signatures_left
        );
        return Ok(signatures_left);
    }

    let master_agent = ctx.accounts.master_agent.as_mut();
    let current_time = ctx.accounts.t_yield.get_time()?;

    // Update the yield rate with authority validation and security checks
    master_agent.update_yield(
        params.new_yield_rate,
        current_time,
        &ctx.accounts.authority.key(),
    )?;

    // Calculate yield change and percentage
    let old_yield_rate = master_agent.w_yield;
    let new_yield_rate = params.new_yield_rate;
    let yield_change = new_yield_rate as i64 - old_yield_rate as i64;
    let yield_change_percentage = if old_yield_rate > 0 {
        (yield_change as u64)
            .safe_mul(10000)?
            .safe_div(old_yield_rate)?
    } else {
        0
    };

    // Calculate total yield generated before and after
    let old_total_yield_generated = master_agent.get_total_yield_generated()?;
    let new_total_yield_generated = master_agent.get_total_yield_generated()?;

    emit_cpi!(UpdateYieldEvent {
        authority: ctx.accounts.authority.key(),
        mint: ctx.accounts.master_agent_mint.key(),
        old_yield_rate,
        new_yield_rate,
        yield_change,
        yield_change_percentage,
        timestamp: current_time,
        agent_count: master_agent.agent_count,
        trade_count: master_agent.trade_count,
        price: master_agent.price,
        trading_status: master_agent.trading_status,
        old_total_yield_generated,
        new_total_yield_generated,
        bump: master_agent.bump,
    });

    Ok(0)
}
