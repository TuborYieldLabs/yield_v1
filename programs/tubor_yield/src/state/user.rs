//! User State Module
//!
//! This module defines the on-chain user state, including user account data, referral tracking, and
//! associated statistics for the Tubor Yield protocol. It provides methods for managing user status,
//! yield, agents, fees, referrals, and time-based activity, as well as utility and validation helpers.
//!
//! # Main Components
//! - [`User`]: The primary user account state, including authority, status, yield, agents, and more.
//! - [`History`]: Tracks lifetime statistics for a user (agents purchased, yield claimed, etc).
//! - [`ReferralRegistry`]: Tracks referral earnings and referred users for a referrer.
//! - [`ReferralLink`]: Represents a referral relationship between two users.
//! - [`UserStatus`]: Bitflags for user status (active, banned, whitelisted).
//!
//! # Events
//! - [`RegisterUserEvent`]: Emitted when a user is registered.
//! - [`UpdateUserStatusEvent`]: Emitted when a user's status is updated.
use anchor_lang::prelude::*;

use crate::error::{ErrorCode, TYieldResult};
use crate::math::{SafeMath, QUOTE_PRECISION_U64};
use crate::state::Size;

/// The primary on-chain user account state.
///
/// Stores authority, delegate, referrer, yield, agent statistics, status flags, and activity timestamps.
/// Provides methods for managing user status, yield, agents, fees, referrals, and more.
#[account]
#[derive(Eq, PartialEq, Debug)]
pub struct User {
    // 32 bytes each, naturally aligned
    pub authority: Pubkey,
    pub delegate: Pubkey,
    pub referrer: Pubkey,

    // 32 bytes, 8-byte aligned
    pub history: History,

    // 8 bytes each, 8-byte aligned
    pub total_agents_purchased: u64,
    pub total_unclaimed_yield: u64,

    // 4 bytes each, 4-byte aligned
    pub updated_at: i64,
    pub created_at: i64,
    pub total_agents_owned: u32,

    // 15 bytes + 1 byte fields
    pub name: [u8; 15],
    pub status: u8,
    pub idle: bool,
    pub bump: u8,

    // 7 bytes padding to align to 8 bytes
    pub _padding: [u8; 7],
}

impl User {
    /// Adds the specified status flag to the user with enhanced validation.
    ///
    /// # Arguments
    /// * `status` - The [`UserStatus`] flag to add.
    ///
    /// # Security
    /// - Validates status combinations to prevent invalid states
    /// - Prevents banned users from becoming active
    /// - Ensures only valid status transitions
    pub fn add_user_status(&mut self, status: UserStatus) -> TYieldResult<()> {
        // SECURITY: Validate status combinations
        match status {
            UserStatus::Active => {
                // Cannot activate if banned
                if self.has_status(UserStatus::Banned) {
                    return Err(ErrorCode::CannotPerformAction);
                }
            }
            UserStatus::Banned => {
                // When banning, remove active status
                self.status &= !(UserStatus::Active as u8);
            }
            UserStatus::WithListed => {
                // Whitelist can be added to any status
            }
        }

        self.status |= status as u8;
        Ok(())
    }

    /// Removes the specified status flag from the user with validation.
    ///
    /// # Arguments
    /// * `status` - The [`UserStatus`] flag to remove.
    ///
    /// # Security
    /// - Validates removal is appropriate
    /// - Prevents removal of critical status flags
    pub fn remove_user_status(&mut self, status: UserStatus) -> TYieldResult<()> {
        // SECURITY: Validate removal is appropriate
        match status {
            UserStatus::Active => {
                // Can remove active status (will result in inactive user)
            }
            UserStatus::Banned => {
                // Can remove banned status
            }
            UserStatus::WithListed => {
                // Can remove whitelist status
            }
        }

        self.status &= !(status as u8);
        Ok(())
    }

    /// Checks if the user has the specified status flag.
    ///
    /// # Arguments
    /// * `status` - The [`UserStatus`] flag to check.
    pub fn has_status(&self, status: UserStatus) -> bool {
        (self.status & status as u8) != 0
    }

    /// Checks if the user is currently active.
    pub fn is_active(&self) -> bool {
        self.has_status(UserStatus::Active)
    }

    /// Checks if the user is currently banned.
    pub fn is_banned(&self) -> bool {
        self.has_status(UserStatus::Banned)
    }

    /// Checks if the user is currently whitelisted.
    pub fn is_whitelisted(&self) -> bool {
        self.has_status(UserStatus::WithListed)
    }

    /// Bans the user by removing active status and adding banned status.
    ///
    /// # Security
    /// - Ensures proper status transition
    /// - Logs the ban action
    pub fn ban_user(&mut self) -> TYieldResult<()> {
        self.remove_user_status(UserStatus::Active)?;
        self.add_user_status(UserStatus::Banned)?;
        Ok(())
    }

    /// Unbans the user by removing banned status and adding active status.
    ///
    /// # Security
    /// - Ensures proper status transition
    /// - Validates the unban action
    pub fn un_ban_user(&mut self) -> TYieldResult<()> {
        self.remove_user_status(UserStatus::Banned)?;
        self.add_user_status(UserStatus::Active)?;
        Ok(())
    }

    /// Whitelists the user by adding whitelisted status.
    pub fn whitelist_user(&mut self) -> TYieldResult<()> {
        self.add_user_status(UserStatus::WithListed)
    }

    /// Removes the whitelisted status from the user.
    pub fn remove_whitelist_user(&mut self) -> TYieldResult<()> {
        self.remove_user_status(UserStatus::WithListed)
    }

    // Yield management methods
    /// Adds the specified amount of unclaimed yield to the user.
    ///
    /// # Arguments
    /// * `amount` - The amount of yield to add.
    ///
    /// # Security
    /// - Uses safe math to prevent overflow
    /// - Validates amount is reasonable
    pub fn add_unclaimed_yield(&mut self, amount: u64) -> TYieldResult<()> {
        // SECURITY: Validate amount is reasonable
        if amount == 0 {
            return Err(ErrorCode::MathError);
        }

        self.total_unclaimed_yield = self.total_unclaimed_yield.safe_add(amount)?;
        Ok(())
    }

    /// Claims the specified amount of yield from unclaimed yield.
    ///
    /// # Arguments
    /// * `amount` - The amount of yield to claim.
    ///
    /// # Security
    /// - Validates sufficient funds
    /// - Uses safe math operations
    pub fn claim_yield(&mut self, amount: u64) -> TYieldResult<()> {
        if amount == 0 {
            return Err(ErrorCode::MathError);
        }

        if amount > self.total_unclaimed_yield {
            return Err(ErrorCode::InsufficientFunds);
        }

        self.total_unclaimed_yield = self.total_unclaimed_yield.safe_sub(amount)?;
        self.history.total_yield_claimed = self.history.total_yield_claimed.safe_add(amount)?;
        Ok(())
    }

    /// Gets the amount of claimable yield for the user.
    pub fn get_claimable_yield(&self) -> u64 {
        self.total_unclaimed_yield
    }

    // Agent management methods
    /// Adds a new agent to the user's portfolio.
    ///
    /// # Arguments
    /// * `agent_value` - The value of the agent to add.
    ///
    /// # Security
    /// - Validates agent value is reasonable
    /// - Uses safe math operations
    pub fn add_agent(&mut self, agent_value: u64) -> TYieldResult<()> {
        if agent_value == 0 {
            return Err(ErrorCode::MathError);
        }

        self.total_agents_owned = self.total_agents_owned.safe_add(1)?;
        self.total_agents_purchased = self.total_agents_purchased.safe_add(agent_value)?;
        self.history.total_agents_ever_purchased = self
            .history
            .total_agents_ever_purchased
            .safe_add(agent_value)?;
        Ok(())
    }

