use anchor_lang::prelude::*;

use crate::{
    error::ErrorCode,
    math::SafeMath,
    state::{ReferralLink, ReferralRegistry, RegisterUserEvent, Size, TYield, User, UserStatus},
};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct RegisterUserParams {
    name: [u8; 32],
    referrer: Option<Pubkey>,
}

#[derive(Accounts)]
#[instruction(params: RegisterUserParams)]
pub struct RegisterUser<'info> {
    /// New user account PDA.
    ///
    /// PDA seeds: ["user", authority]
    #[account(
        init,
        payer = payer,
        space = User::SIZE,  // 8 for anchor discriminator
        seeds = [b"user", authority.key().as_ref()],
        bump
    )]
    pub user: Box<Account<'info, User>>,

    #[account(
        seeds = [b"user", referrer_user_seeds(params.referrer).as_ref()],
        bump,
    )]
    pub referrer_user: Option<Box<Account<'info, User>>>,

    /// Referral registry PDA.
    ///
    /// Only initialized if a referrer is provided.  
    /// PDA seeds: ["referral_registry", referrer]
    #[account(
        init_if_needed,
        payer = payer,
        space = ReferralRegistry::SIZE,
        seeds = [b"referral_registry", params.referrer.unwrap_or(Pubkey::default()).as_ref()],
        bump
    )]
    pub referral_registry: Option<Box<Account<'info, ReferralRegistry>>>,

    #[account(
        init_if_needed,
        payer = payer,
        space = ReferralLink::SIZE,
        seeds = [b"referral_link", params.referrer.unwrap_or(Pubkey::default()).as_ref(), authority.key().as_ref()],
        bump,
    )]
    pub referral_link: Option<Box<Account<'info, ReferralLink>>>,

    /// The authority (owner) of the user account.
    ///
    /// This is typically a wallet public key that signs future transactions.
    /// CHECK: Not written to, only validated via seeds.
    #[account()]
    pub authority: AccountInfo<'info>,

    /// The account paying for the rent & fees.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The t_yield config PDA (your protocol global state).
    ///
    /// Seeds: ["t_yield"]
    #[account(
        seeds = [b"t_yield"],
        bump = t_yield.t_yield_bump
    )]
    pub t_yield: Account<'info, TYield>,

    /// CHECK: Event authority for CPI event logs.
    /// This is typically derived by Anchor to emit events across programs.
    #[account(
        seeds = [b"__event_authority"],
        bump,
    )]
    pub event_authority: AccountInfo<'info>,

    /// The Solana system program.
    pub system_program: Program<'info, System>,
}

pub fn register_user(ctx: Context<RegisterUser>, params: RegisterUserParams) -> Result<()> {
    if params.referrer.is_none()
        && (ctx.accounts.referral_registry.is_some() || ctx.accounts.referral_link.is_some())
    {
        return Err(ErrorCode::InvalidReferrer.into());
    }

    if let Some(referrer_pubkey) = params.referrer {
        if referrer_pubkey != Pubkey::default() {
            let referrer_user = ctx
                .accounts
                .referrer_user
                .as_ref()
                .ok_or(ErrorCode::ReferrerNotAUser)?;
            if referrer_user.authority != referrer_pubkey {
                return Err(ErrorCode::ReferrerNotAUser.into());
            }
        }
    }

    let user = &mut ctx.accounts.user;
    let current_time = ctx.accounts.t_yield.get_time()?;

    user.authority = ctx.accounts.authority.key();
    user.name = params.name;
    user.add_user_status(UserStatus::Active);
    user.referrer = params.referrer.unwrap_or_default();
    user.updated_at = current_time;
    user.created_at = current_time;
    user.bump = ctx.bumps.user;

    // If there's a referrer, update the referral registry
    if let Some(referrer_pubkey) = params.referrer {
        if referrer_pubkey != Pubkey::default() {
            if let Some(ref mut referral_registry) = ctx.accounts.referral_registry {
                // Initialize referral registry if it's new
                if referral_registry.referrer == Pubkey::default() {
                    referral_registry.referrer = referrer_pubkey;
                    referral_registry.created_at = current_time;
                    referral_registry.bump =
                        ctx.bumps.referral_registry.ok_or(ErrorCode::InvalidBump)?;
                }
                // Increment total referred users and update timestamp (actual referral links are tracked via ReferralLink accounts)
                referral_registry.total_referred_users =
                    referral_registry.total_referred_users.safe_add(1)?;
                referral_registry.updated_at = current_time;
            }

            if let Some(ref mut referral_link) = ctx.accounts.referral_link {
                // Only initialize if new
                if referral_link.referrer == Pubkey::default()
                    && referral_link.referred_user == Pubkey::default()
                {
                    referral_link.referrer = referrer_pubkey;
                    referral_link.referred_user = ctx.accounts.authority.key();
                    referral_link.created_at = current_time;
                    referral_link.bump = ctx.bumps.referral_link.ok_or(ErrorCode::InvalidBump)?;
                }
            }
        }
    }

    msg!(
        "User registered successfully with authority: {}",
        ctx.accounts.authority.key()
    );
    if let Some(ref_pubkey) = params.referrer {
        if ref_pubkey != Pubkey::default() {
            msg!("Referrer: {}", ref_pubkey);
        }
    }

    emit_cpi!(RegisterUserEvent {
        authority: user.authority,
        name: user.name,
        status: user.status,
        referrer: user.referrer,
        created_at: user.created_at
    });

    Ok(())
}

// Helper function for referrer_user seeds
fn referrer_user_seeds(referrer: Option<Pubkey>) -> Pubkey {
    referrer.unwrap_or_default()
}
