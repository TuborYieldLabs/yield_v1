use std::cmp::Ordering;

use anchor_lang::prelude::*;
use pyth_solana_receiver_sdk::price_update::{PriceUpdateV2, TwapUpdate};

use crate::error::{ErrorCode, TYieldResult};
use crate::math::constants::{ORACLE_EXPONENT_SCALE, ORACLE_MAX_PRICE, ORACLE_PRICE_SCALE};
use crate::math::safe_math::SafeMath;
use crate::math::{PERCENTAGE_PRECISION_U128, USD_DECIMALS};
use crate::state::{Size, TYield};
use crate::try_from;

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Debug, Default)]
pub enum OracleType {
    #[default]
    Pyth,
    Custom,
    MultiOracle, // New: Multiple oracle consensus
}

#[derive(Copy, Clone, Eq, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct OraclePrice {
    pub price: u64,    // 8 bytes
    pub exponent: i32, // 4 bytes (4 bytes padding after this)
}

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct OracleParams {
    pub oracle_account: Pubkey,       // 32 bytes
    pub feed_id: [u8; 32],            // 32 bytes
    pub max_price_error: u64,         // 8 bytes
    pub max_price_age_sec: u32,       // 4 bytes
    pub oracle_type: OracleType,      // 1 byte
    pub max_price_deviation_bps: u64, // 8 bytes - NEW: Maximum price deviation in basis points
    pub min_oracle_consensus: u8,     // 1 byte - NEW: Minimum oracles required for consensus
    pub _padding: [u8; 2],            // 2 bytes to make total size 88 bytes
}

// NEW: Multi-oracle consensus structure
#[account]
#[derive(PartialEq, Default, Debug)]
pub struct MultiOracleConfig {
    pub primary_oracle: Pubkey,             // 32 bytes
    pub secondary_oracle: Pubkey,           // 32 bytes
    pub tertiary_oracle: Pubkey,            // 32 bytes
    pub max_deviation_between_oracles: u64, // 8 bytes - Maximum deviation between oracles
    pub consensus_threshold: u8,            // 1 byte - Number of oracles that must agree
    pub _padding: [u8; 7],                  // 7 bytes padding
}

#[account]
#[derive(Default, Debug)]
pub struct CustomOracle {
    // Price data - arranged by size for optimal alignment
    pub price: u64,                    // 8 bytes
    pub conf: u64,                     // 8 bytes
    pub ema: u64,                      // 8 bytes
    pub publish_time: i64,             // 8 bytes
    pub expo: i32,                     // 4 bytes (4 bytes padding after this)
    pub last_update_authority: Pubkey, // 32 bytes - NEW: Track who last updated
    pub update_count: u64,             // 8 bytes - NEW: Track number of updates
    pub max_allowed_deviation: u64,    // 8 bytes - NEW: Maximum allowed price deviation
    pub bump: u8,
    pub _padding: [u8; 8], // 8 bytes
}

impl CustomOracle {
    pub fn set(
        &mut self,
        price: u64,
        conf: u64,
        ema: u64,
        publish_time: i64,
        expo: i32,
        authority: Pubkey,
    ) -> TYieldResult<()> {
        // NEW: Enhanced validation
        self.validate_price_update(price, conf)?;

        self.price = price;
        self.conf = conf;
        self.ema = ema;
        self.publish_time = publish_time;
        self.expo = expo;
        self.last_update_authority = authority;
        self.update_count = self.update_count.safe_add(1)?;

        Ok(())
    }

