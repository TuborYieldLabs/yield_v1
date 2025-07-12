use anchor_lang::prelude::*;

use crate::{
    error::{ErrorCode, TYieldResult},
    msg,
    state::{AdminInstruction, Multisig, TYield, UpdateProtocolEvent},
};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct UpdateProtocolConfigParams {
    pub buy_tax: Option<u64>,
    pub sell_tax: Option<u64>,
    pub max_tax_percentage: Option<u64>,
    pub allow_agent_deploy: Option<bool>,
    pub allow_agent_buy: Option<bool>,
    pub allow_agent_sell: Option<bool>,
    pub allow_withdraw_yield: Option<bool>,
    // Add more config fields as needed
}

#[derive(Accounts)]
pub struct UpdateProtocolConfig<'info> {
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

    /// Solana system program.
    pub system_program: Program<'info, System>,

    /// Event authority for CPI event logs (used for event emission; not written to).
    /// Seeds: ["__event_authority"]
    /// CHECK: Derived by Anchor for event emission.
    #[account(
            seeds = [b"__event_authority"],
            bump,
        )]
    pub event_authority: AccountInfo<'info>,
}

pub fn update_protocol_config<'info>(
    ctx: Context<'_, '_, '_, 'info, UpdateProtocolConfig<'info>>,
    params: UpdateProtocolConfigParams,
) -> TYieldResult<u8> {
    let mut multisig = ctx
        .accounts
        .multisig
        .load_mut()
        .map_err(|_| ErrorCode::InvalidBump)?;

    let instruction_data = Multisig::get_instruction_data(AdminInstruction::PermManager, &params)
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
    if let Some(buy_tax) = params.buy_tax {
        t_yield.buy_tax = buy_tax;
    }
    if let Some(sell_tax) = params.sell_tax {
        t_yield.sell_tax = sell_tax;
    }
    if let Some(max_tax_percentage) = params.max_tax_percentage {
        t_yield.max_tax_percentage = max_tax_percentage;
    }
    if let Some(allow_agent_deploy) = params.allow_agent_deploy {
        t_yield.permissions.allow_agent_deploy = allow_agent_deploy;
    }
    if let Some(allow_agent_buy) = params.allow_agent_buy {
        t_yield.permissions.allow_agent_buy = allow_agent_buy;
    }
    if let Some(allow_agent_sell) = params.allow_agent_sell {
        t_yield.permissions.allow_agent_sell = allow_agent_sell;
    }
    if let Some(allow_withdraw_yield) = params.allow_withdraw_yield {
        t_yield.permissions.allow_withdraw_yield = allow_withdraw_yield;
    }

    msg!("Protocol config updated successfully.");

    emit_cpi!(UpdateProtocolEvent {});

    Ok(0)
}
