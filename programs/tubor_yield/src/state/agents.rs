use anchor_lang::prelude::*;

use crate::error::{ErrorCode, TYieldResult};
use crate::math::SafeMath;
use crate::state::Size;

/// Represents an agent in the Tubor Yield protocol.
///
/// An agent is a tradeable entity that can be bought, sold, and managed by users.
/// Each agent has a boost multiplier that affects trading performance and belongs
/// to a master agent category.
///
/// # Fields
///
/// - `master_agent`: The master agent this agent belongs to
/// - `mint`: The mint address of the agent token
/// - `owner`: The current owner of the agent
/// - `booster`: Boost multiplier as a percentage (e.g., 15000 = 150%)
/// - `created_at`: Timestamp when the agent was created
/// - `last_updated`: Timestamp of the last update to the agent
/// - `is_listed`: Whether the agent is currently listed for trading
/// - `bump`: PDA bump seed for the agent account
/// - `_padding`: Reserved space for future additions
#[account]
#[derive(Eq, PartialEq, Debug, Default)]
pub struct Agent {
    // 8-byte aligned fields (largest first)
    pub master_agent: Pubkey, // 32 bytes
    pub mint: Pubkey,         // 32 bytes
    pub owner: Pubkey,        // 32 bytes
    pub booster: u64,         // 8 bytes

    // 4-byte aligned fields
    pub created_at: i64,   // 4 bytes
    pub last_updated: i64, // 4 bytes

    // 1-byte aligned fields (smallest last)
    pub is_listed: bool, // 1 byte
    pub bump: u8,        // 1 byte

    // Future-proofing padding
    pub _padding: [u8; 6], // 6 bytes for future additions
}

/// Event emitted when an agent is bought.
#[event]
pub struct BuyAgentEvent {
    pub agent: Pubkey,
    pub owner: Pubkey,
    pub master_agent: Pubkey,
    pub timestamp: i64,
}

/// Event emitted when an agent is sold.
#[event]
pub struct SellAgentEvent {
    pub agent: Pubkey,
    pub owner: Pubkey,
    pub master_agent: Pubkey,
    pub timestamp: i64,
}

/// Event emitted when an agent is minted.
#[event]
pub struct MintAgentEvent {
    pub agent: Pubkey,
    pub owner: Pubkey,
    pub master_agent: Pubkey,
    pub timestamp: i64,
}

impl Agent {
    /// Initializes a new agent with the specified parameters.
    ///
    /// # Arguments
    ///
    /// * `master_agent` - The master agent this agent belongs to
    /// * `mint` - The mint address of the agent token
    /// * `owner` - The initial owner of the agent
    /// * `booster` - Boost multiplier as a percentage (e.g., 15000 = 150%)
    /// * `current_time` - Current timestamp
    /// * `bump` - PDA bump seed for the agent account
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if initialization fails.
    ///
    /// # Example
    ///
    /// ```
    /// use tubor_yield::state::agents::Agent;
    /// use anchor_lang::solana_program::pubkey::Pubkey;
    ///
    /// let mut agent = Agent::default();
    /// let master_agent = Pubkey::new_unique();
    /// let mint = Pubkey::new_unique();
    /// let owner = Pubkey::new_unique();
    /// let booster = 15000; // 150% boost
    /// let current_time = 1640995200;
    /// let bump = 255;
    ///
    /// let result = agent.initialize(master_agent, mint, owner, booster, current_time, bump);
    /// assert!(result.is_ok());
    /// ```
    pub fn initialize(
        &mut self,
        master_agent: Pubkey,
        mint: Pubkey,
        owner: Pubkey,
        booster: u64,
        current_time: i64,
        bump: u8,
    ) -> TYieldResult<()> {
        self.master_agent = master_agent;
        self.mint = mint;
        self.owner = owner;
        self.booster = booster;
        self.is_listed = true;
        self.created_at = current_time;
        self.last_updated = current_time;
        self.bump = bump;
        Ok(())
    }

    /// Updates the booster value of the agent.
    ///
    /// # Arguments
    ///
    /// * `new_booster` - New boost multiplier as a percentage
    /// * `current_time` - Current timestamp
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if the booster is invalid.
    ///
    /// # Errors
    ///
    /// Returns `ErrorCode::InvalidAccount` if the new booster value is zero.
    pub fn update_booster(&mut self, new_booster: u64, current_time: i64) -> TYieldResult<()> {
        if new_booster == 0 {
            return Err(ErrorCode::InvalidAccount);
        }
        self.booster = new_booster;
        self.last_updated = current_time;
        Ok(())
    }

