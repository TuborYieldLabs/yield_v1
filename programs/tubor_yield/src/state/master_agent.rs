use anchor_lang::prelude::*;

use crate::error::{ErrorCode, TYieldResult};
use crate::math::{SafeMath, PERCENTAGE_PRECISION_U64, QUOTE_PRECISION_U64};
use crate::state::Size;

/// Parameters for initializing a MasterAgent
#[derive(Debug, Clone)]
pub struct MasterAgentInitParams {
    pub authority: Pubkey,
    pub mint: Pubkey,
    pub price: u64,
    pub w_yield: u64,
    pub trading_status: TradingStatus,
    pub max_supply: u64,
    pub auto_relist: bool,
    pub current_time: i32,
    pub bump: u8,
}

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

    // 4-byte aligned fields
    pub last_updated: i32, // 4 bytes
    pub created_at: i32,   // 4 bytes

    // 1-byte aligned fields (smallest last)
    pub trading_status: u8, // 1 byte
    pub auto_relist: bool,  // 1 byte
    pub bump: u8,           // 1 byte

    // Future-proofing padding
    pub _padding: [u8; 8], // 8 bytes for future additions
}

impl MasterAgent {
    /// Initialize a new MasterAgent
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
        Ok(())
    }

    /// Update the price of the master agent
    pub fn update_price(&mut self, new_price: u64, current_time: i32) -> TYieldResult<()> {
        if new_price == 0 {
            return Err(ErrorCode::InvalidEntryPrice);
        }
        self.price = new_price;
        self.last_updated = current_time;
        Ok(())
    }

    /// Update the yield percentage
    pub fn update_yield(&mut self, new_yield: u64, current_time: i32) -> TYieldResult<()> {
        if new_yield > PERCENTAGE_PRECISION_U64 {
            return Err(ErrorCode::MathError);
        }
        self.w_yield = new_yield;
        self.last_updated = current_time;
        Ok(())
    }

    /// Update the maximum supply
    pub fn update_max_supply(
        &mut self,
        new_max_supply: u64,
        current_time: i32,
    ) -> TYieldResult<()> {
        if new_max_supply < self.agent_count {
            return Err(ErrorCode::MathError);
        }
        self.max_supply = new_max_supply;
        self.last_updated = current_time;
        Ok(())
    }

    /// Add an agent to the master agent
    pub fn add_agent(&mut self, current_time: i32) -> TYieldResult<()> {
        if self.agent_count >= self.max_supply {
            return Err(ErrorCode::InsufficientFunds);
        }
        self.agent_count = self.agent_count.safe_add(1)?;
        self.last_updated = current_time;
        Ok(())
    }

    /// Remove an agent from the master agent
    pub fn remove_agent(&mut self, current_time: i32) -> TYieldResult<()> {
        if self.agent_count == 0 {
            return Err(ErrorCode::InsufficientFunds);
        }
        self.agent_count = self.agent_count.safe_sub(1)?;
        self.last_updated = current_time;
        Ok(())
    }

    /// Increment trade count
    pub fn increment_trade_count(&mut self, current_time: i32) -> TYieldResult<()> {
        self.trade_count = self.trade_count.safe_add(1)?;
        self.last_updated = current_time;
        Ok(())
    }

    /// Toggle auto relist setting
    pub fn toggle_auto_relist(&mut self, current_time: i32) {
        self.auto_relist = !self.auto_relist;
        self.last_updated = current_time;
    }

    /// Set auto relist setting
    pub fn set_auto_relist(&mut self, auto_relist: bool, current_time: i32) {
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
    pub fn set_trading_status(&mut self, status: TradingStatus, current_time: i32) {
        self.trading_status = status as u8;
        self.last_updated = current_time;
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
    pub fn toggle_trading_status(&mut self, current_time: i32) {
        let new_status = if self.is_whitelist_mode() {
            TradingStatus::Public
        } else {
            TradingStatus::WhiteList
        };
        self.set_trading_status(new_status, current_time);
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
    pub fn get_days_since_created(&self, current_time: i32) -> i32 {
        (current_time - self.created_at) / 86400 // 86400 seconds in a day
    }

    /// Get days since last update
    pub fn get_days_since_updated(&self, current_time: i32) -> i32 {
        (current_time - self.last_updated) / 86400
    }

    /// Check if the master agent is active (has agents)
    pub fn is_active(&self) -> bool {
        self.agent_count > 0
    }

    /// Check if the master agent is idle (no recent activity)
    pub fn is_idle(&self, current_time: i32, idle_threshold: i32) -> bool {
        let time_since_last_activity = current_time - self.last_updated;
        time_since_last_activity > idle_threshold
    }

    /// Validate the master agent configuration
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
        match self.get_trading_status() {
            TradingStatus::WhiteList | TradingStatus::Public => Ok(()),
        }?;
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
    pub fn get_trading_activity_score(&self, current_time: i32) -> u64 {
        let days_since_created = self.get_days_since_created(current_time);
        if days_since_created == 0 {
            return self.trade_count;
        }
        self.trade_count
            .safe_div(days_since_created as u64)
            .unwrap_or(0)
    }

    /// Get performance metrics
    pub fn get_performance_metrics(&self, current_time: i32) -> TYieldResult<(u64, u64, u64, u64)> {
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
    pub fn needs_attention(&self, current_time: i32) -> bool {
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
}

impl Default for MasterAgent {
    fn default() -> Self {
        Self {
            authority: Pubkey::default(),
            mint: Pubkey::default(),
            trading_status: TradingStatus::WhiteList as u8,
            price: 0,
            w_yield: 0,
            max_supply: 0,
            agent_count: 0,
            trade_count: 0,
            auto_relist: false,
            last_updated: 0,
            created_at: 0,
            bump: 0,
            _padding: [0; 8],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Eq, AnchorDeserialize, AnchorSerialize)]
pub enum TradingStatus {
    WhiteList = 0b00000001,
    Public = 0b00000010,
}

impl Size for MasterAgent {
    const SIZE: usize = 136; // 8 (discriminator) + 128 (struct, including padding) = 136 bytes
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
        };
        master_agent.initialize(params).unwrap();

        master_agent
    }

    #[test]
    fn test_master_agent_size() {
        // On-chain size includes 8 bytes for Anchor discriminator
        assert_eq!(8 + std::mem::size_of::<MasterAgent>(), MasterAgent::SIZE);
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
        assert_eq!(master_agent._padding, [0; 8]);
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
        let current_time = 1640995260; // 1 minute later

        // Test successful price update
        let result = master_agent.update_price(2000000, current_time);
        assert!(result.is_ok());
        assert_eq!(master_agent.price, 2000000);
        assert_eq!(master_agent.last_updated, current_time);

        // Test invalid price (zero)
        let result = master_agent.update_price(0, current_time);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::InvalidEntryPrice);
    }

    #[test]
    fn test_update_yield() {
        let mut master_agent = create_test_master_agent();
        let current_time = 1640995260;

        // Test successful yield update
        let result = master_agent.update_yield(10000, current_time);
        assert!(result.is_ok());
        assert_eq!(master_agent.w_yield, 10000);
        assert_eq!(master_agent.last_updated, current_time);

        // Test yield too high
        let result = master_agent.update_yield(PERCENTAGE_PRECISION_U64 + 1, current_time);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorCode::MathError);
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
        master_agent.set_trading_status(TradingStatus::Public, current_time);
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
        master_agent.set_trading_status(TradingStatus::Public, 1640995260);
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
        master_agent.set_trading_status(TradingStatus::Public, current_time);

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
        master_agent.set_auto_relist(true, current_time);

        let summary = master_agent.get_summary();
        assert_eq!(summary.0, 1); // agent_count
        assert_eq!(summary.1, 1); // trade_count
        assert_eq!(summary.2, 1000000); // price
        assert_eq!(summary.3, 500); // w_yield
        assert_eq!(summary.4, true); // auto_relist
        assert_eq!(summary.5, TradingStatus::WhiteList); // trading_status
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
        master_agent.set_trading_status(TradingStatus::Public, 1640995260);
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
}
