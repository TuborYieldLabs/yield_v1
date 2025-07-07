use anchor_lang::solana_program::native_token::LAMPORTS_PER_SOL; // expo 9
pub const LAMPORTS_PER_SOL_U64: u64 = LAMPORTS_PER_SOL;
pub const LAMPORTS_PER_SOL_I64: i64 = LAMPORTS_PER_SOL as i64;

pub const ORACLE_EXPONENT_SCALE: i32 = -9; // 10^-9
pub const ORACLE_PRICE_SCALE: u64 = 1_000_000_000; // 10^9
pub const ORACLE_MAX_PRICE: u64 = (1 << 28) - 1; // 2^28 - 1

pub const USD_DECIMALS: u8 = 6;

pub const MAX_SIGNERS: usize = 6;

pub const QUOTE_PRECISION: u128 = 1_000_000; // expo = -6
pub const QUOTE_PRECISION_I128: i128 = 1_000_000; // expo = -6
pub const QUOTE_PRECISION_I64: i64 = 1_000_000; // expo = -6
pub const QUOTE_PRECISION_U64: u64 = 1_000_000; // expo = -6

pub const PERCENTAGE_PRECISION: i64 = 10_000; // expo = -4
pub const PERCENTAGE_PRECISION_U128: u128 = 10_000; // expo = -4
pub const PERCENTAGE_PRECISION_I128: i128 = 10_000; // expo = -4
pub const PERCENTAGE_PRECISION_U64: u64 = 10_000; // expo = -4
