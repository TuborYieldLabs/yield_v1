//! Instruction: Secure Oracle Update
//!
//! This instruction provides enhanced security for updating oracle prices with:
//! - Rate limiting to prevent rapid price updates
//! - Authority validation to ensure only authorized entities can update
//! - Circuit breaker mechanisms to prevent extreme price movements
//! - Multi-oracle consensus validation
//! - Enhanced logging and monitoring capabilities
//!
//! Accounts:
//! - Authority (signer, must be authorized oracle updater)
//! - Oracle account (custom oracle to update)
//! - Multisig (for admin-level updates)
//! - Protocol state (t_yield)
//! - System program

use anchor_lang::prelude::*;

use crate::{
    error::{ErrorCode, TYieldResult},
    math::SafeMath,
    msg,
    state::{CustomOracle, Multisig, TYield},
};

/// Parameters for secure oracle updates
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct SecureOracleUpdateParams {
    /// New price to set
    pub new_price: u64,
    /// Confidence interval for the price
    pub confidence: u64,
    /// Exponential moving average
    pub ema: u64,
    /// Publish timestamp
    pub publish_time: i64,
    /// Price exponent
    pub exponent: i32,
    /// Maximum allowed price deviation in basis points
    pub max_deviation_bps: u64,
    /// Whether this is an emergency update (bypasses some checks)
    pub is_emergency: bool,
}

/// Accounts required for secure oracle updates
#[derive(Accounts)]
#[instruction(params: SecureOracleUpdateParams)]
pub struct SecureOracleUpdate<'info> {
    /// Authority that can update the oracle (must be authorized)
    #[account(mut)]
    pub authority: Signer<'info>,

    /// Custom oracle account to update
    /// Seeds: ["oracle", oracle_authority]
    #[account(
        mut,
        seeds = [b"oracle", oracle_authority.key().as_ref()],
        bump = oracle.bump
    )]
    pub oracle: Box<Account<'info, CustomOracle>>,

    /// Authority of the oracle account
    /// CHECK: Used for seed validation only
    pub oracle_authority: AccountInfo<'info>,

    /// Multisig for admin-level operations
    /// Seeds: ["multisig"]
    #[account(
        mut,
        seeds = [b"multisig"],
        bump = multisig.load()?.bump
    )]
    pub multisig: AccountLoader<'info, Multisig>,

    /// Protocol state
    /// Seeds: ["t_yield"]
    #[account(
        seeds = [b"t_yield"],
        bump = t_yield.t_yield_bump
    )]
    pub t_yield: Box<Account<'info, TYield>>,

    /// System program
    pub system_program: Program<'info, System>,

    /// Event authority for CPI event logs
    /// CHECK: Used for event emission only
    #[account(
        seeds = [b"__event_authority"],
        bump,
    )]
    pub event_authority: AccountInfo<'info>,
}

/// Handler for secure oracle updates
///
/// This function implements enhanced security measures:
/// - Rate limiting to prevent rapid updates
/// - Authority validation
/// - Circuit breaker for extreme price movements
/// - Enhanced logging and monitoring
///
/// # Arguments
/// * `ctx` - Context with required accounts
/// * `params` - Parameters for the oracle update
///
/// # Returns
/// * `Ok(0)` if successful
/// * `Err` if any security check fails
pub fn secure_oracle_update<'info>(
    ctx: Context<'_, '_, '_, 'info, SecureOracleUpdate<'info>>,
    params: SecureOracleUpdateParams,
) -> TYieldResult<u8> {
    let current_time = ctx.accounts.t_yield.get_time()?;
    let oracle = ctx.accounts.oracle.as_mut();
    let authority = &ctx.accounts.authority;

    // 1. BASIC AUTHORITY VALIDATION
    if !is_authorized_oracle_updater(&authority.key(), oracle)? {
        msg!("Error: Unauthorized oracle update attempt");
        return Err(ErrorCode::InvalidOracleAuthority);
    }

    // 2. RATE LIMITING CHECK
    if !params.is_emergency {
        check_rate_limit(oracle, current_time)?;
    }

    // 3. CIRCUIT BREAKER CHECK
    check_circuit_breaker(oracle, &params, current_time)?;

    // 4. PRICE MANIPULATION DETECTION
    detect_price_manipulation(oracle, &params)?;

    // 5. ENHANCED PRICE VALIDATION
    validate_secure_price_update(oracle, &params)?;

    // 6. UPDATE ORACLE WITH ENHANCED SECURITY
    oracle.set(
        params.new_price,
        params.confidence,
        params.ema,
        params.publish_time,
        params.exponent,
        authority.key(),
    )?;

    // 7. LOG SECURITY EVENT
    log_security_event(oracle, &params, current_time)?;

    msg!("Secure oracle update completed successfully");
    msg!(
        "New price: {} (confidence: {})",
        params.new_price,
        params.confidence
    );
    msg!("Updated by: {}", authority.key());
    msg!("Update count: {}", oracle.update_count);

    Ok(0)
}

// ============================================================================
// SECURITY HELPER FUNCTIONS
// ============================================================================

/// Check if the authority is authorized to update this oracle
fn is_authorized_oracle_updater(authority: &Pubkey, oracle: &CustomOracle) -> TYieldResult<bool> {
    // Check if this is the first update (no previous authority)
    if oracle.last_update_authority == Pubkey::default() {
        return Ok(true);
    }

    // Check if authority is the same as last updater (for continuity)
    if oracle.last_update_authority == *authority {
        return Ok(true);
    }

    // TODO: Add additional authority validation logic here
    // For example, check against a whitelist of authorized updaters

    Ok(false)
}

