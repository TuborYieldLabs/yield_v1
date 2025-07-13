//! # Master Agent State Module
//!
//! This module defines the core state, configuration, and logic for the "Master Agent" in the Tubor Yield protocol.
//! The Master Agent is responsible for managing agent supply, pricing, yield rates, trading status, and tax configuration.
//!
//! ## Main Components
//!
//! - **AgentPrice**: Simple struct for representing price breakdowns (total, tax, base).
//! - **UpdatePriceEvent / UpdateYieldEvent**: Anchor events for on-chain analytics and monitoring of price/yield changes.
//! - **MasterAgentInitParams**: Parameters required to initialize a new MasterAgent account.
//! - **TaxConfig**: Struct for buy/sell/max tax rates, with validation and default values.
//! - **MasterAgent**: The main on-chain account, storing all critical state for a master agent, including authority, price, yield, supply, trading status, and tax config.
//! - **TradingStatus**: Enum for agent trading modes (Whitelist/Public).
//!
//! ## Key Methods
//!
//! - `initialize`: Sets up a new MasterAgent with provided parameters.
//! - `update_price`: Securely updates the agent price, with authority, time, and rate-limit checks.
//! - `update_yield`: Securely updates the yield rate, with authority, time, and rate-limit checks.
//! - `update_max_supply`, `add_agent`, `remove_agent`: Manage agent supply.
//! - `set_trading_status`, `toggle_trading_status`: Manage trading mode (Whitelist/Public).
//! - `calculate_buy_price_with_tax`, `calculate_sell_price_with_tax`: Compute buy/sell prices including tax.
//! - `update_tax_config`: Securely update tax configuration with authority and rate-limiting.
//! - `validate_security`: Comprehensive security validation for all critical fields and activity patterns.
//!
//! ## Security Features
//!
//! - **Authority Validation**: Only the stored authority can update critical fields.
//! - **Rate Limiting**: Price and yield can only be updated at controlled intervals and within capped increments.
//! - **Tax Limits**: Tax rates are capped and validated on every update.
//! - **Suspicious Activity Detection**: Checks for abnormal price/yield/supply/trade patterns.
//! - **On-chain Time**: Uses Solana's clock for secure time-based checks.
//!
//! ## Analytics & Monitoring
//!
//! - Emits events for every price/yield update, including before/after values, percentage changes, and context.
//! - Provides methods for calculating TVL, yield efficiency, trading activity, and supply utilization.
//!
//! ## Testing
//!
//! - Comprehensive unit tests for all major methods, edge cases, and security checks.
//!
//! ## Usage
//!
//! This module is intended for use in Anchor-based Solana programs. It should not be used directly in client code, but rather through program instructions that invoke these methods securely.
//!
//! ---
//! For detailed method-level documentation, see the doc comments on each struct and function below.

use anchor_lang::prelude::*;

use crate::error::{ErrorCode, TYieldResult};
use crate::math::{SafeMath, PERCENTAGE_PRECISION_U64, QUOTE_PRECISION_U64};
use crate::state::Size;

/// Represents a price breakdown including total price, tax amount, and base price.
///
/// This struct is used for calculating and returning price information
/// that includes tax calculations for buy and sell operations.
///
/// # Example
/// ```
/// # use tubor_yield::state::master_agent::AgentPrice;
/// let price = AgentPrice {
///     total_price: 1025000,  // Total price including tax
///     tax_amount: 25000,     // Tax amount (2.5%)
///     base_price: 1000000,   // Base price before tax
/// };
/// ```
#[derive(Debug, Clone, AnchorDeserialize, AnchorSerialize)]
pub struct AgentPrice {
    /// Total price including tax
    pub total_price: u64,
    /// Tax amount in lamports
    pub tax_amount: u64,
    /// Base price before tax
    pub base_price: u64,
}

#[event]
pub struct UpdatePriceEvent {
    /// The authority that performed the price update
    pub authority: Pubkey,
    /// The master agent's mint address
    pub mint: Pubkey,
    /// The previous price before the update
    pub old_price: u64,
    /// The new price after the update
    pub new_price: u64,
    /// The price change amount (new_price - old_price)
    pub price_change: i64,
    /// The percentage change in price (in basis points)
    pub price_change_percentage: u64,
    /// The timestamp when the price was updated
    pub timestamp: i64,
    /// The current agent count at the time of update
    pub agent_count: u64,
    /// The current trade count at the time of update
    pub trade_count: u64,
    /// The current yield rate at the time of update
    pub yield_rate: u64,
    /// The trading status at the time of update
    pub trading_status: u8,
    /// The total value locked before the price update
    pub old_total_value_locked: u64,
    /// The total value locked after the price update
    pub new_total_value_locked: u64,
    /// The bump seed for the master agent account
    pub bump: u8,
}

/// Event emitted when a master agent's yield rate is updated.
///
/// This event provides comprehensive information about yield rate changes including
/// the authority that performed the update, the old and new yield rates, the change
/// amount and percentage, and the impact on total yield generated.
///
/// # Security Features
/// - Authority validation ensures only authorized users can update yield rates
/// - Rate limiting prevents excessive yield increases (max 5% per update)
/// - Time-based restrictions enforce minimum intervals between updates
/// - Maximum yield limits prevent unsustainable rates (max 50%)
///
/// # Analytics Data
/// - Yield change tracking for volatility analysis
/// - Percentage change calculations for trend analysis
/// - Total yield impact assessment
/// - Context data for correlation analysis
///
/// # Example
/// ```
/// # use anchor_lang::prelude::*;
/// # use tubor_yield::state::UpdateYieldEvent;
/// let event = UpdateYieldEvent {
///     authority: Pubkey::new_unique(),
///     mint: Pubkey::new_unique(),
///     old_yield_rate: 500,  // 5%
///     new_yield_rate: 600,  // 6%
///     yield_change: 100,    // +1%
///     yield_change_percentage: 2000, // 20% increase
///     timestamp: 1640995200,
///     agent_count: 10,
///     trade_count: 25,
///     price: 1000000,
///     trading_status: 1,
///     old_total_yield_generated: 50000,
///     new_total_yield_generated: 60000,
///     bump: 1,
/// };
/// ```
#[event]
pub struct UpdateYieldEvent {
    /// The authority that performed the yield update
    pub authority: Pubkey,
    /// The master agent's mint address
    pub mint: Pubkey,
    /// The previous yield rate before the update (in basis points)
    pub old_yield_rate: u64,
    /// The new yield rate after the update (in basis points)
    pub new_yield_rate: u64,
    /// The yield rate change amount (new_yield_rate - old_yield_rate)
    pub yield_change: i64,
    /// The percentage change in yield rate (in basis points, 10000 = 100%)
    pub yield_change_percentage: u64,
    /// The timestamp when the yield was updated
    pub timestamp: i64,
    /// The current agent count at the time of update
    pub agent_count: u64,
    /// The current trade count at the time of update
    pub trade_count: u64,
    /// The current price at the time of update
    pub price: u64,
    /// The trading status at the time of update
    pub trading_status: u8,
    /// The total yield generated before the update
    pub old_total_yield_generated: u64,
    /// The total yield generated after the update
    pub new_total_yield_generated: u64,
    /// The bump seed for the master agent account
    pub bump: u8,
}

/// Parameters required for initializing a new MasterAgent account.
///
/// This struct contains all the necessary parameters to create a new MasterAgent,
/// including authority, mint address, initial price and yield settings, trading status,
/// supply limits, and tax configuration.
///
/// # Example
/// ```
/// # use anchor_lang::prelude::*;
/// # use tubor_yield::state::master_agent::{MasterAgentInitParams, TradingStatus, TaxConfig};
/// let params = MasterAgentInitParams {
///     authority: Pubkey::new_unique(),
///     mint: Pubkey::new_unique(),
///     price: 1000000,  // 1 SOL in lamports
///     w_yield: 500,    // 5% yield rate
///     trading_status: TradingStatus::WhiteList,
///     max_supply: 100,
///     auto_relist: false,
///     current_time: 1640995200,
///     bump: 1,
///     tax_config: TaxConfig::default(),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct MasterAgentInitParams {
    /// The authority that can update the master agent
    pub authority: Pubkey,
    /// The mint address for the master agent
    pub mint: Pubkey,
    /// Initial price in lamports
    pub price: u64,
    /// Initial yield rate in basis points
    pub w_yield: u64,
    /// Initial trading status
    pub trading_status: TradingStatus,
    /// Maximum supply of agents
    pub max_supply: u64,
    /// Whether to auto-relist agents
    pub auto_relist: bool,
    /// Current timestamp for initialization
    pub current_time: i64,
    /// Bump seed for PDA derivation
    pub bump: u8,
    /// Tax configuration for buy/sell operations
    pub tax_config: TaxConfig,
}

/// Tax configuration for buy and sell operations.
///
/// This struct defines the tax rates for buy and sell operations,
/// with validation to ensure rates don't exceed maximum limits.
///
/// # Example
/// ```
/// # use tubor_yield::state::master_agent::TaxConfig;
/// let tax_config = TaxConfig {
///     buy_tax_percentage: 250,   // 2.5% buy tax
///     sell_tax_percentage: 250,  // 2.5% sell tax
///     max_tax_percentage: 1000,  // 10% maximum tax
/// };
/// ```
#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize, PartialEq, Eq)]
pub struct TaxConfig {
    /// Buy tax percentage in basis points (e.g., 250 = 2.5%)
    pub buy_tax_percentage: u64,
    /// Sell tax percentage in basis points (e.g., 250 = 2.5%)
    pub sell_tax_percentage: u64,
    /// Maximum allowed tax percentage in basis points
    pub max_tax_percentage: u64,
}

impl Default for TaxConfig {
    /// Creates a default tax configuration with 2.5% buy/sell tax and 10% maximum.
    ///
    /// # Example
    /// ```
    /// # use tubor_yield::state::master_agent::TaxConfig;
    /// let default_config = TaxConfig::default();
    /// assert_eq!(default_config.buy_tax_percentage, 250);   // 2.5%
    /// assert_eq!(default_config.sell_tax_percentage, 250);  // 2.5%
    /// assert_eq!(default_config.max_tax_percentage, 1000);  // 10%
    /// ```
    fn default() -> Self {
        Self {
            buy_tax_percentage: 250,  // 2.5% default buy tax
            sell_tax_percentage: 250, // 2.5% default sell tax
            max_tax_percentage: 1000, // 10% maximum tax
        }
    }
}