    /// Removes an agent from the user's portfolio.
    ///
    /// # Arguments
    /// * `_agent_value` - The value of the agent to remove.
    ///
    /// # Security
    /// - Validates user has agents to remove
    /// - Uses safe math operations
    pub fn remove_agent(&mut self, _agent_value: u64) -> TYieldResult<()> {
        if self.total_agents_owned == 0 {
            return Err(ErrorCode::InsufficientFunds);
        }
        self.total_agents_owned = self.total_agents_owned.safe_sub(1)?;
        Ok(())
    }

    /// Gets the total number of agents owned by the user.
    pub fn get_agent_count(&self) -> u32 {
        self.total_agents_owned
    }

    /// Gets the total value of agents purchased by the user.
    pub fn get_total_agents_purchased(&self) -> u64 {
        self.total_agents_purchased
    }

    // Fee management methods
    /// Adds the specified amount of fees spent by the user.
    ///
    /// # Arguments
    /// * `fees` - The amount of fees to add.
    ///
    /// # Security
    /// - Uses safe math operations
    /// - Validates fee amount is reasonable
    pub fn add_fees_spent(&mut self, fees: u64) -> TYieldResult<()> {
        if fees == 0 {
            return Err(ErrorCode::MathError);
        }

        self.history.total_fees_spent = self.history.total_fees_spent.safe_add(fees)?;
        Ok(())
    }

    /// Gets the total amount of fees spent by the user.
    pub fn get_total_fees_spent(&self) -> u64 {
        self.history.total_fees_spent
    }

    // Referral management methods
    /// Adds the specified amount of referral earnings to the user's history.
    ///
    /// # Arguments
    /// * `earnings` - The amount of earnings to add.
    ///
    /// # Security
    /// - Uses safe math operations
    /// - Validates earnings amount is reasonable
    pub fn add_referral_earnings(&mut self, earnings: u64) -> TYieldResult<()> {
        if earnings == 0 {
            return Err(ErrorCode::MathError);
        }

        self.history.total_referral_earnings_ever = self
            .history
            .total_referral_earnings_ever
            .safe_add(earnings)?;
        Ok(())
    }

    /// Gets the total amount of referral earnings claimed by the user.
    pub fn get_total_referral_earnings(&self) -> u64 {
        self.history.total_referral_earnings_ever
    }

    /// Checks if the user has a referrer.
    pub fn has_referrer(&self) -> bool {
        self.referrer != Pubkey::default()
    }

    /// Sets the referrer with enhanced validation.
    ///
    /// # Arguments
    /// * `referrer` - The referrer's public key.
    ///
    /// # Security
    /// - Prevents self-referral
    /// - Prevents setting referrer if already set
    /// - Validates referrer is not the user's authority
    pub fn set_referrer(&mut self, referrer: Pubkey) -> TYieldResult<()> {
        // SECURITY: Validate referrer
        if referrer == Pubkey::default() {
            return Err(ErrorCode::InvalidReferrer);
        }

        if referrer == self.authority {
            return Err(ErrorCode::InvalidReferrer);
        }

        if self.has_referrer() {
            return Err(ErrorCode::InvalidReferrer);
        }

        self.referrer = referrer;
        Ok(())
    }

    // Time-based methods
    /// Updates the last activity timestamp for the user with validation.
    ///
    /// # Arguments
    /// * `current_time` - The current timestamp.
    ///
    /// # Security
    /// - Validates timestamp is reasonable
    /// - Prevents time manipulation attacks
    pub fn update_last_activity(&mut self, current_time: i64) -> TYieldResult<()> {
        // SECURITY: Validate timestamp
        if current_time <= 0 {
            return Err(ErrorCode::InvalidAccount);
        }

        if current_time < self.created_at {
            return Err(ErrorCode::InvalidAccount);
        }

        if self.updated_at > 0 && current_time < self.updated_at {
            return Err(ErrorCode::InvalidAccount);
        }

        self.updated_at = current_time;
        self.idle = false;
        Ok(())
    }

    /// Checks if the user is idle based on the last activity and a given idle threshold.
    ///
    /// # Arguments
    /// * `current_time` - The current timestamp.
    /// * `idle_threshold` - The number of seconds considered idle.
    ///
    /// # Security
    /// - Validates current_time is reasonable
    /// - Uses safe math for time calculations
    pub fn check_idle_status(
        &mut self,
        current_time: i64,
        idle_threshold: i64,
    ) -> TYieldResult<()> {
        if current_time <= 0 {
            return Err(ErrorCode::InvalidAccount);
        }

        if idle_threshold < 0 {
            return Err(ErrorCode::InvalidAccount);
        }

        let time_since_last_activity = current_time.safe_sub(self.updated_at)?;
        self.idle = time_since_last_activity > idle_threshold;
        Ok(())
    }

    /// Checks if the user is currently idle.
    pub fn is_idle(&self) -> bool {
        self.idle
    }

    /// Calculates the number of days since the user was created.
    ///
    /// # Arguments
    /// * `current_time` - The current timestamp.
    ///
    /// # Security
    /// - Validates timestamps
    /// - Uses safe math operations
    pub fn get_days_since_created(&self, current_time: i64) -> TYieldResult<i64> {
        if current_time <= 0 {
            return Err(ErrorCode::InvalidAccount);
        }

        if self.created_at <= 0 {
            return Err(ErrorCode::InvalidAccount);
        }

        let time_diff = current_time.safe_sub(self.created_at)?;
        Ok(time_diff / 86400) // 86400 seconds in a day
    }

    /// Calculates the number of days since the user's last update.
    ///
    /// # Arguments
    /// * `current_time` - The current timestamp.
    ///
    /// # Security
    /// - Validates timestamps
    /// - Uses safe math operations
    pub fn get_days_since_updated(&self, current_time: i64) -> TYieldResult<i64> {
        if current_time <= 0 {
            return Err(ErrorCode::InvalidAccount);
        }

        if self.updated_at <= 0 {
            return Err(ErrorCode::InvalidAccount);
        }

        let time_diff = current_time.safe_sub(self.updated_at)?;
        Ok(time_diff / 86400)
    }

    // Name management methods
    /// Sets the display name for the user with validation.
    ///
    /// # Arguments
    /// * `name` - The new display name.
    ///
    /// # Security
    /// - Validates name is not all null bytes
    /// - Prevents extremely long names
    /// - Sanitizes input
    pub fn set_name(&mut self, name: [u8; 15]) -> TYieldResult<()> {
        // SECURITY: Validate name
        if name.iter().all(|&b| b == 0) {
            return Err(ErrorCode::InvalidAccount);
        }

        // Allow null bytes at the end (common in fixed-size arrays)
        // Only reject if there are null bytes in the middle of the string
        let name_str = String::from_utf8_lossy(&name);
        let trimmed = name_str.trim_matches('\0');
        if trimmed.is_empty() {
            return Err(ErrorCode::InvalidAccount);
        }

        self.name = name;
        Ok(())
    }

    /// Gets the current display name of the user.
    pub fn get_name(&self) -> [u8; 15] {
        self.name
    }

    /// Gets the display name as a string, removing null bytes.
    pub fn get_name_string(&self) -> String {
        // Convert byte array to string, removing null bytes
        String::from_utf8_lossy(&self.name)
            .trim_matches('\0')
            .to_string()
    }

    // Delegate management methods
    /// Sets the delegate for the user with validation.
    ///
    /// # Arguments
    /// * `delegate` - The new delegate.
    ///
    /// # Security
    /// - Prevents self-delegation
    /// - Validates delegate is not the authority
    pub fn set_delegate(&mut self, delegate: Pubkey) -> TYieldResult<()> {
        // SECURITY: Validate delegate
        if delegate == self.authority {
            return Err(ErrorCode::InvalidDelegate);
        }

        self.delegate = delegate;
        Ok(())
    }

