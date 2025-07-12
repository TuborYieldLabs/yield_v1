use crate::msg;

use crate::error::TYieldResult;
use crate::math::bn::U192;
use crate::math::casting::Cast;
use crate::math::safe_math::SafeMath;
use crate::math_error;

/// Standardizes a value to the nearest multiple of `step_size`, returning the standardized value and the remainder.
///
/// # Arguments
///
/// * `value` - The value to standardize (can be negative).
/// * `step_size` - The step size to standardize to (must be positive).
///
/// # Returns
///
/// Returns a tuple `(standardized_value, remainder)` where:
/// - `standardized_value` is the largest multiple of `step_size` less than or equal to `value` (preserving sign).
/// - `remainder` is the difference between `value` and `standardized_value` (preserving sign).
///
/// # Errors
///
/// Returns an error if arithmetic overflows or casting fails.
pub fn standardize_value_with_remainder_i128(
    value: i128,
    step_size: u128,
) -> TYieldResult<(i128, i128)> {
    let remainder = value
        .unsigned_abs()
        .checked_rem_euclid(step_size)
        .ok_or_else(math_error!())?
        .cast::<i128>()?
        .safe_mul(value.signum())?;

    let standardized_value = value.safe_sub(remainder)?;

    Ok((standardized_value, remainder))
}

/// Calculates the proportional value of `value` using a ratio (`numerator`/`denominator`), preserving the sign of `value`.
///
/// # Arguments
///
/// * `value` - The value to scale (can be negative).
/// * `numerator` - The numerator of the ratio.
/// * `denominator` - The denominator of the ratio.
///
/// # Returns
///
/// Returns the scaled value as `i128`, preserving the sign of `value`.
///
/// # Errors
///
/// Returns an error if arithmetic overflows or casting fails.
pub fn get_proportion_i128(value: i128, numerator: u128, denominator: u128) -> TYieldResult<i128> {
    let proportional_u128 = get_proportion_u128(value.unsigned_abs(), numerator, denominator)?;
    let proportional_value = proportional_u128.cast::<i128>()?.safe_mul(value.signum())?;

    Ok(proportional_value)
}

/// Calculates the proportional value of `value` using a ratio (`numerator`/`denominator`) for unsigned integers.
///
/// Handles large values by using 192-bit arithmetic when necessary.
///
/// # Arguments
///
/// * `value` - The value to scale.
/// * `numerator` - The numerator of the ratio.
/// * `denominator` - The denominator of the ratio.
///
/// # Returns
///
/// Returns the scaled value as `u128`.
///
/// # Errors
///
/// Returns an error if arithmetic overflows or casting fails.
pub fn get_proportion_u128(value: u128, numerator: u128, denominator: u128) -> TYieldResult<u128> {
    // we use u128::max.sqrt() here
    let large_constant = u64::MAX.cast::<u128>()?;

    let proportional_value = if numerator == denominator {
        value
    } else if value >= large_constant || numerator >= large_constant {
        let value = U192::from(value)
            .safe_mul(U192::from(numerator))?
            .safe_div(U192::from(denominator))?;

        value.cast::<u128>()?
    } else if numerator > denominator / 2 && denominator > numerator {
        // get values to ensure a ceiling division
        let (std_value, r) = standardize_value_with_remainder_i128(
            value
                .safe_mul(denominator.safe_sub(numerator)?)?
                .cast::<i128>()?,
            denominator,
        )?;

        // perform ceiling division by subtracting one if there is a remainder
        value
            .safe_sub(std_value.cast::<u128>()?.safe_div(denominator)?)?
            .safe_sub(r.signum().cast::<u128>()?)?
    } else {
        value.safe_mul(numerator)?.safe_div(denominator)?
    };

    Ok(proportional_value)
}

/// Calculates the time remaining until the next update, rounding to the nearest update period (e.g., on the hour).
///
/// # Arguments
///
/// * `now` - The current timestamp.
/// * `last_update_ts` - The timestamp of the last update.
/// * `update_period` - The update period in seconds.
///
/// # Returns
///
/// Returns the number of seconds remaining until the next update.
///
/// # Errors
///
/// Returns an error if arithmetic overflows or division by zero occurs.
pub fn on_the_hour_update(now: i64, last_update_ts: i64, update_period: i64) -> TYieldResult<i64> {
    let time_since_last_update = now.safe_sub(last_update_ts)?;

    // round next update time to be available on the hour
    let mut next_update_wait = update_period;
    if update_period > 1 {
        let last_update_delay = last_update_ts.rem_euclid(update_period);
        if last_update_delay != 0 {
            let max_delay_for_next_period = update_period.safe_div(3)?;

            let two_funding_periods = update_period.safe_mul(2)?;

            if last_update_delay > max_delay_for_next_period {
                // too late for on the hour next period, delay to following period
                next_update_wait = two_funding_periods.safe_sub(last_update_delay)?;
            } else {
                // allow update on the hour
                next_update_wait = update_period.safe_sub(last_update_delay)?;
            }

            if next_update_wait > two_funding_periods {
                next_update_wait = next_update_wait.safe_sub(update_period)?;
            }
        }
    }

    let time_remaining_until_update = next_update_wait.safe_sub(time_since_last_update)?.max(0);

    Ok(time_remaining_until_update)
}

/// Computes the base-10 logarithm of a number recursively.
///
/// # Arguments
///
/// * `n` - The number to compute the logarithm for.
///
/// # Returns
///
/// Returns the integer part of the base-10 logarithm of `n`.
#[cfg(test)]
#[allow(clippy::comparison_chain)]
pub fn log10(n: u128) -> u128 {
    if n < 10 {
        0
    } else if n == 10 {
        1
    } else {
        log10(n / 10) + 1
    }
}

/// Computes the base-10 logarithm of a number iteratively.
///
/// # Arguments
///
/// * `n` - The number to compute the logarithm for.
///
/// # Returns
///
/// Returns the integer part of the base-10 logarithm of `n`.
pub fn log10_iter(n: u128) -> u128 {
    let mut result = 0;
    let mut n_copy = n;

    while n_copy >= 10 {
        result += 1;
        n_copy /= 10;
    }

    result
}

/// Example of using the SafeUnwrap trait
///
/// ```rust
/// use tubor_yield::math::safe_unwrap::SafeUnwrap;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let x: Option<u64> = Some(42);
///     let _y = x.safe_unwrap().map_err(|e| format!("{:?}", e))?;
///     Ok(())
/// }
/// ```
pub fn example_safe_unwrap_usage() -> Result<(), Box<dyn std::error::Error>> {
    use crate::math::safe_unwrap::SafeUnwrap;
    let x: Option<u64> = Some(42);
    let _y = x.safe_unwrap().map_err(|e| format!("{:?}", e))?;
    Ok(())
}