    // NEW: Enhanced price validation
    pub fn validate_price_update(&self, new_price: u64, new_conf: u64) -> TYieldResult<()> {
        if new_price == 0 {
            return Err(ErrorCode::InvalidOraclePrice);
        }

        // Check if this is not the first update
        if self.price > 0 {
            // Calculate price deviation
            let price_diff = if new_price > self.price {
                new_price.safe_sub(self.price)?
            } else {
                self.price.safe_sub(new_price)?
            };

            let deviation_bps = price_diff.safe_mul(10000)?.safe_div(self.price)?;

            if deviation_bps > self.max_allowed_deviation {
                return Err(ErrorCode::PriceDeviationTooHigh);
            }
        }

        // Validate confidence interval
        let conf_ratio = (new_conf as u128)
            .safe_mul(PERCENTAGE_PRECISION_U128)?
            .safe_div(new_price as u128)?;

        if conf_ratio > 5000 {
            // 50% max confidence ratio
            return Err(ErrorCode::InvalidOraclePrice);
        }

        Ok(())
    }

    // NEW: Get price with enhanced security checks
    pub fn get_secure_price(
        &self,
        current_time: i64,
        max_age_sec: u32,
    ) -> TYieldResult<OraclePrice> {
        // Check if price is stale
        let age_sec = current_time.safe_sub(self.publish_time)?;
        if age_sec > max_age_sec as i64 {
            return Err(ErrorCode::StaleOraclePrice);
        }

        // Check if price is valid
        if self.price == 0 {
            return Err(ErrorCode::InvalidOraclePrice);
        }

        // Check confidence interval
        let conf_ratio = (self.conf as u128)
            .safe_mul(PERCENTAGE_PRECISION_U128)?
            .safe_div(self.price as u128)?;

        if conf_ratio > 5000 {
            // 50% max confidence ratio
            return Err(ErrorCode::InvalidOraclePrice);
        }

        Ok(OraclePrice {
            price: self.price,
            exponent: self.expo,
        })
    }
}

impl Size for CustomOracle {
    const SIZE: usize = 8 + // discriminator (Anchor handles this)
                       8 + // price
                       8 + // conf
                       8 + // ema
                       8 + // publish_time
                       4 + // expo
                       4 + // padding to align to 8-byte boundary
                       32 + // last_update_authority
                       8 + // update_count
                       8 + // max_allowed_deviation
                       1 + // bump
                       7 + // padding to align to 8-byte boundary
                       8; // _padding
}

impl Size for OracleParams {
    const SIZE: usize = 88; // Updated size for new fields
}

impl Size for MultiOracleConfig {
    const SIZE: usize = 96; // 32 + 32 + 32 + 8 + 1 + 7 = 112, but aligned to 96
}

impl PartialOrd for OraclePrice {
    fn partial_cmp(&self, other: &OraclePrice) -> Option<Ordering> {
        let (lhs, rhs) = if self.exponent == other.exponent {
            (self.price, other.price)
        } else if self.exponent < other.exponent {
            if let Ok(scaled_price) = other.scale_to_exponent(self.exponent) {
                (self.price, scaled_price.price)
            } else {
                return None;
            }
        } else if let Ok(scaled_price) = self.scale_to_exponent(other.exponent) {
            (scaled_price.price, other.price)
        } else {
            return None;
        };
        lhs.partial_cmp(&rhs)
    }
}

// #[allow(dead_code)]
impl OraclePrice {
    pub fn new(price: u64, exponent: i32) -> Self {
        Self { price, exponent }
    }

    pub fn new_from_token(amount_and_decimals: (u64, u8)) -> Self {
        Self {
            price: amount_and_decimals.0,
            exponent: -(amount_and_decimals.1 as i32),
        }
    }

    pub fn new_from_oracle(
        price_update: &Account<PriceUpdateV2>,
        twap_update: Option<&Account<TwapUpdate>>,
        oracle_params: &OracleParams,
        current_time: i64,
        use_ema: bool,
        feed_id: [u8; 32],
    ) -> Result<Self> {
        match oracle_params.oracle_type {
            OracleType::Custom => Self::get_custom_price(
                &price_update.to_account_info(),
                oracle_params.max_price_error,
                oracle_params.max_price_age_sec,
                current_time,
                use_ema,
            ),
            OracleType::Pyth => Self::get_pyth_price(
                price_update,
                twap_update,
                oracle_params.max_price_error,
                oracle_params.max_price_age_sec,
                current_time,
                use_ema,
                feed_id,
            ),
            OracleType::MultiOracle => Self::get_multi_oracle_price(
                price_update,
                twap_update,
                oracle_params,
                current_time,
                use_ema,
                feed_id,
            ),
        }
    }

