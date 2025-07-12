//! Error handling module for the Tubor Yield protocol
//!
//! This module defines custom error types and provides utilities for error handling
//! throughout the Tubor Yield Solana program. It includes:
//!
//! - Custom error codes with descriptive messages
//! - Type aliases for result handling
//! - Conversion implementations for external error types
//! - Macros for error logging and math error handling
//!
//! # Usage
//!
//! ```rust
//! use tubor_yield::error::{TYieldResult, ErrorCode};
//!
//! fn some_function() -> TYieldResult<u64> {
//!     // Your logic here
//!     Ok(42)
//! }
//! ```

use anchor_lang::error::Error as AnchorError;
// use anchor_lang::prelude::PR
use anchor_lang::prelude::*;

/// Type alias for results that can return a custom error
///
/// This is the primary result type used throughout the Tubor Yield protocol.
/// It wraps the standard Result type with our custom ErrorCode.
pub type TYieldResult<T = ()> = std::result::Result<T, ErrorCode>;

/// Converts Anchor framework errors to our custom error type
impl From<AnchorError> for ErrorCode {
    fn from(_error: AnchorError) -> Self {
        ErrorCode::AnchorError
    }
}

/// Converts Solana program errors to our custom error type
impl From<ProgramError> for ErrorCode {
    fn from(_error: ProgramError) -> Self {
        ErrorCode::ProgramError
    }
}

/// Custom error codes for the Tubor Yield protocol
///
/// Each variant represents a specific error condition that can occur
/// during program execution. All variants include descriptive error messages
/// that are displayed to users when errors occur.
#[error_code]
#[derive(PartialEq, Eq)]
pub enum ErrorCode {
    /// Conversion to u128/u64 failed with an overflow or underflow
    #[msg("Conversion to u128/u64 failed with an overflow or underflow")]
    BnConversionError,

    /// Casting operation failed
    #[msg("Casting Failure")]
    CastingFailure,

    /// Mathematical operation failed (overflow, underflow, division by zero, etc.)
    #[msg("Math Error")]
    MathError,

    /// Failed to unwrap an Option or Result
    #[msg("Failed Unwrap")]
    FailedUnwrap,

    /// Required signature is missing from the transaction
    #[msg("Required Signature Is Missing")]
    MissingRequiredSignature,

    /// Multisig account is not authorized for this operation
    #[msg("Multisig account is not authorized")]
    MultisigAccountNotAuthorized,

    /// Invalid instruction hash provided
    #[msg("Invalid instruction hash")]
    InvalidInstructionHash,

    /// Multisig transaction has already been signed
    #[msg("Multisig transaction already signed")]
    MultisigAlreadySigned,

    /// Multisig transaction has already been executed
    #[msg("Multisig transaction already executed")]
    MultisigAlreadyExecuted,

    /// Invalid referrer account provided
    #[msg("Invalid referrer provided")]
    InvalidReferrer,

    /// Invalid bump seed provided for PDA derivation
    #[msg("Invalid bump provided")]
    InvalidBump,

    /// Constraint owner check failed
    #[msg("Constraint owner check failed")]
    ConstraintOwner,

    /// Invalid program executable account
    #[msg("Invalid program executable")]
    InvalidProgramExecutable,

    /// Invalid authority account
    #[msg("Invalid authority")]
    InvalidAuthority,

    /// Invalid account provided for operation
    #[msg("Invalid account provided")]
    InvalidAccount,

    /// Insufficient funds for the requested operation
    #[msg("Insufficient funds for operation")]
    InsufficientFunds,

    // Trade-specific errors
    /// Trade size cannot be zero
    #[msg("Trade size cannot be zero")]
    InvalidTradeSize,

    /// Entry price cannot be zero
    #[msg("Entry price cannot be zero")]
    InvalidEntryPrice,

    /// Take profit must be higher than entry price for buy orders
    #[msg("Take profit must be higher than entry price for buy orders")]
    InvalidTakeProfitBuy,

    /// Take profit must be lower than entry price for sell orders
    #[msg("Take profit must be lower than entry price for sell orders")]
    InvalidTakeProfitSell,

    /// Stop loss must be lower than entry price for buy orders
    #[msg("Stop loss must be lower than entry price for buy orders")]
    InvalidStopLossBuy,

    /// Stop loss must be higher than entry price for sell orders
    #[msg("Stop loss must be higher than entry price for sell orders")]
    InvalidStopLossSell,

    /// The provided referrer is not a registered user
    #[msg("This referrer is not a user")]
    ReferrerNotAUser,

