//! Instruction: Sell Agent
//!
//! Allows a user to sell an agent NFT back to the protocol (or master agent), transferring ownership and updating protocol/user state.
//! Handles price calculation, tax, and all token/NFT transfers. Enforces ban and protocol constraints.
//!
//! Accounts:
//! - authority: The user selling the agent (signer)
//! - system_program: Solana system program
//! - token_program: SPL token program
//! - associated_token_program: SPL associated token program
//! - sysvar_instructions: Instructions sysvar (for Metaplex CPI)
//! - metadata_program: Metaplex token metadata program
//! - metadata: Metadata account for the agent NFT (Metaplex)
//! - user: User account PDA
//! - agent: Agent account PDA (the agent being sold)
//! - master_agent: Master agent account PDA (parent/master of the agent)
//! - mint: Mint account for the agent NFT
//! - t_yield: Protocol global state/config PDA
//! - transfer_authority: Transfer authority PDA (protocol authority for token/NFT transfers)
//! - transfer_authority_ta: Protocol's token account holding the agent NFT (receiver)
//! - user_agent_ta: User's token account holding the agent NFT (sender)
//! - user_y_mint_ta: User's Y-mint token account (receiver of payment)
//! - transfer_authority_y_mint_ta: Protocol's Y-mint token account (payer of payment)
//! - y_mint: Y-mint SPL token mint (payment token for protocol)
//! - event_authority: Event authority for CPI event logs (used for event emission)

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount},
};

use mpl_token_metadata::ID as METADATA_PROGRAM_ID;

use crate::{
    error::{ErrorCode, TYieldResult},
    math::SafeMath,
    state::{Agent, MasterAgent, SellAgentEvent, TYield, TransferAgentParams, User},
    try_from,
};

#[derive(Accounts)]
pub struct SellAgent<'info> {
    /// The user selling the agent. Must sign the transaction.
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

    /// Agent account PDA (the agent being sold).
    /// PDA: ["agent", mint]
    #[account(
        mut,
        seeds = [b"agent".as_ref(), mint.key().as_ref()],
        bump,
    )]
    pub agent: Box<Account<'info, Agent>>,

    /// Master agent account PDA (parent/master of the agent).
    /// PDA: ["master_agent", agent.master_agent]
    #[account(
        mut,
        seeds = [b"master_agent".as_ref(), agent.master_agent.as_ref()],
        bump = master_agent.bump,
    )]
    pub master_agent: Box<Account<'info, MasterAgent>>,

    /// Mint account for the agent NFT being sold.
    #[account(mut)]
    pub mint: Box<Account<'info, Mint>>,

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
    /// Protocol's token account to receive the agent NFT.
    /// Must have mint == agent mint.
    #[account(
        mut,
        constraint = transfer_authority_ta.mint == mint.key()
    )]
    pub transfer_authority_ta: Box<Account<'info, TokenAccount>>,

    /// User's token account holding the agent NFT (sender).
    /// Must have mint == agent mint.
    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = mint,
        associated_token::authority = authority,
    )]
    pub user_agent_ta: Box<Account<'info, TokenAccount>>,

    /// User's Y-mint token account (receiver of payment).
    /// Must have mint == t_yield.y_mint.
    #[account(
        mut,
        constraint = user_y_mint_ta.mint == t_yield.y_mint
    )]
    pub user_y_mint_ta: Box<Account<'info, TokenAccount>>,

    /// Protocol's Y-mint token account (payer of payment).
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
    /// Event authority for CPI event logs (used for event emission; not written to).
    /// Seeds: ["__event_authority"]
    /// CHECK: Derived by Anchor for event emission.
    #[account(
        seeds = [b"__event_authority"],
        bump,
    )]
    pub event_authority: AccountInfo<'info>,
}

pub fn sell_agent<'info>(ctx: Context<'_, '_, '_, 'info, SellAgent<'info>>) -> TYieldResult<()> {
    let current_time = ctx.accounts.t_yield.get_time()?;
    let master_agent = ctx.accounts.master_agent.as_mut();
    let user = ctx.accounts.user.as_mut();
    let agents = ctx.accounts.agent.as_mut();
    let t_yield = ctx.accounts.t_yield.as_mut();

    if user.has_status(crate::state::UserStatus::Banned) {
        return Err(ErrorCode::CannotPerformAction);
    }

    // Validate agent ownership
    if !agents.is_owned_by(&ctx.accounts.authority.key()) {
        return Err(ErrorCode::CannotPerformAction);
    }

    if !agents.belongs_to_master_agent(&master_agent.key()) {
        return Err(ErrorCode::CannotPerformAction);
    }

    let price = master_agent.calculate_sell_price_with_tax()?;

    let mint =
        try_from!(Account<Mint>, ctx.accounts.y_mint).map_err(|_| ErrorCode::AccountFromError)?;

    TYield::transfer_tokens(
        ctx.accounts.transfer_authority_y_mint_ta.to_account_info(),
        mint.to_account_info(),
        ctx.accounts.user_y_mint_ta.to_account_info(),
        ctx.accounts.transfer_authority.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        price.0, // net_price (what user receives)
        mint.decimals,
    )
    .map_err(|_| ErrorCode::InvalidInstructionHash)?;

    t_yield.protocol_total_fees = t_yield.protocol_total_fees.safe_add(price.1)?;
    t_yield.protocol_current_holding = t_yield.protocol_current_holding.safe_sub(price.0)?; // Use net_price instead of base_price
    t_yield.protocol_total_balance_usd = t_yield.protocol_total_balance_usd.safe_sub(price.1)?;
    t_yield.protocol_total_earnings = t_yield.protocol_total_earnings.safe_add(price.1)?;

    let transfer_agent_params = TransferAgentParams {
        payer: ctx.accounts.authority.to_account_info(),
        sender_nft_token_account: ctx.accounts.user_agent_ta.to_account_info(),
        authority: ctx.accounts.authority.to_account_info(),
        receiver_token_account: ctx.accounts.transfer_authority_ta.to_account_info(),
        receiver: ctx.accounts.transfer_authority.to_account_info(),
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

    master_agent.add_agent(current_time)?;

    agents.transfer_ownership(ctx.accounts.transfer_authority.key(), current_time)?;
    agents.list(current_time)?;

    user.remove_agent(1)?;
    user.history.add_fees_spent(price.1)?;

    user.validate_user()?;

    emit_cpi!(SellAgentEvent {
        agent: agents.key(),
        owner: agents.owner,
        master_agent: agents.master_agent,
        timestamp: current_time
    });

    Ok(())
}