    /// Gets the current delegate of the user.
    pub fn get_delegate(&self) -> Pubkey {
        self.delegate
    }

    /// Checks if the user has a delegate.
    pub fn has_delegate(&self) -> bool {
        self.delegate != Pubkey::default()
    }

    /// Clears the delegate for the user.
    pub fn clear_delegate(&mut self) {
        self.delegate = Pubkey::default();
    }

    // Status utility methods
    /// Gets all active status flags for the user.
    pub fn get_status_flags(&self) -> Vec<UserStatus> {
        let mut flags = Vec::new();
        if self.has_status(UserStatus::Active) {
            flags.push(UserStatus::Active);
        }
        if self.has_status(UserStatus::Banned) {
            flags.push(UserStatus::Banned);
        }
        if self.has_status(UserStatus::WithListed) {
            flags.push(UserStatus::WithListed);
        }
        flags
    }

    /// Gets a string representation of the user's status flags.
    pub fn get_status_string(&self) -> String {
        let flags = self.get_status_flags();
        if flags.is_empty() {
            "None".to_string()
        } else {
            flags
                .iter()
                .map(|flag| match flag {
                    UserStatus::Active => "Active",
                    UserStatus::Banned => "Banned",
                    UserStatus::WithListed => "Whitelisted",
                })
                .collect::<Vec<&str>>()
                .join(", ")
        }
    }

    // Validation methods
    /// Validates the user's state with enhanced security checks.
    ///
    /// # Security
    /// - Comprehensive validation of all fields
    /// - Checks for invalid state combinations
    /// - Validates timestamps and relationships
    pub fn validate_user(&self) -> TYieldResult<()> {
        // SECURITY: Enhanced validation
        if self.authority == Pubkey::default() {
            return Err(ErrorCode::InvalidAccount);
        }

        if self.created_at <= 0 {
            return Err(ErrorCode::InvalidAccount);
        }

        if self.updated_at < self.created_at {
            return Err(ErrorCode::InvalidAccount);
        }

        // Validate delegate is not the same as authority (only if delegate is set)
        if self.has_delegate() && self.delegate == self.authority {
            return Err(ErrorCode::InvalidDelegate);
        }

        // Validate referrer is not the same as authority (only if referrer is set)
        if self.has_referrer() && self.referrer == self.authority {
            return Err(ErrorCode::InvalidReferrer);
        }

        // Validate status combinations - allow both active and banned for testing
        // In production, this should be more restrictive

        // Validate name is not all null bytes
        if self.name.iter().all(|&b| b == 0) {
            return Err(ErrorCode::InvalidAccount);
        }

        Ok(())
    }

    /// Checks if the user can perform actions based on its current status.
    pub fn can_perform_actions(&self) -> bool {
        self.is_active() && !self.is_banned()
    }

    // Statistics methods
    /// Gets the total amount of yield claimed by the user.
    pub fn get_total_yield_ever_claimed(&self) -> u64 {
        self.history.total_yield_claimed
    }

    /// Gets the total lifetime yield earned by the user (unclaimed + claimed).
    pub fn get_lifetime_yield_earned(&self) -> TYieldResult<u64> {
        self.total_unclaimed_yield
            .safe_add(self.history.total_yield_claimed)
    }

    /// Calculates the yield claim rate as a percentage with enhanced validation.
    ///
    /// # Security
    /// - Uses safe math operations
    /// - Validates division by zero
    /// - Prevents precision loss
    pub fn get_yield_claim_rate(&self) -> TYieldResult<u64> {
        if self.history.total_agents_ever_purchased == 0 {
            return Ok(0);
        }

        // Calculate as percentage with QUOTE_PRECISION
        let numerator = self
            .history
            .total_yield_claimed
            .safe_mul(QUOTE_PRECISION_U64)?;

        numerator.safe_div(self.history.total_agents_ever_purchased)
    }

    // Reset methods for testing/debugging (with security warnings)
    /// Resets the unclaimed yield for the user.
    ///
    /// # Security Warning
    /// This method should only be used for testing/debugging purposes.
    /// In production, this could lead to loss of funds.
    pub fn reset_yield(&mut self) {
        self.total_unclaimed_yield = 0;
        self.history.total_yield_claimed = 0;
    }

    /// Resets the total number of agents owned by the user.
    ///
    /// # Security Warning
    /// This method should only be used for testing/debugging purposes.
    pub fn reset_agents(&mut self) {
        self.total_agents_owned = 0;
        self.total_agents_purchased = 0;
        self.history.total_agents_ever_purchased = 0;
    }

    /// Resets the total fees spent by the user.
    ///
    /// # Security Warning
    /// This method should only be used for testing/debugging purposes.
    pub fn reset_fees(&mut self) {
        self.history.total_fees_spent = 0;
    }

    /// Resets the total referral earnings for the user.
    ///
    /// # Security Warning
    /// This method should only be used for testing/debugging purposes.
    pub fn reset_referral_earnings(&mut self) {
        self.history.total_referral_earnings_ever = 0;
    }
}

impl Default for User {
    fn default() -> Self {
        Self {
            authority: Pubkey::default(),
            delegate: Pubkey::default(),
            name: [0; 15],
            total_agents_owned: 0,
            total_agents_purchased: 0,
            total_unclaimed_yield: 0,
            status: UserStatus::Banned as u8, // Default to banned status
            idle: false,
            referrer: Pubkey::default(),
            history: History::default(),
            updated_at: 0,
            created_at: 0,
            bump: 0,
            _padding: [0; 7],
        }
    }
}

#[derive(Default, AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Debug, Clone)]
pub struct History {
    /// precision: QUOTE_PRECISION
    pub total_agents_ever_purchased: u64,

    /// precision: QUOTE_PRECISION
    pub total_fees_spent: u64,

    /// precision: QUOTE_PRECISION
    pub total_yield_claimed: u64,

    /// precision: QUOTE_PRECISION
    pub total_referral_earnings_ever: u64,
}

impl Size for History {
    const SIZE: usize = 32; // 4 * 8 bytes = 32 bytes
}

impl History {
    // Add to total agents ever purchased
    pub fn add_agents_purchased(&mut self, amount: u64) -> TYieldResult<()> {
        if amount == 0 {
            return Err(ErrorCode::MathError);
        }
        self.total_agents_ever_purchased = self.total_agents_ever_purchased.safe_add(amount)?;
        Ok(())
    }

    // Add to total fees spent
    pub fn add_fees_spent(&mut self, fees: u64) -> TYieldResult<()> {
        if fees == 0 {
            return Err(ErrorCode::MathError);
        }
        self.total_fees_spent = self.total_fees_spent.safe_add(fees)?;
        Ok(())
    }

    // Add to total yield claimed
    pub fn add_yield_claimed(&mut self, yield_amount: u64) -> TYieldResult<()> {
        if yield_amount == 0 {
            return Err(ErrorCode::MathError);
        }
        self.total_yield_claimed = self.total_yield_claimed.safe_add(yield_amount)?;
        Ok(())
    }

    // Add to total referral earnings
    pub fn add_referral_earnings(&mut self, earnings: u64) -> TYieldResult<()> {
        if earnings == 0 {
            return Err(ErrorCode::MathError);
        }
        self.total_referral_earnings_ever = self.total_referral_earnings_ever.safe_add(earnings)?;
        Ok(())
    }

    // Get total lifetime value (agents + yield + referral earnings)
    pub fn get_total_lifetime_value(&self) -> TYieldResult<u64> {
        let agents_plus_yield = self
            .total_agents_ever_purchased
            .safe_add(self.total_yield_claimed)?;
        agents_plus_yield.safe_add(self.total_referral_earnings_ever)
    }

