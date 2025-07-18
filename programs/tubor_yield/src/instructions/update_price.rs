//! Instruction: Update Price
//!
//! Allows a master agent (with multisig approval) to update their agent price.
//! Requires multisig signatures and enforces protocol-level constraints on price changes.
//! Emits an `UpdatePriceEvent` on success.
//!
//! Accounts:
//! - authority: The signer proposing/signing the price update (must be a multisig signer)
//! - multisig: Protocol multisig PDA (controls admin actions)
//! - t_yield: Protocol global state/config PDA
//! - master_agent: Master agent account PDA (whose price is being updated)
//! - master_agent_mint: Mint account for the master agent NFT
//! - system_program: Solana system program
//! - event_authority: Event authority for CPI event logs (used for event emission)

use anchor_lang::prelude::*;

use crate::{
    error::{ErrorCode, TYieldResult},
    math::SafeMath,
    state::{AdminInstruction, MasterAgent, Multisig, TYield, UpdatePriceEvent},
};

/// Parameters for updating the master agent's price.
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct UpdatePriceParams {
    /// The new price to set for the master agent (scaled integer, e.g. 1e6 = 1.0 if using 6 decimals)
    pub new_price: u64,
}

/// Accounts required for updating a master agent's price.
///
/// This instruction requires multisig approval and enforces protocol-level constraints on price changes.
#[derive(Accounts)]
pub struct UpdatePrice<'info> {
    /// The signer proposing/signing the price update (must be a multisig signer).
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

    /// Master agent account PDA (whose price is being updated).
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

/// Handler for updating a master agent's price.
///
/// Requires multisig approval. Enforces protocol-level constraints on price changes and emits an event.
///
/// # Arguments
/// * `ctx` - Context with the required accounts.
/// * `params` - Parameters containing the new price.
///
/// # Returns
/// * `Ok(signatures_left)` - If more multisig signatures are required.
/// * `Ok(0)` - If the price was updated successfully.
/// * `Err` - If any validation or protocol constraint fails.
pub fn update_price<'info>(
    ctx: Context<'_, '_, '_, 'info, UpdatePrice<'info>>,
    params: UpdatePriceParams,
) -> TYieldResult<u8> {
    let mut multisig = ctx
        .accounts
        .multisig
        .load_mut()
        .map_err(|_| ErrorCode::InvalidBump)?;

    let instruction_data = Multisig::get_instruction_data(AdminInstruction::UpdatePrice, &params)
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
    let t_yield = ctx.accounts.t_yield.as_ref();
    let current_time = ctx.accounts.t_yield.get_time()?;

    master_agent.can_update_price_secure_with_time(
        params.new_price,
        t_yield.max_agent_price_new,
        &ctx.accounts.authority.key(),
        current_time,
    )?;

    master_agent.update_price(
        params.new_price,
        current_time,
        &ctx.accounts.authority.key(),
    )?;

    // Calculate price change and percentage
    let old_price = master_agent.price;
    let new_price = params.new_price;
    let price_change = new_price as i64 - old_price as i64;
    let price_change_percentage = if old_price > 0 {
        (price_change as u64).safe_mul(10000)?.safe_div(old_price)?
    } else {
        0
    };

    // Calculate total value locked before and after
    let old_total_value_locked = master_agent.get_total_value_locked();
    let new_total_value_locked = master_agent.agent_count.safe_mul(new_price)?;

    emit_cpi!(UpdatePriceEvent {
        authority: ctx.accounts.authority.key(),
        mint: ctx.accounts.master_agent_mint.key(),
        old_price,
        new_price,
        price_change,
        price_change_percentage,
        timestamp: current_time,
        agent_count: master_agent.agent_count,
        trade_count: master_agent.trade_count,
        yield_rate: master_agent.w_yield,
        trading_status: master_agent.trading_status,
        old_total_value_locked,
        new_total_value_locked,
        bump: master_agent.bump,
    });

    Ok(0)
}