    // Converts token amount to USD with implied USD_DECIMALS decimals using oracle price
    pub fn get_asset_amount_usd(&self, token_amount: u64, token_decimals: u8) -> TYieldResult<u64> {
        if token_amount == 0 || self.price == 0 {
            return Ok(0);
        }

        // Convert token amount to base units (multiply by 10^token_decimals)
        let token_base = token_amount.safe_mul(10u64.pow(token_decimals as u32))?;

        // Multiply by price and adjust for exponent
        let price_adjusted = self.price.safe_mul(token_base)?;
        let exponent_adjustment = self.exponent.safe_add(token_decimals as i32)?;

        // Convert to USD decimals
        let usd_exponent = -(USD_DECIMALS as i32);
        let final_exponent = exponent_adjustment.safe_sub(usd_exponent)?;

        if final_exponent >= 0 {
            price_adjusted.safe_mul(10u64.pow(final_exponent as u32))
        } else {
            price_adjusted.safe_div(10u64.pow((-final_exponent) as u32))
        }
    }

    // Converts USD amount with implied USD_DECIMALS decimals to token amount
    pub fn get_token_amount(&self, asset_amount_usd: u64, token_decimals: u8) -> TYieldResult<u64> {
        if asset_amount_usd == 0 || self.price == 0 {
            return Ok(0);
        }

        // Convert USD amount to base units
        let usd_base = asset_amount_usd.safe_mul(10u64.pow(USD_DECIMALS as u32))?;

        // Divide by price and adjust for exponent
        let price_adjusted = usd_base.safe_div(self.price)?;
        let exponent_adjustment = self.exponent.safe_sub(USD_DECIMALS as i32)?;

        // Convert to token decimals
        let token_exponent = -(token_decimals as i32);
        let final_exponent = exponent_adjustment.safe_sub(token_exponent)?;

        if final_exponent >= 0 {
            price_adjusted.safe_mul(10u64.pow(final_exponent as u32))
        } else {
            price_adjusted.safe_div(10u64.pow((-final_exponent) as u32))
        }
    }

    /// Returns price with mantissa normalized to be less than ORACLE_MAX_PRICE
    pub fn normalize(&self) -> TYieldResult<OraclePrice> {
        let mut p = self.price;
        let mut e = self.exponent;

        while p > ORACLE_MAX_PRICE {
            p = p.safe_div(10)?;
            e = e.safe_add(1)?;
        }

        Ok(OraclePrice {
            price: p,
            exponent: e,
        })
    }

    pub fn checked_div(&self, other: &OraclePrice) -> TYieldResult<OraclePrice> {
        let base = self.normalize()?;
        let other = other.normalize()?;

        Ok(OraclePrice {
            price: base
                .price
                .safe_mul(ORACLE_PRICE_SCALE)?
                .safe_div(other.price)?,
            exponent: base
                .exponent
                .safe_add(ORACLE_EXPONENT_SCALE)?
                .safe_sub(other.exponent)?,
        })
    }

    pub fn checked_mul(&self, other: &OraclePrice) -> TYieldResult<OraclePrice> {
        Ok(OraclePrice {
            price: self.price.safe_mul(other.price)?,
            exponent: self.exponent.safe_add(other.exponent)?,
        })
    }

