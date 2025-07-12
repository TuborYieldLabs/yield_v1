//! Safe unwrapping utilities for the Tubor Yield protocol.
//!
//! This module provides a `SafeUnwrap` trait that offers a safer alternative to the standard
//! `unwrap()` method. Instead of panicking when encountering `None` or `Err` values,
//! `safe_unwrap()` returns a `TYieldResult` with proper error handling and logging.
//!
//! # Features
//!
//! - **No Panics**: Replaces panicking `unwrap()` calls with controlled error handling
//! - **Caller Tracking**: Automatically logs the file and line number where unwrapping failed
//! - **Protocol Integration**: Returns `TYieldResult` for seamless integration with the protocol
//! - **Zero Cost**: Uses `#[inline(always)]` for optimal performance
//!
//! # Examples
//!
//! ```rust
//! use tubor_yield::math::safe_unwrap::SafeUnwrap;
//! // Safe unwrapping of Option
//! let some_value: Option<u64> = Some(42);
//! let result = some_value.safe_unwrap(); // Ok(42)
//!
//! let none_value: Option<u64> = None;
//! let result = none_value.safe_unwrap(); // Err(ErrorCode::FailedUnwrap) with logged location
//!
//! // Safe unwrapping of Result
//! let ok_result: Result<u64, &str> = Ok(42);
//! let result = ok_result.safe_unwrap(); // Ok(42)
//!
//! let err_result: Result<u64, &str> = Err("error");
//! let result = err_result.safe_unwrap(); // Err(ErrorCode::FailedUnwrap) with logged location
//! ```

use crate::error::{ErrorCode, TYieldResult};
use crate::msg;
use std::panic::Location;

/// A trait providing safe unwrapping functionality for `Option` and `Result` types.
///
/// This trait replaces the standard `unwrap()` method with a safer alternative that
/// doesn't panic but instead returns a `TYieldResult` with proper error handling.
/// When unwrapping fails, it logs the caller's location for debugging purposes.
///
/// # Safety
///
/// Unlike `unwrap()`, this method never panics. It always returns a `TYieldResult`,
/// making it safe for use in production environments where panics are unacceptable.
///
/// # Performance
///
/// The trait methods are marked with `#[inline(always)]` to ensure zero-cost abstraction
/// when the compiler can optimize the calls.
pub trait SafeUnwrap {
    /// The type contained within the unwrapped value.
    type Item;

    /// Safely unwraps the value, returning `Ok(value)` on success or `Err(ErrorCode::FailedUnwrap)` on failure.
    ///
    /// # Behavior
    ///
    /// - For `Option<T>`: Returns `Ok(value)` if `Some(value)`, otherwise `Err(ErrorCode::FailedUnwrap)`
    /// - For `Result<T, E>`: Returns `Ok(value)` if `Ok(value)`, otherwise `Err(ErrorCode::FailedUnwrap)`
    ///
    /// # Error Handling
    ///
    /// When unwrapping fails, this method:
    /// 1. Logs the file and line number where the failure occurred using `msg!()`
    /// 2. Returns `Err(ErrorCode::FailedUnwrap)` for consistent error handling
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tubor_yield::math::safe_unwrap::SafeUnwrap;
    /// let value = Some(42).safe_unwrap().unwrap();
    /// assert_eq!(value, 42);
    /// let result = None::<i32>.safe_unwrap();
    /// assert!(result.is_err());
    /// ```
    #[track_caller]
    // #[inline(always)]
    fn safe_unwrap(self) -> TYieldResult<Self::Item>;
}

impl<T> SafeUnwrap for Option<T> {
    type Item = T;

    /// Safely unwraps an `Option<T>`.
    ///
    /// Returns `Ok(value)` if the option is `Some(value)`, otherwise returns
    /// `Err(ErrorCode::FailedUnwrap)` and logs the caller's location.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tubor_yield::math::safe_unwrap::SafeUnwrap;
    /// let some_value: Option<u64> = Some(42);
    /// assert_eq!(some_value.safe_unwrap().unwrap(), 42);
    ///
    /// let none_value: Option<u64> = None;
    /// assert!(none_value.safe_unwrap().is_err());
    /// ```
    #[track_caller]
    #[inline(always)]
    fn safe_unwrap(self) -> TYieldResult<T> {
        match self {
            Some(v) => Ok(v),
            None => {
                let caller = Location::caller();
                msg!("Unwrap error thrown at {}:{}", caller.file(), caller.line());
                Err(ErrorCode::FailedUnwrap)
            }
        }
    }
}

impl<T, U> SafeUnwrap for Result<T, U> {
    type Item = T;

    /// Safely unwraps a `Result<T, U>`.
    ///
    /// Returns `Ok(value)` if the result is `Ok(value)`, otherwise returns
    /// `Err(ErrorCode::FailedUnwrap)` and logs the caller's location.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tubor_yield::math::safe_unwrap::SafeUnwrap;
    /// let ok_result: Result<u64, &str> = Ok(42);
    /// assert_eq!(ok_result.safe_unwrap().unwrap(), 42);
    ///
    /// let err_result: Result<u64, &str> = Err("error message");
    /// assert!(err_result.safe_unwrap().is_err());
    /// ```
    #[track_caller]
    #[inline(always)]
    fn safe_unwrap(self) -> TYieldResult<T> {
        match self {
            Ok(v) => Ok(v),
            Err(_) => {
                let caller = Location::caller();
                msg!("Unwrap error thrown at {}:{}", caller.file(), caller.line());
                Err(ErrorCode::FailedUnwrap)
            }
        }
    }
}
