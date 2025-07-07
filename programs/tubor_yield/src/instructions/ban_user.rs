use anchor_lang::prelude::*;

use crate::{
    error::{ErrorCode, TYieldResult},
    msg,
    state::{AdminInstruction, Multisig, User},
};

#[derive(Accounts)]
pub struct BanUser<'info> {
    /// Admin or operator initiating the ban.
    /// Must sign the transaction.
    #[account(mut)]
    pub admin: Signer<'info>,

    /// The multisig config account that controls protocol-level permissions.
    ///
    /// Seeds: ["multisig"]
    #[account(
        mut,
        seeds = [b"multisig"],
        bump = multisig.load()?.bump
    )]
    pub multisig: AccountLoader<'info, Multisig>,

    /// The user account to be banned.
    ///
    /// Seeds: ["user", user_authority]
    #[account(
        mut,
        seeds = [b"user", user_authority.key().as_ref()],
        bump = user.bump
    )]
    pub user: Box<Account<'info, User>>,

    /// CHECK: The authority (owner) of the user account.
    /// Provided for seed validation only.
    pub user_authority: AccountInfo<'info>,

    /// CHECK: Event authority for CPI event logs.
    #[account(
        seeds = [b"__event_authority"],
        bump,
    )]
    pub event_authority: AccountInfo<'info>,

    /// System program for rent / CPI calls.
    pub system_program: Program<'info, System>,
}

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

    let user = ctx.accounts.user.as_mut();
    user.ban_user();

    msg!(
        "User banned successfully. Authority: {}",
        ctx.accounts.user_authority.key()
    );

    Ok(0)
}
