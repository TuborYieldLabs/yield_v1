//! Big number types

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
    /// 256-bit unsigned integer.
    pub struct U256(4);
}

impl U256 {
    /// Convert u256 to u64
    pub fn to_u64(self) -> Option<u64> {
        self.try_to_u64().ok()
    }

    /// Convert u256 to u64
    pub fn try_to_u64(self) -> TYieldResult<u64> {
        self.try_into().map_err(|_| BnConversionError)
    }

    /// Convert u256 to u128
    pub fn to_u128(self) -> Option<u128> {
        self.try_to_u128().ok()
    }

    /// Convert u256 to u128
    pub fn try_to_u128(self) -> TYieldResult<u128> {
        self.try_into().map_err(|_| BnConversionError)
    }

    /// Convert from little endian bytes
    pub fn from_le_bytes(bytes: [u8; 32]) -> Self {
        U256::from_little_endian(&bytes)
    }

    /// Convert to little endian bytes
    pub fn to_le_bytes(self) -> [u8; 32] {
        let buf: Vec<u8> = Vec::with_capacity(size_of::<Self>());
        self.to_little_endian();

        let mut bytes: [u8; 32] = [0u8; 32];
        bytes.copy_from_slice(buf.as_slice());
        bytes
    }
}

construct_uint! {
    /// 192-bit unsigned integer.
    pub struct U192(3);
}

impl U192 {
    /// Convert u192 to u64
    pub fn to_u64(self) -> Option<u64> {
        self.try_to_u64().ok()
    }

    /// Convert u192 to u64
    pub fn try_to_u64(self) -> TYieldResult<u64> {
        self.try_into().map_err(|_| BnConversionError)
    }

    /// Convert u192 to u128
    pub fn to_u128(self) -> Option<u128> {
        self.try_to_u128().ok()
    }

    /// Convert u192 to u128
    pub fn try_to_u128(self) -> TYieldResult<u128> {
        self.try_into().map_err(|_| BnConversionError)
    }

    /// Convert from little endian bytes
    pub fn from_le_bytes(bytes: [u8; 24]) -> Self {
        U192::from_little_endian(&bytes)
    }

    /// Convert to little endian bytes
    pub fn to_le_bytes(self) -> [u8; 24] {
        let buf: Vec<u8> = Vec::with_capacity(size_of::<Self>());
        self.to_little_endian();

        let mut bytes: [u8; 24] = [0u8; 24];
        bytes.copy_from_slice(buf.as_slice());
        bytes
    }
}
