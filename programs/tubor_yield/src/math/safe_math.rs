//! Safe mathematical operations for Solana programs.
//!
//! This module provides safe arithmetic operations that prevent overflow, underflow,
//! and division by zero errors. All operations return `TYieldResult<T>` instead of
//! panicking, making them suitable for on-chain programs where panics are not allowed.
//!
//! # Features
//!
//! - **Safe arithmetic**: Addition, subtraction, multiplication, and division with overflow protection
//! - **Ceiling and floor division**: Specialized division operations for different rounding needs
//! - **Error handling**: All operations return `Result` types with descriptive error messages
//! - **Caller tracking**: Error messages include file and line information for debugging
//!
//! # Supported Types
//!
//! The following numeric types implement safe math operations:
//! - `u8`, `u16`, `u32`, `u64`, `u128` (unsigned integers)
//! - `i8`, `i16`, `i32`, `i64`, `i128` (signed integers)
//! - `U192`, `U256` (custom big integer types)
//!
//! # Examples
//!
//! ```rust
//! use tubor_yield::math::safe_math::SafeMath;
//! // Safe addition
//! let result = 5_u128.safe_add(3).unwrap(); // Ok(8)
//! // Safe multiplication with overflow protection
//! let result = 2_u128.safe_mul(u128::MAX);
//! assert!(result.is_err()); // Err(MathError)
//! // Safe division
//! let result = 10_u128.safe_div(2).unwrap(); // Ok(5)
//! let result = 10_u128.safe_div(0);
//! assert!(result.is_err()); // Err(MathError)
//! ```

use crate::error::{ErrorCode, TYieldResult};
use crate::math::bn::{U192, U256};
use crate::math::ceil_div::CheckedCeilDiv;
use crate::math::floor_div::CheckedFloorDiv;
use crate::msg;
use std::panic::Location;

/// Trait providing safe arithmetic operations for numeric types.
///
/// This trait extends numeric types with safe arithmetic methods that prevent
/// overflow, underflow, and division by zero. All methods return `TYieldResult<T>`
/// instead of panicking, making them suitable for on-chain programs.
///
/// # Safety Guarantees
///
/// - **No panics**: All operations handle errors gracefully
/// - **Overflow protection**: Addition and multiplication check for overflow
/// - **Underflow protection**: Subtraction checks for underflow
/// - **Division safety**: Division checks for zero divisors
/// - **Debugging support**: Error messages include caller location
///
/// # Implemented Types
///
/// This trait is implemented for all standard integer types and custom big integer types:
/// - `u8`, `u16`, `u32`, `u64`, `u128`
/// - `i8`, `i16`, `i32`, `i64`, `i128`
/// - `U192`, `U256`
pub trait SafeMath: Sized {
    /// Safely adds two values, returning an error if overflow occurs.
    ///
    /// # Arguments
    ///
    /// * `rhs` - The right-hand side value to add
    ///
    /// # Returns
    ///
    /// - `Ok(result)` if the addition succeeds
    /// - `Err(MathError)` if overflow occurs
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tubor_yield::math::safe_math::SafeMath;
    /// let result = 5_u128.safe_add(3).unwrap(); // Ok(8)
    /// let overflow = u128::MAX.safe_add(1);
    /// assert!(overflow.is_err()); // Err(MathError)
    /// ```
    fn safe_add(self, rhs: Self) -> TYieldResult<Self>;

    /// Safely subtracts two values, returning an error if underflow occurs.
    ///
    /// # Arguments
    ///
    /// * `rhs` - The right-hand side value to subtract
    ///
    /// # Returns
    ///
    /// - `Ok(result)` if the subtraction succeeds
    /// - `Err(MathError)` if underflow occurs
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tubor_yield::math::safe_math::SafeMath;
    /// let result = 5_u128.safe_sub(3).unwrap(); // Ok(2)
    /// let underflow = 0_u128.safe_sub(1);
    /// assert!(underflow.is_err()); // Err(MathError)
    /// ```
    fn safe_sub(self, rhs: Self) -> TYieldResult<Self>;

