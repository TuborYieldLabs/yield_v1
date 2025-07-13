//! Update a user's status (e.g., ban, activate, etc.) via multisig-controlled admin/operator action.
//!
//! This instruction allows an admin or operator, governed by a multisig, to update the status of a user account.
//! The action is subject to multisig approval and emits an event upon completion.

use anchor_lang::prelude::*;

use crate::{
    error::{ErrorCode, TYieldResult},
    state::{AdminInstruction, Multisig, TYield, UpdateUserStatusEvent, User, UserStatus},
};

/// Parameters for updating a user's status.
#[derive(Clone, Copy, AnchorDeserialize, AnchorSerialize)]
pub struct UpdateStatusParams {
    /// The new status to assign to the user (e.g., banned, active, etc.).
    pub status: UserStatus,
}

/// Accounts required for updating a user's status.
///
/// Account order:
/// 1. Admin/operator (signer)
/// 2. Target user account (mut)
/// 3. Authority of the user account (for seed validation)
/// 4. Multisig config (mut, protocol-level permissions)
/// 5. Protocol state (t_yield)
/// 6. Event authority (for CPI event logs)
/// 7. System program
#[derive(Accounts)]

pub struct UpdateStatus<'info> {
    /// Admin or operator initiating the status update.
    /// Must sign the transaction. Must be a member of the multisig.
    #[account(mut)]
    pub admin: Signer<'info>,

    /// The user account whose status will be updated.
    ///
    /// Seeds: ["user", user_authority]
    #[account(
        mut,
        seeds = [b"user", user_authority.key().as_ref()],
        bump = user.bump
    )]
    pub user: Box<Account<'info, User>>,

    /// The authority (owner) of the user account.
    /// Provided for seed validation only. Not mutated.
    /// CHECK: Only used as a seed.
    pub user_authority: AccountInfo<'info>,

    /// The multisig config account that controls protocol-level permissions.
    ///
    /// Seeds: ["multisig"]
    #[account(
        mut,
        seeds = [b"multisig"],
        bump = multisig.load()?.bump
    )]
    pub multisig: AccountLoader<'info, Multisig>,

    /// The protocol state account.
    ///
    /// Seeds: ["t_yield"]
    #[account(
        seeds = [b"t_yield"],
        bump = t_yield.t_yield_bump
    )]
    pub t_yield: Box<Account<'info, TYield>>,

    /// Event authority for CPI event logs.
    ///
    /// Seeds: ["__event_authority"]
    /// CHECK: Only used for event emission.
    #[account(
        seeds = [b"__event_authority"],
        bump,
    )]
    pub event_authority: AccountInfo<'info>,

    /// System program for rent / CPI calls.
    pub system_program: Program<'info, System>,
}

/// Handler for updating a user's status via multisig-controlled admin/operator action.
///
/// - Requires multisig approval.
/// - Emits an `UpdateUserStatusEvent` upon success.
/// - Returns the number of signatures still required (0 if fully approved and executed).
///
pub fn update_status<'info>(
    ctx: Context<'_, '_, '_, 'info, UpdateStatus<'info>>,
    params: &UpdateStatusParams,
) -> TYieldResult<u8> {
    let mut multisig = ctx
        .accounts
        .multisig
        .load_mut()
        .map_err(|_| ErrorCode::InvalidBump)?;

    let instruction_data = Multisig::get_instruction_data(AdminInstruction::PermManager, &params)
        .map_err(|_| ErrorCode::InvalidInstructionHash)?;

    let current_time = ctx.accounts.t_yield.get_time()?;
    let nonce = current_time as u64; // Use current time as nonce for simplicity

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

    let user = ctx.accounts.user.as_mut();
    user.add_user_status(params.status)?;

    emit_cpi!(UpdateUserStatusEvent {
        authority: user.authority,
        name: user.name,
        status: user.status,
        updated_at: current_time,
    });

    Ok(0)
}
