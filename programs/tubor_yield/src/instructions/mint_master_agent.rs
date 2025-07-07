use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, TokenAccount},
    token_2022::Token2022,
};

use mpl_token_metadata::ID as METADATA_PROGRAM_ID;

use crate::state::{Multisig, TYield};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct MintMasterAgentParams {
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub seller_fee_basis_points: u16,
}

#[derive(Accounts)]
#[instruction(params: MintMasterAgentParams)]
pub struct MintMasterAgent<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"multisig"],
        bump = multisig.load()?.bump
    )]
    pub multisig: AccountLoader<'info, Multisig>,

    #[account(
        init,
        payer = payer,
        mint::decimals = 0,
        mint::authority = authority,
        mint::freeze_authority = authority,
    )]
    pub mint: Box<Account<'info, Mint>>,

    /// The t_yield config PDA (your protocol global state).
    ///
    /// Seeds: ["t_yield"]
    #[account(
        seeds = [b"t_yield"],
        bump = t_yield.t_yield_bump
    )]
    pub t_yield: Account<'info, TYield>,

    /// CHECK: empty PDA, authority for token accounts
    #[account(
            seeds = [b"transfer_authority"],
            bump = t_yield.transfer_authority_bump
        )]
    pub authority: AccountInfo<'info>,

    /// CHECK: Metadata account initialized by Metaplex program
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

    /// CHECK: This is the Metaplex token metadata program
    #[account(address = METADATA_PROGRAM_ID)]
    pub metadata_program: AccountInfo<'info>,

    /// CHECK: Master edition account initialized by Metaplex program
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

    #[account(
        mut,
        constraint = token_account.mint == mint.key()
        // init_if_needed,
        // payer = admin,
        // associated_token::mint = mint,
        // associated_token::authority = transfer_authority,
    )]
    pub token_account: Box<Account<'info, TokenAccount>>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// CHECK: This is the instructions sysvar, used by Metaplex CPI
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub sysvar_instructions: AccountInfo<'info>,
}
