use anchor_lang::prelude::*;

use crate::{
    error::{ErrorCode, TYieldResult},
    state::{Agent, TYield, User},
};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct TransferAgentParams {
    pub new_owner: Pubkey,
}

#[derive(Accounts)]
#[instruction(params: TransferAgentParams)]
pub struct TransferAgent<'info> {
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
        seeds = [b"agent", agent.key().as_ref()],
        bump = agent.bump
    )]
    pub agent: Box<Account<'info, Agent>>,

    #[account(
        mut,
        seeds = [b"t_yield"],
        bump = t_yield.t_yield_bump
    )]
    pub t_yield: Box<Account<'info, TYield>>,

    #[account(
        mut,
        seeds = [b"user", params.new_owner.as_ref()],
        bump
    )]
    pub new_owner_user: Box<Account<'info, User>>,
}

pub fn transfer_agent<'info>(
    ctx: Context<'_, '_, '_, 'info, TransferAgent<'info>>,
    params: TransferAgentParams,
) -> TYieldResult<u8> {
    let current_time = ctx.accounts.t_yield.get_time()?;
    let sender = ctx.accounts.user.as_mut();
    let receiver = ctx.accounts.new_owner_user.as_mut();
    let agent = ctx.accounts.agent.as_mut();

    // 1. Check sender is the current owner
    if !agent.is_owned_by(&sender.authority) {
        return Err(ErrorCode::CannotPerformAction);
    }
    // 2. Check sender is not banned
    if sender.is_banned() {
        return Err(ErrorCode::CannotPerformAction);
    }
    // 3. Check receiver is not banned
    if receiver.is_banned() {
        return Err(ErrorCode::CannotPerformAction);
    }
    // 4. Update agent ownership
    agent.transfer_ownership(params.new_owner, current_time)?;
    // 5. Update user stats
    sender.remove_agent(1)?;
    receiver.add_agent(1)?;
    // 6. Validate both users
    sender.validate_user()?;
    receiver.validate_user()?;
    Ok(0)
}
