//! Provides checked floor division for signed integer types.
//!
//! This module defines the `CheckedFloorDiv` trait, which allows for safe floor division
//! (rounding towards negative infinity) with overflow and division-by-zero checks.
//! Implementations are provided for all standard signed integer types.

use num_traits::{One, Zero};

/// Trait for checked floor division (rounding towards negative infinity).
///
/// This trait provides a method to perform floor division that returns `None` on
/// division by zero or arithmetic overflow, instead of panicking.
///
/// # Examples
/// ```rust
/// use tubor_yield::math::floor_div::CheckedFloorDiv;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let x = -3_i128;
///     assert_eq!(x.checked_floor_div(2), Some(-2));
///     assert_eq!(x.checked_floor_div(0), None);
///     Ok(())
/// }
/// ```
pub trait CheckedFloorDiv: Sized {
    /// Performs checked floor division, rounding towards negative infinity.
    ///
    /// Returns `None` if division by zero or arithmetic overflow occurs.
    ///
    /// # Arguments
    /// * `rhs` - The divisor.
    ///
    /// # Returns
    /// * `Some(quotient)` if the operation is valid.
    /// * `None` if division by zero or overflow occurs.
    fn checked_floor_div(&self, rhs: Self) -> Option<Self>;
}

/// Macro to implement `CheckedFloorDiv` for a signed integer type.
///
/// This macro provides a checked implementation of floor division for the given type,
/// handling division by zero and overflow safely.
macro_rules! checked_impl {
    ($t:ty) => {
        impl CheckedFloorDiv for $t {
            #[track_caller]
            #[inline]
            fn checked_floor_div(&self, rhs: $t) -> Option<$t> {
                let quotient = self.checked_div(rhs)?;

                let remainder = self.checked_rem(rhs)?;

                if remainder != <$t>::zero() {
                    quotient.checked_sub(<$t>::one())
                } else {
                    Some(quotient)
                }
            }
        }
    };
}

checked_impl!(i128);
checked_impl!(i64);
checked_impl!(i32);
checked_impl!(i16);
checked_impl!(i8);

#[cfg(test)]
mod test {
    use crate::math::floor_div::CheckedFloorDiv;

    /// Tests for the `CheckedFloorDiv` trait implementation.
    #[test]
    fn test() {
        let x = -3_i128;

        assert_eq!(x.checked_floor_div(2), Some(-2));
        assert_eq!(x.checked_floor_div(0), None);
    }
}

/// Example of using the FloorDiv trait
///
/// ```rust
/// use tubor_yield::math::floor_div::CheckedFloorDiv;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let result = (-7_i64).checked_floor_div(3).ok_or("Division failed")?;
///     Ok(())
/// }
/// ```
pub fn example_floor_div_usage() -> Result<(), Box<dyn std::error::Error>> {
    let _result = (-7_i64).checked_floor_div(3).ok_or("Division failed")?;
    Ok(())
}
