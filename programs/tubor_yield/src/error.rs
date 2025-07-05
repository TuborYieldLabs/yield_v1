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