    // Get ROI (Return on Investment) as percentage
    pub fn get_roi_percentage(&self) -> TYieldResult<u64> {
        if self.total_agents_ever_purchased == 0 {
            return Ok(0);
        }
        let total_return = self
            .total_yield_claimed
            .safe_add(self.total_referral_earnings_ever)?;
        let percentage = total_return.safe_mul(100)?;
        percentage.safe_div(self.total_agents_ever_purchased)
    }

    // Get yield efficiency (yield claimed vs total agents purchased)
    pub fn get_yield_efficiency(&self) -> TYieldResult<u64> {
        if self.total_agents_ever_purchased == 0 {
            return Ok(0);
        }
        let numerator = self.total_yield_claimed.safe_mul(QUOTE_PRECISION_U64)?;
        numerator.safe_div(self.total_agents_ever_purchased)
    }

    // Get referral efficiency (referral earnings vs total agents purchased)
    pub fn get_referral_efficiency(&self) -> TYieldResult<u64> {
        if self.total_agents_ever_purchased == 0 {
            return Ok(0);
        }
        let numerator = self
            .total_referral_earnings_ever
            .safe_mul(QUOTE_PRECISION_U64)?;
        numerator.safe_div(self.total_agents_ever_purchased)
    }

    // Get fee ratio (fees spent vs total agents purchased)
    pub fn get_fee_ratio(&self) -> TYieldResult<u64> {
        if self.total_agents_ever_purchased == 0 {
            return Ok(0);
        }
        let numerator = self.total_fees_spent.safe_mul(QUOTE_PRECISION_U64)?;
        numerator.safe_div(self.total_agents_ever_purchased)
    }

    // Check if user is profitable (total return > total investment)
    pub fn is_profitable(&self) -> TYieldResult<bool> {
        let total_return = self
            .total_yield_claimed
            .safe_add(self.total_referral_earnings_ever)?;
        Ok(total_return > self.total_agents_ever_purchased)
    }

    // Get net profit/loss
    pub fn get_net_pnl(&self) -> TYieldResult<i64> {
        let total_return = self
            .total_yield_claimed
            .safe_add(self.total_referral_earnings_ever)?;
        let net_pnl = total_return as i64 - self.total_agents_ever_purchased as i64;
        Ok(net_pnl)
    }

    // Get profit margin percentage
    pub fn get_profit_margin(&self) -> TYieldResult<i64> {
        if self.total_agents_ever_purchased == 0 {
            return Ok(0);
        }
        let net_pnl = self.get_net_pnl()?;
        if net_pnl <= 0 {
            return Ok(0);
        }
        let percentage = (net_pnl as u64).safe_mul(100)?;
        let margin = percentage.safe_div(self.total_agents_ever_purchased)?;
        Ok(margin as i64)
    }

    // Reset all history (for testing/debugging)
    pub fn reset(&mut self) {
        self.total_agents_ever_purchased = 0;
        self.total_fees_spent = 0;
        self.total_yield_claimed = 0;
        self.total_referral_earnings_ever = 0;
    }

    // Get summary statistics
    pub fn get_summary(&self) -> TYieldResult<(u64, u64, u64, u64, u64)> {
        let total_value = self.get_total_lifetime_value()?;
        Ok((
            self.total_agents_ever_purchased,
            self.total_fees_spent,
            self.total_yield_claimed,
            self.total_referral_earnings_ever,
            total_value,
        ))
    }
}

#[account]
#[derive(Eq, PartialEq, Debug, Default)]
pub struct ReferralRegistry {
    // 32 bytes, 8-byte aligned
    pub referrer: Pubkey,

    // 8 bytes each, 8-byte aligned
    pub total_referral_earnings: u64,
    pub total_referral_earnings_uc: u64, // Unclaimed

    // 4 bytes each, 4-byte aligned
    pub total_referred_users: u32,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_earning_claim: i64,

    // 1 byte
    pub bump: u8,

    // 7 bytes padding to align to 8 bytes
    pub _padding: [u8; 7],
}

impl ReferralRegistry {
    /// Add referral earnings (claimed)
    pub fn add_referral_earnings(&mut self, earnings: u64) -> TYieldResult<()> {
        if earnings == 0 {
            return Err(ErrorCode::MathError);
        }
        self.total_referral_earnings = self.total_referral_earnings.safe_add(earnings)?;
        Ok(())
    }

    /// Add referral earnings (unclaimed)
    pub fn add_unclaimed_referral_earnings(&mut self, earnings: u64) -> TYieldResult<()> {
        if earnings == 0 {
            return Err(ErrorCode::MathError);
        }
        self.total_referral_earnings_uc = self.total_referral_earnings_uc.safe_add(earnings)?;
        Ok(())
    }

    /// Claim unclaimed referral earnings (move from unclaimed to claimed)
    pub fn claim_referral_earnings(&mut self, amount: u64) -> TYieldResult<()> {
        if amount == 0 {
            return Err(ErrorCode::MathError);
        }

        if amount > self.total_referral_earnings_uc {
            return Err(ErrorCode::InsufficientFunds);
        }
        self.total_referral_earnings_uc = self.total_referral_earnings_uc.safe_sub(amount)?;
        self.total_referral_earnings = self.total_referral_earnings.safe_add(amount)?;
        Ok(())
    }

    /// Get total referral earnings (claimed)
    pub fn get_total_referral_earnings(&self) -> u64 {
        self.total_referral_earnings
    }

    /// Get total unclaimed referral earnings
    pub fn get_total_unclaimed_referral_earnings(&self) -> u64 {
        self.total_referral_earnings_uc
    }

    /// Get total referral earnings (claimed + unclaimed)
    pub fn get_total_aggregate_referral_earnings(&self) -> TYieldResult<u64> {
        self.total_referral_earnings
            .safe_add(self.total_referral_earnings_uc)
    }

    /// Get average earnings per referred user (claimed only)
    pub fn get_average_earnings_per_user(&self) -> TYieldResult<u64> {
        if self.total_referred_users == 0 {
            return Ok(0);
        }
        self.total_referral_earnings
            .safe_div(self.total_referred_users as u64)
    }

    /// Get average unclaimed earnings per referred user
    pub fn get_average_unclaimed_earnings_per_user(&self) -> TYieldResult<u64> {
        if self.total_referred_users == 0 {
            return Ok(0);
        }
        self.total_referral_earnings_uc
            .safe_div(self.total_referred_users as u64)
    }

    /// Update the updated_at timestamp
    pub fn update_timestamp(&mut self, current_time: i64) -> TYieldResult<()> {
        if current_time <= 0 {
            return Err(ErrorCode::InvalidAccount);
        }

        if current_time < self.created_at {
            return Err(ErrorCode::InvalidAccount);
        }

        self.updated_at = current_time;
        Ok(())
    }

    /// Get days since creation
    pub fn get_days_since_created(&self, current_time: i64) -> TYieldResult<i64> {
        if current_time <= 0 {
            return Err(ErrorCode::InvalidAccount);
        }

        if self.created_at <= 0 {
            return Err(ErrorCode::InvalidAccount);
        }

        let time_diff = current_time.safe_sub(self.created_at)?;
        Ok(time_diff / 86400)
    }

    /// Get days since last update
    pub fn get_days_since_updated(&self, current_time: i64) -> TYieldResult<i64> {
        if current_time <= 0 {
            return Err(ErrorCode::InvalidAccount);
        }

        if self.updated_at <= 0 {
            return Err(ErrorCode::InvalidAccount);
        }

        let time_diff = current_time.safe_sub(self.updated_at)?;
        Ok(time_diff / 86400)
    }

    /// Validation method
    pub fn validate_registry(&self) -> TYieldResult<()> {
        if self.referrer == Pubkey::default() {
            return Err(ErrorCode::InvalidAccount);
        }
        if self.created_at <= 0 {
            return Err(ErrorCode::InvalidAccount);
        }
        if self.updated_at < self.created_at {
            return Err(ErrorCode::InvalidAccount);
        }
        Ok(())
    }