    /// Lists the agent for trading.
    ///
    /// # Arguments
    ///
    /// * `current_time` - Current timestamp
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if the agent is already listed.
    ///
    /// # Errors
    ///
    /// Returns `ErrorCode::InvalidAccount` if the agent is already listed.
    pub fn list(&mut self, current_time: i64) -> TYieldResult<()> {
        if self.is_listed {
            return Err(ErrorCode::InvalidAccount);
        }
        self.is_listed = true;
        self.last_updated = current_time;
        Ok(())
    }

    /// Unlists the agent from trading.
    ///
    /// # Arguments
    ///
    /// * `current_time` - Current timestamp
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if the agent is not listed.
    ///
    /// # Errors
    ///
    /// Returns `ErrorCode::InvalidAccount` if the agent is not currently listed.
    pub fn unlist(&mut self, current_time: i64) -> TYieldResult<()> {
        if !self.is_listed {
            return Err(ErrorCode::InvalidAccount);
        }
        self.is_listed = false;
        self.last_updated = current_time;
        Ok(())
    }

    /// Toggles the listing status of the agent.
    ///
    /// If the agent is listed, it will be unlisted. If it's unlisted, it will be listed.
    ///
    /// # Arguments
    ///
    /// * `current_time` - Current timestamp
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if the operation fails.
    pub fn toggle_listing(&mut self, current_time: i64) -> TYieldResult<()> {
        if self.is_listed {
            self.unlist(current_time)
        } else {
            self.list(current_time)
        }
    }

    /// Transfers ownership of the agent to a new owner.
    ///
    /// # Arguments
    ///
    /// * `new_owner` - The new owner's public key
    /// * `current_time` - Current timestamp
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if the new owner is invalid.
    ///
    /// # Errors
    ///
    /// Returns `ErrorCode::InvalidAccount` if the new owner is the default public key.
    pub fn transfer_ownership(&mut self, new_owner: Pubkey, current_time: i64) -> TYieldResult<()> {
        if new_owner == Pubkey::default() {
            return Err(ErrorCode::InvalidAccount);
        }
        self.owner = new_owner;
        self.last_updated = current_time;
        Ok(())
    }

    /// Checks if the agent is currently listed for trading.
    ///
    /// # Returns
    ///
    /// Returns `true` if the agent is listed, `false` otherwise.
    pub fn is_listed_for_trading(&self) -> bool {
        self.is_listed
    }

    /// Checks if the agent belongs to a specific owner.
    ///
    /// # Arguments
    ///
    /// * `owner` - The owner's public key to check against
    ///
    /// # Returns
    ///
    /// Returns `true` if the agent belongs to the specified owner, `false` otherwise.
    pub fn is_owned_by(&self, owner: &Pubkey) -> bool {
        self.owner == *owner
    }

    /// Checks if the agent belongs to a specific master agent.
    ///
    /// # Arguments
    ///
    /// * `master_agent` - The master agent's public key to check against
    ///
    /// # Returns
    ///
    /// Returns `true` if the agent belongs to the specified master agent, `false` otherwise.
    pub fn belongs_to_master_agent(&self, master_agent: &Pubkey) -> bool {
        self.master_agent == *master_agent
    }

    /// Gets the boost multiplier as a percentage.
    ///
    /// # Returns
    ///
    /// Returns the boost multiplier as a percentage (e.g., 15000 = 150%).
    pub fn get_boost_percentage(&self) -> u64 {
        self.booster
    }

    /// Calculates the effective boost multiplier as a decimal.
    ///
    /// # Returns
    ///
    /// Returns the boost multiplier as a decimal (e.g., 15000 returns 1.5).
    pub fn get_boost_multiplier(&self) -> f64 {
        self.booster as f64 / 10000.0
    }

    /// Calculates the number of days since the agent was created.
    ///
    /// # Arguments
    ///
    /// * `current_time` - Current timestamp
    ///
    /// # Returns
    ///
    /// Returns the number of days since the agent was created.
    pub fn get_days_since_created(&self, current_time: i64) -> i64 {
        let seconds_diff = current_time - self.created_at;
        seconds_diff / 86400 // 86400 seconds in a day
    }

    /// Calculates the number of days since the agent was last updated.
    ///
    /// # Arguments
    ///
    /// * `current_time` - Current timestamp
    ///
    /// # Returns
    ///
    /// Returns the number of days since the agent was last updated.
    pub fn get_days_since_updated(&self, current_time: i64) -> i64 {
        let seconds_diff = current_time - self.last_updated;
        seconds_diff / 86400 // 86400 seconds in a day
    }