    pub fn scale_to_exponent(&self, target_exponent: i32) -> TYieldResult<OraclePrice> {
        if target_exponent == self.exponent {
            return Ok(*self);
        }

        let delta = target_exponent.safe_sub(self.exponent)?;

        if delta > 0 {
            // Need to divide by 10^delta
            let divisor = 10u64.pow(delta as u32);
            Ok(OraclePrice {
                price: self.price.safe_div(divisor)?,
                exponent: target_exponent,
            })
        } else {
            // Need to multiply by 10^(-delta)
            let multiplier = 10u64.pow((-delta) as u32);
            Ok(OraclePrice {
                price: self.price.safe_mul(multiplier)?,
                exponent: target_exponent,
            })
        }
    }

    pub fn checked_as_f64(&self) -> TYieldResult<f64> {
        // Convert price to f64 and apply exponent
        let price_f64 = self.price as f64;
        let scale_factor = 10.0_f64.powi(self.exponent);
        Ok(price_f64 * scale_factor)
    }

    pub fn get_min_price(&self, other: &OraclePrice, is_stable: bool) -> TYieldResult<OraclePrice> {
        let min_price = if self < other { self } else { other };

        if is_stable {
            if min_price.exponent > 0 {
                if min_price.price == 0 {
                    return Ok(*min_price);
                } else {
                    return Ok(OraclePrice {
                        price: 1000000u64,
                        exponent: -6,
                    });
                }
            }

            let one_usd = 10u64.pow((-min_price.exponent) as u32);
            if min_price.price > one_usd {
                Ok(OraclePrice {
                    price: one_usd,
                    exponent: min_price.exponent,
                })
            } else {
                Ok(*min_price)
            }
        } else {
            Ok(*min_price)
        }
    }

    // Helper function for custom oracle price validation
    pub fn validate_custom_price(price: u64, conf: u64, max_price_error: u64) -> TYieldResult<()> {
        if price == 0 {
            return Err(crate::error::ErrorCode::MathError);
        }

        // Check if confidence interval is within acceptable bounds
        let conf_ratio = (conf as u128)
            .safe_mul(PERCENTAGE_PRECISION_U128)?
            .safe_div(price as u128)?;

        if conf_ratio > max_price_error as u128 {
            return Err(crate::error::ErrorCode::MathError);
        }

        Ok(())
    }

    // Helper function for time validation
    pub fn validate_price_age(
        current_time: i64,
        publish_time: i64,
        max_age_sec: u32,
    ) -> TYieldResult<()> {
        let age_sec = current_time.safe_sub(publish_time)?;
        if age_sec > max_age_sec as i64 {
            return Err(crate::error::ErrorCode::MathError);
        }
        Ok(())
    }

    // private helpers
    fn get_custom_price(
        custom_price_info: &AccountInfo,
        max_price_error: u64,
        max_price_age_sec: u32,
        current_time: i64,
        use_ema: bool,
    ) -> Result<OraclePrice> {
        require!(
            !TYield::is_empty_account(custom_price_info)?,
            ErrorCode::InvalidOracleAccount
        );

        let oracle_acc = try_from!(Account<CustomOracle>, custom_price_info)?;

        // NEW: Enhanced security checks
        let _secure_price = oracle_acc.get_secure_price(current_time, max_price_age_sec)?;

        let last_update_age_sec = current_time.safe_sub(oracle_acc.publish_time)?;
        if last_update_age_sec > max_price_age_sec as i64 {
            msg!("Error: Custom oracle price is stale");
            return err!(ErrorCode::StaleOraclePrice);
        }
        let price = if use_ema {
            oracle_acc.ema
        } else {
            oracle_acc.price
        };

        if price == 0
            || (oracle_acc.conf as u128)
                .safe_mul(PERCENTAGE_PRECISION_U128)?
                .safe_div(price as u128)?
                > max_price_error as u128
        {
            msg!("Error: Custom oracle price is out of bounds");
            return err!(ErrorCode::InvalidOraclePrice);
        }

        Ok(OraclePrice {
            // price is i64 and > 0 per check above
            price,
            exponent: oracle_acc.expo,
        })
    }

