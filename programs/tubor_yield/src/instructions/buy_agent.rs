//! Instruction: Buy Agent
//!
//! Allows a user to purchase an agent NFT from a master agent, transferring ownership and updating protocol/user state.
//! Handles tax, price calculation, and all token/NFT transfers. Enforces whitelist, ban, and protocol constraints.

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount},
};

use mpl_token_metadata::ID as METADATA_PROGRAM_ID;

use crate::{
    error::{ErrorCode, TYieldResult},
    math::SafeMath,
    state::{Agent, BuyAgentEvent, MasterAgent, TYield, TransferAgentParams, User},
    try_from,
};

/// Accounts required for buying an agent NFT from a master agent.
///
/// This instruction:
/// - Transfers the agent NFT from protocol to user
/// - Transfers payment (Y-mint) from user to protocol
/// - Updates protocol and user state
/// - Enforces whitelist, ban, and protocol constraints
#[derive(Accounts)]
pub struct BuyAgent<'info> {
    /// The user purchasing the agent. Must sign the transaction.
    #[account(mut)]
    pub authority: Signer<'info>,

    // --- System & Program Accounts ---
    /// Solana system program.
    pub system_program: Program<'info, System>,
    /// SPL token program.
    pub token_program: Program<'info, Token>,
    /// SPL associated token program.
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// CHECK: Instructions sysvar (required for Metaplex CPI).
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub sysvar_instructions: AccountInfo<'info>,

    /// CHECK: Metaplex token metadata program.
    #[account(address = METADATA_PROGRAM_ID)]
    pub metadata_program: AccountInfo<'info>,

    /// CHECK: Metadata account for the agent NFT (validated by Metaplex CPI).
    /// PDA: ["metadata", METADATA_PROGRAM_ID, mint]
    #[account(
        mut,
        seeds = [
            b"metadata",
            METADATA_PROGRAM_ID.as_ref(),
            mint.key().as_ref(),
        ],
        bump,
        seeds::program = METADATA_PROGRAM_ID,
    )]
    pub metadata: AccountInfo<'info>,

    // --- Protocol/User/Agent State ---
    /// User account PDA.
    /// PDA: ["user", authority]
    #[account(
        mut,
        seeds = [b"user".as_ref(), authority.key().as_ref()],
        bump = user.bump
    )]
    pub user: Box<Account<'info, User>>,

    /// Agent account PDA (the agent being purchased).
    /// PDA: ["agent", mint]
    #[account(
        mut,
        seeds = [b"agent".as_ref(), mint.key().as_ref()],
        bump,
    )]
    pub agent: Box<Account<'info, Agent>>,

    /// Master agent account PDA (the parent/master of the agent).
    /// PDA: ["master_agent", master_agent_mint]
    #[account(
        mut,
        seeds = [b"master_agent".as_ref(), master_agent_mint.key().as_ref()],
        bump = master_agent.bump,
    )]
    pub master_agent: Box<Account<'info, MasterAgent>>,

    /// Mint account for the agent NFT being purchased.
    pub mint: Box<Account<'info, Mint>>,

    /// Mint account for the master agent NFT.
    pub master_agent_mint: Box<Account<'info, Mint>>,

    /// Protocol global state PDA.
    /// PDA: ["t_yield"]
    #[account(
        seeds = [b"t_yield"],
        bump = t_yield.t_yield_bump
    )]
    pub t_yield: Box<Account<'info, TYield>>,

    /// CHECK: Transfer authority PDA (protocol authority for token/NFT transfers).
    /// PDA: ["transfer_authority"]
    #[account(
        seeds = [b"transfer_authority"],
        bump = t_yield.transfer_authority_bump
    )]
    pub transfer_authority: AccountInfo<'info>,

    // --- Token Accounts ---
    /// Protocol's token account holding the agent NFT (source for transfer).
    /// Must have mint == agent mint.
    #[account(
        mut,
        constraint = transfer_authority_ta.mint == mint.key()
    )]
    pub transfer_authority_ta: Box<Account<'info, TokenAccount>>,

    /// User's token account to receive the agent NFT.
    /// Must have mint == agent mint.
    #[account(
        mut,
        constraint = user_agent_ta.mint == mint.key()
    )]
    pub user_agent_ta: Box<Account<'info, TokenAccount>>,

    /// User's Y-mint token account (payer for the purchase).
    /// Must have mint == t_yield.y_mint.
    #[account(
        mut,
        constraint = user_y_mint_ta.mint == t_yield.y_mint
    )]
    pub user_y_mint_ta: Box<Account<'info, TokenAccount>>,

    /// Protocol's Y-mint token account (receiver of payment).
    /// Must have mint == t_yield.y_mint.
    #[account(
        mut,
        constraint = transfer_authority_y_mint_ta.mint == t_yield.y_mint
    )]
    pub transfer_authority_y_mint_ta: Box<Account<'info, TokenAccount>>,

    /// CHECK: Y-mint SPL token mint (payment token for protocol).
    #[account(address = t_yield.y_mint)]
    pub y_mint: AccountInfo<'info>,

    // --- Misc ---
    /// CHECK: Event authority for CPI event logs (used for event emission; not written to).
    #[account(
        seeds = [b"__event_authority"],
        bump,
    )]
    pub event_authority: AccountInfo<'info>,
}