    /// Checks if the agent is active.
    ///
    /// An agent is considered active if it has been created and the last update
    /// timestamp is not before the creation timestamp.
    ///
    /// # Returns
    ///
    /// Returns `true` if the agent is active, `false` otherwise.
    pub fn is_active(&self) -> bool {
        self.created_at > 0 && self.last_updated >= self.created_at
    }

    /// Checks if the agent is idle based on the specified threshold.
    ///
    /// # Arguments
    ///
    /// * `current_time` - Current timestamp
    /// * `idle_threshold_days` - Number of days of inactivity to consider the agent idle
    ///
    /// # Returns
    ///
    /// Returns `true` if the agent has been inactive for more than the threshold, `false` otherwise.
    pub fn is_idle(&self, current_time: i64, idle_threshold_days: i64) -> bool {
        let days_since_update = self.get_days_since_updated(current_time);
        days_since_update > idle_threshold_days
    }

    /// Validates the agent's data integrity.
    ///
    /// Checks that all required fields have valid values.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the agent is valid, or an error if validation fails.
    ///
    /// # Errors
    ///
    /// Returns `ErrorCode::InvalidAccount` if any required field is invalid.
    pub fn validate(&self) -> TYieldResult<()> {
        if self.master_agent == Pubkey::default() {
            return Err(ErrorCode::InvalidAccount);
        }
        if self.mint == Pubkey::default() {
            return Err(ErrorCode::InvalidAccount);
        }
        if self.owner == Pubkey::default() {
            return Err(ErrorCode::InvalidAccount);
        }
        if self.booster == 0 {
            return Err(ErrorCode::InvalidAccount);
        }
        if self.created_at <= 0 {
            return Err(ErrorCode::InvalidAccount);
        }
        if self.last_updated < self.created_at {
            return Err(ErrorCode::InvalidAccount);
        }
        Ok(())
    }

    /// Checks if the agent can perform actions.
    ///
    /// An agent can perform actions if it is active.
    ///
    /// # Returns
    ///
    /// Returns `true` if the agent can perform actions, `false` otherwise.
    pub fn can_perform_actions(&self) -> bool {
        self.is_active()
    }

    /// Gets a string representation of the agent's status.
    ///
    /// # Returns
    ///
    /// Returns a string describing the agent's status: "Inactive", "Listed", or "Unlisted".
    pub fn get_status_string(&self) -> String {
        if !self.is_active() {
            "Inactive".to_string()
        } else if self.is_listed {
            "Listed".to_string()
        } else {
            "Unlisted".to_string()
        }
    }

    /// Gets a string representation of the agent's listing status.
    ///
    /// # Returns
    ///
    /// Returns "Listed" if the agent is listed, "Unlisted" otherwise.
    pub fn get_listing_status_string(&self) -> String {
        if self.is_listed {
            "Listed".to_string()
        } else {
            "Unlisted".to_string()
        }
    }

    /// Gets summary statistics for the agent.
    ///
    /// # Returns
    ///
    /// Returns a tuple containing: (master_agent, mint, owner, booster, is_listed, created_at).
    pub fn get_summary(&self) -> (Pubkey, Pubkey, Pubkey, u64, bool, i64) {
        (
            self.master_agent,
            self.mint,
            self.owner,
            self.booster,
            self.is_listed,
            self.created_at,
        )
    }

    /// Checks if the agent needs attention based on age and activity.
    ///
    /// An agent needs attention if it has been inactive for more than 30 days,
    /// was created more than 365 days ago, or is not active.
    ///
    /// # Arguments
    ///
    /// * `current_time` - Current timestamp
    ///
    /// # Returns
    ///
    /// Returns `true` if the agent needs attention, `false` otherwise.
    pub fn needs_attention(&self, current_time: i64) -> bool {
        let days_since_update = self.get_days_since_updated(current_time);
        let days_since_created = self.get_days_since_created(current_time);

        // Needs attention if:
        // 1. No activity in the last 30 days
        // 2. Created more than 365 days ago
        // 3. Not active
        days_since_update > 30 || days_since_created > 365 || !self.is_active()
    }

    /// Resets the agent to its initial state (for testing/debugging).
    ///
    /// This method is intended for testing purposes and should not be used in production.
    pub fn reset(&mut self) {
        self.is_listed = false;
        self.last_updated = self.created_at;
    }

