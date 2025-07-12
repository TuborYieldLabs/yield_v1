//! Instruction: Get Pair Price
//!
//! Returns the current price for a trading pair using the provided oracle and optional TWAP account.
//! This is a read-only query; no state is mutated.
//!
//! Accounts:
//! - Protocol global state (t_yield, PDA: ["t_yield"])
//! - Oracle price account (PDA: per Pyth)
//! - Optional TWAP account (PDA: per Pyth)
//! - Trade account (PDA: ["trade", ...])
//! - Master agent account (PDA: ["master_agent", ...])

use anchor_lang::prelude::*;
use pyth_solana_receiver_sdk::price_update::{PriceUpdateV2, TwapUpdate};

use crate::{
    error::{ErrorCode, TYieldResult},
    state::{trade::Trade, MasterAgent, OraclePrice, TYield},
};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct GetPairParams {
    /// The feed ID for the oracle price feed (e.g., Pyth price feed ID)
    pub feed_id: [u8; 32],
}

/// Accounts required to query the current price for a trading pair.
///
/// This instruction does not mutate any state and can be called by anyone.
#[derive(Accounts)]
pub struct GetPairPrice<'info> {
    /// Protocol global state.
    /// PDA: ["t_yield"]
    #[account(
        seeds = [b"t_yield"],
        bump = t_yield.t_yield_bump
    )]
    pub t_yield: Account<'info, TYield>,

    /// Oracle price account for the trading pair (Pyth V2 price account).
    pub pair_oracle_account: Account<'info, PriceUpdateV2>,

    /// Optional TWAP (Time-Weighted Average Price) account for the trading pair.
    pub pair_twap_account: Option<Account<'info, TwapUpdate>>,

    /// Trade account for the current trade (PDA: ["trade", ...]).
    #[account()]
    pub trade: Box<Account<'info, Trade>>,

    /// Master agent account (PDA: ["master_agent", ...]).
    #[account()]
    pub master_agent: Box<Account<'info, MasterAgent>>,
}

/// Returns the current price for a trading pair using the provided oracle and optional TWAP account.
///
/// # Arguments
/// * `ctx` - Context with the required accounts.
/// * `params` - Parameters including the oracle feed ID.
///
/// # Returns
/// * `OraclePrice` - Struct containing the current price and exponent (always 0 for scaled result).
pub fn get_pair_price(
    ctx: Context<GetPairPrice>,
    params: GetPairParams,
) -> TYieldResult<OraclePrice> {
    let current_time = ctx.accounts.t_yield.get_time()?;

    // Get current price from oracle
    let token_price = OraclePrice::new_from_oracle(
        &ctx.accounts.pair_oracle_account,
        ctx.accounts.pair_twap_account.as_ref(),
        &ctx.accounts.t_yield.oracle_param,
        current_time,
        false,
        params.feed_id,
    )
    .map_err(|_| ErrorCode::InvalidOraclePrice)?;

    let current_price = token_price.scale_to_exponent(0)?.price;

    Ok(OraclePrice {
        price: current_price,
        exponent: 0,
    })
}
