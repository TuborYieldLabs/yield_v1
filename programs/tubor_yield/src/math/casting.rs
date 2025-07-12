//! Casting utilities for safe type conversions with error handling.
//!
//! This module provides the [`Cast`] trait, which adds a convenient [`cast`] method to numeric types and other primitives.
//! The [`cast`] method attempts to convert a value to another type using [`TryFrom`], returning a custom error and logging the location if the conversion fails.
use crate::error::{ErrorCode, TYieldResult};
use crate::math::bn::U192;
use crate::msg;
use std::convert::TryInto;
use std::panic::Location;

/// Trait for safe casting between types with error reporting.
///
/// This trait provides a [`cast`] method that attempts to convert a value to another type using [`TryFrom`].
/// If the conversion fails, it logs the file and line number where the error occurred and returns a [`CastingFailure`] error.
pub trait Cast: Sized {
    /// Attempts to cast `self` to type `T` using [`TryFrom`].
    ///
    /// # Errors
    /// Returns [`ErrorCode::CastingFailure`] if the conversion fails, and logs the location of the failure.
    ///
    /// # Example
    /// ```rust
    /// use tubor_yield::math::casting::Cast;
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let x: u64 = 42;
    ///     let _y: u8 = x.cast().map_err(|e| format!("{:?}", e))?;
    ///     Ok(())
    /// }
    /// ```
    #[track_caller]
    #[inline(always)]
    fn cast<T: std::convert::TryFrom<Self>>(self) -> TYieldResult<T> {
        match self.try_into() {
            Ok(result) => Ok(result),
            Err(_) => {
                let caller = Location::caller();
                msg!(
                    "Casting error thrown at {}:{}",
                    caller.file(),
                    caller.line()
                );
                Err(ErrorCode::CastingFailure)
            }
        }
    }
}

// Blanket implementations for all supported primitive types and U192.
// This allows calling `.cast()` on these types directly.
impl Cast for U192 {}
impl Cast for u128 {}
impl Cast for u64 {}
impl Cast for u32 {}
impl Cast for u16 {}
impl Cast for u8 {}
impl Cast for usize {}
impl Cast for i128 {}
impl Cast for i64 {}
impl Cast for i32 {}
impl Cast for i16 {}
impl Cast for i8 {}
impl Cast for bool {}

/// Example of using the Cast trait
///
/// ```rust
/// use tubor_yield::math::casting::Cast;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let x: u64 = 42;
///     let _y: u8 = x.cast().map_err(|e| format!("{:?}", e))?;
///     Ok(())
/// }
/// ```
pub fn example_cast_usage() -> Result<(), Box<dyn std::error::Error>> {
    let x: u64 = 42;
    let _y: u8 = x.cast().map_err(|e| format!("{:?}", e))?;
    Ok(())
}
