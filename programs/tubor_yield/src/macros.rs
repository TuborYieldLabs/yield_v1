/// A macro that provides logging functionality similar to `solana_program::msg!` but can be compiled away for off-chain use.
///
/// This macro wraps the standard Solana program logging macro and provides the same interface,
/// making it easier to use in cross-platform code that needs to work both on-chain and off-chain.
///
/// # Examples
///
/// Example usage with a string literal:
///     msg!("Hello, world!");
///
/// Example usage with formatted strings:
///     let value = 42;
///     msg!("The value is: {}", value);
///
/// # Parameters
///
/// - `$msg:expr` - A string expression for simple messages
/// - `$($arg:tt)*` - Format arguments for complex formatted strings
///
/// # Returns
///
/// This macro expands to `anchor_lang::solana_program::msg!` calls and doesn't return a value.
#[macro_export]
macro_rules! msg {
    ($msg:expr) => {
        anchor_lang::solana_program::msg!($msg)
    };
    ($($arg:tt)*) => {
        (anchor_lang::solana_program::msg!(&format!($($arg)*)));
    }
}

/// A macro for safely converting account references to specific types using `try_from`.
///
/// This macro provides a safe way to convert `AccountInfo` references to specific account types
/// by using `try_from` instead of direct deserialization. It includes a transmute operation
/// that is necessary for compatibility with Anchor's account validation system.
///
/// # Safety
///
/// This macro uses `unsafe` code internally with `core::mem::transmute`. The safety relies on:
/// - The account being properly validated by Anchor's account validation system
/// - The target type implementing `TryFrom<AccountInfo>`
/// - The account data being properly initialized and valid
///
/// # Examples
///
/// Example converting to a user account:
///     let user_account = try_from!(UserAccount, &user_account_info);
///
/// Example converting to a trade account:
///     let trade_account = try_from!(TradeAccount, &trade_account_info);
///
/// # Parameters
///
/// - `$ty:ty` - The target type to convert to (must implement `TryFrom<AccountInfo>`)
/// - `$acc:expr` - The account reference to convert from
///
/// # Returns
///
/// Returns a `Result<T, E>` where `T` is the target type and `E` is the error type
/// from the `TryFrom` implementation.
///
/// # References
///
/// This implementation is based on the Anchor PR: https://github.com/coral-xyz/anchor/pull/2770
#[macro_export]
macro_rules! try_from {
    ($ty: ty, $acc: expr) => {{
        let acc_ref = $acc.as_ref();
        <$ty>::try_from(unsafe {
            core::mem::transmute::<
                &anchor_lang::prelude::AccountInfo<'_>,
                &anchor_lang::prelude::AccountInfo<'_>,
            >(acc_ref)
        })
    }};
}

/// Example of using the msg macro
pub fn example_msg_usage() {
    // Example implementation
}

/// Example of using the msg macro with formatting
pub fn example_msg_formatting() {
    // Example implementation
}

/// Example of using the try_from macro
pub fn example_try_from_usage() {
    // Example implementation
}

/// Example of using the try_from macro with TradeAccount
pub fn example_try_from_trade_usage() {
    // Example implementation
}
