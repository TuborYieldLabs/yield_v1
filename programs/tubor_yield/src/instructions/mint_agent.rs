//! Instruction: Mint Agent
//!
//! Mints a new agent NFT under a master agent, initializing all relevant protocol state and Metaplex metadata.
//! Requires multisig approval. Handles all account creations and protocol state updates.
//!
//! Accounts:
//! - Payer (signer)
//! - Multisig PDA (protocol admin control)
//! - Protocol state PDA (t_yield)
//! - Transfer authority PDA
//! - Mint account for the new agent NFT
//! - Metadata and master edition accounts (Metaplex)
//! - Master agent mint and account
//! - Agent account (PDA)
//! - Token account for the agent NFT
//! - System, Token2022, Associated Token programs
//! - Sysvar instructions (for Metaplex CPI)

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, TokenAccount},
    token_2022::Token2022,
};

use mpl_token_metadata::ID as METADATA_PROGRAM_ID;

use crate::{
    error::{ErrorCode, TYieldResult},
    state::{AdminInstruction, Agent, MasterAgent, MintAgentEvent, Multisig, Size, TYield},
};

/// Parameters for minting a new agent NFT.
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct MintAgentParams {
    /// Name for the agent NFT (Metaplex metadata)
    pub name: String,
    /// Symbol for the agent NFT (Metaplex metadata)
    pub symbol: String,
    /// URI for the agent NFT metadata (Metaplex)
    pub uri: String,
    /// Seller fee basis points (Metaplex royalty)
    pub seller_fee_basis_points: u16,
}

/// Accounts required for minting a new agent NFT under a master agent.
///
/// This instruction:
/// - Creates and initializes the agent mint and metadata (Metaplex)
/// - Initializes the agent account (PDA)
/// - Updates protocol and master agent state
/// - Requires multisig approval
#[derive(Accounts)]
#[instruction(params: MintAgentParams)]
pub struct MintAgent<'info> {
    /// The account paying for all rent and fees. Must sign the transaction.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Multisig PDA for protocol admin control.
    /// Seeds: [b"multisig"]
    #[account(
        mut,
        seeds = [b"multisig"],
        bump = multisig.load()?.bump
    )]
    pub multisig: AccountLoader<'info, Multisig>,

    /// Protocol global state PDA.
    /// Seeds: [b"t_yield"]
    #[account(
        seeds = [b"t_yield"],
        bump = t_yield.t_yield_bump
    )]
    pub t_yield: Box<Account<'info, TYield>>,

    /// Transfer authority PDA (protocol authority for token/NFT operations).
    /// Seeds: [b"transfer_authority"]
    /// CHECK: Only used as authority.
    #[account(
        seeds = [b"transfer_authority"],
        bump = t_yield.transfer_authority_bump
    )]
    pub authority: AccountInfo<'info>,

    /// Mint account for the new agent NFT.
    #[account(
        init,
        payer = payer,
        mint::decimals = 0,
        mint::authority = authority,
        mint::freeze_authority = authority,
    )]
    pub mint: Box<Account<'info, Mint>>,

    /// CHECK: Metadata account for the agent NFT (Metaplex)
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

    /// CHECK: Master edition account for the agent NFT (Metaplex)
    /// PDA: ["metadata", METADATA_PROGRAM_ID, mint, "edition"]
    #[account(
        mut,
        seeds = [
            b"metadata",
            METADATA_PROGRAM_ID.as_ref(),
            mint.key().as_ref(),
            b"edition",
        ],
        bump,
        seeds::program = METADATA_PROGRAM_ID,
    )]
    pub master_edition: AccountInfo<'info>,

    /// CHECK: Metaplex token metadata program
    #[account(address = METADATA_PROGRAM_ID)]
    pub metadata_program: AccountInfo<'info>,

    /// Mint account for the master agent NFT (parent/master of the agent)
    #[account(mut)]
    pub master_agent_mint: Box<Account<'info, Mint>>,

    /// Master agent account PDA (parent/master of the agent)
    /// Seeds: [b"master_agent", master_agent_mint]
    #[account(
        mut,
        seeds = [b"master_agent".as_ref(), master_agent_mint.key().as_ref()],
        bump = master_agent.bump,
    )]
    pub master_agent: Box<Account<'info, MasterAgent>>,

    /// Agent account PDA for the new agent NFT.
    /// Seeds: [b"agent", mint]
    #[account(
        init_if_needed,
        payer = payer,
        space = Agent::SIZE,
        seeds = [b"agent".as_ref(), mint.key().as_ref()],
        bump,
    )]
    pub agent: Box<Account<'info, Agent>>,

    /// Token account for the agent NFT (owned by protocol authority)
    /// Must have mint == agent mint.
    #[account(
        mut,
        constraint = token_account.mint == mint.key()
    )]
    pub token_account: Box<Account<'info, TokenAccount>>,

    /// Solana system program.
    pub system_program: Program<'info, System>,
    /// SPL Token2022 program.
    pub token_program: Program<'info, Token2022>,
    /// SPL associated token program.
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// CHECK: Instructions sysvar (required for Metaplex CPI)
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub sysvar_instructions: AccountInfo<'info>,

    // --- Misc ---
    /// CHECK: Event authority for CPI event logs (used for event emission; not written to).
    #[account(
        seeds = [b"__event_authority"],
        bump,
    )]
    pub event_authority: AccountInfo<'info>,
}