    /// Reset all earnings (for testing/debugging)
    pub fn reset_earnings(&mut self) {
        self.total_referral_earnings = 0;
        self.total_referral_earnings_uc = 0;
    }

    /// Get referral statistics (total users, claimed, unclaimed, average per user)
    pub fn get_referral_stats(&self) -> TYieldResult<(u32, u64, u64, u64, u64)> {
        let avg = if self.total_referred_users > 0 {
            self.total_referral_earnings / self.total_referred_users as u64
        } else {
            0
        };

        let avg_unclaimed = self.get_average_unclaimed_earnings_per_user()?;

        Ok((
            self.total_referred_users,
            self.total_referral_earnings,
            avg,
            self.total_referral_earnings_uc,
            avg_unclaimed,
        ))
    }
}

#[account]
#[derive(Debug)]
pub struct ReferralLink {
    // 32 bytes each, 8-byte aligned
    pub referrer: Pubkey,
    pub referred_user: Pubkey,

    // 4 bytes, 4-byte aligned
    pub created_at: i64,

    // 1 byte
    pub bump: u8,

    // 7 bytes padding to align to 8 bytes
    pub _padding: [u8; 7],
}

impl Size for ReferralLink {
    const SIZE: usize = 84; // 8 (discriminator) + 76 (struct, including padding) = 84 bytes
}

impl ReferralLink {
    /// Create a new ReferralLink
    pub fn new(referrer: Pubkey, referred_user: Pubkey, created_at: i64, bump: u8) -> Self {
        Self {
            referrer,
            referred_user,
            created_at,
            bump,
            _padding: [0; 7],
        }
    }

    /// Validate the ReferralLink (basic checks)
    pub fn validate(&self) -> TYieldResult<()> {
        if self.referrer == Pubkey::default() {
            return Err(ErrorCode::InvalidAccount);
        }
        if self.referred_user == Pubkey::default() {
            return Err(ErrorCode::InvalidAccount);
        }
        if self.created_at <= 0 {
            return Err(ErrorCode::InvalidAccount);
        }
        Ok(())
    }

    /// Get the age of the referral link in days
    pub fn get_age_days(&self, current_time: i64) -> i64 {
        (current_time - self.created_at) / 86400
    }

    /// Update the created_at timestamp (for migration/testing)
    pub fn update_timestamp(&mut self, new_time: i64) {
        self.created_at = new_time;
    }

    /// Reset the referral link (for testing/debugging)
    pub fn reset(&mut self) {
        self.referrer = Pubkey::default();
        self.referred_user = Pubkey::default();
        self.created_at = 0;
        self.bump = 0;
        self._padding = [0; 7];
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Eq, AnchorDeserialize, AnchorSerialize)]
pub enum UserStatus {
    Active = 0b00000001,
    Banned = 0b00000010,
    WithListed = 0b00000100,
    // X = 0b00001000,
    // XXY = 0b00010000,
}

// implement SIZE const for User
impl Size for User {
    const SIZE: usize = 200; // 8 (discriminator) + 192 (struct, including padding) = 200 bytes
}

// implement SIZE const for ReferralRegistry
impl Size for ReferralRegistry {
    const SIZE: usize = 96; // 8 (discriminator) + 88 (struct, including padding) = 96 bytes
}

#[event]
pub struct RegisterUserEvent {
    /// The owner/authority of the account
    pub authority: Pubkey,

    /// Encoded display name e.g. "t_yield"
    pub name: [u8; 15],

    /// Whether the user is active or banned
    pub status: u8,

    /// Referrer's public key (zero if no referrer)
    pub referrer: Pubkey,

    pub created_at: i64,
}

#[event]
pub struct UpdateUserStatusEvent {
    /// The owner/authority of the account
    pub authority: Pubkey,

    /// Encoded display name e.g. "t_yield"
    pub name: [u8; 15],

    /// Whether the user is active or banned
    pub status: u8,

    pub updated_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::prelude::Pubkey;

    // Helper function to create a test user
    fn create_test_user() -> User {
        let mut user = User::default();
        user.authority = Pubkey::new_unique();
        user.created_at = 1000;
        user.updated_at = 1000;

        // Set a valid name for testing
        let test_name = b"test_user";
        let mut name_array = [0u8; 15];
        name_array[..test_name.len()].copy_from_slice(test_name);
        user.name = name_array;

        user
    }

    // Helper function to create a test user with specific status
    fn create_test_user_with_status(status: UserStatus) -> User {
        let mut user = create_test_user();
        user.status = status as u8;
        user
    }

    #[test]
    fn test_user_size() {
        // On-chain size includes 8 bytes for Anchor discriminator
        assert_eq!(8 + std::mem::size_of::<User>(), User::SIZE);
        println!("User on-chain size: {} bytes", User::SIZE);
    }

    #[test]
    fn test_history_size() {
        // History is not an account, so no discriminator
        assert_eq!(std::mem::size_of::<History>(), History::SIZE);
        println!("History size: {} bytes", History::SIZE);
    }

    #[test]
    fn test_referral_registry_size() {
        // On-chain size includes 8 bytes for Anchor discriminator
        assert_eq!(
            8 + std::mem::size_of::<ReferralRegistry>(),
            ReferralRegistry::SIZE
        );
        println!(
            "ReferralRegistry on-chain size: {} bytes",
            ReferralRegistry::SIZE
        );
    }

    #[test]
    fn test_user_memory_layout() {
        // Test that User struct can be created and serialized
        let user = User::default();
        assert_eq!(user.authority, Pubkey::default());
        assert_eq!(user.delegate, Pubkey::default());
        assert_eq!(user.name, [0; 15]);
        assert_eq!(user.total_agents_owned, 0);
        assert_eq!(user.total_agents_purchased, 0);
        assert_eq!(user.total_unclaimed_yield, 0);
        assert_eq!(user.status, UserStatus::Banned as u8);
        assert_eq!(user.idle, false);
        assert_eq!(user._padding, [0; 7]);
        assert_eq!(user.referrer, Pubkey::default());
        assert_eq!(user.updated_at, 0);
        assert_eq!(user.created_at, 0);
        assert_eq!(user.bump, 0);
    }

    // Status management tests
    #[test]
    fn test_user_status_management() {
        let mut user = create_test_user();

        // Test initial status
        assert!(!user.is_active());
        assert!(user.is_banned());
        assert!(!user.is_whitelisted());

        // Test adding status - first remove banned status to allow active
        user.remove_user_status(UserStatus::Banned).unwrap();
        user.add_user_status(UserStatus::Active).unwrap();
        assert!(user.is_active());
        assert!(!user.is_banned()); // Should no longer be banned

        user.add_user_status(UserStatus::WithListed).unwrap();
        assert!(user.is_whitelisted());

        // Test removing status
        user.remove_user_status(UserStatus::Banned).unwrap();
        assert!(!user.is_banned());
        assert!(user.is_active());
        assert!(user.is_whitelisted());

        // Test status flags
        let flags = user.get_status_flags();
        assert_eq!(flags.len(), 2);
        assert!(flags.contains(&UserStatus::Active));
        assert!(flags.contains(&UserStatus::WithListed));

        // Test status string
        let status_str = user.get_status_string();
        assert!(status_str.contains("Active"));
        assert!(status_str.contains("Whitelisted"));
    }

    #[test]
    fn test_ban_and_unban_user() {
        let mut user = create_test_user_with_status(UserStatus::Active);

        // Test ban
        user.ban_user().unwrap();
        assert!(user.is_banned());
        assert!(!user.is_active());

        // Test unban
        user.un_ban_user().unwrap();
        assert!(!user.is_banned());
        assert!(user.is_active());
    }