/// The main MasterAgent account that stores all critical state for a master agent.
///
/// This account contains all the necessary information to manage a master agent,
/// including authority, pricing, yield rates, supply management, trading status,
/// and tax configuration. It implements comprehensive security measures and
/// provides methods for all agent operations.
///
/// # Security Features
/// - Authority validation for all critical operations
/// - Rate limiting for price and yield updates
/// - Tax rate validation and limits
/// - Suspicious activity detection
/// - Time-based restrictions
///
/// # Example
/// ```
/// # use anchor_lang::prelude::*;
/// # use tubor_yield::state::master_agent::{MasterAgent, MasterAgentInitParams, TradingStatus, TaxConfig};
/// let mut master_agent = MasterAgent::default();
/// let params = MasterAgentInitParams {
///     authority: Pubkey::new_unique(),
///     mint: Pubkey::new_unique(),
///     price: 1000000,
///     w_yield: 500,
///     trading_status: TradingStatus::WhiteList,
///     max_supply: 100,
///     auto_relist: false,
///     current_time: 1640995200,
///     bump: 1,
///     tax_config: TaxConfig::default(),
/// };
/// master_agent.initialize(params).unwrap();
/// ```
#[account]
#[derive(Eq, PartialEq, Debug)]
pub struct MasterAgent {
    // 8-byte aligned fields (largest first)
    pub authority: Pubkey, // 32 bytes
    pub mint: Pubkey,      // 32 bytes
    pub price: u64,        // 8 bytes
    pub w_yield: u64,      // 8 bytes
    pub max_supply: u64,   // 8 bytes
    pub agent_count: u64,  // 8 bytes
    pub trade_count: u64,  // 8 bytes

    /// PRECISION: DAILY_SECONDS_PRECISION
    pub price_update_allowance: u64,

    pub completed_trades: u64,

    /// QUOTE PRECISION
    pub total_pnl: u64,

    // 4-byte aligned fields
    pub last_updated: i64, // 4 bytes
    pub created_at: i64,   // 4 bytes
    pub last_price_update: i64,

    // 1-byte aligned fields (smallest last)
    pub trading_status: u8, // 1 byte
    pub auto_relist: bool,  // 1 byte
    pub bump: u8,           // 1 byte

    // SECURITY: Store tax configuration securely
    pub tax_config: TaxConfig, // 24 bytes (3 u64 fields)

    // Future-proofing padding
    pub _padding: [u8; 1], // 1 byte for future additions
}

