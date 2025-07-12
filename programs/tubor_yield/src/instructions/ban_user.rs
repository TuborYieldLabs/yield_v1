use anchor_lang::prelude::*;

use crate::{
    error::{ErrorCode, TYieldResult},
    msg,
    state::{AdminInstruction, Multisig, TYield, UpdateUserStatusEvent, User},
};

/// Accounts required for banning a user from the protocol.
///
/// This instruction allows an admin or operator (with multisig approval) to ban a user account.
/// The user account is marked as banned and must be validated after the operation.
#[derive(Accounts)]
pub struct BanUser<'info> {
    /// The admin or operator initiating the ban.
    /// Must be a signer and authorized by the multisig.
    #[account(mut)]
    pub admin: Signer<'info>,

    /// The multisig config account that controls protocol-level permissions.
    ///
    /// Seeds: ["multisig"]
    /// Must be mutable to update signature state.
    #[account(
        mut,
        seeds = [b"multisig"],
        bump = multisig.load()?.bump
    )]
    pub multisig: AccountLoader<'info, Multisig>,

    #[account(
        seeds = [b"t_yield"],
        bump = t_yield.t_yield_bump
    )]
    pub t_yield: Box<Account<'info, TYield>>,

    /// The user account to be banned.
    ///
    /// Seeds: ["user", user_authority]
    /// Must be mutable to update the banned status.
    #[account(
        mut,
        seeds = [b"user", user_authority.key().as_ref()],
        bump = user.bump
    )]
    pub user: Box<Account<'info, User>>,

    /// The authority (owner) of the user account.
    /// CHECK: Used only for seed validation; not written to.
    pub user_authority: AccountInfo<'info>,

    /// Event authority for CPI event logs.
    /// CHECK: Used for event emission; not written to.
    #[account(
        seeds = [b"__event_authority"],
        bump,
    )]
    pub event_authority: AccountInfo<'info>,

    /// System program for rent and CPI calls.
    pub system_program: Program<'info, System>,
}

/// Handler for banning a user from the protocol.
///
/// Requires multisig approval. Marks the user as banned and validates the user account.
/// Returns the number of signatures still required for multisig execution (0 if complete).
pub fn ban_user<'info>(ctx: Context<'_, '_, '_, 'info, BanUser<'info>>) -> TYieldResult<u8> {
    let mut multisig = ctx
        .accounts
        .multisig
        .load_mut()
        .map_err(|_| ErrorCode::InvalidBump)?;

    let instruction_data = Multisig::get_instruction_data(AdminInstruction::BanUser, &())
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

    let current_time = ctx.accounts.t_yield.get_time()?;
    let user = ctx.accounts.user.as_mut();

    user.ban_user();
    user.validate_user()?;

    emit_cpi!(UpdateUserStatusEvent {
        authority: user.authority,
        name: user.name,
        status: user.status,
        updated_at: current_time,
    });

    Ok(0)
}
