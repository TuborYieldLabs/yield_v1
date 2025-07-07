use std::cmp::Ordering;

use anchor_lang::prelude::*;

use crate::error::TYieldResult;
use crate::math::constants::{ORACLE_EXPONENT_SCALE, ORACLE_MAX_PRICE, ORACLE_PRICE_SCALE};
use crate::math::safe_math::SafeMath;
use crate::math::USD_DECIMALS;
use crate::state::Size;

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Debug, Default)]
pub enum OracleType {
    #[default]
    Pyth,
    Custom,
}

#[derive(Copy, Clone, Eq, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct OraclePrice {
    pub price: u64,    // 8 bytes
    pub exponent: i32, // 4 bytes (4 bytes padding after this)
}

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct OracleParams {
    // Oracle configuration - arranged by size for optimal alignment
    pub oracle_account: Pubkey,    // 32 bytes
    pub feed_id: [u8; 32],         // 32 bytes
    pub max_price_error: u64,      // 8 bytes
    pub max_price_age_sec: u32,    // 4 bytes
    pub oracle_type: OracleType,   // 1 byte
    pub _padding: [u8; 3],         // 3 bytes padding to align to 8-byte boundary
    pub _future_padding: [u8; 29], // 29 bytes for future-proofing
}

#[account]
#[derive(Default, Debug)]
pub struct CustomOracle {
    // Price data - arranged by size for optimal alignment
    pub price: u64,         // 8 bytes
    pub conf: u64,          // 8 bytes
    pub ema: u64,           // 8 bytes
    pub publish_time: i64,  // 8 bytes
    pub expo: i32,          // 4 bytes (4 bytes padding after this)
    pub _padding: [u8; 32], // 32 bytes
}

impl CustomOracle {
    pub fn set(&mut self, price: u64, conf: u64, ema: u64, publish_time: i64, expo: i32) {
        self.price = price;
        self.conf = conf;
        self.ema = ema;
        self.publish_time = publish_time;
        self.expo = expo;
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
                       32; // _padding
}

impl Size for OracleParams {
    const SIZE: usize = 32 + // oracle_account
                       32 + // feed_id
                       8 + // max_price_error
                       4 + // max_price_age_sec
                       1 + // oracle_type
                       3 + // _padding to align to 8-byte boundary
                       29 + // _future_padding
                       3; // additional padding for alignment
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
    // fn validate_custom_price(price: u64, conf: u64, max_price_error: u64) -> TYieldResult<()> {
    //     if price == 0 {
    //         return Err(crate::error::ErrorCode::MathError);
    //     }

    //     // Check if confidence interval is within acceptable bounds
    //     let conf_ratio = (conf as u128)
    //         .safe_mul(BPS_POWER)?
    //         .safe_div(price as u128)?;

    //     if conf_ratio > max_price_error as u128 {
    //         return Err(crate::error::ErrorCode::MathError);
    //     }

    //     Ok(())
    // }

    // Helper function for time validation
    // fn validate_price_age(
    //     current_time: i64,
    //     publish_time: i64,
    //     max_age_sec: u32,
    // ) -> TYieldResult<()> {
    //     let age_sec = current_time.safe_sub(publish_time)?;
    //     if age_sec > max_age_sec as i64 {
    //         return Err(crate::error::ErrorCode::MathError);
    //     }
    //     Ok(())
    // }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_oracle_size() {
        // On-chain size includes 8 bytes for Anchor discriminator
        assert_eq!(8 + std::mem::size_of::<CustomOracle>(), CustomOracle::SIZE);
        println!("CustomOracle on-chain size: {} bytes", CustomOracle::SIZE);
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
        assert_eq!(oracle._padding, [0; 32]);
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
        assert_eq!(params._padding, [0; 3]);
        assert_eq!(params._future_padding, [0; 29]);
    }
}
