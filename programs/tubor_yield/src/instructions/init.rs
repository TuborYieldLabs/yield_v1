use crate::program::Tuboryield;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Token, TokenAccount},
};

use {
    crate::{
        error::TYieldResult,
        state::{Multisig, Size, TYield},
    },
    anchor_lang::prelude::*,
};

#[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone)]
pub struct InitParams {
    pub min_signatures: u8,
    pub allow_agent_deploy: bool,
    pub allow_agent_buy: bool,
    pub allow_agent_sell: bool,
    pub allow_withdraw_yield: bool,
    pub buy_tax: u64,
    pub sell_tax: u64,
    pub ref_earn_percentage: u64,
    pub supported_mint: Pubkey,
}

#[derive(Accounts)]
#[instruction(params:InitParams)]
pub struct Init<'info> {
    #[account(mut)]
    pub upgrade_authority: Signer<'info>,

    #[account(
        init,
        payer = upgrade_authority,
        space = Multisig::SIZE,
        seeds = [b"multisig"],
        bump
    )]
    pub multisig: AccountLoader<'info, Multisig>,

    #[account(
        init,
        payer = upgrade_authority,
        space = TYield::SIZE,
        seeds = [b"t_yield"],
        bump
    )]
    pub t_yield: Box<Account<'info, TYield>>,

    /// CHECK: This is safe because transfer_authority is a program-derived address (PDA) controlled by the program and is only used as an authority for token operations.
    #[account(
        init,
        payer = upgrade_authority,
        space = 0,
        seeds = [b"transfer_authority"],
        bump
    )]
    pub transfer_authority: AccountInfo<'info>,

    /// CHECK: ProgramData account, doesn't work in tests
    #[account()]
    pub t_yield_program_data: AccountInfo<'info>,

    pub t_yield_program: Program<'info, Tuboryield>,

    #[account(
        address = params.supported_mint
    )]
    pub supported_mint: AccountInfo<'info>,

    #[account(
         init_if_needed,
        payer = upgrade_authority,
        associated_token::mint = supported_mint,
        associated_token::authority = transfer_authority,
    )]
    pub supported_mint_token_account: Box<Account<'info, TokenAccount>>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    // remaining accounts: 1 to Multisig::MAX_SIGNERS admin signers (read-only, unsigned)
}

pub fn init(ctx: Context<Init>, params: &InitParams) -> TYieldResult<()> {
    TYield::validate_upgrade_authority(
        ctx.accounts.upgrade_authority.key(),
        &ctx.accounts.t_yield_program_data.to_account_info(),
        &ctx.accounts.t_yield_program,
    )
    .map_err(|_| crate::error::ErrorCode::InvalidAuthority)?;

    // initialize multisig, this will fail if account is already initialized
    let mut multisig = ctx
        .accounts
        .multisig
        .load_init()
        .map_err(|_| crate::error::ErrorCode::InvalidBump)?;

    multisig
        .set_signers(ctx.remaining_accounts, params.min_signatures)
        .map_err(|_| crate::error::ErrorCode::InvalidBump)?;

    // record multisig PDA bump
    multisig.bump = ctx.bumps.multisig;

    // record perpetuals
    let t_yield = ctx.accounts.t_yield.as_mut();

    t_yield.permissions.allow_agent_deploy = params.allow_agent_deploy;
    t_yield.permissions.allow_agent_buy = params.allow_agent_buy;
    t_yield.permissions.allow_agent_sell = params.allow_agent_sell;
    t_yield.permissions.allow_withdraw_yield = params.allow_withdraw_yield;

    t_yield.transfer_authority_bump = ctx.bumps.transfer_authority;
    t_yield.t_yield_bump = ctx.bumps.t_yield;

    t_yield.y_mint = ctx.accounts.supported_mint.key();

    t_yield.buy_tax = params.buy_tax;
    t_yield.sell_tax = params.sell_tax;

    t_yield.inception_time = t_yield.get_time()?;

    Ok(())
}
