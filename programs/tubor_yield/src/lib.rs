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
    use crate::{
        error::TYieldResult,
        state::{AgentPrice, OraclePrice},
    };

    use super::*;

    pub fn init<'info>(
        ctx: Context<'_, '_, '_, 'info, Init<'info>>,
        params: InitParams,
    ) -> TYieldResult<()> {
        instructions::init(ctx, &params)
    }

    pub fn update_trade<'info>(
        ctx: Context<'_, '_, '_, 'info, UpdateTrade<'info>>,
        params: UpdateTradeParams,
    ) -> TYieldResult<u8> {
        instructions::update_trade(ctx, params)
    }

    // NEW: Secure oracle update instruction
    pub fn secure_oracle_update<'info>(
        ctx: Context<'_, '_, '_, 'info, SecureOracleUpdate<'info>>,
        params: SecureOracleUpdateParams,
    ) -> TYieldResult<u8> {
        instructions::secure_oracle_update(ctx, params)
    }
    pub fn update_yield<'info>(
        ctx: Context<'_, '_, '_, 'info, UpdateYield<'info>>,
        params: UpdateYieldParams,
    ) -> TYieldResult<u8> {
        instructions::update_yield(ctx, params)
    }

    pub fn ban_user<'info>(ctx: Context<'_, '_, '_, 'info, BanUser<'info>>) -> TYieldResult<u8> {
        instructions::ban_user(ctx)
    }

    pub fn update_status<'info>(
        ctx: Context<'_, '_, '_, 'info, UpdateStatus<'info>>,
        params: UpdateStatusParams,
    ) -> TYieldResult<u8> {
        instructions::update_status(ctx, &params)
    }

    pub fn mint_master_agent<'info>(
        ctx: Context<'_, '_, '_, 'info, MintMasterAgent<'info>>,
        params: MintMasterAgentParams,
    ) -> TYieldResult<u8> {
        instructions::mint_master_agent(ctx, params)
    }

    pub fn mint_agent<'info>(
        ctx: Context<'_, '_, '_, 'info, MintAgent<'info>>,
        params: MintAgentParams,
    ) -> TYieldResult<u8> {
        instructions::mint_agent(ctx, params)
    }

    pub fn register_user<'info>(
        ctx: Context<'_, '_, '_, 'info, RegisterUser<'info>>,
        params: RegisterUserParams,
    ) -> TYieldResult<()> {
        instructions::register_user(ctx, params)
    }

    pub fn buy_agent<'info>(ctx: Context<'_, '_, '_, 'info, BuyAgent<'info>>) -> TYieldResult<()> {
        instructions::buy_agent(ctx)
    }

    pub fn sell_agent<'info>(
        ctx: Context<'_, '_, '_, 'info, SellAgent<'info>>,
    ) -> TYieldResult<()> {
        instructions::sell_agent(ctx)
    }

    pub fn open_trade<'info>(
        ctx: Context<'_, '_, '_, 'info, OpenTrade<'info>>,
        params: OpenTradeParams,
    ) -> TYieldResult<u8> {
        instructions::open_trade(ctx, params)
    }

    pub fn close_trade<'info>(
        ctx: Context<'_, '_, '_, 'info, CloseTrade<'info>>,
        params: CloseTradeParams,
    ) -> TYieldResult<u8> {
        instructions::close_trade(ctx, params)
    }

    // pub fn transfer_agent<'info>(
    //     ctx: Context<'_, '_, '_, 'info, TransferAgent<'info>>,
    //     params: TransferAgentParams,
    // ) -> TYieldResult<u8> {
    //     instructions::transfer_agent(ctx, params)
    // }

    pub fn claim_referral_rewards<'info>(
        ctx: Context<'_, '_, '_, 'info, ClaimReferralRewards<'info>>,
    ) -> TYieldResult<u8> {
        instructions::claim_referral_rewards(ctx)
    }

    pub fn withdraw_yield<'info>(
        ctx: Context<'_, '_, '_, 'info, WithdrawYield<'info>>,
        params: WithdrawYieldParams,
    ) -> TYieldResult<u8> {
        instructions::withdraw_yield(ctx, params)
    }

    pub fn update_price<'info>(
        ctx: Context<'_, '_, '_, 'info, UpdatePrice<'info>>,
        params: UpdatePriceParams,
    ) -> TYieldResult<u8> {
        instructions::update_price(ctx, params)
    }

    pub fn update_protocol_config<'info>(
        ctx: Context<'_, '_, '_, 'info, UpdateProtocolConfig<'info>>,
        params: UpdateProtocolConfigParams,
    ) -> TYieldResult<u8> {
        instructions::update_protocol_config(ctx, params)
    }

    pub fn pause_protocol<'info>(
        ctx: Context<'_, '_, '_, 'info, PauseProtocol<'info>>,
    ) -> TYieldResult<u8> {
        instructions::pause_protocol(ctx)
    }

    pub fn unpause_protocol<'info>(
        ctx: Context<'_, '_, '_, 'info, UnpauseProtocol<'info>>,
    ) -> TYieldResult<u8> {
        instructions::unpause_protocol(ctx)
    }

    pub fn get_buy_agent_price<'info>(
        ctx: Context<'_, '_, '_, 'info, GetBuyAgentPrice<'info>>,
    ) -> TYieldResult<AgentPrice> {
        instructions::get_buy_agent_price(ctx)
    }

    pub fn get_sell_agent_price<'info>(
        ctx: Context<'_, '_, '_, 'info, GetSellAgentPrice<'info>>,
    ) -> TYieldResult<AgentPrice> {
        instructions::get_sell_agent_price(ctx)
    }

    pub fn get_pair_price<'info>(
        ctx: Context<'_, '_, '_, 'info, GetPairPrice<'info>>,
        params: GetPairParams,
    ) -> TYieldResult<OraclePrice> {
        instructions::get_pair_price(ctx, params)
    }
}
