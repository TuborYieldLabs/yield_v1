//! Instruction: Update Trade
//!
//! Checks the current price for a trade and updates its status if take profit (TP) or stop loss (SL) is hit.
//! Can be called by anyone. If TP/SL is hit, the trade is completed and the master agent's stats are updated.
//!
//! Accounts:
//! - authority: The caller (anyone can call)
//! - t_yield: Protocol global state/config PDA
//! - pair_oracle_account: Oracle price account for the trading pair
//! - pair_twap_account: (Optional) TWAP oracle account for the trading pair
//! - trade: Trade account to update (mut)
//! - master_agent: Master agent account for the trade (mut)
//! - event_authority: Event authority for CPI event logs (used for event emission)
//! - system_program: Solana system program

use anchor_lang::prelude::*;
use pyth_solana_receiver_sdk::price_update::{PriceUpdateV2, TwapUpdate};

use crate::{
    error::{ErrorCode, TYieldResult},
    math::SafeMath,
    state::{
        trade::{Trade, TradeResult, TradeStatus},
        MasterAgent, OraclePrice, TYield,
    },
};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct UpdateTradeParams {
    // No parameters needed as this can be called by anyone
    // The function will automatically check current price against TP/SL
}

/// Accounts required for updating a trade's status based on current price.
///
/// This instruction can be called by anyone. It checks the current price for the trade and updates its status if TP/SL is hit.
///
/// # Account Ordering
/// - `authority`: The caller (anyone can call)
/// - `t_yield`: Protocol global state/config PDA
/// - `pair_oracle_account`: Oracle price account for the trading pair
/// - `pair_twap_account`: (Optional) TWAP oracle account for the trading pair
/// - `trade`: Trade account to update (mut)
/// - `master_agent`: Master agent account for the trade (mut)
/// - `event_authority`: Event authority for CPI event logs (used for event emission)
/// - `system_program`: Solana system program
#[derive(Accounts)]
pub struct UpdateTrade<'info> {
    /// The caller (anyone can call this instruction).
    pub authority: Signer<'info>,

    /// Protocol global state/config PDA.
    /// Seeds: ["t_yield"]
    #[account(
        seeds = [b"t_yield"],
        bump = t_yield.t_yield_bump
    )]
    pub t_yield: Account<'info, TYield>,

    /// Oracle price account for the trading pair (Pyth V2 price account).
    pub pair_oracle_account: Account<'info, PriceUpdateV2>,

    /// Optional TWAP (Time-Weighted Average Price) account for the trading pair.
    pub pair_twap_account: Option<Account<'info, TwapUpdate>>,

    /// Trade account to update (mut).
    #[account(mut)]
    pub trade: Box<Account<'info, Trade>>,

    /// Master agent account for the trade (mut).
    #[account(mut)]
    pub master_agent: Box<Account<'info, MasterAgent>>,

    /// Event authority for CPI event logs (used for event emission; not written to).
    /// Seeds: ["__event_authority"]
    /// CHECK: Derived by Anchor for event emission.
    #[account(
        seeds = [b"__event_authority"],
        bump,
    )]
    pub event_authority: AccountInfo<'info>,

    /// Solana system program.
    pub system_program: Program<'info, System>,
}

pub fn update_trade<'info>(
    ctx: Context<'_, '_, '_, 'info, UpdateTrade<'info>>,
    _params: UpdateTradeParams,
) -> TYieldResult<u8> {
    let current_time = ctx.accounts.t_yield.get_time()?;
    let trade = ctx.accounts.trade.as_mut();
    let master_agent = ctx.accounts.master_agent.as_mut();

    // Check if trade is already completed or cancelled
    if !trade.is_active() {
        msg!(
            "Trade is already {} - no update needed",
            if trade.is_completed() {
                "completed"
            } else {
                "cancelled"
            }
        );
        return Ok(0);
    }

    // Get current price from oracle
    let token_price = OraclePrice::new_from_oracle(
        &ctx.accounts.pair_oracle_account,
        ctx.accounts.pair_twap_account.as_ref(),
        &ctx.accounts.t_yield.oracle_param,
        current_time,
        false,
        trade.feed_id,
    )
    .map_err(|_| ErrorCode::InvalidOraclePrice)?;

    let current_price = token_price.scale_to_exponent(0)?.price;

    msg!("Current price: {}", current_price);
    msg!("Trade entry price: {}", trade.entry_price);
    msg!("Trade take profit: {}", trade.take_profit);
    msg!("Trade stop loss: {}", trade.stop_loss);

    // Check if trade has hit take profit
    if trade.has_hit_take_profit(current_price) {
        msg!("Trade has hit take profit at price {}", current_price);

        // Calculate PnL
        let pnl = trade.calculate_pnl_safe(current_price)?;
        msg!("Trade PnL: {}", pnl);

        // Complete the trade with success result
        trade.complete(TradeResult::Success);

        // Update master agent trade count and PnL
        master_agent.completed_trades = master_agent.completed_trades.safe_add(1)?;
        master_agent.total_pnl = master_agent.total_pnl.safe_add(pnl.unsigned_abs())?;

        // Emit trade event
        emit_cpi!(crate::state::trade::TradeEvent {
            trade: trade.key(),
            status: TradeStatus::Completed,
            trade_type: trade.get_trade_type(),
            result: TradeResult::Success,
            pnl,
            created_at: current_time,
        });

        msg!("Trade completed successfully with profit");
        return Ok(1); // Return 1 to indicate TP hit
    }

    // Check if trade has hit stop loss
    if trade.has_hit_stop_loss(current_price) {
        msg!("Trade has hit stop loss at price {}", current_price);

        // Calculate PnL
        let pnl = trade.calculate_pnl_safe(current_price)?;
        msg!("Trade PnL: {}", pnl);

        // Complete the trade with failed result
        trade.complete(TradeResult::Failed);

        // Update master agent trade count and PnL
        master_agent.completed_trades = master_agent.completed_trades.safe_add(1)?;
        master_agent.total_pnl = master_agent.total_pnl.safe_add(pnl.unsigned_abs())?;

        // Emit trade event
        emit_cpi!(crate::state::trade::TradeEvent {
            trade: trade.key(),
            status: TradeStatus::Completed,
            trade_type: trade.get_trade_type(),
            result: TradeResult::Failed,
            pnl,
            created_at: current_time,
        });

        msg!("Trade completed with stop loss");
        return Ok(2); // Return 2 to indicate SL hit
    }

    // Calculate unrealized PnL for active trade
    let unrealized_pnl = trade.calculate_unrealized_pnl(current_price)?;
    msg!("Unrealized PnL: {}", unrealized_pnl);

    // Update trade timestamp
    trade.updated_at = current_time;

    msg!("Trade is still active - no TP/SL hit yet");
    Ok(0) // Return 0 to indicate no TP/SL hit
}