    /// Safely multiplies two values, returning an error if overflow occurs.
    ///
    /// # Arguments
    ///
    /// * `rhs` - The right-hand side value to multiply by
    ///
    /// # Returns
    ///
    /// - `Ok(result)` if the multiplication succeeds
    /// - `Err(MathError)` if overflow occurs
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tubor_yield::math::safe_math::SafeMath;
    /// let result = 5_u128.safe_mul(3).unwrap(); // Ok(15)
    /// let overflow = u128::MAX.safe_mul(2);
    /// assert!(overflow.is_err()); // Err(MathError)
    /// ```
    fn safe_mul(self, rhs: Self) -> TYieldResult<Self>;

    /// Safely divides two values, returning an error if division by zero occurs.
    ///
    /// # Arguments
    ///
    /// * `rhs` - The right-hand side value to divide by
    ///
    /// # Returns
    ///
    /// - `Ok(result)` if the division succeeds
    /// - `Err(MathError)` if division by zero occurs
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tubor_yield::math::safe_math::SafeMath;
    /// let result = 10_u128.safe_div(2).unwrap(); // Ok(5)
    /// let error = 10_u128.safe_div(0);
    /// assert!(error.is_err()); // Err(MathError)
    /// ```
    fn safe_div(self, rhs: Self) -> TYieldResult<Self>;

    /// Safely performs ceiling division, returning an error if division by zero occurs.
    ///
    /// Ceiling division rounds up to the nearest integer. For example, 7 ÷ 3 = 3.
    ///
    /// # Arguments
    ///
    /// * `rhs` - The right-hand side value to divide by
    ///
    /// # Returns
    ///
    /// - `Ok(result)` if the division succeeds
    /// - `Err(MathError)` if division by zero occurs
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tubor_yield::math::safe_math::SafeMath;
    /// let result = 7_u128.safe_div_ceil(3).unwrap(); // Ok(3)
    /// let result = 6_u128.safe_div_ceil(3).unwrap(); // Ok(2)
    /// ```
    fn safe_div_ceil(self, rhs: Self) -> TYieldResult<Self>;
}

macro_rules! checked_impl {
    ($t:ty) => {
        impl SafeMath for $t {
            #[track_caller]
            #[inline(always)]
            fn safe_add(self, v: $t) -> TYieldResult<$t> {
                match self.checked_add(v) {
                    Some(result) => Ok(result),
                    None => {
                        let caller = Location::caller();
                        msg!("Math error thrown at {}:{}", caller.file(), caller.line());
                        Err(ErrorCode::MathError)
                    }
                }
            }

            #[track_caller]
            #[inline(always)]
            fn safe_sub(self, v: $t) -> TYieldResult<$t> {
                match self.checked_sub(v) {
                    Some(result) => Ok(result),
                    None => {
                        let caller = Location::caller();
                        msg!("Math error thrown at {}:{}", caller.file(), caller.line());
                        Err(ErrorCode::MathError)
                    }
                }
            }

            #[track_caller]
            #[inline(always)]
            fn safe_mul(self, v: $t) -> TYieldResult<$t> {
                match self.checked_mul(v) {
                    Some(result) => Ok(result),
                    None => {
                        let caller = Location::caller();
                        msg!("Math error thrown at {}:{}", caller.file(), caller.line());
                        Err(ErrorCode::MathError)
                    }
                }
            }

            #[track_caller]
            #[inline(always)]
            fn safe_div(self, v: $t) -> TYieldResult<$t> {
                match self.checked_div(v) {
                    Some(result) => Ok(result),
                    None => {
                        let caller = Location::caller();
                        msg!("Math error thrown at {}:{}", caller.file(), caller.line());
                        Err(ErrorCode::MathError)
                    }
                }
            }

            #[track_caller]
            #[inline(always)]
            fn safe_div_ceil(self, v: $t) -> TYieldResult<$t> {
                match self.checked_ceil_div(v) {
                    Some(result) => Ok(result),
                    None => {
                        let caller = Location::caller();
                        msg!("Math error thrown at {}:{}", caller.file(), caller.line());
                        Err(ErrorCode::MathError)
                    }
                }
            }
        }
    };
}

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

