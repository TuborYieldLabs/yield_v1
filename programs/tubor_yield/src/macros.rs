/// same as `solana_program::msg!` but it can compile away for off-chain use
#[macro_export]
macro_rules! msg {
    ($msg:expr) => {
        anchor_lang::solana_program::msg!($msg)
    };
    ($($arg:tt)*) => {
        (anchor_lang::solana_program::msg!(&format!($($arg)*)));
    }
}