    /// Gets the agent's age in days.
    ///
    /// # Arguments
    ///
    /// * `current_time` - Current timestamp
    ///
    /// # Returns
    ///
    /// Returns the number of days since the agent was created.
    pub fn get_age_days(&self, current_time: i64) -> i64 {
        self.get_days_since_created(current_time)
    }

    /// Checks if the agent is newly created (less than 7 days old).
    ///
    /// # Arguments
    ///
    /// * `current_time` - Current timestamp
    ///
    /// # Returns
    ///
    /// Returns `true` if the agent is less than 7 days old, `false` otherwise.
    pub fn is_new(&self, current_time: i64) -> bool {
        self.get_age_days(current_time) < 7
    }

    /// Checks if the agent is mature (more than 30 days old).
    ///
    /// # Arguments
    ///
    /// * `current_time` - Current timestamp
    ///
    /// # Returns
    ///
    /// Returns `true` if the agent is more than 30 days old, `false` otherwise.
    pub fn is_mature(&self, current_time: i64) -> bool {
        self.get_age_days(current_time) >= 30
    }

    /// Calculates a performance score for the agent based on age and activity.
    ///
    /// The score is based on the boost multiplier with bonuses for recent activity
    /// and maturity, and penalties for being very old.
    ///
    /// # Arguments
    ///
    /// * `current_time` - Current timestamp
    ///
    /// # Returns
    ///
    /// Returns a performance score as a u64 value.
    pub fn get_performance_score(&self, current_time: i64) -> u64 {
        let age_days = self.get_age_days(current_time);
        let days_since_update = self.get_days_since_updated(current_time);

        // Base score from boost multiplier
        let mut score = self.booster;

        // Bonus for being active (updated recently), but not at initialization
        if days_since_update <= 7 && current_time != self.created_at {
            score = score.safe_add(1000).unwrap_or(score);
        }

        // Bonus for being mature but not too old
        if (30..=365).contains(&age_days) {
            score = score.safe_add(500).unwrap_or(score);
        }

        // Penalty for being very old
        if age_days > 365 {
            score = score.safe_sub(2000).unwrap_or(0);
        }

        score
    }
}

