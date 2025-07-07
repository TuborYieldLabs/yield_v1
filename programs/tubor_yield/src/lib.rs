#![allow(unexpected_cfgs)]
#![allow(deprecated)]

use anchor_lang::prelude::*;

pub mod error;
pub mod instructions;
pub mod macros;
pub mod math;
pub mod state;

use instructions::*;

declare_id!("EiifDJcZo3QthKQ2ZrdNSMsDufw4A4sGdsEQkZyRnhNs");

#[program]
pub mod tuboryield {
    use crate::error::TYieldResult;

    use super::*;

    pub fn init<'info>(
        ctx: Context<'_, '_, '_, 'info, Init<'info>>,
        params: InitParams,
    ) -> TYieldResult<()> {
        instructions::init(ctx, &params)
    }

    // pub fn ban_user<'info>(
    //     ctx: Context<'_, '_, '_, 'info, ban_user::BanUser<'info>>,
    // ) -> error::TYieldResult<u8> {
    //     instructions::ban_user(ctx)
    // }

    // pub fn update_status<'info>(
    //     ctx: Context<'_, '_, '_, 'info, update_status::UpdateStatus<'info>>,
    //     params: &update_status::UpdateStatusParams,
    // ) -> error::TYieldResult<u8> {
    //     instructions::update_status(ctx, params)
    // }

    // pub fn mint_master_agent(
    //     ctx: Context<mint_master_agent::MintMasterAgent>,
    //     params: mint_master_agent::MintMasterAgentParams,
    // ) -> Result<()> {
    //     instructions::mint_master_agent(ctx, params)
    // }

    // pub fn mint_agent(
    //     ctx: Context<mint_agent::MintAgent>,
    //     params: mint_agent::MintAgentParams,
    // ) -> Result<()> {
    //     instructions::mint_agent(ctx, params)
    // }

    // pub fn register_user(
    //     ctx: Context<register_user::RegisterUser>,
    //     params: register_user::RegisterUserParams,
    // ) -> Result<()> {
    //     instructions::register_user(ctx, params)
    // }
}
