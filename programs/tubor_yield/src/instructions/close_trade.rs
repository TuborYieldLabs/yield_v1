use anchor_lang::prelude::*;
use pyth_solana_receiver_sdk::price_update::{PriceUpdateV2, TwapUpdate};

use crate::{
    error::{ErrorCode, TYieldResult},
    math::SafeMath,
    state::{trade::Trade, MasterAgent, OraclePrice, TYield},
};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CloseTradeParams {
    // Optionally allow specifying a close reason or additional data
}

#[derive(Accounts)]
pub struct CloseTrade<'info> {
    /// User closing the trade
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The t_yield config PDA (protocol global state).
    #[account(
        seeds = [b"t_yield"],
        bump = t_yield.t_yield_bump
    )]
    pub t_yield: Account<'info, TYield>,

    pub pair_oracle_account: Account<'info, PriceUpdateV2>,
    pub pair_twap_account: Option<Account<'info, TwapUpdate>>,

    #[account(mut)]
    pub trade: Box<Account<'info, Trade>>,

    #[account(mut)]
    pub master_agent: Box<Account<'info, MasterAgent>>,
}

pub fn close_trade<'info>(
    ctx: Context<'_, '_, '_, 'info, CloseTrade<'info>>,
    _params: CloseTradeParams,
) -> TYieldResult<u8> {
    let current_time = ctx.accounts.t_yield.get_time()?;
    let trade = ctx.accounts.trade.as_mut();
    let master_agent = ctx.accounts.master_agent.as_mut();

    // 1. Check trade is active (ownership enforcement can be added here if needed)
    if !trade.is_active() {
        msg!("Trade is not active (already completed or cancelled)");
        return Err(ErrorCode::CannotPerformAction);
    }

    // 2. Get current price from oracle
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

    // 3. Complete the trade (set status, result, updated_at)
    let pnl = trade.calculate_pnl_safe(current_price)?;
    // For manual close, treat as Success if PnL >= 0, Failed if < 0
    let result = if pnl >= 0 {
        crate::state::trade::TradeResult::Success
    } else {
        crate::state::trade::TradeResult::Failed
    };
    trade.complete(result);
    trade.updated_at = current_time;

    // 4. Update master agent stats
    master_agent.completed_trades = master_agent.completed_trades.safe_add(1)?;
    master_agent.total_pnl = master_agent.total_pnl.safe_add(pnl.unsigned_abs())?;

    // (Optional) Emit event here if needed
    // emit!(crate::state::trade::TradeEvent { ... });

    Ok(0)
}
