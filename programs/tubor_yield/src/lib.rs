#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;

declare_id!("EiifDJcZo3QthKQ2ZrdNSMsDufw4A4sGdsEQkZyRnhNs");

#[program]
pub mod tubor_yield {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