impl Size for Agent {
    const SIZE: usize = 136; // 8 (discriminator) + 128 (struct, including padding) = 136 bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_size() {
        // On-chain size includes 8 bytes for Anchor discriminator
        let actual_size = 8 + std::mem::size_of::<Agent>();
        println!("Agent struct size: {} bytes", std::mem::size_of::<Agent>());
        println!("Agent on-chain size: {} bytes", actual_size);
        println!("Expected size: {} bytes", Agent::SIZE);
        assert_eq!(actual_size, Agent::SIZE);
        println!("Agent on-chain size: {} bytes", Agent::SIZE);
    }

    #[test]
    fn test_agent_memory_layout() {
        // Test that Agent struct can be created and serialized
        let agent = Agent::default();
        assert_eq!(agent.master_agent, Pubkey::default());
        assert_eq!(agent.mint, Pubkey::default());
        assert_eq!(agent.owner, Pubkey::default());
        assert_eq!(agent.booster, 0);
        assert_eq!(agent.created_at, 0);
        assert_eq!(agent.last_updated, 0);
        assert_eq!(agent.is_listed, false);
        assert_eq!(agent.bump, 0);
        assert_eq!(agent._padding, [0; 6]);
    }

    #[test]
    fn test_agent_initialize() {
        let mut agent = Agent::default();
        let master_agent = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let booster = 15000; // 150% boost
        let current_time = 1640995200; // 2022-01-01 00:00:00 UTC
        let bump = 255;

        let result = agent.initialize(master_agent, mint, owner, booster, current_time, bump);
        assert!(result.is_ok());

        assert_eq!(agent.master_agent, master_agent);
        assert_eq!(agent.mint, mint);
        assert_eq!(agent.owner, owner);
        assert_eq!(agent.booster, booster);
        assert_eq!(agent.created_at, current_time);
        assert_eq!(agent.last_updated, current_time);
        assert_eq!(agent.is_listed, true);
        assert_eq!(agent.bump, bump);
    }

    #[test]
    fn test_agent_listing_operations() {
        let mut agent = Agent::default();
        let current_time = 1640995200;

        // Initialize agent
        agent
            .initialize(
                Pubkey::new_unique(),
                Pubkey::new_unique(),
                Pubkey::new_unique(),
                10000,
                current_time,
                255,
            )
            .unwrap();

        // Test listing
        assert!(agent.is_listed_for_trading());
        let result = agent.list(current_time + 3600);
        assert!(result.is_err());
        assert!(agent.is_listed_for_trading());

        // Test unlisting
        let result = agent.unlist(current_time + 7200);
        assert!(result.is_ok());
        assert!(!agent.is_listed_for_trading());

        // Test toggle
        let result = agent.toggle_listing(current_time + 10800);
        assert!(result.is_ok());
        assert!(agent.is_listed_for_trading());
    }

    #[test]
    fn test_agent_booster_operations() {
        let mut agent = Agent::default();
        let current_time = 1640995200;

        agent
            .initialize(
                Pubkey::new_unique(),
                Pubkey::new_unique(),
                Pubkey::new_unique(),
                10000,
                current_time,
                255,
            )
            .unwrap();

        // Test booster update
        let new_booster = 20000; // 200% boost
        let result = agent.update_booster(new_booster, current_time + 3600);
        assert!(result.is_ok());
        assert_eq!(agent.booster, new_booster);
        assert_eq!(agent.get_boost_percentage(), new_booster);
        assert_eq!(agent.get_boost_multiplier(), 2.0);

        // Test invalid booster (zero)
        let result = agent.update_booster(0, current_time + 7200);
        assert!(result.is_err());
    }

    #[test]
    fn test_agent_ownership_operations() {
        let mut agent = Agent::default();
        let current_time = 1640995200;
        let original_owner = Pubkey::new_unique();
        let new_owner = Pubkey::new_unique();

        agent
            .initialize(
                Pubkey::new_unique(),
                Pubkey::new_unique(),
                original_owner,
                10000,
                current_time,
                255,
            )
            .unwrap();

        // Test ownership check
        assert!(agent.is_owned_by(&original_owner));
        assert!(!agent.is_owned_by(&new_owner));

        // Test ownership transfer
        let result = agent.transfer_ownership(new_owner, current_time + 3600);
        assert!(result.is_ok());
        assert!(agent.is_owned_by(&new_owner));
        assert!(!agent.is_owned_by(&original_owner));

        // Test invalid transfer (default pubkey)
        let result = agent.transfer_ownership(Pubkey::default(), current_time + 7200);
        assert!(result.is_err());
    }

    #[test]
    fn test_agent_validation() {
        let mut agent = Agent::default();
        let current_time = 1640995200;

        // Test uninitialized agent
        assert!(agent.validate().is_err());

        // Test valid agent
        agent
            .initialize(
                Pubkey::new_unique(),
                Pubkey::new_unique(),
                Pubkey::new_unique(),
                10000,
                current_time,
                255,
            )
            .unwrap();
        assert!(agent.validate().is_ok());

        // Test invalid agent with zero booster
        agent.booster = 0;
        assert!(agent.validate().is_err());
    }

    #[test]
    fn test_agent_time_operations() {
        let mut agent = Agent::default();
        let current_time = 1640995200; // 2022-01-01 00:00:00 UTC

        agent
            .initialize(
                Pubkey::new_unique(),
                Pubkey::new_unique(),
                Pubkey::new_unique(),
                10000,
                current_time,
                255,
            )
            .unwrap();

        let future_time = current_time + 86400 * 30; // 30 days later

        // Test age calculations
        assert_eq!(agent.get_age_days(future_time), 30);
        assert!(agent.is_new(current_time + 86400 * 3)); // 3 days old
        assert!(agent.is_mature(future_time)); // 30 days old

        // Test idle detection
        assert!(!agent.is_idle(current_time + 86400 * 10, 30)); // 10 days, threshold 30
        assert!(agent.is_idle(future_time + 86400 * 10, 30)); // 40 days, threshold 30
    }

    #[test]
    fn test_agent_performance_score() {
        let mut agent = Agent::default();
        let current_time = 1640995200;

        agent
            .initialize(
                Pubkey::new_unique(),
                Pubkey::new_unique(),
                Pubkey::new_unique(),
                15000, // 150% boost
                current_time,
                255,
            )
            .unwrap();

        // Test base score
        let base_score = agent.get_performance_score(current_time);
        assert_eq!(base_score, 15000);

        // Test score with recent activity
        let recent_time = current_time + 86400 * 3; // 3 days later
        let score_with_activity = agent.get_performance_score(recent_time);
        assert!(score_with_activity > base_score);

        // Test score with old age
        let old_time = current_time + 86400 * 400; // 400 days later
        let score_old = agent.get_performance_score(old_time);
        assert!(score_old < base_score);
    }
}