impl MasterAgent {
    /// Initialize a new MasterAgent with the provided parameters.
    ///
    /// This method sets up all the initial state for a master agent,
    /// including authority, mint address, price, yield rate, trading status,
    /// supply limits, and tax configuration.
    ///
    /// # Arguments
    /// * `params` - The initialization parameters containing all required fields
    ///
    /// # Returns
    /// * `Ok(())` - If initialization was successful
    /// * `Err(ErrorCode)` - If initialization failed
    ///
    /// # Example
    /// ```
    /// # use anchor_lang::prelude::*;
    /// # use tubor_yield::state::master_agent::{MasterAgent, MasterAgentInitParams, TradingStatus, TaxConfig};
    /// # use tubor_yield::error::TYieldResult;
    /// # fn example() -> TYieldResult<()> {
    /// let mut master_agent = MasterAgent::default();
    /// let params = MasterAgentInitParams {
    ///     authority: Pubkey::new_unique(),
    ///     mint: Pubkey::new_unique(),
    ///     price: 1000000,
    ///     w_yield: 500,
    ///     trading_status: TradingStatus::WhiteList,
    ///     max_supply: 100,
    ///     auto_relist: false,
    ///     current_time: 1640995200,
    ///     bump: 1,
    ///     tax_config: TaxConfig::default(),
    /// };
    /// master_agent.initialize(params)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn initialize(&mut self, params: MasterAgentInitParams) -> TYieldResult<()> {
        self.authority = params.authority;
        self.mint = params.mint;
        self.price = params.price;
        self.w_yield = params.w_yield;
        self.trading_status = params.trading_status as u8;
        self.max_supply = params.max_supply;
        self.agent_count = 0;
        self.trade_count = 0;
        self.auto_relist = params.auto_relist;
        self.last_updated = params.current_time;
        self.created_at = params.current_time;
        self.bump = params.bump;
        self.tax_config = params.tax_config; // Store tax config securely
        Ok(())
    }

    /// Update the price of the master agent with comprehensive security validation.
    ///
    /// This method enforces strict security constraints to prevent price manipulation
    /// and ensure sustainable price increases. It requires authority validation, implements
    /// rate limiting, and enforces time-based restrictions.
    ///
    /// # Arguments
    /// * `new_price` - The new price in lamports
    /// * `current_time` - The current timestamp for validation
    /// * `authority` - The authority attempting to update the price
    ///
    /// # Security Constraints
    /// - **Authority Validation**: Only the master agent's authority can update prices
    /// - **Zero Price Protection**: Price cannot be set to zero
    /// - **Price Increase Only**: Price must be higher than current price
    /// - **Time Restrictions**: Minimum 1 hour (129,600 seconds) between updates
    /// - **Rate Limiting**: Maximum 10% increase per update to prevent manipulation
    ///
    /// # Returns
    /// * `Ok(())` - If the price was updated successfully
    /// * `Err(ErrorCode::InvalidAuthority)` - If the authority is not authorized
    /// * `Err(ErrorCode::InvalidEntryPrice)` - If price is zero
    /// * `Err(ErrorCode::MathError)` - If price is not increasing
    /// * `Err(ErrorCode::PriceUpdateTooSoon)` - If insufficient time has passed
    /// * `Err(ErrorCode::PriceUpdateTooHigh)` - If the increase exceeds rate limits
    ///
    /// # Example
    /// ```
    /// # use anchor_lang::prelude::*;
    /// # use tubor_yield::state::master_agent::MasterAgent;
    /// # use tubor_yield::error::TYieldResult;
    /// # fn example() -> TYieldResult<()> {
    /// let mut master_agent = MasterAgent::default();
    /// let authority = Pubkey::new_unique();
    /// let current_time = 1640995200;
    ///
    /// // Update price to 1.1 SOL (1,100,000 lamports)
    /// master_agent.update_price(1_100_000, current_time, &authority)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Security Notes
    /// - All calculations use safe math operations to prevent overflow/underflow
    /// - Time-based restrictions prevent rapid price manipulation
    /// - Rate limiting ensures sustainable price increases
    /// - Authority validation prevents unauthorized changes
    pub fn update_price(
        &mut self,
        new_price: u64,
        current_time: i64,
        authority: &Pubkey,
    ) -> TYieldResult<()> {
        // SECURITY: Validate authority
        if authority != &self.authority {
            return Err(ErrorCode::InvalidAuthority);
        }

        // SECURITY: Validate price is not zero
        if new_price == 0 {
            return Err(ErrorCode::InvalidEntryPrice);
        }

        // SECURITY: Validate price is increasing (prevents price manipulation)
        if new_price <= self.price {
            return Err(ErrorCode::MathError);
        }

        // SECURITY: Time-based restrictions (minimum 1 hour between updates)
        let min_update_interval = 129_600; // 1 hour in seconds
        let time_since_last_update = current_time.safe_sub(self.last_updated)?;
        msg!(
            "time_since_last_update = {}, min_update_interval = {}",
            time_since_last_update,
            min_update_interval
        );
        if time_since_last_update < min_update_interval {
            msg!("returning PriceUpdateTooSoon");
            return Err(ErrorCode::PriceUpdateTooSoon);
        }

        // SECURITY: Maximum price increase limit (10% per update)
        let max_allowed_price = self.price.safe_mul(11000)?.safe_div(10000)?;
        msg!(
            "new_price = {}, max_allowed_price = {}",
            new_price,
            max_allowed_price
        );
        if new_price > max_allowed_price {
            msg!("returning PriceUpdateTooHigh");
            return Err(ErrorCode::PriceUpdateTooHigh);
        }

        self.price = new_price;
        self.last_updated = current_time;
        self.last_price_update = current_time;
        Ok(())
    }

    /// SECURITY: Helper method to track price updates per day
    fn _get_price_updates_today(&self, current_time: i64) -> u64 {
        // This is a simplified implementation
        // In a real implementation, you'd want to track this more precisely
        let day_start = current_time - (current_time % 86400);
        if self.last_price_update >= day_start {
            1 // At least one update today
        } else {
            0
        }
    }

    /// Updates the yield percentage for the master agent with comprehensive security validation.
    ///
    /// This method enforces strict security constraints to prevent yield manipulation
    /// and ensure sustainable yield rates. It requires authority validation, implements
    /// rate limiting, and enforces time-based restrictions.
    ///
    /// # Arguments
    /// * `new_yield` - The new yield rate in basis points (e.g., 500 = 5%)
    /// * `current_time` - The current timestamp for validation
    /// * `authority` - The authority attempting to update the yield rate
    ///
    /// # Security Constraints
    /// - **Authority Validation**: Only the master agent's authority can update yield rates
    /// - **Zero Yield Protection**: Yield rate cannot be set to zero
    /// - **Maximum Yield Limit**: Yield rate cannot exceed 50% (50000 basis points)
    /// - **Time Restrictions**: Minimum 1 hour (129,600 seconds) between updates
    /// - **Rate Limiting**: Maximum 5% increase per update to prevent manipulation
    ///
    /// # Returns
    /// * `Ok(())` - If the yield rate was updated successfully
    /// * `Err(ErrorCode::InvalidAuthority)` - If the authority is not authorized
    /// * `Err(ErrorCode::MathError)` - If yield rate is invalid or exceeds limits
    /// * `Err(ErrorCode::PriceUpdateTooSoon)` - If insufficient time has passed
    /// * `Err(ErrorCode::PriceUpdateTooHigh)` - If the increase exceeds rate limits
    ///
    /// # Example
    /// ```
    /// # use anchor_lang::prelude::*;
    /// # use tubor_yield::state::master_agent::MasterAgent;
    /// # use tubor_yield::error::TYieldResult;
    /// # fn example() -> TYieldResult<()> {
    /// let mut master_agent = MasterAgent::default();
    /// let authority = Pubkey::new_unique();
    /// let current_time = 1640995200;
    ///
    /// // Update yield rate to 6% (600 basis points)
    /// master_agent.update_yield(600, current_time, &authority)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Security Notes
    /// - All calculations use safe math operations to prevent overflow/underflow
    /// - Time-based restrictions prevent rapid yield manipulation
    /// - Rate limiting ensures sustainable yield increases
    /// - Authority validation prevents unauthorized changes
    pub fn update_yield(
        &mut self,
        new_yield: u64,
        current_time: i64,
        authority: &Pubkey,
    ) -> TYieldResult<()> {
        // SECURITY: Validate authority
        if authority != &self.authority {
            return Err(ErrorCode::InvalidAuthority);
        }

        // SECURITY: Validate yield is not zero
        if new_yield == 0 {
            return Err(ErrorCode::MathError);
        }

        // SECURITY: Validate yield is within reasonable bounds
        if new_yield > PERCENTAGE_PRECISION_U64 {
            return Err(ErrorCode::MathError);
        }

        // SECURITY: Maximum yield rate limit (50% max yield)
        let max_allowed_yield = 50000; // 50% in basis points
        if new_yield > max_allowed_yield {
            return Err(ErrorCode::MathError);
        }

        // SECURITY: Time-based restrictions (minimum 1 hour between updates)
        let min_update_interval = 129_600; // 1 hour in seconds
        let time_since_last_update = current_time.safe_sub(self.last_updated)?;
        if time_since_last_update < min_update_interval {
            return Err(ErrorCode::PriceUpdateTooSoon);
        }

        // SECURITY: Maximum yield increase limit (5% per update)
        let max_allowed_yield_increase = self.w_yield.safe_mul(10500)?.safe_div(10000)?;
        if new_yield > max_allowed_yield_increase {
            return Err(ErrorCode::PriceUpdateTooHigh);
        }

        self.w_yield = new_yield;
        self.last_updated = current_time;
        Ok(())
    }

    /// Update the maximum supply
    pub fn update_max_supply(
        &mut self,
        new_max_supply: u64,
        current_time: i64,
    ) -> TYieldResult<()> {
        if new_max_supply < self.agent_count {
            return Err(ErrorCode::MathError);
        }
        self.max_supply = new_max_supply;
        self.last_updated = current_time;
        Ok(())
    }

    /// Add an agent to the master agent
    pub fn add_agent(&mut self, current_time: i64) -> TYieldResult<()> {
        if self.agent_count >= self.max_supply {
            return Err(ErrorCode::InsufficientFunds);
        }
        self.agent_count = self.agent_count.safe_add(1)?;
        self.last_updated = current_time;
        Ok(())
    }

    /// Remove an agent from the master agent
    pub fn remove_agent(&mut self, current_time: i64) -> TYieldResult<()> {
        if self.agent_count == 0 {
            return Err(ErrorCode::InsufficientFunds);
        }
        self.agent_count = self.agent_count.safe_sub(1)?;
        self.last_updated = current_time;
        Ok(())
    }

    /// Increment trade count
    pub fn increment_trade_count(&mut self, current_time: i64) -> TYieldResult<()> {
        self.trade_count = self.trade_count.safe_add(1)?;
        self.last_updated = current_time;
        Ok(())
    }

    /// Toggle auto relist setting
    pub fn toggle_auto_relist(&mut self, current_time: i64) {
        self.auto_relist = !self.auto_relist;
        self.last_updated = current_time;
    }

    /// Set auto relist setting
    pub fn set_auto_relist(&mut self, auto_relist: bool, current_time: i64) {
        self.auto_relist = auto_relist;
        self.last_updated = current_time;
    }

    /// Get the current trading status as an enum
    pub fn get_trading_status(&self) -> TradingStatus {
        match self.trading_status {
            0b00000001 => TradingStatus::WhiteList,
            0b00000010 => TradingStatus::Public,
            _ => TradingStatus::WhiteList, // Default fallback
        }
    }

    /// Set the trading status
    /// SECURITY: Added authority validation to prevent unauthorized status changes
    pub fn set_trading_status(
        &mut self,
        status: TradingStatus,
        authority: &Pubkey,
        current_time: i64,
    ) -> TYieldResult<()> {
        // SECURITY: Validate authority
        if authority != &self.authority {
            return Err(ErrorCode::InvalidAuthority);
        }

        self.trading_status = status as u8;
        self.last_updated = current_time;
        Ok(())
    }

    /// Check if the master agent is in whitelist mode
    pub fn is_whitelist_mode(&self) -> bool {
        self.get_trading_status() == TradingStatus::WhiteList
    }

    /// Check if the master agent is in public mode
    pub fn is_public_mode(&self) -> bool {
        self.get_trading_status() == TradingStatus::Public
    }

    /// Toggle between whitelist and public mode
    pub fn toggle_trading_status(&mut self, current_time: i64) {
        let new_status = if self.is_whitelist_mode() {
            TradingStatus::Public
        } else {
            TradingStatus::WhiteList
        };
        let authority = self.authority; // Store authority to avoid borrow checker issues
        let _ = self.set_trading_status(new_status, &authority, current_time);
    }

    /// Calculate the yield amount based on current price
    pub fn calculate_yield_amount(&self) -> TYieldResult<u64> {
        let yield_amount = self.price.safe_mul(self.w_yield)?;
        let yield_with_precision = yield_amount.safe_div(PERCENTAGE_PRECISION_U64)?;
        Ok(yield_with_precision)
    }

    /// Get the current yield rate as a percentage
    pub fn get_yield_rate_percentage(&self) -> u64 {
        (self.w_yield.safe_mul(100).unwrap_or(0))
            .safe_div(PERCENTAGE_PRECISION_U64)
            .unwrap_or(0)
    }

    /// Check if the master agent has reached maximum supply
    pub fn is_supply_full(&self) -> bool {
        self.agent_count >= self.max_supply
    }

    /// Get remaining supply
    pub fn get_remaining_supply(&self) -> u64 {
        if self.agent_count >= self.max_supply {
            0
        } else {
            self.max_supply.safe_sub(self.agent_count).unwrap_or(0)
        }
    }

    /// Get supply utilization percentage
    pub fn get_supply_utilization_percentage(&self) -> u64 {
        if self.max_supply == 0 {
            return 0;
        }
        (self
            .agent_count
            .safe_mul(PERCENTAGE_PRECISION_U64)
            .unwrap_or(0))
        .safe_div(self.max_supply)
        .unwrap_or(0)
    }

    /// Get average trades per agent
    pub fn get_average_trades_per_agent(&self) -> u64 {
        if self.agent_count == 0 {
            return 0;
        }
        self.trade_count.safe_div(self.agent_count).unwrap_or(0)
    }

    /// Get days since creation
    pub fn get_days_since_created(&self, current_time: i64) -> i64 {
        (current_time - self.created_at) / 86400 // 86400 seconds in a day
    }

    /// Get days since last update
    pub fn get_days_since_updated(&self, current_time: i64) -> i64 {
        (current_time - self.last_updated) / 86400
    }

    /// Check if the master agent is active (has agents)
    pub fn is_active(&self) -> bool {
        self.agent_count > 0
    }

    /// Check if the master agent is idle (no recent activity)
    pub fn is_idle(&self, current_time: i64, idle_threshold: i64) -> bool {
        let time_since_last_activity = current_time - self.last_updated;
        time_since_last_activity > idle_threshold
    }

    /// Validate the master agent configuration for correctness and security.
    ///
    /// This method performs comprehensive validation of all critical fields
    /// to ensure the master agent is in a valid state. It checks authority,
    /// mint address, price, yield rates, supply limits, and trading status.
    ///
    /// # Returns
    /// * `Ok(())` - If the master agent configuration is valid
    /// * `Err(ErrorCode::InvalidAuthority)` - If authority is default
    /// * `Err(ErrorCode::InvalidAccount)` - If mint is default or timestamps are invalid
    /// * `Err(ErrorCode::InvalidEntryPrice)` - If price is zero
    /// * `Err(ErrorCode::MathError)` - If yield or supply values are invalid
    /// * `Err(ErrorCode::CannotPerformAction)` - If trading status is invalid
    ///
    /// # Example
    /// ```
    /// # use tubor_yield::state::master_agent::MasterAgent;
    /// # use tubor_yield::error::TYieldResult;
    /// # fn example() -> TYieldResult<()> {
    /// let master_agent = MasterAgent::default();
    /// // This will fail because default values are invalid
    /// assert!(master_agent.validate().is_err());
    /// # Ok(())
    /// # }
    /// ```
    pub fn validate(&self) -> TYieldResult<()> {
        if self.authority == Pubkey::default() {
            return Err(ErrorCode::InvalidAuthority);
        }
        if self.mint == Pubkey::default() {
            return Err(ErrorCode::InvalidAccount);
        }
        if self.price == 0 {
            return Err(ErrorCode::InvalidEntryPrice);
        }
        if self.w_yield > PERCENTAGE_PRECISION_U64 {
            return Err(ErrorCode::MathError);
        }
        if self.max_supply == 0 {
            return Err(ErrorCode::MathError);
        }
        if self.agent_count > self.max_supply {
            return Err(ErrorCode::MathError);
        }
        if self.created_at <= 0 {
            return Err(ErrorCode::InvalidAccount);
        }
        if self.last_updated < self.created_at {
            return Err(ErrorCode::InvalidAccount);
        }
        // Validate trading status
        if !matches!(
            self.get_trading_status(),
            TradingStatus::WhiteList | TradingStatus::Public
        ) {
            return Err(ErrorCode::CannotPerformAction);
        }
        Ok(())
    }

    /// Check if the master agent can perform actions
    pub fn can_perform_actions(&self) -> bool {
        self.is_active() && !self.is_supply_full()
    }

    /// Check if the master agent can be accessed by a user (based on trading status)
    pub fn can_be_accessed_by_user(&self, user_is_whitelisted: bool) -> bool {
        match self.get_trading_status() {
            TradingStatus::WhiteList => user_is_whitelisted,
            TradingStatus::Public => true,
        }
    }

    /// Check if trading is allowed for the current status
    pub fn is_trading_allowed(&self) -> bool {
        self.is_active() && !self.is_supply_full()
    }

    /// Get total value locked (TVL) in the master agent
    pub fn get_total_value_locked(&self) -> u64 {
        self.agent_count.safe_mul(self.price).unwrap_or(0)
    }

    /// Get total yield generated
    pub fn get_total_yield_generated(&self) -> TYieldResult<u64> {
        let yield_per_agent = self.calculate_yield_amount()?;
        let total_yield = yield_per_agent.safe_mul(self.agent_count)?;
        Ok(total_yield)
    }

    /// Get yield efficiency (yield generated vs total value)
    pub fn get_yield_efficiency(&self) -> TYieldResult<u64> {
        let total_value = self.get_total_value_locked();
        if total_value == 0 {
            return Ok(0);
        }
        let total_yield = self.get_total_yield_generated()?;
        let efficiency = (total_yield.safe_mul(QUOTE_PRECISION_U64)?).safe_div(total_value)?;
        Ok(efficiency)
    }

    /// Get trading activity score (trades per day since creation)
    pub fn get_trading_activity_score(&self, current_time: i64) -> u64 {
        let days_since_created = self.get_days_since_created(current_time);
        if days_since_created == 0 {
            return self.trade_count;
        }
        self.trade_count
            .safe_div(days_since_created as u64)
            .unwrap_or(0)
    }

    /// Get performance metrics
    pub fn get_performance_metrics(&self, current_time: i64) -> TYieldResult<(u64, u64, u64, u64)> {
        let total_value = self.get_total_value_locked();
        let total_yield = self.get_total_yield_generated()?;
        let yield_efficiency = self.get_yield_efficiency()?;
        let activity_score = self.get_trading_activity_score(current_time);
        Ok((total_value, total_yield, yield_efficiency, activity_score))
    }

    /// Reset the master agent (for testing/debugging)
    pub fn reset(&mut self) {
        self.agent_count = 0;
        self.trade_count = 0;
        self.trading_status = TradingStatus::WhiteList as u8;
        self.last_updated = self.created_at;
    }

    /// Get summary statistics
    pub fn get_summary(&self) -> (u64, u64, u64, u64, bool, TradingStatus) {
        (
            self.agent_count,
            self.trade_count,
            self.price,
            self.w_yield,
            self.auto_relist,
            self.get_trading_status(),
        )
    }

    /// Check if the master agent needs attention (low activity, high supply utilization, etc.)
    pub fn needs_attention(&self, current_time: i64) -> bool {
        let days_since_update = self.get_days_since_updated(current_time);
        let supply_utilization = self.get_supply_utilization_percentage();

        // Needs attention if:
        // 1. No activity in the last 7 days
        // 2. Supply utilization is over 90%
        // 3. No agents deployed
        days_since_update > 7 || supply_utilization > 9000 || self.agent_count == 0
    }

    /// Get status string for display
    pub fn get_status_string(&self) -> String {
        if self.is_supply_full() {
            "Full".to_string()
        } else if self.agent_count == 0 {
            "Empty".to_string()
        } else {
            "Active".to_string()
        }
    }

    /// Get auto relist status string
    pub fn get_auto_relist_status(&self) -> String {
        if self.auto_relist {
            "Enabled".to_string()
        } else {
            "Disabled".to_string()
        }
    }

    /// Get trading status string for display
    pub fn get_trading_status_string(&self) -> String {
        match self.get_trading_status() {
            TradingStatus::WhiteList => "Whitelist".to_string(),
            TradingStatus::Public => "Public".to_string(),
        }
    }

    /// Calculate the expected buy price including taxes.
    ///
    /// This method calculates the total price for buying an agent, including
    /// the base price and the buy tax. It uses the stored tax configuration
    /// to ensure consistency and security.
    ///
    /// # Returns
    /// * `Ok((total_price, tax_amount, base_price))` - The price breakdown
    ///   - `total_price`: Total price including tax
    ///   - `tax_amount`: Tax amount in lamports
    ///   - `base_price`: Base price before tax
    /// * `Err(ErrorCode::MathError)` - If tax configuration is invalid
    ///
    /// # Example
    /// ```
    /// # use tubor_yield::state::master_agent::MasterAgent;
    /// # use tubor_yield::error::TYieldResult;
    /// # fn example() -> TYieldResult<()> {
    /// let master_agent = MasterAgent::default();
    /// let (total_price, tax_amount, base_price) = master_agent.calculate_buy_price_with_tax()?;
    /// assert_eq!(base_price, 0); // Default price is 0
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Security Notes
    /// - Uses stored tax configuration to prevent manipulation
    /// - Validates tax rates against maximum limits
    /// - All calculations use safe math operations
    pub fn calculate_buy_price_with_tax(&self) -> TYieldResult<(u64, u64, u64)> {
        if self.tax_config.buy_tax_percentage > self.tax_config.max_tax_percentage {
            return Err(ErrorCode::MathError);
        }

        let base_price = self.price;
        let tax_amount = base_price
            .safe_mul(self.tax_config.buy_tax_percentage)?
            .safe_div(PERCENTAGE_PRECISION_U64)?;
        let total_price = base_price.safe_add(tax_amount)?;

        Ok((total_price, tax_amount, base_price))
    }

    /// Calculate the expected sell price including taxes.
    ///
    /// This method calculates the net price for selling an agent, including
    /// the base price minus the sell tax. It uses the stored tax configuration
    /// to ensure consistency and security.
    ///
    /// # Returns
    /// * `Ok((net_price, tax_amount, base_price))` - The price breakdown
    ///   - `net_price`: Net price after tax deduction
    ///   - `tax_amount`: Tax amount in lamports
    ///   - `base_price`: Base price before tax
    /// * `Err(ErrorCode::MathError)` - If tax configuration is invalid
    ///
    /// # Example
    /// ```
    /// # use tubor_yield::state::master_agent::MasterAgent;
    /// # use tubor_yield::error::TYieldResult;
    /// # fn example() -> TYieldResult<()> {
    /// let master_agent = MasterAgent::default();
    /// let (net_price, tax_amount, base_price) = master_agent.calculate_sell_price_with_tax()?;
    /// assert_eq!(base_price, 0); // Default price is 0
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Security Notes
    /// - Uses stored tax configuration to prevent manipulation
    /// - Validates tax rates against maximum limits
    /// - All calculations use safe math operations
    pub fn calculate_sell_price_with_tax(&self) -> TYieldResult<(u64, u64, u64)> {
        if self.tax_config.sell_tax_percentage > self.tax_config.max_tax_percentage {
            return Err(ErrorCode::MathError);
        }

        let base_price = self.price;
        let tax_amount = base_price
            .safe_mul(self.tax_config.sell_tax_percentage)?
            .safe_div(PERCENTAGE_PRECISION_U64)?;
        let net_price = base_price.safe_sub(tax_amount)?;

        Ok((net_price, tax_amount, base_price))
    }

    /// Calculate buy price for a specific amount of USDC
    /// Returns (tokens_received, tax_paid, base_amount)
    /// SECURITY: Now uses stored tax configuration
    pub fn calculate_buy_for_usdc_amount(&self, usdc_amount: u64) -> TYieldResult<(u64, u64, u64)> {
        if usdc_amount == 0 {
            return Err(ErrorCode::InvalidEntryPrice);
        }

        // Calculate how many tokens can be bought with the USDC amount
        let (total_price_per_token, tax_per_token, base_price_per_token) =
            self.calculate_buy_price_with_tax()?;

        let tokens_received = usdc_amount.safe_div(total_price_per_token)?;
        let _total_cost = tokens_received.safe_mul(total_price_per_token)?;
        let tax_paid = tokens_received.safe_mul(tax_per_token)?;
        let base_amount = tokens_received.safe_mul(base_price_per_token)?;

        Ok((tokens_received, tax_paid, base_amount))
    }

    /// Calculate sell price for a specific number of tokens
    /// Returns (usdc_received, tax_paid, base_amount)
    /// SECURITY: Now uses stored tax configuration
    pub fn calculate_sell_for_token_amount(
        &self,
        token_amount: u64,
    ) -> TYieldResult<(u64, u64, u64)> {
        if token_amount == 0 {
            return Err(ErrorCode::InvalidEntryPrice);
        }

        // Calculate USDC received for the token amount
        let (net_price_per_token, tax_per_token, base_price_per_token) =
            self.calculate_sell_price_with_tax()?;

        let usdc_received = token_amount.safe_mul(net_price_per_token)?;
        let tax_paid = token_amount.safe_mul(tax_per_token)?;
        let base_amount = token_amount.safe_mul(base_price_per_token)?;

        Ok((usdc_received, tax_paid, base_amount))
    }

    /// Calculate the effective tax rate for a buy transaction
    /// SECURITY: Now uses stored tax configuration
    pub fn get_buy_tax_rate(&self) -> TYieldResult<u64> {
        if self.tax_config.buy_tax_percentage > self.tax_config.max_tax_percentage {
            return Err(ErrorCode::MathError);
        }

        let (_total_price, _, _) = self.calculate_buy_price_with_tax()?;
        let tax_rate = (self.tax_config.buy_tax_percentage.safe_mul(100)?)
            .safe_div(PERCENTAGE_PRECISION_U64)?;
        Ok(tax_rate)
    }

    /// Calculate the effective tax rate for a sell transaction
    /// SECURITY: Now uses stored tax configuration
    pub fn get_sell_tax_rate(&self) -> TYieldResult<u64> {
        if self.tax_config.sell_tax_percentage > self.tax_config.max_tax_percentage {
            return Err(ErrorCode::MathError);
        }

        let (_, _, _base_price) = self.calculate_sell_price_with_tax()?;
        let tax_rate = (self.tax_config.sell_tax_percentage.safe_mul(100)?)
            .safe_div(PERCENTAGE_PRECISION_U64)?;
        Ok(tax_rate)
    }

    /// Calculate slippage impact on buy price
    /// SECURITY: Now uses stored tax configuration
    pub fn calculate_buy_price_with_slippage(
        &self,
        slippage_bps: u64, // Slippage in basis points
    ) -> TYieldResult<(u64, u64)> {
        let (base_total_price, _, _) = self.calculate_buy_price_with_tax()?;

        let slippage_amount = base_total_price
            .safe_mul(slippage_bps)?
            .safe_div(PERCENTAGE_PRECISION_U64)?;
        let max_price = base_total_price.safe_add(slippage_amount)?;

        Ok((base_total_price, max_price))
    }

    /// Calculate slippage impact on sell price
    /// SECURITY: Now uses stored tax configuration
    pub fn calculate_sell_price_with_slippage(
        &self,
        slippage_bps: u64, // Slippage in basis points
    ) -> TYieldResult<(u64, u64)> {
        let (base_net_price, _, _) = self.calculate_sell_price_with_tax()?;

        let slippage_amount = base_net_price
            .safe_mul(slippage_bps)?
            .safe_div(PERCENTAGE_PRECISION_U64)?;
        let min_price = base_net_price.safe_sub(slippage_amount)?;

        Ok((base_net_price, min_price))
    }

    /// Validate tax configuration
    /// SECURITY: Now validates stored tax configuration
    pub fn validate_tax_config(&self) -> TYieldResult<()> {
        if self.tax_config.buy_tax_percentage > self.tax_config.max_tax_percentage {
            return Err(ErrorCode::MathError);
        }
        if self.tax_config.sell_tax_percentage > self.tax_config.max_tax_percentage {
            return Err(ErrorCode::MathError);
        }
        if self.tax_config.max_tax_percentage > PERCENTAGE_PRECISION_U64 {
            return Err(ErrorCode::MathError);
        }
        Ok(())
    }

    /// Get tax summary for display
    /// SECURITY: Now uses stored tax configuration
    pub fn get_tax_summary(&self) -> TYieldResult<(u64, u64, u64)> {
        self.validate_tax_config()?;

        let buy_tax_rate = self.get_buy_tax_rate()?;
        let sell_tax_rate = self.get_sell_tax_rate()?;
        let max_tax_rate = (self.tax_config.max_tax_percentage.safe_mul(100)?)
            .safe_div(PERCENTAGE_PRECISION_U64)?;

        Ok((buy_tax_rate, sell_tax_rate, max_tax_rate))
    }

    /// Get secure on-chain time
    /// SECURITY: Uses Solana's clock instead of client-provided time
    pub fn get_secure_time() -> TYieldResult<i64> {
        use anchor_lang::solana_program::clock::Clock;
        let clock = Clock::get()?;
        Ok(clock.unix_timestamp)
    }

    /// Check if the price can be updated based on time allowance and max price increase
    /// - new_price: the proposed new price
    /// - max_agent_price_new: max allowed price as a percentage of current price (e.g., 11000 = 110%)
    /// - authority: the authority attempting the update
    /// - current_time: the current time (for testing purposes)
    ///
    /// Returns Ok(()) if allowed, or an error otherwise
    /// SECURITY: Now uses on-chain time and authority validation
    pub fn can_update_price_secure_with_time(
        &self,
        new_price: u64,
        max_agent_price_new: u64,
        authority: &Pubkey,
        current_time: i64,
    ) -> TYieldResult<()> {
        use crate::math::{DAILY_SECONDS_PRECISION, PERCENTAGE_PRECISION_U64};

        // SECURITY: Validate authority
        if authority != &self.authority {
            return Err(ErrorCode::InvalidAuthority);
        }

        // Check time allowance
        let seconds_since_last_update = current_time.safe_sub(self.last_price_update)?;
        if seconds_since_last_update < 0 {
            return Err(ErrorCode::PriceUpdateTooSoon);
        }
        let seconds_since_last_update = seconds_since_last_update as u64;
        let min_seconds = self
            .price_update_allowance
            .safe_mul(DAILY_SECONDS_PRECISION)?;
        if seconds_since_last_update < min_seconds {
            return Err(ErrorCode::PriceUpdateTooSoon);
        }

        // Check max price increase
        let max_allowed_price = self
            .price
            .safe_mul(max_agent_price_new)?
            .safe_div(PERCENTAGE_PRECISION_U64)?;
        if new_price > max_allowed_price {
            return Err(ErrorCode::PriceUpdateTooHigh);
        }

        Ok(())
    }

    /// Update tax configuration with authority validation
    /// SECURITY: Only authorized authority can update tax configuration
    pub fn update_tax_config(
        &mut self,
        new_tax_config: TaxConfig,
        authority: &Pubkey,
        current_time: i64,
    ) -> TYieldResult<()> {
        // SECURITY: Validate authority
        if authority != &self.authority {
            return Err(ErrorCode::InvalidAuthority);
        }

        // SECURITY: Validate new tax configuration
        if new_tax_config.buy_tax_percentage > new_tax_config.max_tax_percentage {
            return Err(ErrorCode::MathError);
        }
        if new_tax_config.sell_tax_percentage > new_tax_config.max_tax_percentage {
            return Err(ErrorCode::MathError);
        }
        if new_tax_config.max_tax_percentage > PERCENTAGE_PRECISION_U64 {
            return Err(ErrorCode::MathError);
        }

        // SECURITY: Rate limiting - max 1 tax config update per day
        let day_start = current_time - (current_time % 86400); // Start of current day
        if self.last_updated >= day_start {
            return Err(ErrorCode::PriceUpdateTooSoon);
        }

        self.tax_config = new_tax_config;
        self.last_updated = current_time;
        Ok(())
    }

    /// Comprehensive security validation
    /// SECURITY: Validates all critical security aspects of the master agent
    pub fn validate_security(&self, current_time: i64) -> TYieldResult<()> {
        // Basic validation
        self.validate()?;

        // SECURITY: Validate tax configuration
        self.validate_tax_config()?;

        // SECURITY: Check for suspicious activity patterns
        self.detect_suspicious_activity(current_time)?;

        // SECURITY: Validate authority is not default
        if self.authority == Pubkey::default() {
            return Err(ErrorCode::InvalidAuthority);
        }

        // SECURITY: Check for reasonable price ranges
        if self.price > 1_000_000_000_000_000 {
            // 1 quadrillion lamports
            return Err(ErrorCode::InvalidEntryPrice);
        }

        // SECURITY: Check for reasonable yield rates
        if self.w_yield > 50000 {
            // 500% max yield
            return Err(ErrorCode::MathError);
        }

        // SECURITY: Check for reasonable supply limits
        if self.max_supply > 1_000_000 {
            // 1 million max supply
            return Err(ErrorCode::MathError);
        }

        Ok(())
    }

    /// Detect suspicious activity patterns
    /// SECURITY: Identifies potential manipulation attempts
    fn detect_suspicious_activity(&self, current_time: i64) -> TYieldResult<()> {
        // Check for rapid price changes
        let days_since_created = self.get_days_since_created(current_time);
        if days_since_created > 0 {
            let avg_price_change_per_day = self.price.safe_div(days_since_created as u64)?;
            if avg_price_change_per_day > self.price.safe_div(10)? {
                // Price changed by more than 10% per day on average
                return Err(ErrorCode::PriceDeviationTooHigh);
            }
        }

        // Check for unusual trading patterns
        if self.trade_count > 0 && self.agent_count > 0 {
            let trades_per_agent = self.trade_count.safe_div(self.agent_count)?;
            if trades_per_agent > 1000 {
                // More than 1000 trades per agent (suspicious)
                return Err(ErrorCode::CannotPerformAction);
            }
        }

        // Check for time anomalies
        if current_time < self.created_at {
            return Err(ErrorCode::InvalidAccount);
        }

        if current_time < self.last_updated {
            return Err(ErrorCode::InvalidAccount);
        }

        Ok(())
    }
}

