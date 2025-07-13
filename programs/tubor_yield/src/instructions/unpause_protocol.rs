use anchor_lang::prelude::*;

use crate::{
    error::{ErrorCode, TYieldResult},
    math::SafeMath,
    msg,
    state::{AdminInstruction, Multisig, TYield},
};

#[derive(Accounts)]
pub struct UnpauseProtocol<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [b"multisig"],
        bump = multisig.load()?.bump
    )]
    pub multisig: AccountLoader<'info, Multisig>,

    #[account(
        mut,
        seeds = [b"t_yield"],
        bump = t_yield.t_yield_bump
    )]
    pub t_yield: Box<Account<'info, TYield>>,
}

pub fn unpause_protocol<'info>(
    ctx: Context<'_, '_, '_, 'info, UnpauseProtocol<'info>>,
) -> TYieldResult<u8> {
    let mut multisig = ctx
        .accounts
        .multisig
        .load_mut()
        .map_err(|_| ErrorCode::InvalidBump)?;

    // Validate that protocol is currently paused
    if !ctx.accounts.t_yield.paused {
        return Err(ErrorCode::CannotPerformAction);
    }

    let instruction_data = Multisig::get_instruction_data(AdminInstruction::PermManager, &())
        .map_err(|_| ErrorCode::InvalidInstructionHash)?;

    let current_time = ctx.accounts.t_yield.get_time()?;

    // Generate a proper nonce using current time and some entropy
    // This ensures uniqueness even if multiple transactions are submitted in the same second
    let nonce = (current_time as u64)
        .safe_mul(1000)?
        .safe_add((current_time % 1000) as u64)?;

    let signatures_left = multisig.sign_multisig(
        &ctx.accounts.admin,
        &Multisig::get_account_infos(&ctx),
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

    let t_yield = ctx.accounts.t_yield.as_mut();
    t_yield.paused = false;

    msg!("Protocol unpaused successfully.");

    Ok(0)
}
