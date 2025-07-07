use anchor_lang::prelude::*;

pub type TYieldResult<T = ()> = std::result::Result<T, ErrorCode>;

#[error_code]
#[derive(PartialEq, Eq)]
pub enum ErrorCode {
    #[msg("Conversion to u128/u64 failed with an overflow or underflow")]
    BnConversionError,
    #[msg("Casting Failure")]
    CastingFailure,
    #[msg("Math Error")]
    MathError,
    #[msg("Failed Unwrap")]
    FailedUnwrap,
    #[msg("Required Signature Is Missing")]
    MissingRequiredSignature,
    #[msg("Multisig account is not authorized")]
    MultisigAccountNotAuthorized,
    #[msg("Invalid instruction hash")]
    InvalidInstructionHash,
    #[msg("Multisig transaction already signed")]
    MultisigAlreadySigned,
    #[msg("Multisig transaction already executed")]
    MultisigAlreadyExecuted,
    #[msg("Invalid referrer provided")]
    InvalidReferrer,
    #[msg("Invalid bump provided")]
    InvalidBump,
    #[msg("Constraint owner check failed")]
    ConstraintOwner,
    #[msg("Invalid program executable")]
    InvalidProgramExecutable,
    #[msg("Invalid authority")]
    InvalidAuthority,
    #[msg("Invalid account provided")]
    InvalidAccount,
    #[msg("Insufficient funds for operation")]
    InsufficientFunds,
    // Trade-specific errors
    #[msg("Trade size cannot be zero")]
    InvalidTradeSize,
    #[msg("Entry price cannot be zero")]
    InvalidEntryPrice,
    #[msg("Take profit must be higher than entry price for buy orders")]
    InvalidTakeProfitBuy,
    #[msg("Take profit must be lower than entry price for sell orders")]
    InvalidTakeProfitSell,
    #[msg("Stop loss must be lower than entry price for buy orders")]
    InvalidStopLossBuy,
    #[msg("Stop loss must be higher than entry price for sell orders")]
    InvalidStopLossSell,
    #[msg("This referrer is not a user")]
    ReferrerNotAUser,
}

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
