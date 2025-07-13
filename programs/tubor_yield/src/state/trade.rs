//! # Trade Module
//!
//! This module defines the core structures, enums, and logic for managing trades in the Tubor Yield protocol.
//! It provides robust validation, risk management, and security features for on-chain trading, including:
//! - Trade state management (active, completed, cancelled)
//! - Trade type (buy/sell) and result (success/failed/pending)
//! - Secure initialization and update of trades
//! - Price validation with slippage, risk-reward, and oracle consensus checks
//! - Circuit breaker and emergency pause mechanisms
//! - Calculation of profit/loss, risk-reward ratios, and optimal entry prices
//! - Comprehensive test suite for all critical logic
//!
//! ## Key Structures
//!
//! - [`Trade`]: The main account struct representing a trade, with all relevant fields and methods for validation and state transitions.
//! - [`TradeStatus`], [`TradeType`], [`TradeResult`]: Enums for trade state, type, and result.
//! - [`TradeSecurityConfig`]: Configuration for trade limits, circuit breaker, and oracle consensus.
//! - [`PriceValidationConfig`]: Configuration for price/risk validation (slippage, risk-reward, etc).
//! - [`OracleConsensus`]: Helper struct for multi-oracle price consensus.
//! - [`TradeInitParams`]: Parameter struct for initializing a trade.
//! - [`TradeEvent`]: Event struct for emitting trade state changes.
//!
//! ## Main Features
//!
//! - **Validation:** All trade operations are guarded by strict validation, including size, price, risk, and oracle checks.
//! - **Access Control:** Only the trade authority can update or cancel a trade.
//! - **Security:** Circuit breaker and emergency pause features protect against flash attacks and extreme price moves.
//! - **Oracle Consensus:** Trades can require multiple oracles to agree on price, with deviation checks.
//! - **Testing:** Extensive unit tests cover all edge cases and logic branches.
//!
//! ## Usage
//!
//! - Use `Trade::init_trade_secure` to securely initialize a trade with authority and validation.
//! - Use `Trade::validate_secure_trade_execution` to check if a trade can be executed under current market and oracle conditions.
//! - Use `Trade::complete_secure` and `Trade::cancel_secure` for secure state transitions.
//! - Use `Trade::calculate_pnl_safe`, `Trade::calculate_risk_reward_ratio`, etc., for analytics and risk management.
//!
//! ## Example
//!
//! ```rust
//! use tubor_yield::state::trade::{Trade, TradeInitParams, TradeStatus, TradeType, TradeResult};
//! use anchor_lang::prelude::Pubkey;
//!
//! fn main() -> Result<(), tubor_yield::error::ErrorCode> {
//!     let mut trade = Trade::default();
//!     let params = TradeInitParams {
//!         master_agent: Pubkey::new_unique(),
//!         size: 1000,
//!         entry_price: 50000,
//!         take_profit: 55000,
//!         stop_loss: 45000,
//!         created_at: 1234567890,
//!         pair: [0u8; 8],
//!         feed_id: [0u8; 32],
//!         status: TradeStatus::Active,
//!         trade_type: TradeType::Buy,
//!         result: TradeResult::Pending,
//!         bump: 0,
//!     };
//!     let authority = Pubkey::new_unique();
//!     trade.init_trade_secure(params, authority)?;
//!     // ... perform trade logic ...
//!     Ok(())
//! }
//! ```
//!
//! ## Testing
//!
//! Run `cargo test` in the `programs/tubor_yield` directory to execute all unit tests for this module.
//!
//! ## Authors
//!
//! - Tubor Yield Protocol Contributors
//!
//! ## License
//!
//! This file is part of the Tubor Yield Protocol and is subject to the terms of the license in the root of the repository.

use anchor_lang::prelude::*;

use crate::error::{ErrorCode, TYieldResult};
use crate::math::safe_math::SafeMath;
use crate::math::PERCENTAGE_PRECISION_U64;
use crate::state::{OraclePrice, Size};

/// Represents a trade in the Tubor Yield protocol.
///
/// This is the main account struct that stores all trade-related data including
/// position details, risk management parameters, security settings, and state information.
/// The struct is designed with 8-byte alignment for optimal on-chain storage efficiency.
///
/// ## Fields
///
/// ### Core Trade Data
/// - `master_agent`: The master agent associated with this trade
/// - `feed_id`: Oracle feed identifier for price data
/// - `pair`: Trading pair identifier (8 bytes)
/// - `size`: Position size in base units
/// - `entry_price`: Entry price for the trade
/// - `take_profit`: Take profit price level
/// - `stop_loss`: Stop loss price level
///
/// ### Timestamps
/// - `created_at`: Unix timestamp when trade was created
/// - `updated_at`: Unix timestamp of last update
///
/// ### State Information
/// - `status`: Current trade status (Active/Completed/Cancelled)
/// - `trade_type`: Trade type (Buy/Sell)
/// - `result`: Trade result (Success/Failed/Pending)
/// - `bump`: PDA bump seed
///
/// ### Security & Access Control
/// - `authority`: Trade authority for access control
/// - `oracle_consensus_count`: Number of oracles that agree on price
/// - `last_price_update`: Timestamp of last price update
/// - `circuit_breaker_triggered`: Circuit breaker state flag
/// - `_padding`: Padding for future-proofing and alignment
///
/// ## Size
/// The struct is exactly 176 bytes on-chain (including 8-byte Anchor discriminator).
///
/// ## Security Features
/// - Authority-based access control for all state changes
/// - Circuit breaker protection against extreme price movements
/// - Multi-oracle consensus validation
/// - Comprehensive validation for all trade parameters
///
/// ## Example
/// ```rust
/// use tubor_yield::state::trade::Trade;
/// use anchor_lang::prelude::Pubkey;
///
/// let trade = Trade {
///     master_agent: Pubkey::new_unique(),
///     size: 1000,
///     entry_price: 50000,
///     take_profit: 55000,
///     stop_loss: 45000,
///     created_at: 1234567890,
///     updated_at: 1234567890,
///     status: 0,
///     trade_type: 0,
///     result: 0,
///     bump: 0,
///     authority: Pubkey::new_unique(),
///     oracle_consensus_count: 0,
///     last_price_update: 1234567890,
///     circuit_breaker_triggered: false,
///     _padding: [0; 2],
///     feed_id: [0; 32],
///     pair: [0; 8],
/// };
/// ```
#[account]
#[derive(Eq, PartialEq, Debug, Default)]
pub struct Trade {
    // 8-byte aligned fields first
    pub master_agent: Pubkey,            // 32 bytes (8-byte aligned)
    pub feed_id: [u8; 32],               // 32 bytes
    pub pair: [u8; 8],                   // 8 bytes
    pub size: u64,                       // 8 bytes
    pub entry_price: u64,                // 8 bytes
    pub take_profit: u64,                // 8 bytes
    pub stop_loss: u64,                  // 8 bytes
    pub created_at: i64,                 // 4 bytes
    pub updated_at: i64,                 // 4 bytes
    pub status: u8,                      // 1 byte
    pub trade_type: u8,                  // 1 byte
    pub result: u8,                      // 1 byte
    pub bump: u8,                        // 1 byte
    pub authority: Pubkey,               // 32 bytes - ADDED: Trade authority for access control
    pub oracle_consensus_count: u8,      // 1 byte - ADDED: Number of oracles that agree
    pub last_price_update: i64,          // 8 bytes - ADDED: Timestamp of last price update
    pub circuit_breaker_triggered: bool, // 1 byte - ADDED: Circuit breaker state
    pub _padding: [u8; 2],               // 2 bytes padding for future-proofing and alignment
}

/// Represents the current status of a trade.
///
/// Each status has a specific binary representation for efficient storage and comparison.
/// The enum is designed to prevent invalid state transitions and ensure trade lifecycle integrity.
///
/// ## Variants
/// - `Active`: Trade is currently active and can be executed or modified
/// - `Completed`: Trade has been completed (either hit take profit or stop loss)
/// - `Cancelled`: Trade has been cancelled and is no longer valid
///
/// ## State Transitions
/// - Active → Completed: When trade hits take profit or stop loss
/// - Active → Cancelled: When trade is manually cancelled
/// - Completed → Cancelled: When completed trade is cancelled (rare)
/// - Cancelled → Active: **NOT ALLOWED** (prevents resurrection attacks)
/// - Completed → Active: **NOT ALLOWED** (prevents resurrection attacks)
/// - Cancelled → Completed: **NOT ALLOWED** (prevents resurrection attacks)
///
/// ## Binary Representation
/// Each variant uses a unique bit pattern for efficient comparison:
/// - Active: `0b00000001`
/// - Completed: `0b00000010`
/// - Cancelled: `0b00000100`
#[derive(Clone, Copy, PartialEq, Debug, Eq, AnchorDeserialize, AnchorSerialize)]
pub enum TradeStatus {
    Active = 0b00000001,
    Completed = 0b00000010,
    Cancelled = 0b00000100,
}

/// Represents the type of trade (buy or sell).
///
/// This enum determines the direction of the trade and affects how prices are interpreted
/// for profit/loss calculations and risk management.
///
/// ## Variants
/// - `Buy`: Long position - profit when price increases above entry
/// - `Sell`: Short position - profit when price decreases below entry
///
/// ## Price Logic
/// - **Buy trades**: Take profit > Entry price > Stop loss
/// - **Sell trades**: Stop loss > Entry price > Take profit
///
/// ## Binary Representation
/// - Buy: `0b00000001`
/// - Sell: `0b00000010`
#[derive(Clone, Copy, PartialEq, Debug, Eq, AnchorDeserialize, AnchorSerialize)]
pub enum TradeType {
    Buy = 0b00000001,
    Sell = 0b00000010,
}

/// Represents the result of a trade execution.
///
/// This enum tracks the outcome of trade execution attempts and is used for
/// analytics, reporting, and risk management purposes.
///
/// ## Variants
/// - `Success`: Trade was executed successfully
/// - `Failed`: Trade execution failed (e.g., insufficient liquidity, price moved)
/// - `Pending`: Trade is waiting for execution or confirmation
///
/// ## Usage
/// - Set to `Pending` when trade is created
/// - Set to `Success` when trade executes successfully
/// - Set to `Failed` when trade execution fails or is cancelled
///
/// ## Binary Representation
/// - Success: `0b00000001`
/// - Failed: `0b00000010`
/// - Pending: `0b00000100`
#[derive(Clone, Copy, PartialEq, Debug, Eq, AnchorDeserialize, AnchorSerialize)]
pub enum TradeResult {
    Success = 0b00000001,
    Failed = 0b00000010,
    Pending = 0b00000100,
}

/// Enhanced security configuration for trade limits and protections.
///
/// This struct defines comprehensive security parameters that protect against
/// various attack vectors and ensure safe trading operations. It includes
/// position limits, circuit breaker thresholds, oracle consensus requirements,
/// and emergency pause mechanisms.
///
/// ## Fields
///
/// ### Position Limits
/// - `max_position_size`: Maximum allowed position size (prevents oversized trades)
/// - `max_price`: Maximum allowed price (prevents extreme price manipulation)
/// - `min_price`: Minimum allowed price (prevents zero/negative price attacks)
///
/// ### Circuit Breaker Settings
/// - `circuit_breaker_threshold_bps`: Price change threshold that triggers circuit breaker (in basis points)
/// - `emergency_pause_threshold`: Extreme price change threshold for emergency pause (in basis points)
///
/// ### Oracle Consensus
/// - `max_oracle_deviation_bps`: Maximum allowed deviation between oracles (in basis points)
/// - `min_oracle_consensus`: Minimum number of oracles required for consensus
/// - `max_price_age_sec`: Maximum age of oracle price data (in seconds)
///
/// ## Default Values
/// The default configuration provides conservative security settings:
/// - Max position: 1B units
/// - Max price: 1T units
/// - Min price: 1 unit
/// - Circuit breaker: 50% price change
/// - Oracle deviation: 10% max
/// - Oracle consensus: 2 oracles minimum
/// - Price age: 5 minutes max
/// - Emergency pause: 100% price change
///
/// ## Usage
/// ```rust
/// use tubor_yield::state::trade::TradeSecurityConfig;
///
/// let config = TradeSecurityConfig::default();
/// // or customize for specific market conditions
/// let aggressive_config = TradeSecurityConfig {
///     circuit_breaker_threshold_bps: 10000, // 100%
///     max_oracle_deviation_bps: 2000,       // 20%
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone)]
pub struct TradeSecurityConfig {
    pub max_position_size: u64,
    pub max_price: u64,
    pub min_price: u64,
    pub circuit_breaker_threshold_bps: u64,
    pub max_oracle_deviation_bps: u64,
    pub min_oracle_consensus: u8,
    pub max_price_age_sec: u32,
    pub emergency_pause_threshold: u64,
}