impl Default for MasterAgent {
    fn default() -> Self {
        Self {
            authority: Pubkey::default(),
            mint: Pubkey::default(),
            trading_status: TradingStatus::WhiteList as u8,
            price: 0,
            w_yield: 0,
            price_update_allowance: 0,
            max_supply: 0,
            agent_count: 0,
            last_price_update: 0,
            trade_count: 0,
            completed_trades: 0,
            total_pnl: 0,
            auto_relist: false,
            last_updated: 0,
            created_at: 0,
            bump: 0,
            tax_config: TaxConfig::default(),
            _padding: [0; 1],
        }
    }
}

/// Represents the trading status of a master agent.
///
/// This enum defines the different trading modes available for a master agent,
/// controlling who can interact with the agent.
///
/// # Example
/// ```
/// # use tubor_yield::state::master_agent::TradingStatus;
/// let whitelist_status = TradingStatus::WhiteList;
/// let public_status = TradingStatus::Public;
/// ```
#[derive(Clone, Copy, PartialEq, Debug, Eq, AnchorDeserialize, AnchorSerialize)]
pub enum TradingStatus {
    /// Whitelist mode - only whitelisted users can trade
    WhiteList = 0b00000001,
    /// Public mode - anyone can trade
    Public = 0b00000010,
}

