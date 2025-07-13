//! Instruction: Get Buy Agent Price
//!
//! Returns the price (including tax) to buy an agent NFT from a master agent.
//! This is a read-only query; no state is mutated.
//!
//! Accounts:
//! - Agent account (PDA: ["agent", mint])
//! - Mint account for the agent NFT
//! - Master agent account (PDA: ["master_agent", master_agent_mint])
//! - Mint account for the master agent NFT
//! - Protocol global state (t_yield, PDA: ["t_yield"])

use anchor_lang::prelude::*;
use anchor_spl::token::Mint;

use crate::{
    error::TYieldResult,
    state::{Agent, AgentPrice, MasterAgent, TYield},
};

/// Accounts required to query the buy price for an agent NFT from a master agent.
///
/// This instruction does not mutate any state and can be called by anyone.
#[derive(Accounts)]
pub struct GetBuyAgentPrice<'info> {
    /// Agent account for the NFT being queried.
    /// PDA: ["agent", mint]
    #[account(
        seeds = [b"agent".as_ref(), mint.key().as_ref()],
        bump,
    )]
    pub agent: Box<Account<'info, Agent>>,

    /// Mint account for the agent NFT.
    pub mint: Box<Account<'info, Mint>>,

    /// Master agent account (parent/master of the agent).
    /// PDA: ["master_agent", master_agent_mint]
    #[account(
        seeds = [b"master_agent".as_ref(), master_agent_mint.key().as_ref()],
        bump = master_agent.bump,
    )]
    pub master_agent: Box<Account<'info, MasterAgent>>,

    /// Mint account for the master agent NFT.
    pub master_agent_mint: Box<Account<'info, Mint>>,

    /// Protocol global state.
    /// PDA: ["t_yield"]
    #[account(
        seeds = [b"t_yield"],
        bump = t_yield.t_yield_bump
    )]
    pub t_yield: Box<Account<'info, TYield>>,
}

/// Returns the price (including tax) to buy an agent NFT from a master agent.
///
/// # Arguments
/// * `ctx` - Context with the required accounts.
///
/// # Returns
/// * `AgentPrice` - Struct containing total price, tax amount, and base price.
pub fn get_buy_agent_price(ctx: Context<GetBuyAgentPrice>) -> TYieldResult<AgentPrice> {
    let master_agent = ctx.accounts.master_agent.as_ref();
    let _t_yield = ctx.accounts.t_yield.as_ref();

    let result = master_agent.calculate_buy_price_with_tax()?;

    Ok(AgentPrice {
        total_price: result.0,
        tax_amount: result.1,
        base_price: result.2,
    })
}
