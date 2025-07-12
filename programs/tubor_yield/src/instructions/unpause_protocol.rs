use anchor_lang::prelude::*;

use crate::{
    error::{ErrorCode, TYieldResult},
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

    let instruction_data = Multisig::get_instruction_data(AdminInstruction::PermManager, &())
        .map_err(|_| ErrorCode::InvalidInstructionHash)?;

    let signatures_left = multisig.sign_multisig(
        &ctx.accounts.admin,
        &Multisig::get_account_infos(&ctx)[1..],
        &instruction_data,
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