/// Handler for minting a new agent NFT under a master agent.
///
/// - Requires multisig approval.
/// - Initializes agent mint, metadata, and agent account.
/// - Updates protocol and master agent state.
///
/// # Arguments
/// * `ctx` - Context with the required accounts.
/// * `params` - Parameters for the agent NFT (name, symbol, uri, royalty).
///
/// # Returns
/// * `Ok(0)` if successful, or the number of signatures left for multisig approval.
pub fn mint_agent<'info>(
    ctx: Context<'_, '_, '_, 'info, MintAgent<'info>>,
    params: MintAgentParams,
) -> TYieldResult<u8> {
    let mut multisig = ctx
        .accounts
        .multisig
        .load_mut()
        .map_err(|_| ErrorCode::InvalidBump)?;

    let instruction_data = Multisig::get_instruction_data(AdminInstruction::DeployAgent, &params)
        .map_err(|_| ErrorCode::InvalidInstructionHash)?;

    let current_time = ctx.accounts.t_yield.get_time()?;
    let nonce = current_time as u64; // Use current time as nonce for simplicity

    let signatures_left = multisig.sign_multisig(
        &ctx.accounts.payer,
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

    // Validate protocol permissions
    if !ctx.accounts.t_yield.permissions.allow_agent_deploy {
        return Err(ErrorCode::CannotPerformAction);
    }

    // Validate master agent has available supply
    if ctx.accounts.master_agent.is_supply_full() {
        return Err(ErrorCode::CannotPerformAction);
    }

    {
        let master_agent = ctx.accounts.master_agent.as_mut();
        let agent = ctx.accounts.agent.as_mut();

        // Initialize agent with protocol ownership and default booster
        agent.initialize(
            master_agent.key(),
            ctx.accounts.mint.key(),
            ctx.accounts.authority.key(), // Protocol authority owns the agent initially
            10,
            current_time,
            ctx.bumps.agent,
        )?;

        agent.validate()?;

        // Add agent to master agent's count
        master_agent.add_agent(current_time)?;
    }

    // Create metadata and mint the NFT
    ctx.accounts
        .t_yield
        .mint_agent(&ctx, params)
        .map_err(|_| ErrorCode::InvalidInstructionHash)?;

    let agent = ctx.accounts.agent.as_ref();

    emit_cpi!(MintAgentEvent {
        agent: agent.key(),
        owner: agent.owner, // This is the protocol authority
        master_agent: agent.master_agent,
        timestamp: current_time
    });

    Ok(0)
}
