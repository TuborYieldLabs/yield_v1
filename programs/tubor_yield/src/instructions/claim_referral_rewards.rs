use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::{
    error::{ErrorCode, TYieldResult},
    math::SafeMath,
    state::{ReferralRegistry, TYield, User},
    try_from,
};

#[derive(Accounts)]
pub struct ClaimReferralRewards<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [b"user", authority.key().as_ref()],
        bump = user.bump
    )]
    pub user: Box<Account<'info, User>>,

    /// Referral registry PDA.
    #[account(
        seeds = [b"referral_registry", authority.key().as_ref()],
        bump =  referral_registry.bump
    )]
    pub referral_registry: Box<Account<'info, ReferralRegistry>>,

    #[account(
        mut,
        seeds = [b"t_yield"],
        bump = t_yield.t_yield_bump
    )]
    pub t_yield: Box<Account<'info, TYield>>,

    ///CHECK: y_mint
    #[account(address = t_yield.y_mint)]
    pub y_mint: AccountInfo<'info>,

    /// CHECK: empty PDA, authority for token accounts
    #[account(
            seeds = [b"transfer_authority"],
            bump = t_yield.transfer_authority_bump
        )]
    pub transfer_authority: AccountInfo<'info>,

    #[account(mut,
        constraint = user_token_account.mint == t_yield.y_mint
    )]
    pub user_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut,
        constraint = protocol_token_account.mint == t_yield.y_mint
    )]
    pub protocol_token_account: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,

    // --- Misc ---
    /// CHECK: Event authority for CPI event logs (used for event emission; not written to).
    #[account(
        seeds = [b"__event_authority"],
        bump,
    )]
    pub event_authority: AccountInfo<'info>,
}

pub fn claim_referral_rewards<'info>(
    ctx: Context<'_, '_, '_, 'info, ClaimReferralRewards<'info>>,
) -> TYieldResult<u8> {
    let user = ctx.accounts.user.as_mut();
    let referral_registry = ctx.accounts.referral_registry.as_mut();
    let t_yield = ctx.accounts.t_yield.as_mut();

    if !user.can_perform_actions() {
        return Err(ErrorCode::CannotPerformAction);
    }

    // 1. Check user has referral rewards
    let unclaimed_referral_earnings = referral_registry.get_total_unclaimed_referral_earnings();

    // 2. Transfer tokens from protocol to user

    let mint =
        try_from!(Account<Mint>, ctx.accounts.y_mint).map_err(|_| ErrorCode::AccountFromError)?;

    TYield::transfer_tokens(
        ctx.accounts.protocol_token_account.to_account_info(),
        mint.to_account_info(),
        ctx.accounts.user_token_account.to_account_info(),
        ctx.accounts.transfer_authority.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        unclaimed_referral_earnings,
        mint.decimals,
    )
    .map_err(|_| ErrorCode::InvalidInstructionHash)?;

    // 3. Update protocol/user balances

    referral_registry.claim_referral_earnings(unclaimed_referral_earnings)?;

    t_yield.protocol_total_balance_usd = t_yield
        .protocol_total_balance_usd
        .safe_sub(unclaimed_referral_earnings)?;

    user.history
        .add_referral_earnings(unclaimed_referral_earnings)?;

    // 4. Return Ok(0) or error
    Ok(0)
}
