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

#[macro_export]
macro_rules! try_from {
    //https://github.com/coral-xyz/anchor/pull/2770
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