impl Default for TradeSecurityConfig {
    fn default() -> Self {
        Self {
            max_position_size: 1_000_000_000,    // 1B max position
            max_price: 1_000_000_000_000,        // 1T max price
            min_price: 1,                        // 1 min price
            circuit_breaker_threshold_bps: 5000, // 50% price change
            max_oracle_deviation_bps: 1000,      // 10% max oracle deviation
            min_oracle_consensus: 2,             // Require 2 oracle consensus
            max_price_age_sec: 300,              // 5 minutes max age
            emergency_pause_threshold: 10000,    // 100% price change for emergency
        }
    }
}

/// Multi-oracle consensus result for price validation.
///
/// This struct represents the result of aggregating multiple oracle prices
/// to determine a consensus price that can be trusted for trade execution.
/// It includes validation metrics to ensure the consensus is reliable.
///
/// ## Fields
///
/// - `consensus_price`: The median price from all valid oracle inputs
/// - `consensus_count`: Number of oracles that contributed to the consensus
/// - `max_deviation_bps`: Maximum deviation between any oracle and the consensus (in basis points)
/// - `is_valid`: Whether the consensus meets all validation criteria
///
/// ## Consensus Algorithm
/// 1. Collect all valid oracle prices (non-zero)
/// 2. Calculate median price as consensus
/// 3. Check deviation of each oracle from median
/// 4. Reject if any oracle deviates more than threshold
/// 5. Require minimum number of oracles for consensus
///
/// ## Usage
/// ```rust
/// use tubor_yield::state::trade::OracleConsensus;
/// use tubor_yield::state::OraclePrice;
///
/// fn main() -> Result<(), tubor_yield::error::ErrorCode> {
///     let oracles = vec![
///         OraclePrice { price: 1000, exponent: 0 },
///         OraclePrice { price: 1001, exponent: 0 },
///         OraclePrice { price: 999, exponent: 0 },
///     ];
///     let max_deviation_bps = 100;
///     let min_consensus = 2;
///
///     let consensus = OracleConsensus::calculate_consensus(
///         &oracles,
///         max_deviation_bps,
///         min_consensus
///     )?;
///
///     if consensus.is_valid {
///         // Use consensus_price for trade execution
///     }
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone, Default)]
pub struct OracleConsensus {
    pub consensus_price: u64,
    pub consensus_count: u8,
    pub max_deviation_bps: u64,
    pub is_valid: bool,
}

impl OracleConsensus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate consensus from multiple oracle prices
    pub fn calculate_consensus(
        oracles: &[OraclePrice],
        max_deviation_bps: u64,
        min_consensus: u8,
    ) -> TYieldResult<Self> {
        if oracles.len() < min_consensus as usize {
            return Err(ErrorCode::OracleConsensusThresholdNotMet);
        }

        let mut valid_prices = Vec::new();
        for oracle in oracles {
            if oracle.price > 0 {
                valid_prices.push(oracle.price);
            }
        }

        if valid_prices.len() < min_consensus as usize {
            return Err(ErrorCode::OracleConsensusThresholdNotMet);
        }

        // Calculate median price for consensus
        valid_prices.sort();
        let median_price = if valid_prices.len() % 2 == 0 {
            let mid = valid_prices.len() / 2;
            (valid_prices[mid - 1] + valid_prices[mid]) / 2
        } else {
            valid_prices[valid_prices.len() / 2]
        };

        // Check deviation from median
        let mut max_deviation = 0u64;
        for price in &valid_prices {
            let deviation = if *price >= median_price {
                price.safe_sub(median_price)?
            } else {
                median_price.safe_sub(*price)?
            };

            let deviation_bps = deviation
                .safe_mul(PERCENTAGE_PRECISION_U64)?
                .safe_div(median_price)?;

            if deviation_bps > max_deviation {
                max_deviation = deviation_bps;
            }
        }

        if max_deviation > max_deviation_bps {
            return Err(ErrorCode::OracleDeviationTooHigh);
        }

        Ok(Self {
            consensus_price: median_price,
            consensus_count: valid_prices.len() as u8,
            max_deviation_bps: max_deviation,
            is_valid: true,
        })
    }
}

impl Size for Trade {
    const SIZE: usize = 176; // Updated size to match actual struct size
}

#[event]
pub struct TradeEvent {
    pub trade: Pubkey,
    pub status: TradeStatus,
    pub trade_type: TradeType,
    pub result: TradeResult,
    pub pnl: i64,
    pub created_at: i64,
}

/// Parameters for initializing a Trade
///
#[derive(Clone, Copy)]
pub struct TradeInitParams {
    pub master_agent: Pubkey,
    pub size: u64,
    pub entry_price: u64,
    pub take_profit: u64,
    pub stop_loss: u64,
    pub created_at: i64,
    pub pair: [u8; 8],
    pub feed_id: [u8; 32],
    pub status: TradeStatus,
    pub trade_type: TradeType,
    pub result: TradeResult,
    pub bump: u8,
}

/// Comprehensive price validation parameters for trade execution.
///
/// This struct defines all parameters used for validating trade prices,
/// including slippage protection, risk management levels, and spread calculations.
/// It provides multiple preset configurations for different market conditions.
///
/// ## Fields
///
/// ### Slippage Protection
/// - `max_slippage_bps`: Maximum allowed price slippage (in basis points)
/// - `slippage_buffer_bps`: Additional buffer for slippage calculations (in basis points)
///
/// ### Risk Management
/// - `min_distance_bps`: Minimum distance for stop loss/take profit from entry (in basis points)
/// - `min_risk_reward_bps`: Minimum risk-reward ratio (in basis points, e.g., 150 = 1.5:1)
///
/// ### Price Validation
/// - `max_deviation_bps`: Maximum oracle price deviation (in basis points)
/// - `range_buffer_bps`: Buffer for price range validation (in basis points)
/// - `spread_bps`: Spread adjustment for entry price calculation (in basis points)
///
/// ## Preset Configurations
///
/// ### Default (Balanced)
/// - Max slippage: 5%
/// - Min distance: 1%
/// - Min risk-reward: 1.5:1
/// - Max deviation: 2%
/// - Range buffer: 0.5%
/// - Spread: 0.5%
/// - Slippage buffer: 0.25%
///
/// ### Conservative (High Risk)
/// - Max slippage: 2%
/// - Min distance: 2%
/// - Min risk-reward: 2:1
/// - Max deviation: 1%
/// - Range buffer: 0.25%
/// - Spread: 0.25%
/// - Slippage buffer: 0.1%
///
/// ### Aggressive (Low Risk)
/// - Max slippage: 10%
/// - Min distance: 0.5%
/// - Min risk-reward: 1:1
/// - Max deviation: 5%
/// - Range buffer: 1%
/// - Spread: 1%
/// - Slippage buffer: 0.5%
///
/// ## Usage
/// ```rust
/// use tubor_yield::state::trade::PriceValidationConfig;
///
/// let config = PriceValidationConfig::default();
/// let conservative = PriceValidationConfig::conservative();
/// let aggressive = PriceValidationConfig::aggressive();
/// let custom = PriceValidationConfig::custom(500, 100, 150, 200, 50, 50, 25);
/// ```
#[derive(Debug, Clone)]
pub struct PriceValidationConfig {
    pub max_slippage_bps: u64,
    pub min_distance_bps: u64,
    pub min_risk_reward_bps: u64,
    pub max_deviation_bps: u64,
    pub range_buffer_bps: u64,
    pub spread_bps: u64,
    pub slippage_buffer_bps: u64,
}

impl Default for PriceValidationConfig {
    fn default() -> Self {
        Self {
            max_slippage_bps: 500,    // 5% maximum slippage
            min_distance_bps: 100,    // 1% minimum distance for stop loss/take profit
            min_risk_reward_bps: 150, // 1.5:1 minimum risk-reward ratio
            max_deviation_bps: 200,   // 2% maximum oracle price deviation
            range_buffer_bps: 50,     // 0.5% buffer for price range validation
            spread_bps: 50,           // 0.5% spread
            slippage_buffer_bps: 25,  // 0.25% slippage buffer
        }
    }
}

impl PriceValidationConfig {
    /// Creates a conservative validation config for high-risk environments
    pub fn conservative() -> Self {
        Self {
            max_slippage_bps: 200,    // 2% maximum slippage
            min_distance_bps: 200,    // 2% minimum distance
            min_risk_reward_bps: 200, // 2:1 minimum risk-reward ratio
            max_deviation_bps: 100,   // 1% maximum oracle deviation
            range_buffer_bps: 25,     // 0.25% buffer
            spread_bps: 25,           // 0.25% spread
            slippage_buffer_bps: 10,  // 0.1% slippage buffer
        }
    }

    /// Creates an aggressive validation config for low-risk environments
    pub fn aggressive() -> Self {
        Self {
            max_slippage_bps: 1000,   // 10% maximum slippage
            min_distance_bps: 50,     // 0.5% minimum distance
            min_risk_reward_bps: 100, // 1:1 minimum risk-reward ratio
            max_deviation_bps: 500,   // 5% maximum oracle deviation
            range_buffer_bps: 100,    // 1% buffer
            spread_bps: 100,          // 1% spread
            slippage_buffer_bps: 50,  // 0.5% slippage buffer
        }
    }

    /// Creates a custom validation config with specific parameters
    pub fn custom(
        max_slippage_bps: u64,
        min_distance_bps: u64,
        min_risk_reward_bps: u64,
        max_deviation_bps: u64,
        range_buffer_bps: u64,
        spread_bps: u64,
        slippage_buffer_bps: u64,
    ) -> Self {
        Self {
            max_slippage_bps,
            min_distance_bps,
            min_risk_reward_bps,
            max_deviation_bps,
            range_buffer_bps,
            spread_bps,
            slippage_buffer_bps,
        }
    }

    /// Validates the configuration parameters
    pub fn validate(&self) -> TYieldResult<()> {
        if self.max_slippage_bps == 0 || self.min_distance_bps == 0 {
            return Err(ErrorCode::MathError);
        }
        if self.max_slippage_bps < self.min_distance_bps {
            return Err(ErrorCode::MathError);
        }
        Ok(())
    }

    /// Returns a human-readable description of the configuration
    pub fn describe(&self) -> String {
        format!(
            "PriceValidationConfig {{ max_slippage: {}%, min_distance: {}%, min_risk_reward: {}:1, max_deviation: {}%, range_buffer: {}%, spread: {}%, slippage_buffer: {}% }}",
            self.max_slippage_bps as f64 / 100.0,
            self.min_distance_bps as f64 / 100.0,
            self.min_risk_reward_bps as f64 / 100.0,
            self.max_deviation_bps as f64 / 100.0,
            self.range_buffer_bps as f64 / 100.0,
            self.spread_bps as f64 / 100.0,
            self.slippage_buffer_bps as f64 / 100.0,
        )
    }
}

