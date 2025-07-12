//! Provides a trait and implementations for checked ceiling division on various integer types.
//!
//! Ceiling division computes the smallest integer greater than or equal to the result of a division.
//! This is useful when you want to ensure that any remainder results in rounding up.

use crate::math::bn::{U192, U256};
use num_traits::{One, Zero};

/// Trait for checked ceiling division.
///
/// This trait defines a method for performing ceiling division that returns `None` on division by zero or overflow.
///
/// # Example
///
/// ```rust
/// use tubor_yield::math::ceil_div::CheckedCeilDiv;
/// let result = 7u32.checked_ceil_div(3u32);
/// assert_eq!(result, Some(3));
/// ```
pub trait CheckedCeilDiv: Sized {
    /// Perform checked ceiling division.
    ///
    /// Returns `Some(quotient)` where `quotient` is the smallest integer greater than or equal to `self / rhs`.
    /// Returns `None` if division by zero or overflow occurs.
    ///
    /// # Arguments
    ///
    /// * `rhs` - The divisor.
    fn checked_ceil_div(&self, rhs: Self) -> Option<Self>;
}

/// Macro to implement `CheckedCeilDiv` for a given integer type.
///
/// This macro generates an implementation of the `CheckedCeilDiv` trait for the specified type,
/// using checked division and checked remainder to ensure safety.
macro_rules! checked_impl {
    ($t:ty) => {
        impl CheckedCeilDiv for $t {
            #[track_caller]
            #[inline]
            fn checked_ceil_div(&self, rhs: $t) -> Option<$t> {
                let quotient = self.checked_div(rhs)?;

                let remainder = self.checked_rem(rhs)?;

                if remainder > <$t>::zero() {
                    quotient.checked_add(<$t>::one())
                } else {
                    Some(quotient)
                }
            }
        }
    };
}

// Implement CheckedCeilDiv for various integer types, including custom big integer types.
checked_impl!(U256);
checked_impl!(U192);
checked_impl!(u128);
checked_impl!(u64);
checked_impl!(u32);
checked_impl!(u16);
checked_impl!(u8);
checked_impl!(i128);
checked_impl!(i64);
checked_impl!(i32);
checked_impl!(i16);
checked_impl!(i8);

/// Example of using the CeilDiv trait
///
/// ```rust
/// use tubor_yield::math::ceil_div::CheckedCeilDiv;
/// let result = 7_u64.checked_ceil_div(3).ok_or("Division failed")?;
/// Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn example_ceil_div_usage() -> Result<(), Box<dyn std::error::Error>> {
    let _result = 7_u64.checked_ceil_div(3).ok_or("Division failed")?;
    Ok(())
}
