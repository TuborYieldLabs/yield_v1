use anchor_lang::prelude::*;
use pyth_solana_receiver_sdk::price_update::{PriceUpdateV2, TwapUpdate};

use crate::{error::{ErrorCode, TYieldResult}, math::SafeMath, state::{ trade::{PriceValidationConfig, Trade, TradeInitParams, TradeResult, TradeStatus, TradeType}, AdminInstruction, MasterAgent, Multisig, OraclePrice, Size, TYield}};

/// Parameters for opening a new trade.
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct OpenTradeParams {
    /// The requested entry price for the trade (scaled integer, e.g. 1e6 = 1.0 if using 6 decimals)
    pub entry_price: u64,
    /// The take profit price for the trade (scaled integer)
    pub take_profit: u64,
    /// The size of the trade (scaled integer, e.g. in base units)
    pub size: u64,
    /// The stop loss price for the trade (scaled integer)
    pub stop_loss: u64,
    /// The type of trade (Buy or Sell)
    pub trade_type: TradeType,
    /// The feed ID for the oracle price feed (32 bytes)
    pub feed_id: [u8; 32],
    /// The trading pair identifier (8 bytes)
    pub trade_pair: [u8; 8],
}

/// Accounts required for opening a new trade.
///
/// # Account Ordering
/// - `authority`: The user opening the trade and paying for account creation.
/// - `t_yield`: The protocol's global state/config PDA.
/// - `multisig`: The protocol's multisig PDA for admin/multisig actions.
/// - `pair_oracle_account`: The oracle account providing the current price.
/// - `pair_twap_account`: (Optional) The oracle account providing the TWAP price.
/// - `master_agent`: The user's master agent account (agent NFT state).
/// - `master_agent_mint`: The mint account for the agent NFT (checked, not written).
/// - `trade`: The trade account to be created (PDA, initialized here).
/// - `system_program`: The system program for account creation.
#[derive(Accounts)]
pub struct OpenTrade<'info> {
    /// The user opening the trade and paying for account creation.
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The protocol's global state/config PDA.
    ///
    /// Seeds: ["t_yield"]
    #[account(
        seeds = [b"t_yield"],
        bump = t_yield.t_yield_bump
    )]
    pub t_yield: Account<'info, TYield>,

    /// The protocol's multisig PDA for admin/multisig actions.
    #[account(
        mut,
        seeds = [b"multisig"],
        bump = multisig.load()?.bump
    )]
    pub multisig: AccountLoader<'info, Multisig>,

    /// The oracle account providing the current price.
    pub pair_oracle_account: Account<'info, PriceUpdateV2>,
    /// (Optional) The oracle account providing the TWAP price.
    pub pair_twap_account: Option<Account<'info, TwapUpdate>>,

    /// The user's master agent account (agent NFT state).
    #[account(mut,
        seeds = [b"master_agent".as_ref(), master_agent_mint.key().as_ref()],
        bump = master_agent.bump,
    )]
    pub master_agent: Box<Account<'info, MasterAgent>>,

    /// CHECK: Mint account for the NFT representing the agent
    /// Must match the mint in the master_agent account.
    #[account(
       constraint = master_agent_mint.key() == master_agent.mint
    )]
    pub master_agent_mint: AccountInfo<'info>,

    /// The trade account to be created (PDA, initialized here).
    #[account(
        init,
        payer = authority, 
        space = Trade::SIZE,
        seeds = [b"trade".as_ref(), master_agent.trade_count.saturating_add(1).to_le_bytes().as_ref()],
        bump,
    )]
    pub trade: Box<Account<'info, Trade>>,

    /// The system program for account creation.
    pub system_program: Program<'info, System>,
}


