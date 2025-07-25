use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::{
    error::{ErrorCode, TYieldResult},
    math::SafeMath,
    state::{TYield, User},
};

use crate::try_from;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct WithdrawYieldParams {
    pub amount: Option<u64>, // None = withdraw all
}

#[derive(Accounts)]
pub struct WithdrawYield<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [b"user", authority.key().as_ref()],
        bump = user.bump
    )]
    pub user: Box<Account<'info, User>>,

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

    #[account(mut, constraint = user_token_account.mint == t_yield.y_mint)]
    pub user_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut, constraint = protocol_token_account.mint == t_yield.y_mint)]
    pub protocol_token_account: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}

pub fn withdraw_yield<'info>(
    ctx: Context<'_, '_, '_, 'info, WithdrawYield<'info>>,
    params: WithdrawYieldParams,
) -> TYieldResult<u8> {
    let user = ctx.accounts.user.as_mut();
    let t_yield = ctx.accounts.t_yield.as_mut();

    if !user.can_perform_actions() {
        return Err(ErrorCode::CannotPerformAction);
    }

    // 1. Determine withdrawable amount
    let claimable = user.get_claimable_yield();
    let amount = match params.amount {
        Some(a) => {
            if a > claimable {
                return Err(ErrorCode::InsufficientFunds);
            }
            a
        }
        None => {
            if claimable == 0 {
                return Err(ErrorCode::InsufficientFunds);
            }
            claimable
        }
    };

    // 2. Transfer tokens from protocol to user
    let mint =
        try_from!(Account<Mint>, ctx.accounts.y_mint).map_err(|_| ErrorCode::AccountFromError)?;

    crate::state::TYield::transfer_tokens(
        ctx.accounts.protocol_token_account.to_account_info(),
        mint.to_account_info(),
        ctx.accounts.user_token_account.to_account_info(),
        ctx.accounts.transfer_authority.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        amount,
        mint.decimals,
    )
    .map_err(|_| ErrorCode::InvalidInstructionHash)?;

    // 3. Update protocol/user balances
    user.claim_yield(amount)?;
    t_yield.protocol_total_balance_usd = t_yield.protocol_total_balance_usd.safe_sub(amount)?;

    // 4. Return Ok(0) or error
    Ok(0)
}