/// Trait providing safe floor division for signed integer types.
///
/// Floor division rounds down to the nearest integer. For negative numbers,
/// this means rounding away from zero. For example, -7 ÷ 3 = -3.
///
/// # Implemented Types
///
/// This trait is implemented for all signed integer types:
/// - `i8`, `i16`, `i32`, `i64`, `i128`
///
/// # Examples
///
/// ```rust
/// use tubor_yield::math::safe_math::SafeDivFloor;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let result = (-7_i128).safe_div_floor(3).unwrap(); // Ok(-3)
///     let result = (7_i128).safe_div_floor(3).unwrap(); // Ok(2)
///     Ok(())
/// }
/// ```
pub trait SafeDivFloor: Sized {
    /// Perform floor division, returning an error if division by zero occurs.
    ///
    /// # Arguments
    ///
    /// * `rhs` - The right-hand side value to divide by
    ///
    /// # Returns
    ///
    /// - `Ok(result)` if the division succeeds
    /// - `Err(MathError)` if division by zero occurs
    fn safe_div_floor(self, rhs: Self) -> TYieldResult<Self>;
}

macro_rules! div_floor_impl {
    ($t:ty) => {
        impl SafeDivFloor for $t {
            #[track_caller]
            #[inline(always)]
            fn safe_div_floor(self, v: $t) -> TYieldResult<$t> {
                match self.checked_floor_div(v) {
                    Some(result) => Ok(result),
                    None => {
                        let caller = Location::caller();
                        msg!("Math error thrown at {}:{}", caller.file(), caller.line());
                        Err(ErrorCode::MathError)
                    }
                }
            }
        }
    };
}

div_floor_impl!(i128);
div_floor_impl!(i64);
div_floor_impl!(i32);
div_floor_impl!(i16);
div_floor_impl!(i8);

#[cfg(test)]
mod test {
    use crate::error::ErrorCode;
    use crate::math::safe_math::{SafeDivFloor, SafeMath};

    /// Test safe addition operations
    #[test]
    fn safe_add() {
        // Test successful addition
        assert_eq!(1_u128.safe_add(1).unwrap(), 2);

        // Test overflow protection
        assert_eq!(1_u128.safe_add(u128::MAX), Err(ErrorCode::MathError));
    }

    /// Test safe subtraction operations
    #[test]
    fn safe_sub() {
        // Test successful subtraction
        assert_eq!(1_u128.safe_sub(1).unwrap(), 0);

        // Test underflow protection
        assert_eq!(0_u128.safe_sub(1), Err(ErrorCode::MathError));
    }

    /// Test safe multiplication operations
    #[test]
    fn safe_mul() {
        // Test successful multiplication
        assert_eq!(8_u128.safe_mul(80).unwrap(), 640);
        assert_eq!(1_u128.safe_mul(1).unwrap(), 1);

        // Test overflow protection
        assert_eq!(2_u128.safe_mul(u128::MAX), Err(ErrorCode::MathError));
    }

    /// Test safe division operations
    #[test]
    fn safe_div() {
        // Test successful division
        assert_eq!(155_u128.safe_div(8).unwrap(), 19);
        assert_eq!(159_u128.safe_div(8).unwrap(), 19);
        assert_eq!(160_u128.safe_div(8).unwrap(), 20);

        // Test edge cases
        assert_eq!(1_u128.safe_div(1).unwrap(), 1);
        assert_eq!(1_u128.safe_div(100).unwrap(), 0);

        // Test division by zero protection
        assert_eq!(1_u128.safe_div(0), Err(ErrorCode::MathError));
    }

    /// Test safe floor division operations for signed integers
    #[test]
    fn safe_div_floor() {
        // Test negative number floor division
        assert_eq!((-155_i128).safe_div_floor(8).unwrap(), -20);
        assert_eq!((-159_i128).safe_div_floor(8).unwrap(), -20);
        assert_eq!((-160_i128).safe_div_floor(8).unwrap(), -20);
    }
}