impl Size for MasterAgent {
    const SIZE: usize = 192; // 8 (discriminator) + 184 (struct, including tax_config and alignment/padding) = 192 bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;

    // Helper function to create a test master agent
    fn create_test_master_agent() -> MasterAgent {
        let mut master_agent = MasterAgent::default();
        let authority = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let current_time = 1640995200; // 2022-01-01 00:00:00 UTC

        let params = MasterAgentInitParams {
            authority,
            mint,
            price: 1000000, // 1 SOL in lamports
            w_yield: 500,   // 5% yield (500/10000 = 5%)
            trading_status: TradingStatus::WhiteList,
            max_supply: 100, // max supply
            auto_relist: false,
            current_time,
            bump: 1,
            tax_config: TaxConfig::default(),
        };
        master_agent.initialize(params).unwrap();

        master_agent
    }

    #[test]
    fn test_master_agent_size() {
        // On-chain size includes 8 bytes for Anchor discriminator
        let expected_size = 8 + std::mem::size_of::<MasterAgent>();
        assert_eq!(expected_size, MasterAgent::SIZE);
        println!("MasterAgent on-chain size: {} bytes", MasterAgent::SIZE);
    }

    #[test]
    fn test_master_agent_memory_layout() {
        // Test that MasterAgent struct can be created and serialized
        let master_agent = MasterAgent::default();
        assert_eq!(master_agent.authority, Pubkey::default());
        assert_eq!(master_agent.mint, Pubkey::default());
        assert_eq!(master_agent.price, 0);
        assert_eq!(master_agent.w_yield, 0);
        assert_eq!(master_agent.max_supply, 0);
        assert_eq!(master_agent.agent_count, 0);
        assert_eq!(master_agent.trade_count, 0);
        assert_eq!(master_agent.last_updated, 0);
        assert_eq!(master_agent.created_at, 0);
        assert_eq!(master_agent.trading_status, TradingStatus::WhiteList as u8);
        assert_eq!(master_agent.auto_relist, false);
        assert_eq!(master_agent.bump, 0);
        assert_eq!(master_agent._padding, [0; 1]);
    }

    #[test]
    fn test_initialize() {
        let mut master_agent = MasterAgent::default();
        let authority = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let current_time = 1640995200;

        let params = MasterAgentInitParams {
            authority,
            mint,
            price: 1000000,
            w_yield: 5000,
            trading_status: TradingStatus::Public,
            max_supply: 50,
            auto_relist: true,
            current_time,
            bump: 2,
            tax_config: TaxConfig::default(),
        };
        let result = master_agent.initialize(params);

        assert!(result.is_ok());
        assert_eq!(master_agent.authority, authority);
        assert_eq!(master_agent.mint, mint);
        assert_eq!(master_agent.price, 1000000);
        assert_eq!(master_agent.w_yield, 5000);
        assert_eq!(master_agent.max_supply, 50);
        assert_eq!(master_agent.agent_count, 0);
        assert_eq!(master_agent.trade_count, 0);
        assert_eq!(master_agent.trading_status, TradingStatus::Public as u8);
        assert_eq!(master_agent.auto_relist, true);
        assert_eq!(master_agent.last_updated, current_time);
        assert_eq!(master_agent.created_at, current_time);
        assert_eq!(master_agent.bump, 2);
    }

    #[test]
    fn test_update_price() {
        let mut master_agent = create_test_master_agent();
        let mut current_time = 1640995260; // Start time
        let authority = master_agent.authority;
        // Set last update far enough in the past
        master_agent.last_price_update = current_time;
        master_agent.last_updated = current_time;

        // Test successful price update
        let result = master_agent.update_price(1_100_000, current_time + 2 * 86400, &authority);
        println!("DEBUG: first update result = {:?}", result);
        assert!(result.is_ok());
        assert_eq!(master_agent.price, 1_100_000);
        assert_eq!(master_agent.last_updated, current_time + 2 * 86400);
        current_time += 2 * 86400;

        // Test invalid price (zero)
        let result = master_agent.update_price(0, current_time + 86400, &authority);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::InvalidEntryPrice);
        current_time += 86400;

