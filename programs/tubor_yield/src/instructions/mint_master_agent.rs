//! Instruction: Mint Master Agent
//!
//! Mints a new master agent NFT, initializing protocol state and Metaplex metadata.
//! Requires multisig approval. Handles all account creations and protocol state updates.
//!
//! Accounts:
//! - Payer (signer)
//! - Multisig PDA (protocol admin control)
//! - Master agent account (PDA)
//! - Mint account for the master agent NFT
//! - Protocol state PDA (t_yield)
//! - Transfer authority PDA
//! - Metadata and master edition accounts (Metaplex)
//! - Metaplex metadata program
//! - Token account for the master agent NFT
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
    state::{
        AdminInstruction, MasterAgent, MasterAgentInitParams, Multisig, Size, TYield, TaxConfig,
        TradingStatus,
    },
};

/// Parameters for minting a new master agent NFT.
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct MintMasterAgentParams {
    /// Name for the master agent NFT (Metaplex metadata)
    pub name: String,
    /// Symbol for the master agent NFT (Metaplex metadata)
    pub symbol: String,
    /// URI for the master agent NFT metadata (Metaplex)
    pub uri: String,
    /// Seller fee basis points (Metaplex royalty)
    pub seller_fee_basis_points: u16,
    /// Initial price for the master agent
    pub price: u64,
    /// Initial yield (fixed-point, protocol-specific)
    pub w_yield: u64,
    /// Maximum supply of agents under this master agent
    pub max_supply: u64,
    /// Trading status for the master agent
    pub trading_status: TradingStatus,
    /// Whether to auto-relist the master agent
    pub auto_relist: bool,
}

/// Accounts required for minting a new master agent NFT.
///
/// This instruction:
/// - Creates and initializes the master agent mint and metadata (Metaplex)
/// - Initializes the master agent account (PDA)
/// - Updates protocol state
/// - Requires multisig approval
#[derive(Accounts)]
#[instruction(params: MintMasterAgentParams)]
pub struct MintMasterAgent<'info> {
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

    /// Master agent account PDA for the new master agent NFT.
    /// Seeds: [b"master_agent", mint]
    #[account(
        init,
        payer = payer,
        space = MasterAgent::SIZE,
        seeds = [b"master_agent".as_ref(), mint.key().as_ref()],
        bump,
    )]
    pub master_agent: Box<Account<'info, MasterAgent>>,

    /// Mint account for the new master agent NFT.
    #[account(
        init,
        payer = payer,
        mint::decimals = 0,
        mint::authority = authority,
        mint::freeze_authority = authority,
    )]
    pub mint: Box<Account<'info, Mint>>,

    /// Protocol global state PDA.
    /// Seeds: [b"t_yield"]
    #[account(
        seeds = [b"t_yield"],
        bump = t_yield.t_yield_bump
    )]
    pub t_yield: Account<'info, TYield>,

    /// Transfer authority PDA (protocol authority for token/NFT operations).
    /// Seeds: [b"transfer_authority"]
    /// CHECK: Only used as authority.
    #[account(
        seeds = [b"transfer_authority"],
        bump = t_yield.transfer_authority_bump
    )]
    pub authority: AccountInfo<'info>,

    /// Metadata account for the master agent NFT (Metaplex)
    /// PDA: ["metadata", METADATA_PROGRAM_ID, mint]
    /// CHECK: Created and validated by Metaplex CPI
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

    /// Metaplex token metadata program
    /// CHECK: Used for Metaplex CPI only
    #[account(address = METADATA_PROGRAM_ID)]
    pub metadata_program: AccountInfo<'info>,

    /// Master edition account for the master agent NFT (Metaplex)
    /// PDA: ["metadata", METADATA_PROGRAM_ID, mint, "edition"]
    /// CHECK: Created and validated by Metaplex CPI
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

    /// Token account for the master agent NFT (owned by protocol authority)
    /// Associated token account for the mint and authority
    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = authority,
    )]
    pub token_account: Box<Account<'info, TokenAccount>>,

    /// Solana system program.
    pub system_program: Program<'info, System>,
    /// SPL Token2022 program.
    pub token_program: Program<'info, Token2022>,
    /// SPL associated token program.
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// Instructions sysvar (required for Metaplex CPI)
    /// CHECK: Used for Metaplex CPI only
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub sysvar_instructions: AccountInfo<'info>,
}

/// Handler for minting a new master agent NFT.
///
/// - Requires multisig approval.
/// - Initializes the master agent account and mint.
/// - Creates Metaplex metadata and master edition accounts.
/// - Updates protocol state.
///
/// # Arguments
/// * `ctx` - Context with the required accounts.
/// * `params` - Minting parameters (name, symbol, uri, etc).
///
/// # Returns
/// * `Ok(signatures_left)` - If more multisig signatures are required.
/// * `Ok(0)` - If minting is complete.
/// * `Err` - On error (invalid bumps, instruction hash, etc).
pub fn mint_master_agent<'info>(
    ctx: Context<'_, '_, '_, 'info, MintMasterAgent<'info>>,
    params: MintMasterAgentParams,
) -> TYieldResult<u8> {
    // SECURITY: Check protocol state first
    if ctx.accounts.t_yield.paused {
        return Err(ErrorCode::EmergencyPauseActive);
    }

    let current_time = ctx.accounts.t_yield.get_time()?;

    // SECURITY: Check circuit breaker
    ctx.accounts.t_yield.check_circuit_breaker(current_time)?;

    // SECURITY: Check rate limiting
    ctx.accounts.t_yield.check_rate_limit(current_time)?;

    // SECURITY: Validate input parameters
    if params.price == 0 {
        return Err(ErrorCode::InvalidEntryPrice);
    }
    if params.w_yield == 0 {
        return Err(ErrorCode::InvalidState);
    }
    if params.max_supply == 0 {
        return Err(ErrorCode::InvalidState);
    }

    let mut multisig = ctx
        .accounts
        .multisig
        .load_mut()
        .map_err(|_| ErrorCode::InvalidBump)?;

    let instruction_data = Multisig::get_instruction_data(AdminInstruction::DeployAgent, &params)
        .map_err(|_| ErrorCode::InvalidInstructionHash)?;

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

    let master_agent = ctx.accounts.master_agent.as_mut();

    let master_agent_init_params = MasterAgentInitParams {
        authority: ctx.accounts.authority.key(),
        mint: ctx.accounts.mint.key(),
        price: params.price,
        w_yield: params.w_yield,
        trading_status: params.trading_status,
        max_supply: params.max_supply,
        auto_relist: params.auto_relist,
        current_time,
        bump: ctx.bumps.master_agent,
        tax_config: TaxConfig::default(), // SECURITY: Use default tax config
    };

    master_agent.initialize(master_agent_init_params)?;

    // SECURITY: Validate the initialized master agent
    master_agent.validate()?;
    master_agent.validate_security(current_time)?;

    ctx.accounts
        .t_yield
        .mint_master_agent(&ctx, params)
        .map_err(|_| ErrorCode::InvalidInstructionHash)?;

    Ok(0)
}