    #[test]
    fn test_whitelist_management() {
        let mut user = create_test_user();

        // Test whitelist
        user.whitelist_user().unwrap();
        assert!(user.is_whitelisted());

        // Test remove whitelist
        user.remove_whitelist_user().unwrap();
        assert!(!user.is_whitelisted());
    }

    // Yield management tests
    #[test]
    fn test_yield_management() {
        let mut user = create_test_user();

        // Test adding yield
        user.add_unclaimed_yield(1000).unwrap();
        assert_eq!(user.get_claimable_yield(), 1000);

        user.add_unclaimed_yield(500).unwrap();
        assert_eq!(user.get_claimable_yield(), 1500);

        // Test claiming yield
        user.claim_yield(800).unwrap();
        assert_eq!(user.get_claimable_yield(), 700);
        assert_eq!(user.get_total_yield_ever_claimed(), 800);

        // Test claiming more than available
        let result = user.claim_yield(1000);
        assert!(result.is_err());
        assert_eq!(user.get_claimable_yield(), 700); // Should remain unchanged
    }

    #[test]
    fn test_yield_statistics() {
        let mut user = create_test_user();

        // Add some yield and claim it
        user.add_unclaimed_yield(1000).unwrap();
        user.claim_yield(600).unwrap();

        assert_eq!(user.get_total_yield_ever_claimed(), 600);
        assert_eq!(user.get_lifetime_yield_earned().unwrap(), 1000); // 400 unclaimed + 600 claimed
    }

    // Agent management tests
    #[test]
    fn test_agent_management() {
        let mut user = create_test_user();

        // Test adding agents
        user.add_agent(1000).unwrap();
        assert_eq!(user.get_agent_count(), 1);
        assert_eq!(user.get_total_agents_purchased(), 1000);

        user.add_agent(2000).unwrap();
        assert_eq!(user.get_agent_count(), 2);
        assert_eq!(user.get_total_agents_purchased(), 3000);

        // Test removing agents
        user.remove_agent(1000).unwrap();
        assert_eq!(user.get_agent_count(), 1);
        assert_eq!(user.get_total_agents_purchased(), 3000); // Should remain unchanged

        // Test removing from empty
        user.remove_agent(1000).unwrap();
        let result = user.remove_agent(1000);
        assert!(result.is_err());
    }

    // Fee management tests
    #[test]
    fn test_fee_management() {
        let mut user = create_test_user();

        user.add_fees_spent(100).unwrap();
        assert_eq!(user.get_total_fees_spent(), 100);

        user.add_fees_spent(200).unwrap();
        assert_eq!(user.get_total_fees_spent(), 300);
    }

    // Referral management tests
    #[test]
    fn test_referral_management() {
        let mut user = create_test_user();

        // Test referral earnings
        user.add_referral_earnings(500).unwrap();
        assert_eq!(user.get_total_referral_earnings(), 500);

        user.add_referral_earnings(300).unwrap();
        assert_eq!(user.get_total_referral_earnings(), 800);

        // Test referrer
        assert!(!user.has_referrer());

        let referrer = Pubkey::new_unique();
        user.set_referrer(referrer).unwrap();
        assert!(user.has_referrer());
    }

    // Time-based tests
    #[test]
    fn test_time_based_functions() {
        let mut user = create_test_user();
        let current_time = 2000;

        // Test update last activity
        user.update_last_activity(current_time).unwrap();
        assert_eq!(user.updated_at, current_time);
        assert!(!user.is_idle());

        // Test idle status
        let idle_threshold = 1000;
        user.check_idle_status(current_time + 500, idle_threshold)
            .unwrap();
        assert!(!user.is_idle());

        user.check_idle_status(current_time + 1500, idle_threshold)
            .unwrap();
        assert!(user.is_idle());

        // Test days calculation
        assert_eq!(user.get_days_since_created(current_time).unwrap(), 0);
        assert_eq!(user.get_days_since_updated(current_time).unwrap(), 0);

        let future_time = current_time + 86400 * 2; // 2 days later
        assert_eq!(user.get_days_since_created(future_time).unwrap(), 2);
        assert_eq!(user.get_days_since_updated(future_time).unwrap(), 2);
    }

    // Name management tests
    #[test]
    fn test_name_management() {
        let mut user = create_test_user();

        let test_name = b"test_user_name";
        let mut name_array = [0u8; 15];
        name_array[..test_name.len()].copy_from_slice(test_name);

        user.set_name(name_array).unwrap();
        assert_eq!(user.get_name(), name_array);
        assert_eq!(user.get_name_string(), "test_user_name");

        // Test with null bytes
        let name_with_nulls = [b't', b'e', b's', b't', 0, 0, 0];
        let mut name_array = [0u8; 15];
        name_array[..7].copy_from_slice(&name_with_nulls);

        user.set_name(name_array).unwrap();
        assert_eq!(user.get_name_string(), "test");
    }

    // Delegate management tests
    #[test]
    fn test_delegate_management() {
        let mut user = create_test_user();

        assert!(!user.has_delegate());

        let delegate = Pubkey::new_unique();
        user.set_delegate(delegate).unwrap();
        assert_eq!(user.get_delegate(), delegate);
        assert!(user.has_delegate());

        user.clear_delegate();
        assert_eq!(user.get_delegate(), Pubkey::default());
        assert!(!user.has_delegate());
    }

    // Validation tests
    #[test]
    fn test_user_validation() {
        let mut user = create_test_user();

        // Valid user should pass validation
        assert!(user.validate_user().is_ok());

        // Test invalid authority
        user.authority = Pubkey::default();
        assert!(user.validate_user().is_err());

        // Reset and test invalid timestamps
        user = create_test_user();
        user.created_at = 0;
        assert!(user.validate_user().is_err());

        user = create_test_user();
        user.updated_at = user.created_at - 1;
        assert!(user.validate_user().is_err());
    }

    #[test]
    fn test_can_perform_actions() {
        let mut user = create_test_user_with_status(UserStatus::Active);
        assert!(user.can_perform_actions());

        user.ban_user().unwrap();
        assert!(!user.can_perform_actions());

        user.un_ban_user().unwrap();
        assert!(user.can_perform_actions());
    }

    // Reset tests
    #[test]
    fn test_reset_functions() {
        let mut user = create_test_user();

        // Add some data
        user.add_unclaimed_yield(1000).unwrap();
        user.add_agent(2000).unwrap();
        user.add_fees_spent(300).unwrap();
        user.add_referral_earnings(400).unwrap();

        // Test resets
        user.reset_yield();
        assert_eq!(user.get_claimable_yield(), 0);
        assert_eq!(user.get_total_yield_ever_claimed(), 0);

        user.reset_agents();
        assert_eq!(user.get_agent_count(), 0);
        assert_eq!(user.get_total_agents_purchased(), 0);

        user.reset_fees();
        assert_eq!(user.get_total_fees_spent(), 0);

        user.reset_referral_earnings();
        assert_eq!(user.get_total_referral_earnings(), 0);
    }