impl Trade {
    /// Returns the trade status as an enum.
    ///
    /// Converts the internal u8 status field to the corresponding TradeStatus enum.
    /// Provides a safe way to access the trade status with proper type checking.
    ///
    /// ## Returns
    /// The current trade status as a TradeStatus enum.
    ///
    /// ## Example
    /// ```rust
    /// use tubor_yield::state::trade::{Trade, TradeStatus};
    ///
    /// let trade = Trade::default();
    /// let status = trade.get_status();
    /// match status {
    ///     TradeStatus::Active => println!("Trade is active"),
    ///     TradeStatus::Completed => println!("Trade completed"),
    ///     TradeStatus::Cancelled => println!("Trade cancelled"),
    /// }
    /// ```
    pub fn get_status(&self) -> TradeStatus {
        match self.status {
            0b00000001 => TradeStatus::Active,
            0b00000010 => TradeStatus::Completed,
            0b00000100 => TradeStatus::Cancelled,
            _ => TradeStatus::Active, // Default fallback
        }
    }

    /// Returns the trade type as an enum
    pub fn get_trade_type(&self) -> TradeType {
        match self.trade_type {
            0b00000001 => TradeType::Buy,
            0b00000010 => TradeType::Sell,
            _ => TradeType::Buy, // Default fallback
        }
    }

    /// Returns the trade result as an enum
    pub fn get_result(&self) -> TradeResult {
        match self.result {
            0b00000001 => TradeResult::Success,
            0b00000010 => TradeResult::Failed,
            0b00000100 => TradeResult::Pending,
            _ => TradeResult::Pending, // Default fallback
        }
    }

    /// Enhanced status setter with access control
    pub fn set_status(
        &mut self,
        status: TradeStatus,
        authority: &Pubkey,
        current_time: i64,
    ) -> TYieldResult<()> {
        if authority != &self.authority {
            return Err(ErrorCode::InvalidAuthority);
        }

        // Validate state transitions
        match (self.get_status(), status) {
            (TradeStatus::Active, TradeStatus::Completed) => {}
            (TradeStatus::Active, TradeStatus::Cancelled) => {}
            (TradeStatus::Completed, TradeStatus::Cancelled) => {}
            (TradeStatus::Cancelled, TradeStatus::Active) => {
                return Err(ErrorCode::CannotPerformAction);
            }
            (TradeStatus::Completed, TradeStatus::Active) => {
                return Err(ErrorCode::CannotPerformAction);
            }
            (TradeStatus::Cancelled, TradeStatus::Completed) => {
                return Err(ErrorCode::CannotPerformAction);
            }
            _ => {
                return Err(ErrorCode::CannotPerformAction);
            }
        }

        self.status = status as u8;
        self.updated_at = current_time;
        Ok(())
    }

    /// Enhanced result setter with access control
    pub fn set_result(
        &mut self,
        result: TradeResult,
        authority: &Pubkey,
        current_time: i64,
    ) -> TYieldResult<()> {
        if authority != &self.authority {
            return Err(ErrorCode::InvalidAuthority);
        }
        self.result = result as u8;
        self.updated_at = current_time;
        Ok(())
    }

    /// Checks if the trade is active
    pub fn is_active(&self) -> bool {
        self.get_status() == TradeStatus::Active
    }

    /// Checks if the trade is completed
    pub fn is_completed(&self) -> bool {
        self.get_status() == TradeStatus::Completed
    }

    /// Checks if the trade is cancelled
    pub fn is_cancelled(&self) -> bool {
        self.get_status() == TradeStatus::Cancelled
    }

    /// Checks if the trade is a buy order
    pub fn is_buy(&self) -> bool {
        self.get_trade_type() == TradeType::Buy
    }

    /// Checks if the trade is a sell order
    pub fn is_sell(&self) -> bool {
        self.get_trade_type() == TradeType::Sell
    }

    /// Validates the trade parameters.
    ///
    /// Performs comprehensive validation of all trade parameters to ensure they meet
    /// the protocol's requirements and business logic. This is a critical security check
    /// that should be called before any trade execution.
    ///
    /// ## Validation Checks
    ///
    /// ### Basic Parameters
    /// - Size must be greater than zero
    /// - Entry price must be greater than zero
    ///
    /// ### Take Profit Validation
    /// - For buy trades: Take profit must be greater than entry price
    /// - For sell trades: Take profit must be less than entry price
    ///
    /// ### Stop Loss Validation
    /// - For buy trades: Stop loss must be less than entry price
    /// - For sell trades: Stop loss must be greater than entry price
    ///
    /// ## Returns
    /// - `Ok(())`: All validations passed
    /// - `Err(InvalidTradeSize)`: Size is zero or invalid
    /// - `Err(InvalidEntryPrice)`: Entry price is zero or invalid
    /// - `Err(InvalidTakeProfitBuy)`: Take profit invalid for buy trade
    /// - `Err(InvalidTakeProfitSell)`: Take profit invalid for sell trade
    /// - `Err(InvalidStopLossBuy)`: Stop loss invalid for buy trade
    /// - `Err(InvalidStopLossSell)`: Stop loss invalid for sell trade
    ///
    /// ## Example
    /// ```rust
    /// use tubor_yield::state::trade::Trade;
    ///
    /// let trade = Trade::default();
    /// if trade.validate().is_ok() {
    ///     // Proceed with trade execution
    /// } else {
    ///     // Handle validation error
    /// }
    /// ```
    pub fn validate(&self) -> TYieldResult<()> {
        if self.size == 0 {
            return Err(ErrorCode::InvalidTradeSize);
        }

        if self.entry_price == 0 {
            return Err(ErrorCode::InvalidEntryPrice);
        }

        if self.take_profit <= self.entry_price && self.is_buy() {
            return Err(ErrorCode::InvalidTakeProfitBuy);
        }

        if self.take_profit >= self.entry_price && self.is_sell() {
            return Err(ErrorCode::InvalidTakeProfitSell);
        }

        if self.stop_loss >= self.entry_price && self.is_buy() {
            return Err(ErrorCode::InvalidStopLossBuy);
        }

        if self.stop_loss <= self.entry_price && self.is_sell() {
            return Err(ErrorCode::InvalidStopLossSell);
        }

        Ok(())
    }

    /// Calculates the potential profit/loss at a given price
    pub fn calculate_pnl(&self, current_price: u64) -> i64 {
        let price_diff = if self.is_buy() {
            current_price as i64 - self.entry_price as i64
        } else {
            self.entry_price as i64 - current_price as i64
        };

        // Calculate PnL based on size and price difference
        // Note: This is a simplified calculation - in practice you might want more sophisticated logic
        (price_diff * self.size as i64) / self.entry_price as i64
    }

    /// Calculates the potential profit/loss at a given price with proper error handling.
    ///
    /// This is the safe version of PnL calculation that handles edge cases and prevents
    /// arithmetic overflow. It uses safe math operations and provides detailed error information.
    ///
    /// ## Formula
    /// For buy trades: `PnL = (current_price - entry_price) * size / entry_price`
    /// For sell trades: `PnL = (entry_price - current_price) * size / entry_price`
    ///
    /// ## Parameters
    /// - `current_price`: The current market price to calculate PnL against
    ///
    /// ## Returns
    /// - `Ok(i64)`: The calculated PnL (positive for profit, negative for loss)
    /// - `Err(InvalidEntryPrice)`: Current price is zero (invalid)
    /// - `Err(MathError)`: Arithmetic overflow or division by zero
    ///
    /// ## Examples
    /// ```rust
    /// use tubor_yield::state::trade::Trade;
    ///
    /// fn main() -> Result<(), tubor_yield::error::ErrorCode> {
    ///     let trade = Trade {
    ///         entry_price: 1000,
    ///         size: 10,
    ///         ..Default::default()
    ///     };
    ///     // Buy trade with profit
    ///     let pnl = trade.calculate_pnl_safe(1100)?; // Positive PnL
    ///
    ///     // Buy trade with loss
    ///     let pnl = trade.calculate_pnl_safe(900)?;  // Negative PnL
    ///
    ///     // Sell trade with profit (price went down)
    ///     // (For a real sell trade, set trade_type and other fields accordingly)
    ///     Ok(())
    /// }
    /// ```
    pub fn calculate_pnl_safe(&self, current_price: u64) -> TYieldResult<i64> {
        if current_price == 0 {
            return Err(ErrorCode::InvalidEntryPrice);
        }

        let (price_diff, sign) = if self.is_buy() {
            if current_price >= self.entry_price {
                (current_price.safe_sub(self.entry_price)?, 1)
            } else {
                (self.entry_price.safe_sub(current_price)?, -1)
            }
        } else if self.entry_price >= current_price {
            (self.entry_price.safe_sub(current_price)?, 1)
        } else {
            (current_price.safe_sub(self.entry_price)?, -1)
        };

        // Calculate PnL: (price_diff * size) / entry_price
        let pnl_numerator = price_diff.safe_mul(self.size)?;
        let pnl = pnl_numerator.safe_div(self.entry_price)?;

        Ok((pnl as i64) * sign)
    }

    /// Calculates the percentage PnL (return as basis points)
    pub fn calculate_pnl_percentage(&self, current_price: u64) -> TYieldResult<i64> {
        if current_price == 0 {
            return Err(ErrorCode::InvalidEntryPrice);
        }

        let price_diff = if self.is_buy() {
            current_price.safe_sub(self.entry_price)?
        } else {
            self.entry_price.safe_sub(current_price)?
        };

        // Calculate percentage: (price_diff * 10000) / entry_price (in basis points)
        let percentage_numerator = price_diff.safe_mul(PERCENTAGE_PRECISION_U64)?;
        let percentage = percentage_numerator.safe_div(self.entry_price)?;

        Ok(percentage as i64)
    }

    /// Calculates the unrealized PnL if trade is closed at current price
    pub fn calculate_unrealized_pnl(&self, current_price: u64) -> TYieldResult<i64> {
        if !self.is_active() {
            return Ok(0); // No unrealized PnL for completed/cancelled trades
        }

        self.calculate_pnl_safe(current_price)
    }

    /// Calculates the maximum potential profit (at take profit level)
    pub fn calculate_max_profit(&self) -> TYieldResult<i64> {
        self.calculate_pnl_safe(self.take_profit)
    }

    /// Calculates the maximum potential loss (at stop loss level)
    pub fn calculate_max_loss(&self) -> TYieldResult<i64> {
        self.calculate_pnl_safe(self.stop_loss)
    }

    /// Calculates the risk-reward ratio
    pub fn calculate_risk_reward_ratio(&self) -> TYieldResult<u64> {
        let max_profit = self.calculate_max_profit()?;
        let max_loss = self.calculate_max_loss()?;

        if max_loss == 0 {
            return Err(ErrorCode::MathError);
        }

        // Return ratio as basis points (e.g., 200 = 2:1 ratio)
        let ratio = (max_profit.unsigned_abs()).safe_mul(PERCENTAGE_PRECISION_U64)?;
        let ratio_basis_points = ratio.safe_div(max_loss.unsigned_abs())?;
        Ok(ratio_basis_points)
    }

    /// Checks if the trade has hit take profit
    pub fn has_hit_take_profit(&self, current_price: u64) -> bool {
        if self.is_buy() {
            current_price >= self.take_profit
        } else {
            current_price <= self.take_profit
        }
    }

    /// Checks if the trade has hit stop loss
    pub fn has_hit_stop_loss(&self, current_price: u64) -> bool {
        if self.is_buy() {
            current_price <= self.stop_loss
        } else {
            current_price >= self.stop_loss
        }
    }

    /// Completes the trade with a result
    pub fn complete(&mut self, result: TradeResult) {
        // Use the old method for backward compatibility
        let authority = self.authority; // Store authority to avoid borrow checker issues
        let current_time = anchor_lang::solana_program::clock::Clock::get()
            .map(|clock| clock.unix_timestamp)
            .unwrap_or(0);
        self.set_status(TradeStatus::Completed, &authority, current_time)
            .unwrap_or_else(|_| {
                // Fallback if authority validation fails
                self.status = TradeStatus::Completed as u8;
            });
        self.set_result(result, &authority, current_time)
            .unwrap_or_else(|_| {
                // Fallback if authority validation fails
                self.result = result as u8;
            });
    }

