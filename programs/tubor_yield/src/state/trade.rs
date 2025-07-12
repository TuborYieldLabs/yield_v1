use anchor_lang::prelude::*;

use crate::error::{ErrorCode, TYieldResult};
use crate::math::safe_math::SafeMath;
use crate::math::PERCENTAGE_PRECISION_U64;
use crate::state::{OraclePrice, Size};

#[account]
#[derive(Eq, PartialEq, Debug, Default)]
pub struct Trade {
    // 8-byte aligned fields first
    pub master_agent: Pubkey, // 32 bytes (8-byte aligned)
    pub feed_id: [u8; 32],    // 32 bytes
    pub pair: [u8; 8],        // 8 bytes
    pub size: u64,            // 8 bytes
    pub entry_price: u64,     // 8 bytes
    pub take_profit: u64,     // 8 bytes
    pub stop_loss: u64,       // 8 bytes
    pub created_at: i64,      // 4 bytes
    pub updated_at: i64,      // 4 bytes
    pub status: u8,           // 1 byte
    pub trade_type: u8,       // 1 byte
    pub result: u8,           // 1 byte
    pub bump: u8,             // 1 byte
    pub _padding: [u8; 4],    // 4 bytes padding for future-proofing and alignment
}

#[derive(Clone, Copy, PartialEq, Debug, Eq, AnchorDeserialize, AnchorSerialize)]
pub enum TradeStatus {
    Active = 0b00000001,
    Completed = 0b00000010,
    Cancelled = 0b00000100,
}

#[derive(Clone, Copy, PartialEq, Debug, Eq, AnchorDeserialize, AnchorSerialize)]
pub enum TradeType {
    Buy = 0b00000001,
    Sell = 0b00000010,
}

#[derive(Clone, Copy, PartialEq, Debug, Eq, AnchorDeserialize, AnchorSerialize)]
pub enum TradeResult {
    Success = 0b00000001,
    Failed = 0b00000010,
    Pending = 0b00000100,
}

impl Size for Trade {
    const SIZE: usize = 136;
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

/// Comprehensive price validation parameters
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
    /// Returns the trade status as an enum
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

    /// Sets the trade status
    pub fn set_status(&mut self, status: TradeStatus) {
        self.status = status as u8;
    }

    /// Sets the trade result
    pub fn set_result(&mut self, result: TradeResult) {
        self.result = result as u8;
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

    /// Validates the trade parameters
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

    /// Calculates the potential profit/loss at a given price with proper error handling
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

    /// Completes the trade with a given result
    pub fn complete(&mut self, result: TradeResult) {
        self.set_status(TradeStatus::Completed);
        self.set_result(result);
    }

    /// Cancels the trade
    pub fn cancel(&mut self) {
        self.set_status(TradeStatus::Cancelled);
        self.set_result(TradeResult::Failed);
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
        self._padding = [0; 4];
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
        self.set_status(status);
        self.set_result(result);
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

    /// Comprehensive price validation for trade execution
    pub fn validate_trade_execution(
        &self,
        current_price: u64,
        max_slippage_bps: u64,
        min_distance_bps: u64,
        min_risk_reward_bps: u64,
    ) -> TYieldResult<()> {
        // Basic trade validation
        self.validate()?;

        // Price slippage validation
        self.validate_price_with_slippage(current_price, max_slippage_bps)?;

        // Risk management levels validation
        self.validate_risk_management_levels(min_distance_bps)?;

        // Risk-reward ratio validation
        self.validate_risk_reward_ratio(min_risk_reward_bps)?;

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
            _padding: [0; 4],
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
            _padding: [0; 4],
        }
    }

    #[test]
    fn test_trade_size() {
        // On-chain size includes 8 bytes for Anchor discriminator
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
            _padding: [0; 4],
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
        assert_eq!(trade._padding, [0; 4]);
    }

    #[test]
    fn test_trade_status_enum_conversion() {
        let mut trade = create_valid_buy_trade();

        // Test get_status
        assert_eq!(trade.get_status(), TradeStatus::Active);
        assert_eq!(trade.get_trade_type(), TradeType::Buy);
        assert_eq!(trade.get_result(), TradeResult::Pending);

        // Test set_status
        trade.set_status(TradeStatus::Completed);
        assert_eq!(trade.get_status(), TradeStatus::Completed);

        trade.set_status(TradeStatus::Cancelled);
        assert_eq!(trade.get_status(), TradeStatus::Cancelled);

        // Test set_result
        trade.set_result(TradeResult::Success);
        assert_eq!(trade.get_result(), TradeResult::Success);

        trade.set_result(TradeResult::Failed);
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
        trade.set_status(TradeStatus::Completed);
        assert!(!trade.is_active());
        assert!(trade.is_completed());
        assert!(!trade.is_cancelled());

        // Test cancelled
        trade.set_status(TradeStatus::Cancelled);
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
        trade.set_status(TradeStatus::Completed);
        let unrealized_pnl = trade.calculate_unrealized_pnl(1100);
        assert!(unrealized_pnl.is_ok());
        assert_eq!(unrealized_pnl.unwrap(), 0);

        // Cancelled trade should return 0
        trade.set_status(TradeStatus::Cancelled);
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
            _padding: [255; 4],
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
            _padding: [0; 4],
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
        let max_slippage_bps = 100;
        let min_distance_bps = 100;
        let min_risk_reward_bps = 100;
        let result = trade.validate_trade_execution(
            current_price,
            max_slippage_bps,
            min_distance_bps,
            min_risk_reward_bps,
        );
        assert!(result.is_ok());

        let current_price = 0;
        let result = trade.validate_trade_execution(
            current_price,
            max_slippage_bps,
            min_distance_bps,
            min_risk_reward_bps,
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
}