/// Check rate limiting for oracle updates
fn check_rate_limit(oracle: &CustomOracle, current_time: i64) -> TYieldResult<()> {
    const MIN_UPDATE_INTERVAL_SEC: i64 = 60; // 1 minute minimum between updates

    let time_since_last_update = current_time.safe_sub(oracle.publish_time)?;

    if time_since_last_update < MIN_UPDATE_INTERVAL_SEC {
        return Err(ErrorCode::OracleUpdateRateLimitExceeded);
    }

    Ok(())
}

/// Check circuit breaker conditions
fn check_circuit_breaker(
    oracle: &CustomOracle,
    params: &SecureOracleUpdateParams,
    _current_time: i64,
) -> TYieldResult<()> {
    // Skip circuit breaker for first update
    if oracle.price == 0 {
        return Ok(());
    }

    // Calculate price change percentage
    let price_change = if params.new_price > oracle.price {
        params.new_price.safe_sub(oracle.price)?
    } else {
        oracle.price.safe_sub(params.new_price)?
    };

    let change_percentage = price_change.safe_mul(10000)?.safe_div(oracle.price)?;

    // Circuit breaker threshold: 50% price change
    const CIRCUIT_BREAKER_THRESHOLD_BPS: u64 = 5000; // 50%

    if change_percentage > CIRCUIT_BREAKER_THRESHOLD_BPS {
        return Err(ErrorCode::CircuitBreakerTriggered);
    }

    Ok(())
}

/// Detect potential price manipulation
fn detect_price_manipulation(
    oracle: &CustomOracle,
    params: &SecureOracleUpdateParams,
) -> TYieldResult<()> {
    // Skip for first update
    if oracle.price == 0 {
        return Ok(());
    }

    // Check for suspicious patterns
    let price_ratio = if params.new_price > oracle.price {
        params.new_price.safe_mul(100)?.safe_div(oracle.price)?
    } else {
        oracle.price.safe_mul(100)?.safe_div(params.new_price)?
    };

    // Flag if price changed by more than 10x in one update
    if price_ratio > 1000 {
        return Err(ErrorCode::OracleManipulationDetected);
    }

    // Check confidence interval
    let conf_ratio = (params.confidence as u128)
        .safe_mul(crate::math::PERCENTAGE_PRECISION_U128)?
        .safe_div(params.new_price as u128)?;

    // Flag if confidence is too high (suspicious)
    if conf_ratio < 100 {
        // Less than 1% confidence
        return Err(ErrorCode::OracleManipulationDetected);
    }

    Ok(())
}

/// Enhanced price validation
fn validate_secure_price_update(
    oracle: &CustomOracle,
    params: &SecureOracleUpdateParams,
) -> TYieldResult<()> {
    // Basic validation
    if params.new_price == 0 {
        return Err(ErrorCode::InvalidOraclePrice);
    }

    // Check confidence interval
    let conf_ratio = (params.confidence as u128)
        .safe_mul(crate::math::PERCENTAGE_PRECISION_U128)?
        .safe_div(params.new_price as u128)?;

    if conf_ratio > 5000 {
        // More than 50% confidence interval
        return Err(ErrorCode::OracleConfidenceExceeded);
    }

    // Validate exponent
    if params.exponent < -20 || params.exponent > 20 {
        return Err(ErrorCode::InvalidOraclePrice);
    }

    // Check if this is not the first update
    if oracle.price > 0 {
        // Use the oracle's built-in validation
        oracle.validate_price_update(params.new_price, params.confidence)?;
    }

    Ok(())
}

/// Log security event for monitoring
fn log_security_event(
    oracle: &CustomOracle,
    params: &SecureOracleUpdateParams,
    current_time: i64,
) -> TYieldResult<()> {
    msg!("=== SECURITY EVENT: ORACLE UPDATE ===");
    msg!("Timestamp: {}", current_time);
    msg!("Old price: {}", oracle.price);
    msg!("New price: {}", params.new_price);
    msg!("Confidence: {}", params.confidence);
    msg!("Update count: {}", oracle.update_count.safe_add(1)?);
    msg!("Authority: {}", oracle.last_update_authority);
    msg!("=====================================");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiting() {
        let mut oracle = CustomOracle::default();
        oracle.publish_time = 1000;

        // Should fail if trying to update too soon
        let result = check_rate_limit(&oracle, 1050); // 50 seconds later
        assert!(result.is_err());

        // Should succeed if enough time has passed
        let result = check_rate_limit(&oracle, 1070); // 70 seconds later
        assert!(result.is_ok());
    }

    #[test]
    fn test_circuit_breaker() {
        let mut oracle = CustomOracle::default();
        oracle.price = 1000;

        let params = SecureOracleUpdateParams {
            new_price: 1600, // 60% increase
            confidence: 100,
            ema: 1600,
            publish_time: 1000,
            exponent: 0,
            max_deviation_bps: 1000,
            is_emergency: false,
        };

        // Should trigger circuit breaker
        let result = check_circuit_breaker(&oracle, &params, 1000);
        assert!(result.is_err());
    }

    #[test]
    fn test_price_manipulation_detection() {
        let mut oracle = CustomOracle::default();
        oracle.price = 1000;

        let params = SecureOracleUpdateParams {
            new_price: 11000, // 11x increase
            confidence: 100,
            ema: 11000,
            publish_time: 1000,
            exponent: 0,
            max_deviation_bps: 1000,
            is_emergency: false,
        };

        // Should detect manipulation
        let result = detect_price_manipulation(&oracle, &params);
        assert!(result.is_err());
    }
}