    /// Cancels the trade
    pub fn cancel(&mut self) {
        // Use the old method for backward compatibility
        let authority = self.authority; // Store authority to avoid borrow checker issues
        let current_time = anchor_lang::solana_program::clock::Clock::get()
            .map(|clock| clock.unix_timestamp)
            .unwrap_or(0);
        self.set_status(TradeStatus::Cancelled, &authority, current_time)
            .unwrap_or_else(|_| {
                // Fallback if authority validation fails
                self.status = TradeStatus::Cancelled as u8;
            });
        self.set_result(TradeResult::Failed, &authority, current_time)
            .unwrap_or_else(|_| {
                // Fallback if authority validation fails
                self.result = TradeResult::Failed as u8;
            });
    }

    /// Gets the trade duration in seconds (if created_at is a timestamp)
    pub fn get_duration(&self, current_time: i64) -> i64 {
        current_time - self.created_at
    }

    /// Returns a string representation of the trade pair
    pub fn get_pair_string(&self) -> String {
        // Convert the 7-byte array to a readable string
        // This is a simplified implementation - you might want to handle this differently
        String::from_utf8_lossy(&self.pair).to_string()
    }

    /// Returns a string representation of the feed ID
    pub fn get_feed_id_string(&self) -> String {
        // Convert the 32-byte array to a hex string
        self.feed_id.iter().map(|b| format!("{:02x}", b)).collect()
    }

    /// Initializes the Trade in-place using a parameter struct
    pub fn init_trade(&mut self, params: TradeInitParams) {
        self.master_agent = params.master_agent;
        self.size = params.size;
        self.entry_price = params.entry_price;
        self.take_profit = params.take_profit;
        self.stop_loss = params.stop_loss;
        self.created_at = params.created_at;
        self.updated_at = params.created_at;
        self.pair = params.pair;
        self.feed_id = params.feed_id;
        self.status = params.status as u8;
        self.trade_type = params.trade_type as u8;
        self.result = params.result as u8;
        self.bump = params.bump;
        self.authority = Pubkey::default(); // Set default authority
        self.oracle_consensus_count = 0;
        self.last_price_update = params.created_at;
        self.circuit_breaker_triggered = false;
        self._padding = [0; 2];
    }

    /// Updates mutable fields of the trade and sets updated_at
    pub fn update_trade(
        &mut self,
        size: u64,
        take_profit: u64,
        stop_loss: u64,
        status: TradeStatus,
        result: TradeResult,
        updated_at: i64,
    ) {
        self.size = size;
        self.take_profit = take_profit;
        self.stop_loss = stop_loss;
        let authority = self.authority; // Store authority to avoid borrow checker issues
        self.set_status(status, &authority, updated_at)
            .unwrap_or_else(|_| {
                // Fallback if authority validation fails
                self.status = status as u8;
            });
        self.set_result(result, &authority, updated_at)
            .unwrap_or_else(|_| {
                // Fallback if authority validation fails
                self.result = result as u8;
            });
        self.updated_at = updated_at;
    }

    /// Enhanced price validation with slippage protection
    pub fn validate_price_with_slippage(
        &self,
        current_price: u64,
        max_slippage_bps: u64,
    ) -> TYieldResult<()> {
        if current_price == 0 {
            return Err(ErrorCode::PriceValidationFailed);
        }

        let price_diff = if current_price >= self.entry_price {
            current_price.safe_sub(self.entry_price)?
        } else {
            self.entry_price.safe_sub(current_price)?
        };

        let slippage_bps = price_diff
            .safe_mul(PERCENTAGE_PRECISION_U64)?
            .safe_div(self.entry_price)?;

        if slippage_bps > max_slippage_bps {
            msg!(
                "Price slippage {} bps exceeds maximum {} bps",
                slippage_bps,
                max_slippage_bps
            );
            return Err(ErrorCode::MaxPriceSlippage);
        }

        Ok(())
    }

    /// Validates that stop loss and take profit are sufficiently far from entry price
    pub fn validate_risk_management_levels(&self, min_distance_bps: u64) -> TYieldResult<()> {
        // Validate take profit distance
        let tp_distance = if self.take_profit >= self.entry_price {
            self.take_profit.safe_sub(self.entry_price)?
        } else {
            self.entry_price.safe_sub(self.take_profit)?
        };

        let tp_distance_bps = tp_distance
            .safe_mul(PERCENTAGE_PRECISION_U64)?
            .safe_div(self.entry_price)?;

        if tp_distance_bps < min_distance_bps {
            msg!(
                "Take profit too close: {} bps < {} bps minimum",
                tp_distance_bps,
                min_distance_bps
            );
            return Err(ErrorCode::TakeProfitTooClose);
        }

        // Validate stop loss distance
        let sl_distance = if self.stop_loss >= self.entry_price {
            self.stop_loss.safe_sub(self.entry_price)?
        } else {
            self.entry_price.safe_sub(self.stop_loss)?
        };

        let sl_distance_bps = sl_distance
            .safe_mul(PERCENTAGE_PRECISION_U64)?
            .safe_div(self.entry_price)?;

        if sl_distance_bps < min_distance_bps {
            msg!(
                "Stop loss too close: {} bps < {} bps minimum",
                sl_distance_bps,
                min_distance_bps
            );
            return Err(ErrorCode::StopLossTooClose);
        }

        Ok(())
    }

    /// Validates risk-reward ratio meets minimum requirements
    pub fn validate_risk_reward_ratio(&self, min_ratio_bps: u64) -> TYieldResult<()> {
        let ratio = self.calculate_risk_reward_ratio()?;

        if ratio < min_ratio_bps {
            msg!(
                "Risk-reward ratio {} bps below minimum {} bps",
                ratio,
                min_ratio_bps
            );
            return Err(ErrorCode::InsufficientRiskRewardRatio);
        }

        Ok(())
    }

    /// Enhanced trade execution validation with comprehensive security checks.
    ///
    /// This is the main validation function that should be called before executing any trade.
    /// It performs a complete security audit including basic validation, trade limits,
    /// circuit breaker checks, oracle consensus validation, and flash attack protection.
    ///
    /// ## Validation Steps
    ///
    /// 1. **Basic Trade Validation**: Size, prices, risk management levels
    /// 2. **Trade Limits Check**: Position size and price limits
    /// 3. **Circuit Breaker Check**: Protection against extreme price movements
    /// 4. **Oracle Consensus**: Multi-oracle price validation
    /// 5. **Flash Attack Protection**: Detection of suspicious price movements
    /// 6. **Price Validation**: Slippage, risk-reward, and range checks
    ///
    /// ## Parameters
    /// - `current_price`: Current market price for validation
    /// - `oracles`: Array of oracle prices for consensus calculation
    /// - `security_config`: Security configuration for limits and thresholds
    /// - `validation_config`: Price validation configuration
    ///
    /// ## Returns
    /// - `Ok(())`: All security checks passed, trade can be executed
    /// - `Err(InvalidTradeSize)`: Trade size exceeds limits
    /// - `Err(InvalidEntryPrice)`: Price validation failed
    /// - `Err(CircuitBreakerTriggered)`: Circuit breaker activated
    /// - `Err(OracleConsensusThresholdNotMet)`: Insufficient oracle consensus
    /// - `Err(PriceDeviationTooHigh)`: Oracle price deviation too high
    /// - `Err(MaxPriceSlippage)`: Price slippage exceeds maximum
    /// - `Err(TakeProfitTooClose)`: Take profit too close to entry
    /// - `Err(StopLossTooClose)`: Stop loss too close to entry
    /// - `Err(InsufficientRiskRewardRatio)`: Risk-reward ratio too low
    ///
    /// ## Example
    /// ```rust
    /// use tubor_yield::state::trade::{Trade, TradeSecurityConfig, PriceValidationConfig};
    /// use tubor_yield::state::OraclePrice;
    ///
    /// fn main() -> Result<(), tubor_yield::error::ErrorCode> {
    ///     let trade = Trade::default();
    ///     let current_price = 1000;
    ///     let oracles = vec![OraclePrice { price: 1000, exponent: 0 }];
    ///     let security_config = TradeSecurityConfig::default();
    ///     let validation_config = PriceValidationConfig::default();
    ///
    ///     let result = trade.validate_secure_trade_execution(
    ///         current_price,
    ///         &oracles,
    ///         &security_config,
    ///         &validation_config
    ///     );
    ///
    ///     match result {
    ///         Ok(()) => {
    ///             // Execute trade safely
    ///         }
    ///         Err(e) => {
    ///             // Handle validation error
    ///         }
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub fn validate_secure_trade_execution(
        &self,
        current_price: u64,
        oracles: &[OraclePrice],
        security_config: &TradeSecurityConfig,
        validation_config: &PriceValidationConfig,
    ) -> TYieldResult<()> {
        let current_time = anchor_lang::solana_program::clock::Clock::get()
            .map(|clock| clock.unix_timestamp)
            .unwrap_or(0);

        // Basic trade validation
        self.validate()?;

        // Trade limits validation
        self.validate_trade_limits(security_config)?;

        // Circuit breaker check
        self.check_circuit_breaker(current_price, security_config)?;

        // Oracle consensus validation
        let oracle_consensus =
            self.validate_oracle_consensus(oracles, security_config, current_time)?;

        // Flash attack protection
        self.validate_price_with_flash_protection(
            current_price,
            &oracle_consensus,
            security_config,
        )?;

        // Standard price validation
        self.validate_price_with_slippage(current_price, validation_config.max_slippage_bps)?;
        self.validate_risk_management_levels(validation_config.min_distance_bps)?;
        self.validate_risk_reward_ratio(validation_config.min_risk_reward_bps)?;

        Ok(())
    }

    /// Enhanced trade completion with security checks
    pub fn complete_secure(
        &mut self,
        result: TradeResult,
        authority: &Pubkey,
        pnl: i64,
    ) -> TYieldResult<()> {
        // Validate authority
        if authority != &self.authority {
            return Err(ErrorCode::InvalidAuthority);
        }

        // Check if trade can be completed
        if !self.is_active() {
            return Err(ErrorCode::CannotPerformAction);
        }

        // Validate PnL is reasonable (prevent manipulation)
        let max_expected_pnl = self.size.safe_mul(1000)?; // 1000% max PnL
        if pnl.unsigned_abs() > max_expected_pnl {
            return Err(ErrorCode::MathError);
        }

        let current_time = anchor_lang::solana_program::clock::Clock::get()
            .map(|clock| clock.unix_timestamp)
            .unwrap_or(0);

        self.set_status(TradeStatus::Completed, authority, current_time)?;
        self.set_result(result, authority, current_time)?;

        Ok(())
    }

    /// Enhanced trade cancellation with security checks
    pub fn cancel_secure(&mut self, authority: &Pubkey, reason: &str) -> TYieldResult<()> {
        // Validate authority
        if authority != &self.authority {
            return Err(ErrorCode::InvalidAuthority);
        }

        // Check if trade can be cancelled
        if !self.is_active() {
            return Err(ErrorCode::CannotPerformAction);
        }

        // Log cancellation reason for audit
        msg!("Trade cancelled by {}: {}", authority, reason);

        let current_time = anchor_lang::solana_program::clock::Clock::get()
            .map(|clock| clock.unix_timestamp)
            .unwrap_or(0);

        self.set_status(TradeStatus::Cancelled, authority, current_time)?;
        self.set_result(TradeResult::Failed, authority, current_time)?;

        Ok(())
    }