    /// Invalid oracle account provided
    #[msg("Invalid oracle account provided")]
    InvalidOracleAccount,

    /// Oracle price is stale (too old)
    #[msg("Oracle price is stale")]
    StaleOraclePrice,

    /// Oracle price is invalid (zero, negative, or out of range)
    #[msg("Invalid oracle price")]
    InvalidOraclePrice,

    /// Time-weighted average price (TWAP) data is missing
    #[msg("Missing time-weighted average price data")]
    MissingTwap,

    /// Oracle type is not supported
    #[msg("Unsupported oracle type")]
    UnsupportedOracle,

    // Enhanced price validation errors
    /// Current price exceeds maximum allowed slippage
    #[msg("Current price exceeds maximum allowed slippage")]
    MaxPriceSlippage,

    /// Price validation failed - current price is invalid
    #[msg("Price validation failed - current price is invalid")]
    PriceValidationFailed,

    /// Stop loss price is too close to entry price
    #[msg("Stop loss price is too close to entry price")]
    StopLossTooClose,

    /// Take profit price is too close to entry price
    #[msg("Take profit price is too close to entry price")]
    TakeProfitTooClose,

    /// Risk-reward ratio is too low for the trade
    #[msg("Risk-reward ratio is too low")]
    InsufficientRiskRewardRatio,

    /// Current price is outside acceptable trading range
    #[msg("Current price is outside acceptable trading range")]
    PriceOutOfRange,

    /// Oracle price confidence interval is too wide
    #[msg("Oracle price confidence interval is too wide")]
    OracleConfidenceTooLow,

    /// Price deviation from expected range is too high
    #[msg("Price deviation from expected range is too high")]
    PriceDeviationTooHigh,

    /// Cannot perform the requested action in current state
    #[msg("Cannot perform the requested action in current state")]
    CannotPerformAction,

    /// Error occurred while creating account from instruction data
    #[msg("Failed to create account from instruction data")]
    AccountFromError,

    /// Price update attempted too soon after last update
    #[msg("Price update attempted too soon after last update")]
    PriceUpdateTooSoon,

    /// Price update exceeds maximum allowed change
    #[msg("Price update exceeds maximum allowed change")]
    PriceUpdateTooHigh,

    /// Generic Anchor framework error
    #[msg("Anchor framework error occurred")]
    AnchorError,

    /// Generic Solana program error
    #[msg("Solana program error occurred")]
    ProgramError,
}

/// Macro for printing error information with file and line details
///
/// This macro logs error information including the error code, file name, and line number
/// where the error occurred. Useful for debugging and error tracking.
///
/// # Usage
///
/// ```rust
/// use tubor_yield::msg;
/// use tubor_yield::print_error;
/// use tubor_yield::error::{TYieldResult, ErrorCode};
/// let result: TYieldResult<()> = Err(ErrorCode::MathError);
/// let _ = result.map_err(|_| print_error!(ErrorCode::MathError)());
/// ```
#[macro_export]
macro_rules! print_error {
    ($err:expr) => {{
        || {
            let error_code: ErrorCode = $err;
            msg!("{:?} thrown at {}:{}", error_code, file!(), line!());
            $err
        }
    }};
}

/// Macro for creating and logging math errors
///
/// This macro creates a MathError and logs it with file and line information.
/// Convenient for mathematical operations that can fail.
///
/// # Usage
///
/// ```rust
/// use tubor_yield::msg;
/// use tubor_yield::math_error;
/// let result: Result<(), &'static str> = Err("math error");
/// let _ = result.map_err(|_| math_error!());
/// ```
#[macro_export]
macro_rules! math_error {
    () => {{
        || {
            let error_code = $crate::error::ErrorCode::MathError;
            msg!("Error {} thrown at {}:{}", error_code, file!(), line!());
            error_code
        }
    }};
}

/// Example of using the error module
///
/// ```rust
/// use tubor_yield::error::{TYieldResult, ErrorCode};
/// // Your code here
/// Ok::<(), ErrorCode>(())
/// ```
pub fn example_usage() -> TYieldResult<()> {
    Ok(())
}

/// Example of using the math_error macro
///
/// ```rust
/// use tubor_yield::msg;
/// use tubor_yield::math_error;
/// let result: Result<(), &'static str> = Err("math error");
/// let _ = result.map_err(|_| math_error!());
/// ```
pub fn example_math_error_usage() -> TYieldResult<()> {
    Ok(())
}

/// Example of printing errors
///
pub fn example_print_error() -> TYieldResult<()> {
    Ok(())
}