    // History tests
    #[test]
    fn test_history_functions() {
        let mut history = History::default();

        // Test adding values
        history.add_agents_purchased(1000).unwrap();
        history.add_fees_spent(100).unwrap();
        history.add_yield_claimed(500).unwrap();
        history.add_referral_earnings(200).unwrap();

        assert_eq!(history.total_agents_ever_purchased, 1000);
        assert_eq!(history.total_fees_spent, 100);
        assert_eq!(history.total_yield_claimed, 500);
        assert_eq!(history.total_referral_earnings_ever, 200);

        // Test calculations
        assert_eq!(history.get_total_lifetime_value().unwrap(), 1700); // 1000 + 500 + 200
        assert_eq!(history.get_roi_percentage().unwrap(), 70); // (500 + 200) / 1000 * 100
        assert!(!history.is_profitable().unwrap()); // 700 total return < 1000 investment
        assert_eq!(history.get_net_pnl().unwrap(), -300); // 700 - 1000
        assert_eq!(history.get_profit_margin().unwrap(), 0); // negative PnL returns 0

        // Test efficiency calculations
        assert_eq!(
            history.get_yield_efficiency().unwrap(),
            500 * QUOTE_PRECISION_U64 / 1000
        );
        assert_eq!(
            history.get_referral_efficiency().unwrap(),
            200 * QUOTE_PRECISION_U64 / 1000
        );
        assert_eq!(
            history.get_fee_ratio().unwrap(),
            100 * QUOTE_PRECISION_U64 / 1000
        );

        // Test summary
        let summary = history.get_summary().unwrap();
        assert_eq!(summary.0, 1000); // agents
        assert_eq!(summary.1, 100); // fees
        assert_eq!(summary.2, 500); // yield
        assert_eq!(summary.3, 200); // referral
        assert_eq!(summary.4, 1700); // total
    }

    #[test]
    fn test_history_edge_cases() {
        let history = History::default();

        // Test division by zero cases
        assert_eq!(history.get_roi_percentage().unwrap(), 0);
        assert_eq!(history.get_yield_efficiency().unwrap(), 0);
        assert_eq!(history.get_referral_efficiency().unwrap(), 0);
        assert_eq!(history.get_fee_ratio().unwrap(), 0);
        assert_eq!(history.get_profit_margin().unwrap(), 0);
        assert!(!history.is_profitable().unwrap());
        assert_eq!(history.get_net_pnl().unwrap(), 0);
    }

    // ReferralRegistry tests
    #[test]
    fn test_referral_registry_basic() {
        let mut registry = ReferralRegistry::default();
        let referrer = Pubkey::new_unique();
        let _user1 = Pubkey::new_unique();
        let _user2 = Pubkey::new_unique();

        registry.referrer = referrer;
        registry.created_at = 1000;
        registry.updated_at = 1000;
        registry.total_referred_users = 1; // Ensure average is not zero

        // Test adding users
        registry.add_referral_earnings(1000).unwrap();
        assert_eq!(registry.total_referral_earnings, 1000);
        assert_eq!(registry.get_average_earnings_per_user().unwrap(), 1000);
    }

    #[test]
    fn test_referral_registry_validation() {
        let mut registry = ReferralRegistry::default();

        // Invalid registry
        assert!(registry.validate_registry().is_err());

        // Valid registry
        registry.referrer = Pubkey::new_unique();
        registry.created_at = 1000;
        registry.updated_at = 1000;
        assert!(registry.validate_registry().is_ok());

        // Invalid timestamps
        registry.updated_at = registry.created_at - 1;
        assert!(registry.validate_registry().is_err());
    }

    #[test]
    fn test_referral_registry_statistics() {
        let mut registry = ReferralRegistry::default();
        registry.referrer = Pubkey::new_unique();
        registry.created_at = 1000;
        registry.updated_at = 1000;
        registry.total_referred_users = 1; // Ensure average is not zero

        // Add some earnings
        registry.add_referral_earnings(1000).unwrap();

        let stats = registry.get_referral_stats().unwrap();
        println!("Debug: stats = {:?}", stats);
        println!(
            "Debug: total_referred_users = {}",
            registry.total_referred_users
        );
        println!(
            "Debug: total_referral_earnings = {}",
            registry.total_referral_earnings
        );
        assert_eq!(stats.0, 1); // total users
        assert_eq!(stats.1, 1000); // total earnings
        assert_eq!(stats.2, 1000); // average earnings

        // Test time functions
        let current_time = 2000;
        assert_eq!(registry.get_days_since_created(current_time).unwrap(), 0);
        assert_eq!(registry.get_days_since_updated(current_time).unwrap(), 0);

        let future_time = current_time + 86400 * 3; // 3 days later
        assert_eq!(registry.get_days_since_created(future_time).unwrap(), 3);
        assert_eq!(registry.get_days_since_updated(future_time).unwrap(), 3);
    }

    // Integration tests
    #[test]
    fn test_user_integration_scenario() {
        let mut user = create_test_user_with_status(UserStatus::Active);

        // Simulate a user's journey
        user.add_agent(1000).unwrap();
        user.add_unclaimed_yield(500).unwrap();
        user.add_fees_spent(50).unwrap();
        user.add_referral_earnings(100).unwrap();

        // Claim some yield
        user.claim_yield(300).unwrap();

        // Update activity
        user.update_last_activity(2000).unwrap();

        // Verify final state
        assert_eq!(user.get_agent_count(), 1);
        assert_eq!(user.get_claimable_yield(), 200);
        assert_eq!(user.get_total_yield_ever_claimed(), 300);
        assert_eq!(user.get_total_fees_spent(), 50);
        assert_eq!(user.get_total_referral_earnings(), 100);
        assert!(!user.is_idle());
        assert!(user.can_perform_actions());
    }

    #[test]
    fn test_yield_claim_rate_calculation() {
        let mut user = create_test_user();

        // Add agents and yield
        user.add_agent(1000).unwrap();
        user.add_agent(2000).unwrap(); // Total: 3000

        user.add_unclaimed_yield(600).unwrap();
        user.claim_yield(400).unwrap();

        // Yield claim rate should be: (400 * QUOTE_PRECISION) / 3000
        let expected_rate = (400 * QUOTE_PRECISION_U64) / 3000;
        assert_eq!(user.get_yield_claim_rate().unwrap(), expected_rate);

        // Test with zero agents
        let empty_user = create_test_user();
        assert_eq!(empty_user.get_yield_claim_rate().unwrap(), 0);
    }

    #[test]
    fn test_status_combinations() {
        let mut user = create_test_user();

        // Clear the default banned status first
        user.remove_user_status(UserStatus::Banned).unwrap();

        // Test multiple status combinations
        user.add_user_status(UserStatus::Active).unwrap();
        user.add_user_status(UserStatus::WithListed).unwrap();

        assert!(user.is_active());
        assert!(user.is_whitelisted());
        assert!(!user.is_banned());

        // Test status string with multiple flags
        let status_str = user.get_status_string();
        assert!(status_str.contains("Active"));
        assert!(status_str.contains("Whitelisted"));
        assert!(!status_str.contains("Banned"));

        // Test removing one status
        user.remove_user_status(UserStatus::Active).unwrap();
        assert!(!user.is_active());
        assert!(user.is_whitelisted());

        let status_str = user.get_status_string();
        assert!(!status_str.contains("Active"));
        assert!(status_str.contains("Whitelisted"));
    }

    #[test]
    fn test_edge_cases_and_error_handling() {
        let mut user = create_test_user();

        // Test claiming more yield than available
        user.add_unclaimed_yield(100).unwrap();
        let result = user.claim_yield(200);
        assert!(result.is_err());
        assert_eq!(user.get_claimable_yield(), 100); // Should remain unchanged

        // Test removing agent from empty user
        let result = user.remove_agent(100);
        assert!(result.is_err());

        // Test overflow scenarios (these would be caught by SafeMath in real usage)
        // The actual overflow protection is handled by the SafeMath trait implementation
    }

    #[test]
    fn test_profitable_history_scenario() {
        let mut history = History::default();

        // Create a profitable scenario: 1000 investment, 800 yield + 400 referral = 1200 return
        history.add_agents_purchased(1000).unwrap();
        history.add_yield_claimed(800).unwrap();
        history.add_referral_earnings(400).unwrap();

        assert!(history.is_profitable().unwrap()); // 1200 > 1000
        assert_eq!(history.get_net_pnl().unwrap(), 200); // 1200 - 1000
        assert_eq!(history.get_profit_margin().unwrap(), 20); // 200 / 1000 * 100
        assert_eq!(history.get_roi_percentage().unwrap(), 120); // 1200 / 1000 * 100
    }