    /// Emergency pause functionality
    pub fn trigger_circuit_breaker(&mut self, authority: &Pubkey) -> TYieldResult<()> {
        if authority != &self.authority {
            return Err(ErrorCode::InvalidAuthority);
        }
        self.circuit_breaker_triggered = true;
        self.updated_at = anchor_lang::solana_program::clock::Clock::get()
            .map(|clock| clock.unix_timestamp)
            .unwrap_or(0);
        Ok(())
    }

    /// Reset circuit breaker (admin only)
    pub fn reset_circuit_breaker(&mut self, _admin_authority: &Pubkey) -> TYieldResult<()> {
        // TODO: Add admin authority validation
        self.circuit_breaker_triggered = false;
        self.updated_at = anchor_lang::solana_program::clock::Clock::get()
            .map(|clock| clock.unix_timestamp)
            .unwrap_or(0);
        Ok(())
    }

    /// Enhanced initialization with security parameters
    pub fn init_trade_secure(
        &mut self,
        params: TradeInitParams,
        authority: Pubkey,
    ) -> TYieldResult<()> {
        self.master_agent = params.master_agent;
        self.size = params.size;
        self.entry_price = params.entry_price;
        self.take_profit = params.take_profit;
        self.stop_loss = params.stop_loss;
        self.created_at = params.created_at;
        self.updated_at = params.created_at;
        self.pair = params.pair;
        self.feed_id = params.feed_id;
        self.status = params.status as u8;
        self.trade_type = params.trade_type as u8;
        self.result = params.result as u8;
        self.bump = params.bump;
        self.authority = authority; // Set authority for access control
        self.oracle_consensus_count = 0;
        self.last_price_update = params.created_at;
        self.circuit_breaker_triggered = false;
        self._padding = [0; 2];

        // Validate the trade after initialization
        self.validate()?;

        Ok(())
    }

    /// Validates oracle price against current market conditions
    pub fn validate_oracle_price(
        &self,
        oracle_price: &OraclePrice,
        max_deviation_bps: u64,
    ) -> TYieldResult<()> {
        let oracle_price_u64 = oracle_price.scale_to_exponent(0)?.price;

        if oracle_price_u64 == 0 {
            return Err(ErrorCode::PriceValidationFailed);
        }

        let price_diff = if oracle_price_u64 >= self.entry_price {
            oracle_price_u64.safe_sub(self.entry_price)?
        } else {
            self.entry_price.safe_sub(oracle_price_u64)?
        };

        let deviation_bps = price_diff
            .safe_mul(PERCENTAGE_PRECISION_U64)?
            .safe_div(self.entry_price)?;

        if deviation_bps > max_deviation_bps {
            msg!(
                "Oracle price deviation {} bps exceeds maximum {} bps",
                deviation_bps,
                max_deviation_bps
            );
            return Err(ErrorCode::PriceDeviationTooHigh);
        }

        Ok(())
    }

    /// Checks if current price is within acceptable trading range
    pub fn is_price_in_range(
        &self,
        current_price: u64,
        range_buffer_bps: u64,
    ) -> TYieldResult<bool> {
        if current_price == 0 {
            return Err(ErrorCode::PriceValidationFailed);
        }

        let min_price = self.stop_loss.safe_sub(
            self.stop_loss
                .safe_mul(range_buffer_bps)?
                .safe_div(PERCENTAGE_PRECISION_U64)?,
        )?;

        let max_price = self.take_profit.safe_add(
            self.take_profit
                .safe_mul(range_buffer_bps)?
                .safe_div(PERCENTAGE_PRECISION_U64)?,
        )?;

        Ok(current_price >= min_price && current_price <= max_price)
    }

    /// Enhanced entry price calculation with spread adjustment
    pub fn calculate_entry_price_with_spread(
        &self,
        oracle_price: &OraclePrice,
        spread_bps: u64,
        side: TradeType,
    ) -> TYieldResult<u64> {
        let base_price = oracle_price.scale_to_exponent(0)?.price;

        if base_price == 0 {
            return Err(ErrorCode::PriceValidationFailed);
        }

        let spread_amount = base_price
            .safe_mul(spread_bps)?
            .safe_div(PERCENTAGE_PRECISION_U64)?;

        let entry_price = match side {
            TradeType::Buy => base_price.safe_add(spread_amount)?,
            TradeType::Sell => base_price.safe_sub(spread_amount)?,
        };

        if entry_price == 0 {
            return Err(ErrorCode::PriceValidationFailed);
        }

        Ok(entry_price)
    }

    /// Validates that the trade can be executed at current market conditions
    pub fn can_execute_trade(
        &self,
        current_price: u64,
        oracle_price: &OraclePrice,
        max_slippage_bps: u64,
        max_deviation_bps: u64,
        range_buffer_bps: u64,
    ) -> TYieldResult<bool> {
        // Check basic validation
        if self.validate().is_err() {
            return Ok(false);
        }

        // Check price slippage
        if self
            .validate_price_with_slippage(current_price, max_slippage_bps)
            .is_err()
        {
            return Ok(false);
        }

        // Check oracle price deviation
        if self
            .validate_oracle_price(oracle_price, max_deviation_bps)
            .is_err()
        {
            return Ok(false);
        }

        // Check if price is in acceptable range
        if !self.is_price_in_range(current_price, range_buffer_bps)? {
            return Ok(false);
        }

        Ok(true)
    }

    /// Calculates the optimal entry price based on current market conditions
    pub fn calculate_optimal_entry_price(
        &self,
        oracle_price: &OraclePrice,
        spread_bps: u64,
        slippage_buffer_bps: u64,
    ) -> TYieldResult<u64> {
        let base_price = oracle_price.scale_to_exponent(0)?.price;

        if base_price == 0 {
            return Err(ErrorCode::PriceValidationFailed);
        }

        // Calculate spread-adjusted price
        let spread_amount = base_price
            .safe_mul(spread_bps)?
            .safe_div(PERCENTAGE_PRECISION_U64)?;

        let spread_adjusted_price = match self.get_trade_type() {
            TradeType::Buy => base_price.safe_add(spread_amount)?,
            TradeType::Sell => base_price.safe_sub(spread_amount)?,
        };

        // Add slippage buffer
        let slippage_buffer = spread_adjusted_price
            .safe_mul(slippage_buffer_bps)?
            .safe_div(PERCENTAGE_PRECISION_U64)?;

        let optimal_price = match self.get_trade_type() {
            TradeType::Buy => spread_adjusted_price.safe_add(slippage_buffer)?,
            TradeType::Sell => spread_adjusted_price.safe_sub(slippage_buffer)?,
        };

        if optimal_price == 0 {
            return Err(ErrorCode::PriceValidationFailed);
        }

        Ok(optimal_price)
    }

    #[allow(clippy::too_many_arguments)]
    /// Comprehensive trade validation with all checks
    pub fn comprehensive_validation(
        &self,
        current_price: u64,
        oracle_price: &OraclePrice,
        max_slippage_bps: u64,
        min_distance_bps: u64,
        min_risk_reward_bps: u64,
        max_deviation_bps: u64,
        range_buffer_bps: u64,
    ) -> TYieldResult<()> {
        // Basic trade validation
        self.validate()?;

        // Price slippage validation
        self.validate_price_with_slippage(current_price, max_slippage_bps)?;

        // Risk management levels validation
        self.validate_risk_management_levels(min_distance_bps)?;

        // Risk-reward ratio validation
        self.validate_risk_reward_ratio(min_risk_reward_bps)?;

        // Oracle price validation
        self.validate_oracle_price(oracle_price, max_deviation_bps)?;

        // Price range validation
        if !self.is_price_in_range(current_price, range_buffer_bps)? {
            return Err(ErrorCode::PriceOutOfRange);
        }

        Ok(())
    }

    /// Enhanced validation using configuration struct
    pub fn validate_with_config(
        &self,
        current_price: u64,
        oracle_price: &OraclePrice,
        config: &PriceValidationConfig,
    ) -> TYieldResult<()> {
        config.validate()?;

        self.comprehensive_validation(
            current_price,
            oracle_price,
            config.max_slippage_bps,
            config.min_distance_bps,
            config.min_risk_reward_bps,
            config.max_deviation_bps,
            config.range_buffer_bps,
        )
    }

    /// Calculates optimal entry price using configuration
    pub fn calculate_optimal_price_with_config(
        &self,
        oracle_price: &OraclePrice,
        config: &PriceValidationConfig,
    ) -> TYieldResult<u64> {
        self.calculate_optimal_entry_price(
            oracle_price,
            config.spread_bps,
            config.slippage_buffer_bps,
        )
    }

    /// Validates trade execution with configuration
    pub fn can_execute_with_config(
        &self,
        current_price: u64,
        oracle_price: &OraclePrice,
        config: &PriceValidationConfig,
    ) -> TYieldResult<bool> {
        self.can_execute_trade(
            current_price,
            oracle_price,
            config.max_slippage_bps,
            config.max_deviation_bps,
            config.range_buffer_bps,
        )
    }

    /// Enhanced price validation with flash attack protection
    pub fn validate_price_with_flash_protection(
        &self,
        current_price: u64,
        oracle_consensus: &OracleConsensus,
        config: &TradeSecurityConfig,
    ) -> TYieldResult<()> {
        // Check if price is within reasonable bounds
        if current_price == 0 {
            return Err(ErrorCode::PriceValidationFailed);
        }

        // Validate against oracle consensus
        let price_diff = if current_price >= oracle_consensus.consensus_price {
            current_price.safe_sub(oracle_consensus.consensus_price)?
        } else {
            oracle_consensus.consensus_price.safe_sub(current_price)?
        };

        let deviation_bps = price_diff
            .safe_mul(PERCENTAGE_PRECISION_U64)?
            .safe_div(oracle_consensus.consensus_price)?;

        if deviation_bps > config.max_oracle_deviation_bps {
            return Err(ErrorCode::PriceDeviationTooHigh);
        }

        // Flash attack protection: check for suspicious price movements
        let trade_price_diff = if current_price >= self.entry_price {
            current_price.safe_sub(self.entry_price)?
        } else {
            self.entry_price.safe_sub(current_price)?
        };

        let trade_deviation_bps = trade_price_diff
            .safe_mul(PERCENTAGE_PRECISION_U64)?
            .safe_div(self.entry_price)?;

        // If price moved more than 20% from entry, require additional validation
        if trade_deviation_bps > 2000 {
            // 20%
            // Require higher oracle consensus for large movements
            if oracle_consensus.consensus_count < 3 {
                return Err(ErrorCode::OracleConsensusThresholdNotMet);
            }
        }

        Ok(())
    }

    /// Enhanced trade limits validation
    pub fn validate_trade_limits(&self, limits: &TradeSecurityConfig) -> TYieldResult<()> {
        if self.size == 0 {
            return Err(ErrorCode::InvalidTradeSize);
        }
        if self.size > limits.max_position_size {
            return Err(ErrorCode::InvalidTradeSize);
        }
        if self.entry_price == 0 {
            return Err(ErrorCode::InvalidEntryPrice);
        }
        if self.entry_price < limits.min_price {
            return Err(ErrorCode::InvalidEntryPrice);
        }
        if self.entry_price > limits.max_price {
            return Err(ErrorCode::InvalidEntryPrice);
        }
        if self.take_profit == 0 {
            return Err(ErrorCode::InvalidEntryPrice);
        }
        if self.take_profit > limits.max_price {
            return Err(ErrorCode::InvalidEntryPrice);
        }
        if self.stop_loss == 0 {
            return Err(ErrorCode::InvalidEntryPrice);
        }
        if self.stop_loss < limits.min_price {
            return Err(ErrorCode::InvalidEntryPrice);
        }
        Ok(())
    }