pub fn open_trade<'info>(
    ctx: Context<'_, '_, '_, 'info, OpenTrade<'info>>,
    params: OpenTradeParams,
) -> TYieldResult<u8> {
    let mut multisig = ctx
    .accounts
    .multisig
    .load_mut()
    .map_err(|_| ErrorCode::InvalidBump)?;

let instruction_data = Multisig::get_instruction_data(AdminInstruction::OpenTrade, &params)
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


let token_price = OraclePrice::new_from_oracle(
    &ctx.accounts.pair_oracle_account,
    ctx.accounts.pair_twap_account.as_ref(),
    &ctx.accounts.t_yield.oracle_param,
    current_time ,
    false,
    params.feed_id,
).map_err(|_| ErrorCode::InvalidOraclePrice)?;

// Enhanced price validation configuration
let validation_config = PriceValidationConfig::default();

// Create a temporary trade for validation
let temp_trade = Trade {
    master_agent: ctx.accounts.master_agent.key(),
    size: params.size,
    entry_price: params.entry_price,
    take_profit: params.take_profit,
    stop_loss: params.stop_loss,
    created_at: current_time,
    updated_at: current_time,
    pair: params.trade_pair,
    feed_id: params.feed_id,
    status: TradeStatus::Active as u8,
    trade_type: params.trade_type as u8,
    result: TradeResult::Pending as u8,
    bump: 0,
    authority: ctx.accounts.authority.key(),
    oracle_consensus_count: 0,
    last_price_update: current_time,
    circuit_breaker_triggered: false,
    _padding: [0; 2],
};

// Get current market price from oracle
let current_market_price = token_price.scale_to_exponent(0)?.price;

msg!("Current market price: {}", current_market_price);
msg!("Requested entry price: {}", params.entry_price);

// Comprehensive price validation using configuration
temp_trade.validate_with_config(
    current_market_price,
    &token_price,
    &validation_config,
)?;

// Validate that the trade can be executed
if !temp_trade.can_execute_with_config(
    current_market_price,
    &token_price,
    &validation_config,
)? {
    msg!("Trade cannot be executed at current market conditions");
    return Err(ErrorCode::PriceOutOfRange);
}

// Calculate optimal entry price for comparison
let optimal_entry_price = temp_trade.calculate_optimal_price_with_config(
    &token_price,
    &validation_config,
)?;

msg!("Optimal entry price: {}", optimal_entry_price);

// Validate entry price against optimal price
let price_diff = if params.entry_price >= optimal_entry_price {
    params.entry_price.safe_sub(optimal_entry_price)?
} else {
    optimal_entry_price.safe_sub(params.entry_price)?
};

let price_diff_bps = price_diff
    .safe_mul(crate::math::PERCENTAGE_PRECISION_U64)?
    .safe_div(optimal_entry_price)?;

if price_diff_bps > validation_config.max_slippage_bps {
    msg!(
        "Entry price deviation {} bps exceeds maximum {} bps",
        price_diff_bps,
        validation_config.max_slippage_bps
    );
    return Err(ErrorCode::MaxPriceSlippage);
}

// Validate side-specific price requirements
match params.trade_type {
    TradeType::Buy => {
        if params.entry_price < current_market_price {
            msg!("Buy order entry price {} below current market price {}", 
                 params.entry_price, current_market_price);
            return Err(ErrorCode::MaxPriceSlippage);
        }
    },
    TradeType::Sell => {
        if params.entry_price > current_market_price {
            msg!("Sell order entry price {} above current market price {}", 
                 params.entry_price, current_market_price);
            return Err(ErrorCode::MaxPriceSlippage);
        }
    },
}

// Calculate and validate risk-reward ratio
let risk_reward_ratio = temp_trade.calculate_risk_reward_ratio()?;
msg!("Risk-reward ratio: {} bps", risk_reward_ratio);

if risk_reward_ratio < validation_config.min_risk_reward_bps {
    msg!("Risk-reward ratio {} bps below minimum {} bps", 
         risk_reward_ratio, validation_config.min_risk_reward_bps);
    return Err(ErrorCode::InsufficientRiskRewardRatio);
}

// Validate stop loss and take profit distances
temp_trade.validate_risk_management_levels(validation_config.min_distance_bps)?;

// All validations passed, initialize the trade securely
let trade = ctx.accounts.trade.as_mut();

let init_trade_params = TradeInitParams {
    master_agent: ctx.accounts.master_agent.key(),
    size: params.size,
    entry_price: params.entry_price,
    take_profit: params.take_profit,
    stop_loss: params.stop_loss,
    created_at: current_time,
    pair: params.trade_pair,
    feed_id: params.feed_id,
    status: TradeStatus::Active,
    trade_type: params.trade_type,
    result: TradeResult::Pending,
    bump: ctx.bumps.trade, // Use actual PDA bump instead of hardcoded value
};

// Use secure initialization with proper authority
trade.init_trade_secure(init_trade_params, ctx.accounts.authority.key())?;

let master_agent = ctx.accounts.master_agent.as_mut();
master_agent.trade_count = master_agent.trade_count.safe_add(1)?;

msg!("Trade opened successfully with comprehensive price validation");

Ok(0)
}