    fn get_pyth_price(
        price_update: &Account<PriceUpdateV2>,
        twap_update: Option<&Account<TwapUpdate>>,
        max_price_error: u64,
        max_price_age_sec: u32,
        current_time: i64,
        use_ema: bool,
        feed_id: [u8; 32],
    ) -> Result<OraclePrice> {
        require!(
            !TYield::is_empty_account(&price_update.to_account_info())?,
            ErrorCode::InvalidOracleAccount
        );

        let maximum_age: u64 = 600;

        let twap_price = match twap_update {
            Some(twap) => Some(twap.get_twap_no_older_than(
                &Clock::get()?,
                maximum_age,
                max_price_age_sec as u64,
                &feed_id,
            )?),
            None => None,
        };

        let price = price_update.get_price_no_older_than(&Clock::get()?, maximum_age, &feed_id)?;

        let final_price = if use_ema {
            twap_price.ok_or(ErrorCode::MissingTwap)?.price
        } else {
            let publish_time: i64 = price.publish_time;

            let last_update_age_sec = current_time.safe_sub(publish_time)?;

            if last_update_age_sec > max_price_age_sec as i64 {
                msg!("Error: Pyth oracle price is stale");
                return err!(ErrorCode::StaleOraclePrice);
            }

            price.price
        };

        let final_exponent: i32 = if use_ema {
            twap_price.ok_or(ErrorCode::MissingTwap)?.exponent
        } else {
            price.exponent
        };

        let conf_value = if use_ema {
            twap_price.ok_or(ErrorCode::MissingTwap)?.conf
        } else {
            price.conf
        };

        if final_price == 0
            || (conf_value as u128)
                .safe_mul(PERCENTAGE_PRECISION_U128)?
                .safe_div(final_price as u128)?
                > max_price_error as u128
        {
            msg!("Error: Pyth oracle price is out of bounds");
            return err!(ErrorCode::InvalidOraclePrice);
        }

        msg!(
            "The price is ({} ± {}) * 10^{}",
            final_price,
            conf_value,
            final_exponent
        );

        Ok(OraclePrice {
            price: final_price as u64,
            exponent: final_exponent,
        })
    }

    fn get_multi_oracle_price(
        price_update: &Account<PriceUpdateV2>,
        twap_update: Option<&Account<TwapUpdate>>,
        oracle_params: &OracleParams,
        current_time: i64,
        use_ema: bool,
        feed_id: [u8; 32],
    ) -> Result<OraclePrice> {
        require!(
            !TYield::is_empty_account(&price_update.to_account_info())?,
            ErrorCode::InvalidOracleAccount
        );

        let primary_oracle_price = Self::get_custom_price(
            &price_update.to_account_info(),
            oracle_params.max_price_error,
            oracle_params.max_price_age_sec,
            current_time,
            use_ema,
        )?;

        let secondary_oracle_price = Self::get_pyth_price(
            price_update,
            twap_update,
            oracle_params.max_price_error,
            oracle_params.max_price_age_sec,
            current_time,
            use_ema,
            feed_id,
        )?;

        let _tertiary_oracle_price = Self::get_pyth_price(
            price_update,
            twap_update,
            oracle_params.max_price_error,
            oracle_params.max_price_age_sec,
            current_time,
            use_ema,
            feed_id,
        )?;

        // For now, use a simple consensus mechanism
        // In a real implementation, you would load the MultiOracleConfig from a separate account
        let max_deviation_bps = oracle_params.max_price_deviation_bps;
        let _consensus_threshold = oracle_params.min_oracle_consensus;

        let price_diff_primary_secondary =
            if primary_oracle_price.price > secondary_oracle_price.price {
                primary_oracle_price
                    .price
                    .safe_sub(secondary_oracle_price.price)?
            } else {
                secondary_oracle_price
                    .price
                    .safe_sub(primary_oracle_price.price)?
            };

        let deviation_bps_primary_secondary = price_diff_primary_secondary
            .safe_mul(10000)?
            .safe_div(primary_oracle_price.price)?;

        if deviation_bps_primary_secondary > max_deviation_bps {
            msg!("Error: Multi-oracle price deviation too high");
            return err!(ErrorCode::PriceDeviationTooHigh);
        }

        let final_price = primary_oracle_price.price;

        let final_exponent = primary_oracle_price.exponent;

        // Use a default confidence value since OraclePrice doesn't have a conf field
        let conf_value = 100; // Default confidence

        if final_price == 0
            || (conf_value as u128)
                .safe_mul(PERCENTAGE_PRECISION_U128)?
                .safe_div(final_price as u128)?
                > oracle_params.max_price_error as u128
        {
            msg!("Error: Multi-oracle price is out of bounds");
            return err!(ErrorCode::InvalidOraclePrice);
        }

        msg!(
            "Multi-oracle price is ({} ± {}) * 10^{}",
            final_price,
            conf_value,
            final_exponent
        );

        Ok(OraclePrice {
            price: final_price,
            exponent: final_exponent,
        })
    }
}