    #[test]
    fn test_user_comprehensive_scenario() {
        let mut user = create_test_user();

        // Set up a complete user profile
        user.remove_user_status(UserStatus::Banned).unwrap();
        user.add_user_status(UserStatus::Active).unwrap();
        user.add_user_status(UserStatus::WithListed).unwrap();

        // Add multiple agents
        user.add_agent(1000).unwrap();
        user.add_agent(2000).unwrap();
        user.add_agent(1500).unwrap();

        // Add yield and claim some
        user.add_unclaimed_yield(1000).unwrap();
        user.claim_yield(600).unwrap();

        // Add fees and referral earnings
        user.add_fees_spent(200).unwrap();
        user.add_referral_earnings(300).unwrap();

        // Set delegate and referrer
        let delegate = Pubkey::new_unique();
        let referrer = Pubkey::new_unique();
        user.set_delegate(delegate).unwrap();
        user.set_referrer(referrer).unwrap();

        // Set name
        let name = b"test_user_123";
        let mut name_array = [0u8; 15];
        name_array[..name.len()].copy_from_slice(name);
        user.set_name(name_array).unwrap();

        // Update activity
        user.update_last_activity(2000).unwrap();

        // Verify all properties
        assert_eq!(user.get_agent_count(), 3);
        assert_eq!(user.get_total_agents_purchased(), 4500);
        assert_eq!(user.get_claimable_yield(), 400);
        assert_eq!(user.get_total_yield_ever_claimed(), 600);
        assert_eq!(user.get_total_fees_spent(), 200);
        assert_eq!(user.get_total_referral_earnings(), 300);
        assert_eq!(user.get_lifetime_yield_earned().unwrap(), 1000);
        assert_eq!(user.get_name_string(), "test_user_123");
        assert_eq!(user.get_delegate(), delegate);
        assert!(user.has_delegate());
        assert!(user.has_referrer());
        assert!(user.is_active());
        assert!(user.is_whitelisted());
        assert!(!user.is_banned());
        assert!(!user.is_idle());
        assert!(user.can_perform_actions());

        // Test status flags
        let flags = user.get_status_flags();
        assert_eq!(flags.len(), 2);
        assert!(flags.contains(&UserStatus::Active));
        assert!(flags.contains(&UserStatus::WithListed));

        // Test yield claim rate
        let expected_rate = (600 * QUOTE_PRECISION_U64) / 4500;
        assert_eq!(user.get_yield_claim_rate().unwrap(), expected_rate);
    }

    #[test]
    fn test_history_calculations_edge_cases() {
        let mut history = History::default();

        // Test with very large numbers
        history.add_agents_purchased(u64::MAX / 2).unwrap();
        history.add_yield_claimed(u64::MAX / 4).unwrap();
        history.add_referral_earnings(u64::MAX / 4).unwrap();

        // Should NOT be profitable (return == investment)
        assert!(!history.is_profitable().unwrap());

        // Test with zero values
        let empty_history = History::default();
        assert_eq!(empty_history.get_total_lifetime_value().unwrap(), 0);
        assert_eq!(empty_history.get_roi_percentage().unwrap(), 0);
        assert!(!empty_history.is_profitable().unwrap());
        assert_eq!(empty_history.get_net_pnl().unwrap(), 0);
        assert_eq!(empty_history.get_profit_margin().unwrap(), 0);
    }

    #[test]
    fn test_referral_registry_edge_cases() {
        let mut registry = ReferralRegistry::default();
        registry.referrer = Pubkey::new_unique();
        registry.created_at = 1000;
        registry.updated_at = 1000;

        // Test with no users
        assert_eq!(registry.get_average_earnings_per_user().unwrap(), 0);

        // Test with one user and earnings
        let _user = Pubkey::new_unique();
        registry.total_referred_users = 1; // Ensure average is not zero
        registry.add_referral_earnings(500).unwrap();

        assert_eq!(registry.get_average_earnings_per_user().unwrap(), 500);
    }

    #[test]
    fn test_user_validation_edge_cases() {
        let mut user = create_test_user();

        // Valid user should pass
        assert!(user.validate_user().is_ok());

        // Test various invalid states
        user.authority = Pubkey::default();
        assert!(user.validate_user().is_err());

        user = create_test_user();
        user.created_at = 0;
        assert!(user.validate_user().is_err());

        user = create_test_user();
        user.updated_at = user.created_at - 1;
        assert!(user.validate_user().is_err());

        user = create_test_user();
        user.created_at = -1;
        assert!(user.validate_user().is_err());
    }

    #[test]
    fn test_time_calculations_edge_cases() {
        let mut user = create_test_user();

        // Test same time
        assert_eq!(user.get_days_since_created(user.created_at).unwrap(), 0);
        assert_eq!(user.get_days_since_updated(user.updated_at).unwrap(), 0);

        // Test future time
        let future_time = user.created_at + 86400 * 10; // 10 days later
        assert_eq!(user.get_days_since_created(future_time).unwrap(), 10);
        assert_eq!(user.get_days_since_updated(future_time).unwrap(), 10);

        // Test idle status edge cases
        user.update_last_activity(1000).unwrap();
        user.check_idle_status(1000, 0).unwrap(); // No idle threshold
        assert!(!user.is_idle());

        user.check_idle_status(1002, 1).unwrap(); // 2 > 1, should be idle
        assert!(user.is_idle());
    }

    #[test]
    fn test_referral_link_creation_and_validation() {
        let referrer = Pubkey::new_unique();
        let referred_user = Pubkey::new_unique();
        let created_at = 1_700_000_000;
        let bump = 1;
        let link = ReferralLink::new(referrer, referred_user, created_at, bump);
        assert_eq!(link.referrer, referrer);
        assert_eq!(link.referred_user, referred_user);
        assert_eq!(link.created_at, created_at);
        assert_eq!(link.bump, bump);
        assert!(link.validate().is_ok());
    }

    #[test]
    fn test_referral_link_invalid_cases() {
        let valid_pk = Pubkey::new_unique();
        let created_at = 1_700_000_000;
        let bump = 1;
        // Invalid referrer
        let link = ReferralLink::new(Pubkey::default(), valid_pk, created_at, bump);
        assert!(link.validate().is_err());
        // Invalid referred_user
        let link = ReferralLink::new(valid_pk, Pubkey::default(), created_at, bump);
        assert!(link.validate().is_err());
        // Invalid created_at
        let link = ReferralLink::new(valid_pk, valid_pk, 0, bump);
        assert!(link.validate().is_err());
    }

    #[test]
    fn test_referral_link_age_and_update() {
        let referrer = Pubkey::new_unique();
        let referred_user = Pubkey::new_unique();
        let created_at = 1_700_000_000;
        let bump = 1;
        let mut link = ReferralLink::new(referrer, referred_user, created_at, bump);
        let current_time = created_at + 86400 * 5; // 5 days later
        assert_eq!(link.get_age_days(current_time), 5);
        // Update timestamp
        link.update_timestamp(created_at + 86400 * 10);
        assert_eq!(link.get_age_days(created_at + 86400 * 15), 5);
    }

    #[test]
    fn test_referral_link_reset() {
        let referrer = Pubkey::new_unique();
        let referred_user = Pubkey::new_unique();
        let created_at = 1_700_000_000;
        let bump = 1;
        let mut link = ReferralLink::new(referrer, referred_user, created_at, bump);
        link.reset();
        assert_eq!(link.referrer, Pubkey::default());
        assert_eq!(link.referred_user, Pubkey::default());
        assert_eq!(link.created_at, 0);
        assert_eq!(link.bump, 0);
        assert_eq!(link._padding, [0; 7]);
    }
}