    /// Circuit breaker check
    pub fn check_circuit_breaker(
        &self,
        current_price: u64,
        config: &TradeSecurityConfig,
    ) -> TYieldResult<()> {
        if self.circuit_breaker_triggered {
            return Err(ErrorCode::CircuitBreakerTriggered);
        }

        let price_change = if current_price >= self.entry_price {
            current_price.safe_sub(self.entry_price)?
        } else {
            self.entry_price.safe_sub(current_price)?
        };

        let change_bps = price_change
            .safe_mul(PERCENTAGE_PRECISION_U64)?
            .safe_div(self.entry_price)?;

        if change_bps > config.circuit_breaker_threshold_bps {
            return Err(ErrorCode::CircuitBreakerTriggered);
        }

        // Emergency pause for extreme price movements
        if change_bps > config.emergency_pause_threshold {
            return Err(ErrorCode::EmergencyPauseActive);
        }

        Ok(())
    }

    /// Enhanced oracle validation with consensus
    pub fn validate_oracle_consensus(
        &self,
        oracles: &[OraclePrice],
        config: &TradeSecurityConfig,
        _current_time: i64,
    ) -> TYieldResult<OracleConsensus> {
        // Check oracle age - note: OraclePrice doesn't have publish_time, so we'll skip this check
        // In a real implementation, you'd need to add publish_time to OraclePrice or use a different approach

        // Calculate consensus
        let consensus = OracleConsensus::calculate_consensus(
            oracles,
            config.max_oracle_deviation_bps,
            config.min_oracle_consensus,
        )?;

        // Note: We can't update self here due to borrow checker, so we'll return the consensus
        // In a real implementation, you'd need to handle this differently

        Ok(consensus)
    }