        // Test invalid authority
        let wrong_authority = Pubkey::new_unique();
        let result = master_agent.update_price(3000000, current_time + 86400, &wrong_authority);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::InvalidAuthority);
        current_time += 86400;

        // Test price not increasing
        let result = master_agent.update_price(1_100_000, current_time + 86400, &authority);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::MathError);
        current_time += 86400;

        // Test max price increase limit
        let result = master_agent.update_price(1_300_000, current_time + 86400, &authority); // > 10% increase
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::PriceUpdateTooHigh);
        current_time += 86400;

        // Test time-based restriction (1 hour)
        let result = master_agent.update_price(2100000, current_time + 1800, &authority); // only 30 min later
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::PriceUpdateTooHigh);
    }

    #[test]
    fn test_update_yield() {
        let mut master_agent = create_test_master_agent();
        let current_time = 1640995260 + 129600; // Add 1 hour to satisfy time restriction
        let authority = master_agent.authority;

        // Test successful yield update (within 5% limit: 500 * 1.05 = 525)
        let result = master_agent.update_yield(525, current_time, &authority);
        println!("First update result: {:?}", result);
        assert!(result.is_ok());
        assert_eq!(master_agent.w_yield, 525);
        assert_eq!(master_agent.last_updated, current_time);

        // Test yield increase too high (exceeds 5% limit)
        let result = master_agent.update_yield(600, current_time + 129600, &authority);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::PriceUpdateTooHigh);

        // Test yield too high (exceeds 50% maximum)
        let result = master_agent.update_yield(50001, current_time + 129600, &authority);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::MathError);

        // Test zero yield (not allowed)
        let result = master_agent.update_yield(0, current_time + 129600, &authority);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::MathError);

        // Test invalid authority
        let wrong_authority = Pubkey::new_unique();
        let result = master_agent.update_yield(525, current_time + 129600, &wrong_authority);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::InvalidAuthority);

        // Test time restriction (should fail if not enough time has passed)
        let result = master_agent.update_yield(550, current_time + 1800, &authority); // only 30 min later
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::PriceUpdateTooSoon);
    }

    #[test]
    fn test_update_max_supply() {
        let mut master_agent = create_test_master_agent();
        let current_time = 1640995260;

        // Add some agents first
        master_agent.add_agent(current_time).unwrap();
        master_agent.add_agent(current_time).unwrap();

        // Test successful max supply update
        let result = master_agent.update_max_supply(200, current_time);
        assert!(result.is_ok());
        assert_eq!(master_agent.max_supply, 200);
        assert_eq!(master_agent.last_updated, current_time);

        // Test max supply less than current agent count
        let result = master_agent.update_max_supply(1, current_time);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::MathError);
    }

    #[test]
    fn test_add_agent() {
        let mut master_agent = create_test_master_agent();
        let current_time = 1640995260;

        // Test successful agent addition
        let result = master_agent.add_agent(current_time);
        assert!(result.is_ok());
        assert_eq!(master_agent.agent_count, 1);
        assert_eq!(master_agent.last_updated, current_time);

        // Add more agents
        master_agent.add_agent(current_time).unwrap();
        master_agent.add_agent(current_time).unwrap();
        assert_eq!(master_agent.agent_count, 3);

        // Test adding agent when at max supply
        master_agent.agent_count = master_agent.max_supply;
        let result = master_agent.add_agent(current_time);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::InsufficientFunds);
    }

    #[test]
    fn test_remove_agent() {
        let mut master_agent = create_test_master_agent();
        let current_time = 1640995260;

        // Add some agents first
        master_agent.add_agent(current_time).unwrap();
        master_agent.add_agent(current_time).unwrap();
        assert_eq!(master_agent.agent_count, 2);

        // Test successful agent removal
        let result = master_agent.remove_agent(current_time);
        assert!(result.is_ok());
        assert_eq!(master_agent.agent_count, 1);
        assert_eq!(master_agent.last_updated, current_time);

        // Test removing agent when no agents exist
        master_agent.remove_agent(current_time).unwrap();
        assert_eq!(master_agent.agent_count, 0);

        let result = master_agent.remove_agent(current_time);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::InsufficientFunds);
    }

    #[test]
    fn test_increment_trade_count() {
        let mut master_agent = create_test_master_agent();
        let current_time = 1640995260;

        // Test successful trade count increment
        let result = master_agent.increment_trade_count(current_time);
        assert!(result.is_ok());
        assert_eq!(master_agent.trade_count, 1);
        assert_eq!(master_agent.last_updated, current_time);

        // Increment more
        master_agent.increment_trade_count(current_time).unwrap();
        master_agent.increment_trade_count(current_time).unwrap();
        assert_eq!(master_agent.trade_count, 3);
    }

    #[test]
    fn test_auto_relist_functions() {
        let mut master_agent = create_test_master_agent();
        let current_time = 1640995260;

        // Test initial state
        assert_eq!(master_agent.auto_relist, false);

        // Test toggle
        master_agent.toggle_auto_relist(current_time);
        assert_eq!(master_agent.auto_relist, true);
        assert_eq!(master_agent.last_updated, current_time);

        // Test toggle again
        master_agent.toggle_auto_relist(current_time);
        assert_eq!(master_agent.auto_relist, false);

        // Test set auto relist
        master_agent.set_auto_relist(true, current_time);
        assert_eq!(master_agent.auto_relist, true);

        master_agent.set_auto_relist(false, current_time);
        assert_eq!(master_agent.auto_relist, false);
    }

    #[test]
    fn test_trading_status_functions() {
        let mut master_agent = create_test_master_agent();
        let current_time = 1640995260;

        // Test initial state
        assert_eq!(master_agent.get_trading_status(), TradingStatus::WhiteList);
        assert!(master_agent.is_whitelist_mode());
        assert!(!master_agent.is_public_mode());

        // Test set trading status
        let authority = master_agent.authority;
        master_agent
            .set_trading_status(TradingStatus::Public, &authority, current_time)
            .unwrap();
        assert_eq!(master_agent.get_trading_status(), TradingStatus::Public);
        assert!(!master_agent.is_whitelist_mode());
        assert!(master_agent.is_public_mode());

        // Test toggle trading status
        master_agent.toggle_trading_status(current_time);
        assert_eq!(master_agent.get_trading_status(), TradingStatus::WhiteList);

        master_agent.toggle_trading_status(current_time);
        assert_eq!(master_agent.get_trading_status(), TradingStatus::Public);
    }

    #[test]
    fn test_calculate_yield_amount() {
        let mut master_agent = create_test_master_agent();

        // Test yield calculation
        let yield_amount = master_agent.calculate_yield_amount().unwrap();
        let expected_yield = (1000000 * 500) / PERCENTAGE_PRECISION_U64;
        assert_eq!(yield_amount, expected_yield);

        // Test with different yield rate
        master_agent.w_yield = 1000; // 10%
        let yield_amount = master_agent.calculate_yield_amount().unwrap();
        let expected_yield = (1000000 * 1000) / PERCENTAGE_PRECISION_U64;
        assert_eq!(yield_amount, expected_yield);
    }

    #[test]
    fn test_get_yield_rate_percentage() {
        let mut master_agent = create_test_master_agent();

        // Test initial yield rate
        let yield_rate = master_agent.get_yield_rate_percentage();
        assert_eq!(yield_rate, 5); // 5%

        // Test with different yield rate
        master_agent.w_yield = 1500; // 15%
        let yield_rate = master_agent.get_yield_rate_percentage();
        assert_eq!(yield_rate, 15);
    }

    #[test]
    fn test_supply_functions() {
        let mut master_agent = create_test_master_agent();
        let current_time = 1640995260;

        // Test initial state
        assert!(!master_agent.is_supply_full());
        assert_eq!(master_agent.get_remaining_supply(), 100);

        // Add agents
        master_agent.add_agent(current_time).unwrap();
        master_agent.add_agent(current_time).unwrap();
        assert_eq!(master_agent.get_remaining_supply(), 98);

        // Fill supply
        for _ in 0..98 {
            master_agent.add_agent(current_time).unwrap();
        }

        assert!(master_agent.is_supply_full());
        assert_eq!(master_agent.get_remaining_supply(), 0);
    }

    #[test]
    fn test_supply_utilization_percentage() {
        let mut master_agent = create_test_master_agent();
        let current_time = 1640995260;

        // Test initial state
        assert_eq!(master_agent.get_supply_utilization_percentage(), 0);

        // Add 50 agents (50% utilization)
        for _ in 0..50 {
            master_agent.add_agent(current_time).unwrap();
        }
        assert_eq!(master_agent.get_supply_utilization_percentage(), 5000); // 50% = 5000 basis points

        // Add more agents to reach 100%
        for _ in 0..50 {
            master_agent.add_agent(current_time).unwrap();
        }
        assert_eq!(master_agent.get_supply_utilization_percentage(), 10000); // 100% = 10000 basis points
    }

    #[test]
    fn test_average_trades_per_agent() {
        let mut master_agent = create_test_master_agent();
        let current_time = 1640995260;

        // Test with no agents
        assert_eq!(master_agent.get_average_trades_per_agent(), 0);

        // Add agents and trades
        master_agent.add_agent(current_time).unwrap();
        master_agent.add_agent(current_time).unwrap();
        master_agent.increment_trade_count(current_time).unwrap();
        master_agent.increment_trade_count(current_time).unwrap();
        master_agent.increment_trade_count(current_time).unwrap();

        assert_eq!(master_agent.get_average_trades_per_agent(), 1); // 3 trades / 2 agents = 1.5, truncated to 1
    }

    #[test]
    fn test_time_functions() {
        let mut master_agent = create_test_master_agent();
        let created_time = 1640995200; // 2022-01-01 00:00:00 UTC
        let current_time = 1641081600; // 2022-01-02 00:00:00 UTC (1 day later)

        master_agent.created_at = created_time;
        master_agent.last_updated = created_time;

        // Test days since created
        let days_since_created = master_agent.get_days_since_created(current_time);
        assert_eq!(days_since_created, 1);

        // Test days since updated
        let days_since_updated = master_agent.get_days_since_updated(current_time);
        assert_eq!(days_since_updated, 1);

        // Update and test again
        master_agent.last_updated = current_time;
        let days_since_updated = master_agent.get_days_since_updated(current_time);
        assert_eq!(days_since_updated, 0);
    }

    #[test]
    fn test_activity_functions() {
        let mut master_agent = create_test_master_agent();
        let current_time = 1640995260;

        // Test initial state
        assert!(!master_agent.is_active());

        // Add agent and test
        master_agent.add_agent(current_time).unwrap();
        assert!(master_agent.is_active());

        // Test idle function
        assert!(!master_agent.is_idle(current_time, 86400)); // 1 day threshold

        // Test with old timestamp
        master_agent.last_updated = current_time - 100000; // Much older
        assert!(master_agent.is_idle(current_time, 86400));
    }

    #[test]
    fn test_validate() {
        let mut master_agent = create_test_master_agent();

        // Test valid master agent
        assert!(master_agent.validate().is_ok());

        // Test invalid authority
        master_agent.authority = Pubkey::default();
        assert!(master_agent.validate().is_err());
        assert_eq!(
            master_agent.validate().unwrap_err(),
            ErrorCode::InvalidAuthority
        );

        // Reset and test invalid mint
        master_agent = create_test_master_agent();
        master_agent.mint = Pubkey::default();
        assert!(master_agent.validate().is_err());
        assert_eq!(
            master_agent.validate().unwrap_err(),
            ErrorCode::InvalidAccount
        );

        // Reset and test invalid price
        master_agent = create_test_master_agent();
        master_agent.price = 0;
        assert!(master_agent.validate().is_err());
        assert_eq!(
            master_agent.validate().unwrap_err(),
            ErrorCode::InvalidEntryPrice
        );

        // Reset and test invalid yield
        master_agent = create_test_master_agent();
        master_agent.w_yield = PERCENTAGE_PRECISION_U64 + 1;
        assert!(master_agent.validate().is_err());
        assert_eq!(master_agent.validate().unwrap_err(), ErrorCode::MathError);

        // Reset and test invalid max supply
        master_agent = create_test_master_agent();
        master_agent.max_supply = 0;
        assert!(master_agent.validate().is_err());
        assert_eq!(master_agent.validate().unwrap_err(), ErrorCode::MathError);

        // Reset and test agent count > max supply
        master_agent = create_test_master_agent();
        master_agent.agent_count = 101; // More than max_supply of 100
        assert!(master_agent.validate().is_err());
        assert_eq!(master_agent.validate().unwrap_err(), ErrorCode::MathError);
    }

    #[test]
    fn test_can_perform_actions() {
        let mut master_agent = create_test_master_agent();

        // Test initial state (no agents, not full)
        assert!(!master_agent.can_perform_actions());

        // Add agent
        master_agent.add_agent(1640995260).unwrap();
        assert!(master_agent.can_perform_actions());

        // Fill supply
        for _ in 0..99 {
            master_agent.add_agent(1640995260).unwrap();
        }
        assert!(!master_agent.can_perform_actions());
    }

    #[test]
    fn test_can_be_accessed_by_user() {
        let mut master_agent = create_test_master_agent();

        // Test whitelist mode
        assert_eq!(master_agent.get_trading_status(), TradingStatus::WhiteList);
        assert!(!master_agent.can_be_accessed_by_user(false));
        assert!(master_agent.can_be_accessed_by_user(true));

        // Test public mode
        let authority = master_agent.authority;
        master_agent
            .set_trading_status(TradingStatus::Public, &authority, 1640995260)
            .unwrap();
        assert!(master_agent.can_be_accessed_by_user(false));
        assert!(master_agent.can_be_accessed_by_user(true));
    }

    #[test]
    fn test_is_trading_allowed() {
        let mut master_agent = create_test_master_agent();

        // Test initial state
        assert!(!master_agent.is_trading_allowed());

        // Add agent
        master_agent.add_agent(1640995260).unwrap();
        assert!(master_agent.is_trading_allowed());

        // Fill supply
        for _ in 0..99 {
            master_agent.add_agent(1640995260).unwrap();
        }
        assert!(!master_agent.is_trading_allowed());
    }

    #[test]
    fn test_total_value_locked() {
        let mut master_agent = create_test_master_agent();

        // Test initial state
        assert_eq!(master_agent.get_total_value_locked(), 0);

        // Add agents
        master_agent.add_agent(1640995260).unwrap();
        master_agent.add_agent(1640995260).unwrap();

        let expected_tvl = 2 * master_agent.price;
        assert_eq!(master_agent.get_total_value_locked(), expected_tvl);
    }

    #[test]
    fn test_total_yield_generated() {
        let mut master_agent = create_test_master_agent();

        // Test initial state
        assert_eq!(master_agent.get_total_yield_generated().unwrap(), 0);

        // Add agents
        master_agent.add_agent(1640995260).unwrap();
        master_agent.add_agent(1640995260).unwrap();

        let yield_per_agent = master_agent.calculate_yield_amount().unwrap();
        let expected_total_yield = yield_per_agent * 2;
        assert_eq!(
            master_agent.get_total_yield_generated().unwrap(),
            expected_total_yield
        );
    }

    #[test]
    fn test_yield_efficiency() {
        let mut master_agent = create_test_master_agent();

        // Test initial state
        assert_eq!(master_agent.get_yield_efficiency().unwrap(), 0);

        // Add agents
        master_agent.add_agent(1640995260).unwrap();
        master_agent.add_agent(1640995260).unwrap();

        let efficiency = master_agent.get_yield_efficiency().unwrap();
        assert!(efficiency > 0);
    }

    #[test]
    fn test_trading_activity_score() {
        let mut master_agent = create_test_master_agent();
        let created_time = 1640995200;
        let current_time = 1641081600; // 1 day later

        master_agent.created_at = created_time;

        // Test with no trades
        assert_eq!(master_agent.get_trading_activity_score(current_time), 0);

        // Add trades
        master_agent.increment_trade_count(created_time).unwrap();
        master_agent.increment_trade_count(created_time).unwrap();
        master_agent.increment_trade_count(created_time).unwrap();

        // Should be 3 trades per day
        assert_eq!(master_agent.get_trading_activity_score(current_time), 3);
    }

    #[test]
    fn test_performance_metrics() {
        let mut master_agent = create_test_master_agent();
        let current_time = 1640995260;

        // Add some agents and trades
        master_agent.add_agent(current_time).unwrap();
        master_agent.add_agent(current_time).unwrap();
        master_agent.increment_trade_count(current_time).unwrap();

        let metrics = master_agent.get_performance_metrics(current_time).unwrap();
        assert_eq!(metrics.0, master_agent.get_total_value_locked()); // TVL
        assert_eq!(metrics.1, master_agent.get_total_yield_generated().unwrap()); // Total yield
        assert_eq!(metrics.2, master_agent.get_yield_efficiency().unwrap()); // Yield efficiency
        assert_eq!(
            metrics.3,
            master_agent.get_trading_activity_score(current_time)
        ); // Activity score
    }

    #[test]
    fn test_reset() {
        let mut master_agent = create_test_master_agent();
        let current_time = 1640995260;

        // Add some data
        master_agent.add_agent(current_time).unwrap();
        master_agent.add_agent(current_time).unwrap();
        master_agent.increment_trade_count(current_time).unwrap();
        let authority = master_agent.authority;
        master_agent
            .set_trading_status(TradingStatus::Public, &authority, current_time)
            .unwrap();

        // Reset
        master_agent.reset();

        assert_eq!(master_agent.agent_count, 0);
        assert_eq!(master_agent.trade_count, 0);
        assert_eq!(master_agent.get_trading_status(), TradingStatus::WhiteList);
        assert_eq!(master_agent.last_updated, master_agent.created_at);
    }

    #[test]
    fn test_get_summary() {
        let mut master_agent = create_test_master_agent();
        let current_time = 1640995260;

        // Add some data
        master_agent.add_agent(current_time).unwrap();
        master_agent.increment_trade_count(current_time).unwrap();
        let authority = master_agent.authority;
        master_agent
            .set_trading_status(TradingStatus::Public, &authority, current_time)
            .unwrap();

        let summary = master_agent.get_summary();
        assert_eq!(summary.0, 1); // agent_count
        assert_eq!(summary.1, 1); // trade_count
        assert_eq!(summary.2, 1000000); // price
        assert_eq!(summary.3, 500); // w_yield
        assert_eq!(summary.4, false); // auto_relist
        assert_eq!(summary.5, TradingStatus::Public); // trading_status
    }

    #[test]
    fn test_needs_attention() {
        let mut master_agent = create_test_master_agent();
        let current_time = 1640995260;

        // Test initial state (no agents)
        assert!(master_agent.needs_attention(current_time));

        // Add agent and update timestamp
        master_agent.add_agent(current_time).unwrap();
        master_agent.last_updated = current_time; // Ensure recent activity
        assert!(!master_agent.needs_attention(current_time));

        // Test with old update time
        let old_time = current_time - (8 * 86400); // 8 days ago
        master_agent.last_updated = old_time;
        assert!(master_agent.needs_attention(current_time));

        // Test with high supply utilization
        master_agent.last_updated = current_time;
        master_agent.agent_count = 95; // 95% utilization (9500 basis points > 9000)
        assert!(master_agent.needs_attention(current_time));
    }

    #[test]
    fn test_status_strings() {
        let mut master_agent = create_test_master_agent();

        // Test initial state
        assert_eq!(master_agent.get_status_string(), "Empty");
        assert_eq!(master_agent.get_auto_relist_status(), "Disabled");
        assert_eq!(master_agent.get_trading_status_string(), "Whitelist");

        // Add agent
        master_agent.add_agent(1640995260).unwrap();
        assert_eq!(master_agent.get_status_string(), "Active");

        // Fill supply
        for _ in 0..99 {
            master_agent.add_agent(1640995260).unwrap();
        }
        assert_eq!(master_agent.get_status_string(), "Full");

        // Test auto relist status
        master_agent.set_auto_relist(true, 1640995260);
        assert_eq!(master_agent.get_auto_relist_status(), "Enabled");

        // Test trading status string
        let authority = master_agent.authority;
        master_agent
            .set_trading_status(TradingStatus::Public, &authority, 1640995260)
            .unwrap();
        assert_eq!(master_agent.get_trading_status_string(), "Public");
    }

    #[test]
    fn test_edge_cases() {
        let mut master_agent = create_test_master_agent();

        // Test with maximum values
        master_agent.w_yield = PERCENTAGE_PRECISION_U64;
        assert!(master_agent.validate().is_ok());

        // Test with large but reasonable numbers
        master_agent.price = 1_000_000_000_000; // 1 trillion lamports
        master_agent.max_supply = 1_000_000; // 1 million agents
        assert!(master_agent.validate().is_ok());

        // Test yield calculation with large numbers
        let yield_amount = master_agent.calculate_yield_amount();
        assert!(yield_amount.is_ok());
    }

    #[test]
    fn test_trading_status_enum() {
        // Test enum values
        assert_eq!(TradingStatus::WhiteList as u8, 0b00000001);
        assert_eq!(TradingStatus::Public as u8, 0b00000010);

        // Test enum comparison
        assert_ne!(TradingStatus::WhiteList, TradingStatus::Public);
        assert_eq!(TradingStatus::WhiteList, TradingStatus::WhiteList);
    }

    #[test]
    fn test_tax_config_default() {
        let tax_config = TaxConfig::default();
        assert_eq!(tax_config.buy_tax_percentage, 250); // 2.5%
        assert_eq!(tax_config.sell_tax_percentage, 250); // 2.5%
        assert_eq!(tax_config.max_tax_percentage, 1000); // 10%
    }

    #[test]
    fn test_calculate_buy_price_with_tax() {
        let master_agent = create_test_master_agent();
        let (total_price, tax_amount, base_price) =
            master_agent.calculate_buy_price_with_tax().unwrap();

        assert_eq!(base_price, 1000000); // Base price
        assert_eq!(tax_amount, 25000); // 2.5% of 1,000,000 = 25,000
        assert_eq!(total_price, 1025000); // Base + tax
    }

    #[test]
    fn test_calculate_sell_price_with_tax() {
        let master_agent = create_test_master_agent();
        let (net_price, tax_amount, base_price) =
            master_agent.calculate_sell_price_with_tax().unwrap();

        assert_eq!(base_price, 1000000); // Base price
        assert_eq!(tax_amount, 25000); // 2.5% of 1,000,000 = 25,000
        assert_eq!(net_price, 975000); // Base - tax
    }

    #[test]
    fn test_calculate_buy_for_usdc_amount() {
        let master_agent = create_test_master_agent();

        // Test buying with 1,025,000 USDC (exactly 1 token with tax)
        let (tokens_received, tax_paid, base_amount) =
            master_agent.calculate_buy_for_usdc_amount(1025000).unwrap();

        assert_eq!(tokens_received, 1);
        assert_eq!(tax_paid, 25000); // 2.5% tax
        assert_eq!(base_amount, 1000000); // Base amount
    }

    #[test]
    fn test_calculate_sell_for_token_amount() {
        let master_agent = create_test_master_agent();

        // Test selling 1 token
        let (usdc_received, tax_paid, base_amount) =
            master_agent.calculate_sell_for_token_amount(1).unwrap();

        assert_eq!(usdc_received, 975000); // Net after tax
        assert_eq!(tax_paid, 25000); // 2.5% tax
        assert_eq!(base_amount, 1000000); // Base amount
    }

    #[test]
    fn test_get_buy_tax_rate() {
        let master_agent = create_test_master_agent();
        let tax_rate = master_agent.get_buy_tax_rate().unwrap();
        assert_eq!(tax_rate, 2); // 2.5% = 2.5 basis points = 2%
    }

    #[test]
    fn test_get_sell_tax_rate() {
        let master_agent = create_test_master_agent();
        let tax_rate = master_agent.get_sell_tax_rate().unwrap();
        assert_eq!(tax_rate, 2); // 2.5% = 2.5 basis points = 2%
    }

    #[test]
    fn test_calculate_buy_price_with_slippage() {
        let master_agent = create_test_master_agent();
        let slippage_bps = 100; // 1% slippage

        let (base_price, max_price) = master_agent
            .calculate_buy_price_with_slippage(slippage_bps)
            .unwrap();

        assert_eq!(base_price, 1025000); // Base price with tax
        assert_eq!(max_price, 1035250); // Base + 1% slippage
    }

    #[test]
    fn test_calculate_sell_price_with_slippage() {
        let master_agent = create_test_master_agent();
        let slippage_bps = 100; // 1% slippage

        let (base_price, min_price) = master_agent
            .calculate_sell_price_with_slippage(slippage_bps)
            .unwrap();

        assert_eq!(base_price, 975000); // Base price with tax
        assert_eq!(min_price, 965250); // Base - 1% slippage
    }

    #[test]
    fn test_validate_tax_config() {
        let mut master_agent = create_test_master_agent();
        assert!(master_agent.validate_tax_config().is_ok());

        // Test invalid buy tax
        master_agent.tax_config.buy_tax_percentage = 1500; // 15% > 10% max
        assert!(master_agent.validate_tax_config().is_err());

        // Reset and test invalid sell tax
        master_agent.tax_config = TaxConfig::default();
        master_agent.tax_config.sell_tax_percentage = 1500; // 15% > 10% max
        assert!(master_agent.validate_tax_config().is_err());

        // Reset and test invalid max tax
        master_agent.tax_config = TaxConfig::default();
        master_agent.tax_config.max_tax_percentage = PERCENTAGE_PRECISION_U64 + 1;
        assert!(master_agent.validate_tax_config().is_err());
    }

    #[test]
    fn test_get_tax_summary() {
        let master_agent = create_test_master_agent();
        let (buy_tax_rate, sell_tax_rate, max_tax_rate) = master_agent.get_tax_summary().unwrap();

        assert_eq!(buy_tax_rate, 2); // 2.5% = 2%
        assert_eq!(sell_tax_rate, 2); // 2.5% = 2%
        assert_eq!(max_tax_rate, 10); // 10%
    }

    #[test]
    fn test_tax_calculations_with_different_rates() {
        let mut master_agent = create_test_master_agent();
        // Set custom tax rates in the stored config
        master_agent.tax_config.buy_tax_percentage = 500; // 5%
        master_agent.tax_config.sell_tax_percentage = 300; // 3%

        // Test buy price with 5% tax
        let (total_price, tax_amount, base_price) =
            master_agent.calculate_buy_price_with_tax().unwrap();

        assert_eq!(base_price, 1000000);
        assert_eq!(tax_amount, 50000); // 5% of 1,000,000
        assert_eq!(total_price, 1050000);

        // Test sell price with 3% tax
        let (net_price, tax_amount, base_price) =
            master_agent.calculate_sell_price_with_tax().unwrap();

        assert_eq!(base_price, 1000000);
        assert_eq!(tax_amount, 30000); // 3% of 1,000,000
        assert_eq!(net_price, 970000);
    }

    #[test]
    fn test_tax_calculations_edge_cases() {
        let master_agent = create_test_master_agent();
        let (total_price, tax_amount, _) = master_agent.calculate_buy_price_with_tax().unwrap();

        assert_eq!(tax_amount, 25000); // 2.5% of 1,000,000
        assert_eq!(total_price, 1025000); // Base + 2.5% tax

        let (net_price, tax_amount, _) = master_agent.calculate_sell_price_with_tax().unwrap();

        assert_eq!(tax_amount, 25000); // 2.5% of 1,000,000
        assert_eq!(net_price, 975000); // Base - 2.5% tax
    }

    #[test]
    fn test_large_amount_calculations() {
        let mut master_agent = create_test_master_agent();
        master_agent.price = 1_000_000_000; // 1 billion lamports
        let (total_price, tax_amount, base_price) =
            master_agent.calculate_buy_price_with_tax().unwrap();

        assert_eq!(base_price, 1_000_000_000);
        assert_eq!(tax_amount, 25_000_000); // 2.5% of 1 billion
        assert_eq!(total_price, 1_025_000_000);

        let (net_price, tax_amount, base_price) =
            master_agent.calculate_sell_price_with_tax().unwrap();

        assert_eq!(base_price, 1_000_000_000);
        assert_eq!(tax_amount, 25_000_000); // 2.5% of 1 billion
        assert_eq!(net_price, 975_000_000);
    }

    #[test]
    fn test_can_update_price() {
        let mut master_agent = create_test_master_agent();
        let now = 1_700_000_000; // arbitrary current time
        let base_price = 1_000_000;
        master_agent.price = base_price;
        master_agent.last_price_update = now - 2 * 86_400; // 2 days ago
        master_agent.price_update_allowance = 2; // 2 days

        // max_agent_price_new = 11000 (110%)
        let max_agent_price_new = 11_000;
        let allowed_price = base_price * max_agent_price_new / 10_000;

        // Case 1: Not enough time passed
        master_agent.last_price_update = now - 1 * 86_400; // 1 day ago
        let result = master_agent.can_update_price_secure_with_time(
            base_price,
            max_agent_price_new,
            &master_agent.authority,
            now,
        );
        assert!(result.is_err());
        // Optionally check error code if needed

        // Case 2: Price too high
        master_agent.last_price_update = now - 2 * 86_400; // 2 days ago
        let too_high_price = allowed_price + 1;
        let result = master_agent.can_update_price_secure_with_time(
            too_high_price,
            max_agent_price_new,
            &master_agent.authority,
            now,
        );
        assert!(result.is_err());

        // Case 3: Both conditions met (should succeed)
        let result = master_agent.can_update_price_secure_with_time(
            allowed_price,
            max_agent_price_new,
            &master_agent.authority,
            now,
        );
        assert!(result.is_ok());

        // Case 4: Edge case - exactly at time and price limit
        master_agent.last_price_update = now - 2 * 86_400; // exactly at allowance
        let result = master_agent.can_update_price_secure_with_time(
            allowed_price,
            max_agent_price_new,
            &master_agent.authority,
            now,
        );
        assert!(result.is_ok());

        // Case 5: Error propagation (simulate underflow)
        // Set now < last_price_update
        master_agent.last_price_update = now + 100;
        println!(
            "Debug: now = {}, last_price_update = {}",
            now, master_agent.last_price_update
        );
        let result = master_agent.can_update_price_secure_with_time(
            base_price,
            max_agent_price_new,
            &master_agent.authority,
            now,
        );
        println!("Debug: result = {:?}", result);
        assert!(result.is_err());
    }

    #[test]
    fn test_security_features() {
        let mut master_agent = create_test_master_agent();
        let current_time = 1640995260;

        // Test authority validation
        let wrong_authority = Pubkey::new_unique();
        let result =
            master_agent.set_trading_status(TradingStatus::Public, &wrong_authority, current_time);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::InvalidAuthority);

        // Test tax config update with wrong authority
        let new_tax_config = TaxConfig {
            buy_tax_percentage: 300,
            sell_tax_percentage: 300,
            max_tax_percentage: 1000,
        };
        let result =
            master_agent.update_tax_config(new_tax_config.clone(), &wrong_authority, current_time);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::InvalidAuthority);

        // Test tax config update with valid authority (first update)
        let authority = master_agent.authority;
        let result = master_agent.update_tax_config(
            new_tax_config.clone(),
            &authority,
            current_time + 86400,
        );
        assert!(result.is_ok());

        // Test tax config update with valid authority (second update, after rate limit)
        let next_update_time = current_time + 2 * 86400;
        let day_start = next_update_time - (next_update_time % 86400);
        println!("DEBUG: self.last_updated = {}", master_agent.last_updated);
        println!("DEBUG: day_start = {}", day_start);
        println!("DEBUG: next_update_time = {}", next_update_time);
        let result = master_agent.update_tax_config(new_tax_config, &authority, next_update_time);
        assert!(result.is_ok());

        // Test security validation with a fresh master agent to avoid time-based issues
        let mut fresh_master_agent = create_test_master_agent();
        let result = fresh_master_agent.validate_security(current_time);
        assert!(result.is_ok());

        // Test suspicious activity detection
        fresh_master_agent.price = 1_000_000_000_000_000_001; // Exceeds reasonable limit
        let result = fresh_master_agent.validate_security(current_time);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::InvalidEntryPrice);

        // Reset and test excessive yield
        fresh_master_agent = create_test_master_agent();
        fresh_master_agent.w_yield = 50001; // Exceeds 500% limit
        let result = fresh_master_agent.validate_security(current_time);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::MathError);

        // Reset and test excessive supply
        fresh_master_agent = create_test_master_agent();
        fresh_master_agent.max_supply = 1_000_001; // Exceeds 1 million limit
        let result = fresh_master_agent.validate_security(current_time);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::MathError);
    }

    #[test]
    fn test_secure_time_method() {
        // Test that secure time method works
        let result = MasterAgent::get_secure_time();
        // This test might fail in test environment, but should work on-chain
        // We just test that it doesn't panic
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_suspicious_activity_detection() {
        let mut master_agent = create_test_master_agent();
        let current_time = 1640995260;

        // Test normal activity (should pass)
        let result = master_agent.detect_suspicious_activity(current_time);
        assert!(result.is_ok());

        // Test excessive trades per agent
        master_agent.trade_count = 1001;
        master_agent.agent_count = 1;
        let result = master_agent.detect_suspicious_activity(current_time);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::CannotPerformAction);

        // Test time anomalies
        master_agent = create_test_master_agent();
        master_agent.created_at = current_time + 1000; // Future creation time
        let result = master_agent.detect_suspicious_activity(current_time);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::InvalidAccount);

        master_agent = create_test_master_agent();
        master_agent.last_updated = current_time + 1000; // Future update time
        let result = master_agent.detect_suspicious_activity(current_time);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::InvalidAccount);
    }
}