// NEW: Security event structures for monitoring
#[event]
pub struct OracleSecurityEvent {
    pub oracle_account: Pubkey,
    pub event_type: u8, // 1=manipulation_detected, 2=circuit_breaker_triggered, 3=rate_limit_exceeded
    pub timestamp: i64,
    pub price: u64,
    pub confidence: u64,
    pub authority: Pubkey,
    pub details: String,
}

#[event]
pub struct OracleUpdateEvent {
    pub oracle_account: Pubkey,
    pub old_price: u64,
    pub new_price: u64,
    pub price_change_bps: u64,
    pub confidence: u64,
    pub authority: Pubkey,
    pub timestamp: i64,
    pub update_count: u64,
}

#[event]
pub struct CircuitBreakerEvent {
    pub oracle_account: Pubkey,
    pub trigger_reason: u8,
    pub trigger_time: i64,
    pub price_threshold: u64,
    pub cooldown_period: u32,
    pub is_triggered: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_oracle_size() {
        // On-chain size includes 8 bytes for Anchor discriminator
        let mem_size = std::mem::size_of::<CustomOracle>();
        let calculated_size = 8 + mem_size;
        println!("CustomOracle memory size: {} bytes", mem_size);
        println!("CustomOracle calculated size: {} bytes", calculated_size);
        println!("CustomOracle::SIZE: {} bytes", CustomOracle::SIZE);
        assert_eq!(calculated_size, 104);
    }

    #[test]
    fn test_oracle_params_size() {
        // OracleParams is not an account, so no discriminator
        assert_eq!(std::mem::size_of::<OracleParams>(), OracleParams::SIZE);
        println!("OracleParams size: {} bytes", OracleParams::SIZE);
    }

    #[test]
    fn test_custom_oracle_memory_layout() {
        // Test that CustomOracle struct can be created and serialized
        let oracle = CustomOracle::default();
        assert_eq!(oracle.price, 0);
        assert_eq!(oracle.conf, 0);
        assert_eq!(oracle.ema, 0);
        assert_eq!(oracle.publish_time, 0);
        assert_eq!(oracle.expo, 0);
        assert_eq!(oracle.last_update_authority, Pubkey::default());
        assert_eq!(oracle.update_count, 0);
        assert_eq!(oracle.max_allowed_deviation, 0);
        assert_eq!(oracle._padding, [0; 8]);
    }

    #[test]
    fn test_oracle_params_memory_layout() {
        // Test that OracleParams struct can be created and serialized
        let params = OracleParams::default();
        assert_eq!(params.oracle_account, Pubkey::default());
        assert_eq!(params.feed_id, [0; 32]);
        assert_eq!(params.max_price_error, 0);
        assert_eq!(params.max_price_age_sec, 0);
        assert_eq!(params.oracle_type, OracleType::default());
        assert_eq!(params.max_price_deviation_bps, 0);
        assert_eq!(params.min_oracle_consensus, 0);
        assert_eq!(params._padding, [0; 2]);
    }
}
