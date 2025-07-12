//! Big number types for 192-bit and 256-bit unsigned integers, with conversion utilities.
//!
//! This module provides the `U256` and `U192` types for handling large unsigned integers,
//! along with methods for safe conversion to smaller types and byte array representations.

#![allow(clippy::assign_op_pattern)]
#![allow(clippy::ptr_offset_with_cast)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::manual_div_ceil)]

use crate::error::ErrorCode::BnConversionError;
use std::convert::TryInto;
use std::mem::size_of;
use uint::construct_uint;

use crate::error::TYieldResult;

construct_uint! {
    /// 256-bit unsigned integer type for high-precision arithmetic.
    pub struct U256(4);
}

impl U256 {
    /// Converts the `U256` value to a `u64`, returning `None` if it doesn't fit.
    ///
    /// # Returns
    /// * `Some(u64)` if the value fits in a `u64`.
    /// * `None` if the value is too large.
    pub fn to_u64(self) -> Option<u64> {
        self.try_to_u64().ok()
    }

    /// Attempts to convert the `U256` value to a `u64`.
    ///
    /// # Errors
    /// Returns `BnConversionError` if the value does not fit in a `u64`.
    pub fn try_to_u64(self) -> TYieldResult<u64> {
        self.try_into().map_err(|_| BnConversionError)
    }

    /// Converts the `U256` value to a `u128`, returning `None` if it doesn't fit.
    ///
    /// # Returns
    /// * `Some(u128)` if the value fits in a `u128`.
    /// * `None` if the value is too large.
    pub fn to_u128(self) -> Option<u128> {
        self.try_to_u128().ok()
    }

    /// Attempts to convert the `U256` value to a `u128`.
    ///
    /// # Errors
    /// Returns `BnConversionError` if the value does not fit in a `u128`.
    pub fn try_to_u128(self) -> TYieldResult<u128> {
        self.try_into().map_err(|_| BnConversionError)
    }

    /// Creates a `U256` from a 32-byte little-endian array.
    ///
    /// # Arguments
    /// * `bytes` - A 32-byte array in little-endian order.
    ///
    /// # Returns
    /// * `U256` value represented by the byte array.
    pub fn from_le_bytes(bytes: [u8; 32]) -> Self {
        U256::from_little_endian(&bytes)
    }

    /// Converts the `U256` value to a 32-byte little-endian array.
    ///
    /// # Returns
    /// * `[u8; 32]` - The little-endian byte representation of the value.
    pub fn to_le_bytes(self) -> [u8; 32] {
        let buf: Vec<u8> = Vec::with_capacity(size_of::<Self>());
        self.to_little_endian();

        let mut bytes: [u8; 32] = [0u8; 32];
        bytes.copy_from_slice(buf.as_slice());
        bytes
    }
}

construct_uint! {
    /// 192-bit unsigned integer type for high-precision arithmetic.
    pub struct U192(3);
}

impl U192 {
    /// Converts the `U192` value to a `u64`, returning `None` if it doesn't fit.
    ///
    /// # Returns
    /// * `Some(u64)` if the value fits in a `u64`.
    /// * `None` if the value is too large.
    pub fn to_u64(self) -> Option<u64> {
        self.try_to_u64().ok()
    }

    /// Attempts to convert the `U192` value to a `u64`.
    ///
    /// # Errors
    /// Returns `BnConversionError` if the value does not fit in a `u64`.
    pub fn try_to_u64(self) -> TYieldResult<u64> {
        self.try_into().map_err(|_| BnConversionError)
    }

    /// Converts the `U192` value to a `u128`, returning `None` if it doesn't fit.
    ///
    /// # Returns
    /// * `Some(u128)` if the value fits in a `u128`.
    /// * `None` if the value is too large.
    pub fn to_u128(self) -> Option<u128> {
        self.try_to_u128().ok()
    }

    /// Attempts to convert the `U192` value to a `u128`.
    ///
    /// # Errors
    /// Returns `BnConversionError` if the value does not fit in a `u128`.
    pub fn try_to_u128(self) -> TYieldResult<u128> {
        self.try_into().map_err(|_| BnConversionError)
    }

    /// Creates a `U192` from a 24-byte little-endian array.
    ///
    /// # Arguments
    /// * `bytes` - A 24-byte array in little-endian order.
    ///
    /// # Returns
    /// * `U192` value represented by the byte array.
    pub fn from_le_bytes(bytes: [u8; 24]) -> Self {
        U192::from_little_endian(&bytes)
    }

    /// Converts the `U192` value to a 24-byte little-endian array.
    ///
    /// # Returns
    /// * `[u8; 24]` - The little-endian byte representation of the value.
    pub fn to_le_bytes(self) -> [u8; 24] {
        let buf: Vec<u8> = Vec::with_capacity(size_of::<Self>());
        self.to_little_endian();

        let mut bytes: [u8; 24] = [0u8; 24];
        bytes.copy_from_slice(buf.as_slice());
        bytes
    }
}

/// Example of using the U256 type
///
/// ```rust
/// use tubor_yield::math::bn::U256;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let x = U256::from(42);
///     let y = U256::from(10);
///     let result = x + y;
///     Ok(())
/// }
/// ```
pub fn example_u256_usage() -> Result<(), Box<dyn std::error::Error>> {
    let x = U256::from(42);
    let y = U256::from(10);
    let _result = x + y;
    Ok(())
}