/// Instruction: Buy Agent
///
/// Transfers an agent NFT from protocol to user, collects payment, updates protocol/user state, and emits an event.
/// Enforces bans, whitelists, and protocol constraints. Handles all token/NFT transfers and price/tax logic.
pub fn buy_agent<'info>(ctx: Context<'_, '_, '_, 'info, BuyAgent<'info>>) -> TYieldResult<()> {
    let current_time = ctx.accounts.t_yield.get_time()?;
    let master_agent = ctx.accounts.master_agent.as_mut();
    let user = ctx.accounts.user.as_mut();
    let agents = ctx.accounts.agent.as_mut();
    let t_yield = ctx.accounts.t_yield.as_mut();

    // --- Access control checks ---
    if !user.can_perform_actions() {
        return Err(ErrorCode::CannotPerformAction);
    }
    if master_agent.is_whitelist_mode() && !user.is_whitelisted() {
        return Err(ErrorCode::CannotPerformAction);
    }
    if !agents.belongs_to_master_agent(&master_agent.key()) {
        return Err(ErrorCode::CannotPerformAction);
    }

    // --- Price/tax calculation ---
    let price = master_agent.calculate_buy_price_with_tax()?;

    // --- Payment transfer (Y-mint) ---
    let mint =
        try_from!(Account<Mint>, ctx.accounts.y_mint).map_err(|_| ErrorCode::AccountFromError)?;
    TYield::transfer_tokens(
        ctx.accounts.user_y_mint_ta.to_account_info(),
        mint.to_account_info(),
        ctx.accounts.transfer_authority_y_mint_ta.to_account_info(),
        ctx.accounts.authority.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        price.0,
        mint.decimals,
    )
    .map_err(|_| ErrorCode::InvalidInstructionHash)?;

    // --- Remove agent from master agent ---
    master_agent.remove_agent(current_time)?;

    // --- Update protocol state ---
    t_yield.protocol_total_fees = t_yield.protocol_total_fees.safe_add(price.1)?;
    t_yield.protocol_current_holding = t_yield.protocol_current_holding.safe_add(price.2)?;
    t_yield.protocol_total_balance_usd = t_yield.protocol_total_balance_usd.safe_add(price.1)?;
    t_yield.protocol_total_earnings = t_yield.protocol_total_earnings.safe_add(price.0)?;

    // --- Transfer agent NFT to user ---
    let transfer_agent_params = TransferAgentParams {
        payer: ctx.accounts.authority.to_account_info(),
        sender_nft_token_account: ctx.accounts.transfer_authority_ta.to_account_info(),
        authority: ctx.accounts.transfer_authority.to_account_info(),
        receiver_token_account: ctx.accounts.user_agent_ta.to_account_info(),
        receiver: ctx.accounts.authority.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        metadata: ctx.accounts.metadata.to_account_info(),
        metadata_program: ctx.accounts.metadata_program.to_account_info(),
        system_program: ctx.accounts.system_program.to_account_info(),
        associated_token_program: ctx.accounts.associated_token_program.to_account_info(),
        token_program: ctx.accounts.token_program.to_account_info(),
        sysvar_instructions: ctx.accounts.sysvar_instructions.to_account_info(),
    };
    t_yield
        .transfer_agent(transfer_agent_params)
        .map_err(|_| ErrorCode::InvalidInstructionHash)?;

    // --- Update user and agent state ---
    user.add_agent(price.2)?; // Pass base_price to track total value spent
    agents.transfer_ownership(ctx.accounts.authority.key(), current_time)?;
    agents.unlist(current_time)?;
    user.history.add_agents_purchased(price.2)?;
    user.history.add_fees_spent(price.1)?;
    user.validate_user()?;

    // Add trade count increment
    master_agent.increment_trade_count(current_time)?;
    master_agent.validate_security(current_time)?;
    // --- Emit event ---
    emit_cpi!(BuyAgentEvent {
        agent: agents.key(),
        owner: agents.owner,
        master_agent: agents.master_agent,
        timestamp: current_time
    });

    Ok(())
}