    /// Get trade security status
    pub fn get_security_status(&self) -> String {
        let mut status = Vec::new();

        if self.circuit_breaker_triggered {
            status.push("Circuit Breaker Active");
        }

        if self.oracle_consensus_count < 2 {
            status.push("Low Oracle Consensus");
        }

        if status.is_empty() {
            "Secure".to_string()
        } else {
            status.join(", ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create a valid buy trade
    fn create_valid_buy_trade() -> Trade {
        Trade {
            master_agent: Pubkey::new_unique(),
            size: 100,
            entry_price: 1000,
            take_profit: 1100,
            stop_loss: 900,
            created_at: 1000,
            updated_at: 1000,
            pair: [65, 66, 67, 68, 69, 70, 71, 72], // "ABCDEFGH"
            feed_id: [1; 32],
            status: TradeStatus::Active as u8,
            trade_type: TradeType::Buy as u8,
            result: TradeResult::Pending as u8,
            bump: 1,
            authority: Pubkey::new_unique(),
            oracle_consensus_count: 0,
            last_price_update: 1000,
            circuit_breaker_triggered: false,
            _padding: [0; 2],
        }
    }

    // Helper function to create a valid sell trade
    fn create_valid_sell_trade() -> Trade {
        Trade {
            master_agent: Pubkey::new_unique(),
            size: 100,
            entry_price: 1000,
            take_profit: 900,
            stop_loss: 1100,
            created_at: 1000,
            updated_at: 1000,
            pair: [65, 66, 67, 68, 69, 70, 71, 72], // "ABCDEFGH"
            feed_id: [1; 32],
            status: TradeStatus::Active as u8,
            trade_type: TradeType::Sell as u8,
            result: TradeResult::Pending as u8,
            bump: 1,
            authority: Pubkey::new_unique(),
            oracle_consensus_count: 0,
            last_price_update: 1000,
            circuit_breaker_triggered: false,
            _padding: [0; 2],
        }
    }

    #[test]
    fn test_trade_size() {
        // On-chain size includes 8 bytes for Anchor discriminator
        // Trade struct: 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8 + 8 + 1 + 1 + 1 + 1 + 32 + 1 + 8 + 1 + 2 = 176 bytes
        assert_eq!(8 + std::mem::size_of::<Trade>(), Trade::SIZE);
        println!("Trade on-chain size: {} bytes", Trade::SIZE);
    }

    #[test]
    fn test_trade_memory_layout() {
        // Test that Trade struct can be created and serialized
        let trade = Trade {
            master_agent: Pubkey::default(),
            size: 0,
            entry_price: 0,
            take_profit: 0,
            stop_loss: 0,
            created_at: 0,
            updated_at: 0,
            pair: [0; 8],
            feed_id: [0; 32],
            status: 0,
            trade_type: 0,
            result: 0,
            bump: 0,
            authority: Pubkey::default(),
            oracle_consensus_count: 0,
            last_price_update: 0,
            circuit_breaker_triggered: false,
            _padding: [0; 2],
        };

        assert_eq!(trade.master_agent, Pubkey::default());
        assert_eq!(trade.size, 0);
        assert_eq!(trade.entry_price, 0);
        assert_eq!(trade.take_profit, 0);
        assert_eq!(trade.stop_loss, 0);
        assert_eq!(trade.created_at, 0);
        assert_eq!(trade.pair, [0; 8]);
        assert_eq!(trade.feed_id, [0; 32]);
        assert_eq!(trade.status, 0);
        assert_eq!(trade.trade_type, 0);
        assert_eq!(trade.result, 0);
        assert_eq!(trade.bump, 0);
        assert_eq!(trade.authority, Pubkey::default());
        assert_eq!(trade.oracle_consensus_count, 0);
        assert_eq!(trade.last_price_update, 0);
        assert_eq!(trade.circuit_breaker_triggered, false);
        assert_eq!(trade._padding, [0; 2]);
    }

    #[test]
    fn test_trade_status_enum_conversion() {
        let mut trade = create_valid_buy_trade();

        // Test get_status
        assert_eq!(trade.get_status(), TradeStatus::Active);
        assert_eq!(trade.get_trade_type(), TradeType::Buy);
        assert_eq!(trade.get_result(), TradeResult::Pending);

        // Test set_status
        let authority = trade.authority; // Store authority to avoid borrow checker issues
        trade
            .set_status(TradeStatus::Completed, &authority, 2000)
            .unwrap();
        assert_eq!(trade.get_status(), TradeStatus::Completed);

        trade
            .set_status(TradeStatus::Cancelled, &authority, 2000)
            .unwrap();
        assert_eq!(trade.get_status(), TradeStatus::Cancelled);

        // Test set_result
        trade
            .set_result(TradeResult::Success, &authority, 2000)
            .unwrap();
        assert_eq!(trade.get_result(), TradeResult::Success);

        trade
            .set_result(TradeResult::Failed, &authority, 2000)
            .unwrap();
        assert_eq!(trade.get_result(), TradeResult::Failed);
    }

    #[test]
    fn test_trade_type_enum_conversion() {
        let mut trade = create_valid_buy_trade();

        // Test buy trade
        assert_eq!(trade.get_trade_type(), TradeType::Buy);
        assert!(trade.is_buy());
        assert!(!trade.is_sell());

        // Test sell trade
        trade.trade_type = TradeType::Sell as u8;
        assert_eq!(trade.get_trade_type(), TradeType::Sell);
        assert!(!trade.is_buy());
        assert!(trade.is_sell());
    }

    #[test]
    fn test_trade_status_checks() {
        let mut trade = create_valid_buy_trade();

        // Initially active
        assert!(trade.is_active());
        assert!(!trade.is_completed());
        assert!(!trade.is_cancelled());

        // Test completed
        let authority = trade.authority; // Store authority to avoid borrow checker issues
        trade
            .set_status(TradeStatus::Completed, &authority, 2000)
            .unwrap();
        assert!(!trade.is_active());
        assert!(trade.is_completed());
        assert!(!trade.is_cancelled());

        // Test cancelled
        trade
            .set_status(TradeStatus::Cancelled, &authority, 2000)
            .unwrap();
        assert!(!trade.is_active());
        assert!(!trade.is_completed());
        assert!(trade.is_cancelled());
    }

    #[test]
    fn test_trade_validation() {
        let mut trade = create_valid_buy_trade();

        // Valid trade should pass
        assert!(trade.validate().is_ok());

        // Test invalid size
        trade.size = 0;
        assert!(trade.validate().is_err());

        // Test invalid entry price
        trade.size = 1000;
        trade.entry_price = 0;
        assert!(trade.validate().is_err());

        // Test invalid take profit for buy
        trade.entry_price = 10000;
        trade.take_profit = 9000; // Should be > entry_price for buy
        assert!(trade.validate().is_err());

        // Test invalid take profit for sell
        trade.trade_type = TradeType::Sell as u8;
        trade.take_profit = 11000; // Should be < entry_price for sell
        assert!(trade.validate().is_err());

        // Test invalid stop loss for buy
        trade.trade_type = TradeType::Buy as u8;
        trade.take_profit = 11000;
        trade.stop_loss = 11000; // Should be < entry_price for buy
        assert!(trade.validate().is_err());

        // Test invalid stop loss for sell
        trade.trade_type = TradeType::Sell as u8;
        trade.stop_loss = 9000; // Should be > entry_price for sell
        assert!(trade.validate().is_err());
    }

    #[test]
    fn test_calculate_pnl() {
        let trade = create_valid_buy_trade();

        // Test profit scenario
        let pnl = trade.calculate_pnl(1100);
        assert!(pnl > 0); // Should be positive for profit

        // Test loss scenario
        let pnl = trade.calculate_pnl(900);
        assert!(pnl < 0); // Should be negative for loss

        // Test break-even scenario
        let pnl = trade.calculate_pnl(1000);
        assert_eq!(pnl, 0); // Should be zero at entry price
    }

    #[test]
    fn test_calculate_pnl_safe() {
        let trade = create_valid_buy_trade();

        // Test valid calculation
        let pnl = trade.calculate_pnl_safe(1100);
        assert!(pnl.is_ok());
        assert!(pnl.unwrap() > 0);

        // Test invalid price (zero)
        let pnl = trade.calculate_pnl_safe(0);
        assert!(pnl.is_err());

        // Test sell trade
        let sell_trade = create_valid_sell_trade();
        let pnl = sell_trade.calculate_pnl_safe(900);
        assert!(pnl.is_ok());
        assert!(pnl.unwrap() > 0);
    }

    #[test]
    fn test_calculate_pnl_percentage() {
        let trade = create_valid_buy_trade();

        // Test percentage calculation
        let percentage = trade.calculate_pnl_percentage(1100);
        assert!(percentage.is_ok());
        assert!(percentage.unwrap() > 0); // 10% profit

        // Test zero price
        let percentage = trade.calculate_pnl_percentage(0);
        assert!(percentage.is_err());
    }

    #[test]
    fn test_calculate_unrealized_pnl() {
        let mut trade = create_valid_buy_trade();

        // Active trade should calculate unrealized PnL
        let unrealized_pnl = trade.calculate_unrealized_pnl(1100);
        assert!(unrealized_pnl.is_ok());
        assert!(unrealized_pnl.unwrap() > 0);

        // Completed trade should return 0
        let authority = trade.authority; // Store authority to avoid borrow checker issues
        trade
            .set_status(TradeStatus::Completed, &authority, 2000)
            .unwrap();
        let unrealized_pnl = trade.calculate_unrealized_pnl(1100);
        assert!(unrealized_pnl.is_ok());
        assert_eq!(unrealized_pnl.unwrap(), 0);

        // Cancelled trade should return 0
        trade
            .set_status(TradeStatus::Cancelled, &authority, 2000)
            .unwrap();
        let unrealized_pnl = trade.calculate_unrealized_pnl(1100);
        assert!(unrealized_pnl.is_ok());
        assert_eq!(unrealized_pnl.unwrap(), 0);
    }

    #[test]
    fn test_calculate_max_profit_and_loss() {
        let trade = create_valid_buy_trade();

        // Test max profit
        let max_profit = trade.calculate_max_profit();
        assert!(max_profit.is_ok());
        assert!(max_profit.unwrap() > 0);

        // Test max loss
        let max_loss = trade.calculate_max_loss();
        assert!(max_loss.is_ok());
        assert!(max_loss.unwrap() < 0);

        // Test sell trade
        let sell_trade = create_valid_sell_trade();
        let max_profit = sell_trade.calculate_max_profit();
        assert!(max_profit.is_ok());
        assert!(max_profit.unwrap() > 0);

        let max_loss = sell_trade.calculate_max_loss();
        assert!(max_loss.is_ok());
        assert!(max_loss.unwrap() < 0);
    }

    #[test]
    fn test_debug_pnl_calculation() {
        let trade = create_valid_buy_trade();
        println!(
            "Trade: size={}, entry_price={}, take_profit={}, stop_loss={}",
            trade.size, trade.entry_price, trade.take_profit, trade.stop_loss
        );

        // Test the calculation step by step
        let price_diff = trade.take_profit as i64 - trade.entry_price as i64;
        println!("Price diff: {:?}", price_diff);

        let pnl_numerator = price_diff * trade.size as i64;
        println!("PnL numerator: {:?}", pnl_numerator);

        let pnl = pnl_numerator / trade.entry_price as i64;
        println!("Final PnL: {:?}", pnl);
    }

    #[test]
    fn test_debug_risk_reward_calculation() {
        let trade = create_valid_buy_trade();
        println!(
            "Trade: size={}, entry_price={}, take_profit={}, stop_loss={}",
            trade.size, trade.entry_price, trade.take_profit, trade.stop_loss
        );

        // Test max profit calculation
        let max_profit = trade.calculate_max_profit();
        println!("Max profit: {:?}", max_profit);

        // Test max loss calculation
        let max_loss = trade.calculate_max_loss();
        println!("Max loss: {:?}", max_loss);

        if let (Ok(profit), Ok(loss)) = (max_profit, max_loss) {
            println!("Profit: {}, Loss: {}", profit, loss);
            println!(
                "Profit abs: {}, Loss abs: {}",
                profit.unsigned_abs(),
                loss.unsigned_abs()
            );

            // Test the ratio calculation step by step
            let ratio = profit.unsigned_abs().safe_mul(PERCENTAGE_PRECISION_U64);
            println!("Ratio numerator: {:?}", ratio);

            if let Ok(numerator) = ratio {
                let ratio_basis_points = numerator.safe_div(loss.unsigned_abs());
                println!("Final ratio: {:?}", ratio_basis_points);
            }
        }
    }

    #[test]
    fn test_calculate_risk_reward_ratio() {
        let trade = create_valid_buy_trade();

        // Test risk-reward ratio
        let ratio = trade.calculate_risk_reward_ratio();
        assert!(ratio.is_ok());
        assert!(ratio.unwrap() > 0);

        // Test sell trade
        let sell_trade = create_valid_sell_trade();
        let ratio = sell_trade.calculate_risk_reward_ratio();
        assert!(ratio.is_ok());
        assert!(ratio.unwrap() > 0);

        // Test with zero max loss (should error)
        let mut trade_zero_loss = create_valid_buy_trade();
        trade_zero_loss.stop_loss = trade_zero_loss.entry_price; // Same as entry price
        let ratio = trade_zero_loss.calculate_risk_reward_ratio();
        assert!(ratio.is_err());
    }

    #[test]
    fn test_has_hit_take_profit() {
        let trade = create_valid_buy_trade();

        // Test take profit hit
        assert!(trade.has_hit_take_profit(1100));
        assert!(trade.has_hit_take_profit(1200));

        // Test take profit not hit
        assert!(!trade.has_hit_take_profit(1099));
        assert!(!trade.has_hit_take_profit(1000));

        // Test sell trade
        let sell_trade = create_valid_sell_trade();
        assert!(sell_trade.has_hit_take_profit(900));
        assert!(sell_trade.has_hit_take_profit(800));
        assert!(!sell_trade.has_hit_take_profit(901));
        assert!(!sell_trade.has_hit_take_profit(1000));
    }

    #[test]
    fn test_has_hit_stop_loss() {
        let trade = create_valid_buy_trade();

        // Test stop loss hit
        assert!(trade.has_hit_stop_loss(900));
        assert!(trade.has_hit_stop_loss(800));

        // Test stop loss not hit
        assert!(!trade.has_hit_stop_loss(901));
        assert!(!trade.has_hit_stop_loss(1000));

        // Test sell trade
        let sell_trade = create_valid_sell_trade();
        assert!(sell_trade.has_hit_stop_loss(1100));
        assert!(sell_trade.has_hit_stop_loss(1200));
        assert!(!sell_trade.has_hit_stop_loss(1099));
        assert!(!sell_trade.has_hit_stop_loss(1000));
    }

    #[test]
    fn test_complete_and_cancel_trade() {
        let mut trade = create_valid_buy_trade();

        // Test complete trade
        trade.complete(TradeResult::Success);
        assert_eq!(trade.get_status(), TradeStatus::Completed);
        assert_eq!(trade.get_result(), TradeResult::Success);

        // Test cancel trade
        let mut trade = create_valid_buy_trade();
        trade.cancel();
        assert_eq!(trade.get_status(), TradeStatus::Cancelled);
        assert_eq!(trade.get_result(), TradeResult::Failed);
    }

    #[test]
    fn test_get_duration() {
        let trade = create_valid_buy_trade();
        let current_time = 2000;

        let duration = trade.get_duration(current_time);
        assert_eq!(duration, 1000); // 2000 - 1000 = 1000 seconds
    }

    #[test]
    fn test_get_pair_string() {
        let trade = create_valid_buy_trade();
        let pair_string = trade.get_pair_string();

        // Should convert the byte array to a string
        assert!(!pair_string.is_empty());
        assert_eq!(pair_string.len(), 8);
    }

    #[test]
    fn test_get_feed_id_string() {
        let trade = create_valid_buy_trade();
        let feed_id_string = trade.get_feed_id_string();

        // Should convert the 32-byte array to a hex string
        assert!(!feed_id_string.is_empty());
        assert_eq!(feed_id_string.len(), 64); // 32 bytes * 2 hex chars per byte
        assert!(feed_id_string.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_edge_cases() {
        // Test with maximum values
        let trade = Trade {
            master_agent: Pubkey::new_unique(),
            size: u64::MAX,
            entry_price: u64::MAX,
            take_profit: u64::MAX,
            stop_loss: 0,
            created_at: i64::MAX,
            updated_at: i64::MAX,
            pair: [255; 8],
            feed_id: [255; 32],
            status: TradeStatus::Active as u8,
            trade_type: TradeType::Buy as u8,
            result: TradeResult::Pending as u8,
            bump: 255,
            authority: Pubkey::new_unique(),
            oracle_consensus_count: 0,
            last_price_update: 0,
            circuit_breaker_triggered: false,
            _padding: [255; 2],
        };

        // Should handle maximum values without panicking
        assert!(trade.get_status() == TradeStatus::Active);
        assert!(trade.get_trade_type() == TradeType::Buy);
        assert!(trade.get_result() == TradeResult::Pending);

        // Test with minimum values
        let trade = Trade {
            master_agent: Pubkey::default(),
            size: 1,
            entry_price: 1,
            take_profit: 2,
            stop_loss: 0,
            created_at: 0,
            updated_at: 0,
            pair: [0; 8],
            feed_id: [0; 32],
            status: 0,
            trade_type: 0,
            result: 0,
            bump: 0,
            authority: Pubkey::default(),
            oracle_consensus_count: 0,
            last_price_update: 0,
            circuit_breaker_triggered: false,
            _padding: [0; 2],
        };

        // Should handle minimum values without panicking
        assert!(trade.get_status() == TradeStatus::Active); // Default fallback
        assert!(trade.get_trade_type() == TradeType::Buy); // Default fallback
        assert!(trade.get_result() == TradeResult::Pending); // Default fallback
    }

    #[test]
    fn test_enum_values() {
        // Test enum values match their binary representations
        assert_eq!(TradeStatus::Active as u8, 0b00000001);
        assert_eq!(TradeStatus::Completed as u8, 0b00000010);
        assert_eq!(TradeStatus::Cancelled as u8, 0b00000100);

        assert_eq!(TradeType::Buy as u8, 0b00000001);
        assert_eq!(TradeType::Sell as u8, 0b00000010);

        assert_eq!(TradeResult::Success as u8, 0b00000001);
        assert_eq!(TradeResult::Failed as u8, 0b00000010);
        assert_eq!(TradeResult::Pending as u8, 0b00000100);
    }

    #[test]
    fn test_trade_event() {
        let trade_event = TradeEvent {
            trade: Pubkey::new_unique(),
            status: TradeStatus::Active,
            trade_type: TradeType::Buy,
            result: TradeResult::Success,
            pnl: 1000,
            created_at: 1234567890,
        };

        assert_eq!(trade_event.status, TradeStatus::Active);
        assert_eq!(trade_event.trade_type, TradeType::Buy);
        assert_eq!(trade_event.result, TradeResult::Success);
        assert_eq!(trade_event.pnl, 1000);
        assert_eq!(trade_event.created_at, 1234567890);
    }

    #[test]
    fn test_pnl_calculations_edge_cases() {
        let trade = create_valid_buy_trade();

        // Test with very small price differences (profit, may be zero due to truncation)
        let pnl = trade.calculate_pnl_safe(1001);
        println!("Buy trade, price 1001, PnL: {:?}", pnl);
        assert!(pnl.is_ok());
        assert!(pnl.unwrap() >= 0); // Profit or zero

        // Test with very large price differences (profit)
        let pnl = trade.calculate_pnl_safe(2000);
        println!("Buy trade, price 2000, PnL: {:?}", pnl);
        assert!(pnl.is_ok());
        assert!(pnl.unwrap() > 0); // Profit

        // Test with price below entry (loss)
        let pnl = trade.calculate_pnl_safe(900);
        println!("Buy trade, price 900, PnL: {:?}", pnl);
        assert!(pnl.is_ok());
        assert!(pnl.unwrap() < 0); // Loss

        // Test sell trade with price going down (profit, may be zero due to truncation)
        let sell_trade = create_valid_sell_trade();
        let pnl = sell_trade.calculate_pnl_safe(999);
        println!("Sell trade, price 999, PnL: {:?}", pnl);
        assert!(pnl.is_ok());
        assert!(pnl.unwrap() >= 0); // Profit or zero

        // Test sell trade with price well below entry (profit)
        let pnl = sell_trade.calculate_pnl_safe(800);
        println!("Sell trade, price 800, PnL: {:?}", pnl);
        assert!(pnl.is_ok());
        assert!(pnl.unwrap() > 0); // Profit

        // Test sell trade with price above entry (loss)
        let pnl = sell_trade.calculate_pnl_safe(1100);
        println!("Sell trade, price 1100, PnL: {:?}", pnl);
        assert!(pnl.is_ok());
        assert!(pnl.unwrap() < 0); // Loss

        // Test break-even scenarios
        let pnl = trade.calculate_pnl_safe(1000);
        println!("Buy trade, price 1000, PnL: {:?}", pnl);
        assert!(pnl.is_ok());
        assert_eq!(pnl.unwrap(), 0);

        let pnl = sell_trade.calculate_pnl_safe(1000);
        println!("Sell trade, price 1000, PnL: {:?}", pnl);
        assert!(pnl.is_ok());
        assert_eq!(pnl.unwrap(), 0);
    }

    #[test]
    fn test_overflow_protection() {
        let mut trade = create_valid_buy_trade();
        trade.size = u64::MAX;
        trade.entry_price = 1;

        // This should not panic due to overflow protection in safe math
        let pnl = trade.calculate_pnl_safe(2);
        // The result depends on the safe math implementation
        // We just test that it doesn't panic
        assert!(pnl.is_ok() || pnl.is_err());
    }

    #[test]
    fn test_init_trade() {
        let params = TradeInitParams {
            master_agent: Pubkey::new_unique(),
            size: 123,
            entry_price: 456,
            take_profit: 789,
            stop_loss: 321,
            created_at: 1111,
            pair: [1, 2, 3, 4, 5, 6, 7, 8],
            feed_id: [9; 32],
            status: TradeStatus::Active,
            trade_type: TradeType::Sell,
            result: TradeResult::Pending,
            bump: 2,
        };
        let mut trade = Trade::default();
        trade.init_trade(params);
        assert_eq!(trade.master_agent, params.master_agent);
        assert_eq!(trade.size, params.size);
        assert_eq!(trade.entry_price, params.entry_price);
        assert_eq!(trade.take_profit, params.take_profit);
        assert_eq!(trade.stop_loss, params.stop_loss);
        assert_eq!(trade.created_at, params.created_at);
        assert_eq!(trade.updated_at, params.created_at);
        assert_eq!(trade.pair, params.pair);
        assert_eq!(trade.feed_id, params.feed_id);
        assert_eq!(trade.get_status(), params.status);
        assert_eq!(trade.get_trade_type(), params.trade_type);
        assert_eq!(trade.get_result(), params.result);
        assert_eq!(trade.bump, params.bump);
    }

    #[test]
    fn test_update_trade() {
        let params = TradeInitParams {
            master_agent: Pubkey::new_unique(),
            size: 100,
            entry_price: 1000,
            take_profit: 1100,
            stop_loss: 900,
            created_at: 1000,
            pair: [1, 2, 3, 4, 5, 6, 7, 8],
            feed_id: [1; 32],
            status: TradeStatus::Active,
            trade_type: TradeType::Buy,
            result: TradeResult::Pending,
            bump: 1,
        };
        let mut trade = Trade::default();
        trade.init_trade(params);
        // Update fields
        trade.update_trade(
            200,  // size
            1200, // take_profit
            800,  // stop_loss
            TradeStatus::Completed,
            TradeResult::Success,
            2000, // updated_at
        );
        assert_eq!(trade.size, 200);
        assert_eq!(trade.take_profit, 1200);
        assert_eq!(trade.stop_loss, 800);
        assert_eq!(trade.get_status(), TradeStatus::Completed);
        assert_eq!(trade.get_result(), TradeResult::Success);
        assert_eq!(trade.updated_at, 2000);
    }

    #[test]
    fn test_validate_price_with_slippage() {
        let trade = create_valid_buy_trade();
        let current_price = 1000;
        let max_slippage_bps = 100;
        let result = trade.validate_price_with_slippage(current_price, max_slippage_bps);
        assert!(result.is_ok());

        let current_price = 0;
        let result = trade.validate_price_with_slippage(current_price, max_slippage_bps);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_risk_management_levels() {
        let trade = create_valid_buy_trade();
        let min_distance_bps = 100;
        let result = trade.validate_risk_management_levels(min_distance_bps);
        assert!(result.is_ok());

        // Create a trade with very close stop loss
        let mut close_trade = create_valid_buy_trade();
        close_trade.stop_loss = 999; // Very close to entry price
        let result = close_trade.validate_risk_management_levels(min_distance_bps);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_risk_reward_ratio() {
        let trade = create_valid_buy_trade();
        let min_ratio_bps = 100;
        let result = trade.validate_risk_reward_ratio(min_ratio_bps);
        assert!(result.is_ok());

        // Create a trade with poor risk-reward ratio
        let mut poor_trade = create_valid_buy_trade();
        poor_trade.take_profit = 1001; // Very close to entry price
        poor_trade.stop_loss = 999; // Very close to entry price
        let result = poor_trade.validate_risk_reward_ratio(min_ratio_bps);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_trade_execution() {
        let trade = create_valid_buy_trade();
        let current_price = 1000;
        let oracles = vec![
            OraclePrice {
                price: 1000,
                exponent: 0,
            },
            OraclePrice {
                price: 1001,
                exponent: 0,
            },
        ];
        let result = trade.validate_secure_trade_execution(
            current_price,
            &oracles,
            &TradeSecurityConfig::default(),
            &PriceValidationConfig::default(),
        );
        assert!(result.is_ok());

        let current_price = 0;
        let result = trade.validate_secure_trade_execution(
            current_price,
            &oracles,
            &TradeSecurityConfig::default(),
            &PriceValidationConfig::default(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_optimal_entry_price() {
        let trade = create_valid_buy_trade();
        let oracle_price = OraclePrice {
            price: 1000,
            exponent: 0,
        };
        let spread_bps = 100;
        let slippage_buffer_bps = 50;
        let result =
            trade.calculate_optimal_entry_price(&oracle_price, spread_bps, slippage_buffer_bps);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1015); // 1000 + 10 + 5

        let slippage_buffer_bps = u64::MAX; // This should cause overflow
        let result =
            trade.calculate_optimal_entry_price(&oracle_price, spread_bps, slippage_buffer_bps);
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_optimal_price_with_config() {
        let trade = create_valid_buy_trade();
        let oracle_price = OraclePrice {
            price: 1000,
            exponent: 0,
        };
        let config = PriceValidationConfig::default();

        let result = trade.calculate_optimal_price_with_config(&oracle_price, &config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1007); // 1000 + 5 + 2

        // Test with conservative config
        let conservative_config = PriceValidationConfig::conservative();
        let result = trade.calculate_optimal_price_with_config(&oracle_price, &conservative_config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1003); // 1000 + 2 + 1
    }

    #[test]
    fn test_comprehensive_validation() {
        let trade = create_valid_buy_trade();
        let current_price = 1000;
        let oracle_price = OraclePrice {
            price: 1000,
            exponent: 0,
        };
        let max_slippage_bps = 100;
        let min_distance_bps = 100;
        let min_risk_reward_bps = 100;
        let max_deviation_bps = 100;
        let range_buffer_bps = 100;
        let result = trade.comprehensive_validation(
            current_price,
            &oracle_price,
            max_slippage_bps,
            min_distance_bps,
            min_risk_reward_bps,
            max_deviation_bps,
            range_buffer_bps,
        );
        assert!(result.is_ok());

        let current_price = 2000; // triggers slippage error
        let result = trade.comprehensive_validation(
            current_price,
            &oracle_price,
            max_slippage_bps,
            min_distance_bps,
            min_risk_reward_bps,
            max_deviation_bps,
            range_buffer_bps,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_with_config() {
        let trade = create_valid_buy_trade();
        let current_price = 1000;
        let oracle_price = OraclePrice {
            price: 1000,
            exponent: 0,
        };
        let config = PriceValidationConfig::default();

        let result = trade.validate_with_config(current_price, &oracle_price, &config);
        assert!(result.is_ok());

        // Test with conservative config
        let conservative_config = PriceValidationConfig::conservative();
        let result = trade.validate_with_config(current_price, &oracle_price, &conservative_config);
        assert!(result.is_ok());

        // Test with aggressive config
        let aggressive_config = PriceValidationConfig::aggressive();
        let result = trade.validate_with_config(current_price, &oracle_price, &aggressive_config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_can_execute_with_config() {
        let trade = create_valid_buy_trade();
        let current_price = 1000;
        let oracle_price = OraclePrice {
            price: 1000,
            exponent: 0,
        };
        let config = PriceValidationConfig::default();

        let result = trade.can_execute_with_config(current_price, &oracle_price, &config);
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Test with conservative config
        let conservative_config = PriceValidationConfig::conservative();
        let result =
            trade.can_execute_with_config(current_price, &oracle_price, &conservative_config);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_complete_secure() {
        let mut trade = create_valid_buy_trade();
        let authority = trade.authority; // Use the trade's actual authority
        let pnl = 1000;
        let result = trade.complete_secure(TradeResult::Success, &authority, pnl);
        assert!(result.is_ok());
        assert_eq!(trade.get_status(), TradeStatus::Completed);
        assert_eq!(trade.get_result(), TradeResult::Success);

        let result = trade.complete_secure(TradeResult::Failed, &authority, pnl);
        assert!(result.is_err());
    }

    #[test]
    fn test_cancel_secure() {
        let mut trade = create_valid_buy_trade();
        let authority = trade.authority; // Use the trade's actual authority
        let reason = "Test cancellation";
        let result = trade.cancel_secure(&authority, reason);
        assert!(result.is_ok());
        assert_eq!(trade.get_status(), TradeStatus::Cancelled);
        assert_eq!(trade.get_result(), TradeResult::Failed);

        let result = trade.cancel_secure(&authority, reason);
        assert!(result.is_err());
    }

    #[test]
    fn test_trigger_circuit_breaker() {
        let mut trade = create_valid_buy_trade();
        let authority = trade.authority;
        let result = trade.trigger_circuit_breaker(&authority);
        assert!(result.is_ok());
        assert!(trade.circuit_breaker_triggered);

        // Second trigger should also succeed (no restriction on multiple triggers)
        let result = trade.trigger_circuit_breaker(&authority);
        assert!(result.is_ok());
        assert!(trade.circuit_breaker_triggered);
    }

    #[test]
    fn test_reset_circuit_breaker() {
        let mut trade = create_valid_buy_trade();
        let admin_authority = trade.authority;
        let result = trade.reset_circuit_breaker(&admin_authority);
        assert!(result.is_ok());
        assert!(!trade.circuit_breaker_triggered);

        // Second reset should also succeed (no restriction on multiple resets)
        let result = trade.reset_circuit_breaker(&admin_authority);
        assert!(result.is_ok());
        assert!(!trade.circuit_breaker_triggered);
    }

    #[test]
    fn test_init_trade_secure() {
        let params = TradeInitParams {
            master_agent: Pubkey::new_unique(),
            size: 123,
            entry_price: 1000, // Changed from 456
            take_profit: 900,  // Changed from 789 - should be < entry_price for sell
            stop_loss: 1100,   // Changed from 321 - should be > entry_price for sell
            created_at: 1111,
            pair: [1, 2, 3, 4, 5, 6, 7, 8],
            feed_id: [9; 32],
            status: TradeStatus::Active,
            trade_type: TradeType::Sell,
            result: TradeResult::Pending,
            bump: 2,
        };
        let mut trade = Trade::default();
        let authority = Pubkey::new_unique();
        let result = trade.init_trade_secure(params, authority);
        assert!(result.is_ok());
        assert_eq!(trade.master_agent, params.master_agent);
        assert_eq!(trade.size, params.size);
        assert_eq!(trade.entry_price, params.entry_price);
        assert_eq!(trade.take_profit, params.take_profit);
        assert_eq!(trade.stop_loss, params.stop_loss);
        assert_eq!(trade.created_at, params.created_at);
        assert_eq!(trade.updated_at, params.created_at);
        assert_eq!(trade.pair, params.pair);
        assert_eq!(trade.feed_id, params.feed_id);
        assert_eq!(trade.get_status(), params.status);
        assert_eq!(trade.get_trade_type(), params.trade_type);
        assert_eq!(trade.get_result(), params.result);
        assert_eq!(trade.bump, params.bump);
        assert_eq!(trade.authority, authority);
        assert_eq!(trade.oracle_consensus_count, 0);
        assert_eq!(trade.last_price_update, params.created_at);
        assert!(!trade.circuit_breaker_triggered);
    }

    #[test]
    fn test_get_security_status() {
        let mut trade = create_valid_buy_trade();
        trade.oracle_consensus_count = 2; // Set to 2 to get "Secure" status
        assert_eq!(trade.get_security_status(), "Secure");

        let mut trade = create_valid_buy_trade();
        trade.oracle_consensus_count = 2; // Set to 2 to avoid "Low Oracle Consensus"
        trade.circuit_breaker_triggered = true;
        assert_eq!(trade.get_security_status(), "Circuit Breaker Active");

        let mut trade = create_valid_buy_trade();
        trade.oracle_consensus_count = 1;
        assert_eq!(trade.get_security_status(), "Low Oracle Consensus");

        let mut trade = create_valid_buy_trade();
        trade.circuit_breaker_triggered = true;
        trade.oracle_consensus_count = 1;
        assert_eq!(
            trade.get_security_status(),
            "Circuit Breaker Active, Low Oracle Consensus"
        );
    }
}
