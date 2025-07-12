use anchor_lang::prelude::*;

use crate::error::{ErrorCode, TYieldResult};
use crate::math::SafeMath;
use crate::state::Size;

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

#[event]
pub struct BuyAgentEvent {
    pub agent: Pubkey,
    pub owner: Pubkey,
    pub master_agent: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct SellAgentEvent {
    pub agent: Pubkey,
    pub owner: Pubkey,
    pub master_agent: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct MintAgentEvent {
    pub agent: Pubkey,
    pub owner: Pubkey,
    pub master_agent: Pubkey,
    pub timestamp: i64,
}

impl Agent {
    /// Initialize a new Agent
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

    /// Update the booster value
    pub fn update_booster(&mut self, new_booster: u64, current_time: i64) -> TYieldResult<()> {
        if new_booster == 0 {
            return Err(ErrorCode::InvalidAccount);
        }
        self.booster = new_booster;
        self.last_updated = current_time;
        Ok(())
    }

    /// List the agent for trading
    pub fn list(&mut self, current_time: i64) -> TYieldResult<()> {
        if self.is_listed {
            return Err(ErrorCode::InvalidAccount);
        }
        self.is_listed = true;
        self.last_updated = current_time;
        Ok(())
    }

    /// Unlist the agent from trading
    pub fn unlist(&mut self, current_time: i64) -> TYieldResult<()> {
        if !self.is_listed {
            return Err(ErrorCode::InvalidAccount);
        }
        self.is_listed = false;
        self.last_updated = current_time;
        Ok(())
    }

    /// Toggle the listing status
    pub fn toggle_listing(&mut self, current_time: i64) -> TYieldResult<()> {
        if self.is_listed {
            self.unlist(current_time)
        } else {
            self.list(current_time)
        }
    }

    /// Transfer ownership of the agent
    pub fn transfer_ownership(&mut self, new_owner: Pubkey, current_time: i64) -> TYieldResult<()> {
        if new_owner == Pubkey::default() {
            return Err(ErrorCode::InvalidAccount);
        }
        self.owner = new_owner;
        self.last_updated = current_time;
        Ok(())
    }

    /// Check if the agent is currently listed
    pub fn is_listed_for_trading(&self) -> bool {
        self.is_listed
    }

    /// Check if the agent belongs to a specific owner
    pub fn is_owned_by(&self, owner: &Pubkey) -> bool {
        self.owner == *owner
    }

    /// Check if the agent belongs to a specific master agent
    pub fn belongs_to_master_agent(&self, master_agent: &Pubkey) -> bool {
        self.master_agent == *master_agent
    }

    /// Get the boost multiplier as a percentage
    pub fn get_boost_percentage(&self) -> u64 {
        self.booster
    }

    /// Calculate the effective boost multiplier (booster / 10000 for percentage)
    pub fn get_boost_multiplier(&self) -> f64 {
        self.booster as f64 / 10000.0
    }

    /// Get days since the agent was created
    pub fn get_days_since_created(&self, current_time: i64) -> i64 {
        let seconds_diff = current_time - self.created_at;
        seconds_diff / 86400 // 86400 seconds in a day
    }

    /// Get days since the agent was last updated
    pub fn get_days_since_updated(&self, current_time: i64) -> i64 {
        let seconds_diff = current_time - self.last_updated;
        seconds_diff / 86400 // 86400 seconds in a day
    }

    /// Check if the agent is active (created and not too old)
    pub fn is_active(&self) -> bool {
        self.created_at > 0 && self.last_updated >= self.created_at
    }

    /// Check if the agent is idle (no updates for a while)
    pub fn is_idle(&self, current_time: i64, idle_threshold_days: i64) -> bool {
        let days_since_update = self.get_days_since_updated(current_time);
        days_since_update > idle_threshold_days
    }

    /// Validate the agent's data integrity
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

    /// Check if the agent can perform actions
    pub fn can_perform_actions(&self) -> bool {
        self.is_active()
    }

    /// Get the agent's status string
    pub fn get_status_string(&self) -> String {
        if !self.is_active() {
            "Inactive".to_string()
        } else if self.is_listed {
            "Listed".to_string()
        } else {
            "Unlisted".to_string()
        }
    }

    /// Get the listing status string
    pub fn get_listing_status_string(&self) -> String {
        if self.is_listed {
            "Listed".to_string()
        } else {
            "Unlisted".to_string()
        }
    }

    /// Get summary statistics
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

    /// Check if the agent needs attention (old, inactive, etc.)
    pub fn needs_attention(&self, current_time: i64) -> bool {
        let days_since_update = self.get_days_since_updated(current_time);
        let days_since_created = self.get_days_since_created(current_time);

        // Needs attention if:
        // 1. No activity in the last 30 days
        // 2. Created more than 365 days ago
        // 3. Not active
        days_since_update > 30 || days_since_created > 365 || !self.is_active()
    }

    /// Reset the agent (for testing/debugging)
    pub fn reset(&mut self) {
        self.is_listed = false;
        self.last_updated = self.created_at;
    }

    /// Get the agent's age in days
    pub fn get_age_days(&self, current_time: i64) -> i64 {
        self.get_days_since_created(current_time)
    }

    /// Check if the agent is newly created (less than 7 days old)
    pub fn is_new(&self, current_time: i64) -> bool {
        self.get_age_days(current_time) < 7
    }

    /// Check if the agent is mature (more than 30 days old)
    pub fn is_mature(&self, current_time: i64) -> bool {
        self.get_age_days(current_time) >= 30
    }

    /// Get the agent's performance score based on age and activity
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
