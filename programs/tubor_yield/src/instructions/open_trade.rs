use anchor_lang::prelude::*;

use crate::{error::{ErrorCode, TYieldResult}, state::{trade::{Trade, TradeInitParams, TradeResult, TradeStatus, TradeType}, AdminInstruction, MasterAgent, Multisig, Size, TYield}};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct OpenTradeParams {
    pub entry_price: u64,

    pub take_profit: u64,

    pub size: u64,

    pub stop_loss: u64,

    pub trade_type: TradeType,

    pub feed_id: [u8; 32],
    pub trade_pair: [u8; 8],
}

#[derive(Accounts)]
pub struct OpenTrade<'info> {

    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [b"multisig"],
        bump = multisig.load()?.bump
    )]
    pub multisig: AccountLoader<'info, Multisig>,


    /// The t_yield config PDA (your protocol global state).
    ///
    /// Seeds: ["t_yield"]
    #[account(
        seeds = [b"t_yield"],
        bump = t_yield.t_yield_bump
    )]
    pub t_yield: Account<'info, TYield>,

    // pub pair_oracle_account: Account<'info, PriceUpdateV2>,
    // pub pair_twap_account: Option<Account<'info, TwapUpdate>>,


    #[account(mut,
        seeds = [b"master_agent".as_ref(), master_agent_mint.key().as_ref()],
        bump = master_agent.bump,
    )]
    pub master_agent: Box<Account<'info, MasterAgent>>,

    /// CHECK: Mint account for the NFT representing the agent
    #[account(
       constraint = master_agent_mint.key() == master_agent.mint
    )]
    pub master_agent_mint: AccountInfo<'info>,


    #[account(
        init,
        payer = authority, 
        space = Trade::SIZE,
        seeds = [b"trade".as_ref(), master_agent.trade_count.saturating_add(1).to_le_bytes().as_ref()],
        bump,
    )]
    pub trade: Box<Account<'info, Trade>>,

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

let signatures_left = multisig.sign_multisig(
    &ctx.accounts.authority,
    &Multisig::get_account_infos(&ctx)[1..],
    &instruction_data,
)?;
if signatures_left > 0 {
    msg!(
        "Instruction has been signed but more signatures are required: {}",
        signatures_left
    );
    return Ok(signatures_left);
}

let current_time = ctx.accounts.t_yield.get_time()?;

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
    bump: 2,
};

trade.init_trade(init_trade_params);


Ok(0)
}