use anchor_lang::prelude::*;

use crate::error::{ErrorCode, TYieldResult};
use crate::math::{SafeMath, QUOTE_PRECISION_U64};
use crate::state::Size;

#[account]
#[derive(Eq, PartialEq, Debug)]
pub struct User {
    /// The owner/authority of the account
    pub authority: Pubkey,

    /// An addresses that can control the account on the authority's behalf. Has limited power, cant withdraw
    pub delegate: Pubkey,

    /// Encoded display name e.g. "t_yield"
    pub name: [u8; 32],

    /// The total values of agents the user has purchased
    /// precision: QUOTE_PRECISION
    pub total_agents_purchased: u64,

    /// precision: QUOTE_PRECISION
    pub total_unclaimed_yield: u64,

    /// The total values of agents the user has purchased
    pub total_agents_owned: u32,

    /// Whether the user is active or banned
    pub status: u8,

    /// User is idle if they haven't interacted with the protocol in 1 week
    /// Off-chain sleeper bots can ignore users that are idle
    pub idle: bool,

    /// Padding to align to 8-byte boundary
    pub _padding1: [u8; 2],

    /// Referrer's public key (zero if no referrer)
    pub referrer: Pubkey,

    pub history: History,

    pub updated_at: i32,

    pub created_at: i32,

    pub bump: u8,

    /// Padding to align to 8-byte boundary
    pub _padding2: [u8; 3],
}

impl User {
    pub fn add_user_status(&mut self, status: UserStatus) {
        self.status |= status as u8;
    }

    pub fn remove_user_status(&mut self, status: UserStatus) {
        self.status &= !(status as u8);
    }

    pub fn has_status(&self, status: UserStatus) -> bool {
        (self.status & status as u8) != 0
    }

    pub fn is_active(&self) -> bool {
        self.has_status(UserStatus::Active)
    }

    pub fn is_banned(&self) -> bool {
        self.has_status(UserStatus::Banned)
    }

    pub fn is_whitelisted(&self) -> bool {
        self.has_status(UserStatus::WithListed)
    }

    pub fn ban_user(&mut self) {
        self.remove_user_status(UserStatus::Active);
        self.add_user_status(UserStatus::Banned);
    }

    pub fn un_ban_user(&mut self) {
        self.remove_user_status(UserStatus::Banned);
        self.add_user_status(UserStatus::Active);
    }

    pub fn whitelist_user(&mut self) {
        self.add_user_status(UserStatus::WithListed);
    }

    pub fn remove_whitelist_user(&mut self) {
        self.remove_user_status(UserStatus::WithListed);
    }

    // Yield management methods
    pub fn add_unclaimed_yield(&mut self, amount: u64) -> TYieldResult<()> {
        self.total_unclaimed_yield = self.total_unclaimed_yield.safe_add(amount)?;
        Ok(())
    }

    pub fn claim_yield(&mut self, amount: u64) -> TYieldResult<()> {
        if amount > self.total_unclaimed_yield {
            return Err(ErrorCode::InsufficientFunds);
        }
        self.total_unclaimed_yield = self.total_unclaimed_yield.safe_sub(amount)?;
        self.history.total_yield_claimed = self.history.total_yield_claimed.safe_add(amount)?;
        Ok(())
    }

    pub fn get_claimable_yield(&self) -> u64 {
        self.total_unclaimed_yield
    }

    // Agent management methods
    pub fn add_agent(&mut self, agent_value: u64) -> TYieldResult<()> {
        self.total_agents_owned = self.total_agents_owned.safe_add(1)?;
        self.total_agents_purchased = self.total_agents_purchased.safe_add(agent_value)?;
        self.history.total_agents_ever_purchased = self
            .history
            .total_agents_ever_purchased
            .safe_add(agent_value)?;
        Ok(())
    }

    pub fn remove_agent(&mut self, _agent_value: u64) -> TYieldResult<()> {
        if self.total_agents_owned == 0 {
            return Err(ErrorCode::InsufficientFunds);
        }
        self.total_agents_owned = self.total_agents_owned.safe_sub(1)?;
        Ok(())
    }

    pub fn get_agent_count(&self) -> u32 {
        self.total_agents_owned
    }

    pub fn get_total_agents_purchased(&self) -> u64 {
        self.total_agents_purchased
    }

    // Fee management methods
    pub fn add_fees_spent(&mut self, fees: u64) -> TYieldResult<()> {
        self.history.total_fees_spent = self.history.total_fees_spent.safe_add(fees)?;
        Ok(())
    }

    pub fn get_total_fees_spent(&self) -> u64 {
        self.history.total_fees_spent
    }

    // Referral management methods
    pub fn add_referral_earnings(&mut self, earnings: u64) -> TYieldResult<()> {
        self.history.total_referral_earnings_ever = self
            .history
            .total_referral_earnings_ever
            .safe_add(earnings)?;
        Ok(())
    }

    pub fn get_total_referral_earnings(&self) -> u64 {
        self.history.total_referral_earnings_ever
    }

    pub fn has_referrer(&self) -> bool {
        self.referrer != Pubkey::default()
    }

    // Time-based methods
    pub fn update_last_activity(&mut self, current_time: i32) {
        self.updated_at = current_time;
        self.idle = false;
    }

    pub fn check_idle_status(&mut self, current_time: i32, idle_threshold: i32) {
        let time_since_last_activity = current_time - self.updated_at;
        self.idle = time_since_last_activity > idle_threshold;
    }

    pub fn is_idle(&self) -> bool {
        self.idle
    }

    pub fn get_days_since_created(&self, current_time: i32) -> i32 {
        (current_time - self.created_at) / 86400 // 86400 seconds in a day
    }

    pub fn get_days_since_updated(&self, current_time: i32) -> i32 {
        (current_time - self.updated_at) / 86400
    }

    // Name management methods
    pub fn set_name(&mut self, name: [u8; 32]) {
        self.name = name;
    }

    pub fn get_name(&self) -> [u8; 32] {
        self.name
    }

    pub fn get_name_string(&self) -> String {
        // Convert byte array to string, removing null bytes
        String::from_utf8_lossy(&self.name)
            .trim_matches('\0')
            .to_string()
    }

    // Delegate management methods
    pub fn set_delegate(&mut self, delegate: Pubkey) {
        self.delegate = delegate;
    }

    pub fn get_delegate(&self) -> Pubkey {
        self.delegate
    }

    pub fn has_delegate(&self) -> bool {
        self.delegate != Pubkey::default()
    }

    pub fn clear_delegate(&mut self) {
        self.delegate = Pubkey::default();
    }

    // Status utility methods
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
    pub fn validate_user(&self) -> TYieldResult<()> {
        if self.authority == Pubkey::default() {
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

    pub fn can_perform_actions(&self) -> bool {
        self.is_active() && !self.is_banned()
    }

    // Statistics methods
    pub fn get_total_yield_ever_claimed(&self) -> u64 {
        self.history.total_yield_claimed
    }

    pub fn get_lifetime_yield_earned(&self) -> u64 {
        self.total_unclaimed_yield
            .safe_add(self.history.total_yield_claimed)
            .unwrap_or(0)
    }

    pub fn get_yield_claim_rate(&self) -> u64 {
        if self.history.total_agents_ever_purchased == 0 {
            return 0;
        }
        // Calculate as percentage with QUOTE_PRECISION
        (self
            .history
            .total_yield_claimed
            .safe_mul(QUOTE_PRECISION_U64)
            .unwrap_or(0))
        .safe_div(self.history.total_agents_ever_purchased)
        .unwrap_or(0)
    }

    // Reset methods for testing/debugging
    pub fn reset_yield(&mut self) {
        self.total_unclaimed_yield = 0;
        self.history.total_yield_claimed = 0;
    }

    pub fn reset_agents(&mut self) {
        self.total_agents_owned = 0;
        self.total_agents_purchased = 0;
        self.history.total_agents_ever_purchased = 0;
    }

    pub fn reset_fees(&mut self) {
        self.history.total_fees_spent = 0;
    }

    pub fn reset_referral_earnings(&mut self) {
        self.history.total_referral_earnings_ever = 0;
    }
}

impl Default for User {
    fn default() -> Self {
        Self {
            authority: Pubkey::default(),
            delegate: Pubkey::default(),
            name: [0; 32],
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
            _padding1: [0; 2],
            _padding2: [0; 3],
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
        self.total_agents_ever_purchased = self.total_agents_ever_purchased.safe_add(amount)?;
        Ok(())
    }

    // Add to total fees spent
    pub fn add_fees_spent(&mut self, fees: u64) -> TYieldResult<()> {
        self.total_fees_spent = self.total_fees_spent.safe_add(fees)?;
        Ok(())
    }

    // Add to total yield claimed
    pub fn add_yield_claimed(&mut self, yield_amount: u64) -> TYieldResult<()> {
        self.total_yield_claimed = self.total_yield_claimed.safe_add(yield_amount)?;
        Ok(())
    }

    // Add to total referral earnings
    pub fn add_referral_earnings(&mut self, earnings: u64) -> TYieldResult<()> {
        self.total_referral_earnings_ever = self.total_referral_earnings_ever.safe_add(earnings)?;
        Ok(())
    }

    // Get total lifetime value (agents + yield + referral earnings)
    pub fn get_total_lifetime_value(&self) -> u64 {
        self.total_agents_ever_purchased
            .safe_add(self.total_yield_claimed)
            .unwrap_or(0)
            .safe_add(self.total_referral_earnings_ever)
            .unwrap_or(0)
    }

    // Get ROI (Return on Investment) as percentage
    pub fn get_roi_percentage(&self) -> u64 {
        if self.total_agents_ever_purchased == 0 {
            return 0;
        }
        let total_return = self
            .total_yield_claimed
            .safe_add(self.total_referral_earnings_ever)
            .unwrap_or(0);
        (total_return.safe_mul(100).unwrap_or(0))
            .safe_div(self.total_agents_ever_purchased)
            .unwrap_or(0)
    }

    // Get yield efficiency (yield claimed vs total agents purchased)
    pub fn get_yield_efficiency(&self) -> u64 {
        if self.total_agents_ever_purchased == 0 {
            return 0;
        }
        (self
            .total_yield_claimed
            .safe_mul(QUOTE_PRECISION_U64)
            .unwrap_or(0))
        .safe_div(self.total_agents_ever_purchased)
        .unwrap_or(0)
    }

    // Get referral efficiency (referral earnings vs total agents purchased)
    pub fn get_referral_efficiency(&self) -> u64 {
        if self.total_agents_ever_purchased == 0 {
            return 0;
        }
        (self
            .total_referral_earnings_ever
            .safe_mul(QUOTE_PRECISION_U64)
            .unwrap_or(0))
        .safe_div(self.total_agents_ever_purchased)
        .unwrap_or(0)
    }

    // Get fee ratio (fees spent vs total agents purchased)
    pub fn get_fee_ratio(&self) -> u64 {
        if self.total_agents_ever_purchased == 0 {
            return 0;
        }
        (self
            .total_fees_spent
            .safe_mul(QUOTE_PRECISION_U64)
            .unwrap_or(0))
        .safe_div(self.total_agents_ever_purchased)
        .unwrap_or(0)
    }

    // Check if user is profitable (total return > total investment)
    pub fn is_profitable(&self) -> bool {
        let total_return = self
            .total_yield_claimed
            .safe_add(self.total_referral_earnings_ever)
            .unwrap_or(0);
        total_return > self.total_agents_ever_purchased
    }

    // Get net profit/loss
    pub fn get_net_pnl(&self) -> i64 {
        let total_return = self
            .total_yield_claimed
            .safe_add(self.total_referral_earnings_ever)
            .unwrap_or(0);
        total_return as i64 - self.total_agents_ever_purchased as i64
    }

    // Get profit margin percentage
    pub fn get_profit_margin(&self) -> i64 {
        if self.total_agents_ever_purchased == 0 {
            return 0;
        }
        let net_pnl = self.get_net_pnl();
        if net_pnl <= 0 {
            return 0;
        }
        ((net_pnl as u64).safe_mul(100).unwrap_or(0))
            .safe_div(self.total_agents_ever_purchased)
            .unwrap_or(0) as i64
    }

    // Reset all history (for testing/debugging)
    pub fn reset(&mut self) {
        self.total_agents_ever_purchased = 0;
        self.total_fees_spent = 0;
        self.total_yield_claimed = 0;
        self.total_referral_earnings_ever = 0;
    }

    // Get summary statistics
    pub fn get_summary(&self) -> (u64, u64, u64, u64, u64) {
        (
            self.total_agents_ever_purchased,
            self.total_fees_spent,
            self.total_yield_claimed,
            self.total_referral_earnings_ever,
            self.get_total_lifetime_value(),
        )
    }
}

#[account]
#[derive(Eq, PartialEq, Debug)]
pub struct ReferralRegistry {
    /// The referrer's public key
    pub referrer: Pubkey,

    /// Total number of users referred by this referrer
    pub total_referred_users: u32,

    /// Total referral earnings earned by this referrer
    /// precision: QUOTE_PRECISION
    pub total_referral_earnings: u64,

    /// List of referred users (limited to prevent account size issues)
    pub referred_users: [Pubkey; 100], // Max 100 referred users per referrer

    /// Timestamp when this registry was created
    pub created_at: i32,

    /// Last time this registry was updated
    pub updated_at: i32,

    pub bump: u8,

    /// Padding to align to 8-byte boundary
    pub _padding: [u8; 3],
}

impl ReferralRegistry {
    pub fn is_user_referred(&self, authority: Pubkey) -> bool {
        for i in 0..self.total_referred_users as usize {
            if self.referred_users[i] == authority {
                return true;
            }
        }
        false
    }

    // Add a new referred user
    pub fn add_referred_user(&mut self, user_authority: Pubkey) -> TYieldResult<()> {
        if self.total_referred_users >= 100 {
            return Err(ErrorCode::InvalidAccount);
        }
        if self.is_user_referred(user_authority) {
            return Err(ErrorCode::InvalidAccount);
        }

        let index = self.total_referred_users as usize;
        self.referred_users[index] = user_authority;
        self.total_referred_users = self.total_referred_users.safe_add(1)?;
        Ok(())
    }

    // Remove a referred user (if needed)
    pub fn remove_referred_user(&mut self, user_authority: Pubkey) -> TYieldResult<()> {
        for i in 0..self.total_referred_users as usize {
            if self.referred_users[i] == user_authority {
                // Shift remaining users to fill the gap
                for j in i..(self.total_referred_users as usize - 1) {
                    self.referred_users[j] = self.referred_users[j + 1];
                }
                self.referred_users[self.total_referred_users as usize - 1] = Pubkey::default();
                self.total_referred_users = self.total_referred_users.safe_sub(1)?;
                return Ok(());
            }
        }
        Err(ErrorCode::InvalidAccount)
    }

    // Add referral earnings
    pub fn add_referral_earnings(&mut self, earnings: u64) -> TYieldResult<()> {
        self.total_referral_earnings = self.total_referral_earnings.safe_add(earnings)?;
        Ok(())
    }

    // Get average earnings per referred user
    pub fn get_average_earnings_per_user(&self) -> u64 {
        if self.total_referred_users == 0 {
            return 0;
        }
        self.total_referral_earnings
            .safe_div(self.total_referred_users as u64)
            .unwrap_or(0)
    }

    // Get referral success rate (users who earned vs total referred)
    pub fn get_referral_success_rate(&self) -> u64 {
        if self.total_referred_users == 0 {
            return 0;
        }
        let users_with_earnings = self
            .referred_users
            .iter()
            .take(self.total_referred_users as usize)
            .filter(|&&user| user != Pubkey::default())
            .count()
            .cast_signed();

        (users_with_earnings as u64)
            .safe_mul(100)
            .unwrap_or(0)
            .safe_div(self.total_referred_users as u64)
            .unwrap_or(0)
    }

    // Get top referred users by some metric (placeholder for future implementation)
    pub fn get_top_referred_users(&self, limit: usize) -> Vec<Pubkey> {
        self.referred_users
            .iter()
            .take(self.total_referred_users as usize)
            .filter(|&&user| user != Pubkey::default())
            .take(limit)
            .cloned()
            .collect()
    }

    // Check if registry is full
    pub fn is_full(&self) -> bool {
        self.total_referred_users >= 100
    }

    // Get available slots
    pub fn get_available_slots(&self) -> u32 {
        100 - self.total_referred_users
    }

    // Update timestamp
    pub fn update_timestamp(&mut self, current_time: i32) {
        self.updated_at = current_time;
    }

    // Get days since creation
    pub fn get_days_since_created(&self, current_time: i32) -> i32 {
        (current_time - self.created_at) / 86400
    }

    // Get days since last update
    pub fn get_days_since_updated(&self, current_time: i32) -> i32 {
        (current_time - self.updated_at) / 86400
    }

    // Validation method
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
        if self.total_referred_users > 100 {
            return Err(ErrorCode::InvalidAccount);
        }
        Ok(())
    }

    // Reset methods for testing/debugging
    pub fn reset_earnings(&mut self) {
        self.total_referral_earnings = 0;
    }

    pub fn reset_referred_users(&mut self) {
        self.total_referred_users = 0;
        self.referred_users = [Pubkey::default(); 100];
    }

    // Get referral statistics
    pub fn get_referral_stats(&self) -> (u32, u64, u64) {
        (
            self.total_referred_users,
            self.total_referral_earnings,
            self.get_average_earnings_per_user(),
        )
    }
}

impl Default for ReferralRegistry {
    fn default() -> Self {
        Self {
            referrer: Pubkey::default(),
            total_referred_users: 0,
            total_referral_earnings: 0,
            referred_users: [Pubkey::default(); 100],
            created_at: 0,
            updated_at: 0,
            bump: 0,
            _padding: [0; 3],
        }
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
    const SIZE: usize = 208; // 8 (discriminator) + 200 (struct, including padding) = 208 bytes
}

// implement SIZE const for ReferralRegistry
impl Size for ReferralRegistry {
    const SIZE: usize = 3264; // 8 (discriminator) + 3256 (struct, including padding) = 3264 bytes
}

#[event]
pub struct RegisterUserEvent {
    /// The owner/authority of the account
    pub authority: Pubkey,

    /// Encoded display name e.g. "t_yield"
    pub name: [u8; 32],

    /// Whether the user is active or banned
    pub status: u8,

    /// Referrer's public key (zero if no referrer)
    pub referrer: Pubkey,

    pub created_at: i32,
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
        assert_eq!(user.name, [0; 32]);
        assert_eq!(user.total_agents_owned, 0);
        assert_eq!(user.total_agents_purchased, 0);
        assert_eq!(user.total_unclaimed_yield, 0);
        assert_eq!(user.status, UserStatus::Banned as u8);
        assert_eq!(user.idle, false);
        assert_eq!(user._padding1, [0; 2]);
        assert_eq!(user.referrer, Pubkey::default());
        assert_eq!(user.updated_at, 0);
        assert_eq!(user.created_at, 0);
        assert_eq!(user.bump, 0);
        assert_eq!(user._padding2, [0; 3]);
    }

    // Status management tests
    #[test]
    fn test_user_status_management() {
        let mut user = create_test_user();

        // Test initial status
        assert!(!user.is_active());
        assert!(user.is_banned());
        assert!(!user.is_whitelisted());

        // Test adding status
        user.add_user_status(UserStatus::Active);
        assert!(user.is_active());
        assert!(user.is_banned()); // Should still be banned

        user.add_user_status(UserStatus::WithListed);
        assert!(user.is_whitelisted());

        // Test removing status
        user.remove_user_status(UserStatus::Banned);
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
        user.ban_user();
        assert!(user.is_banned());
        assert!(!user.is_active());

        // Test unban
        user.un_ban_user();
        assert!(!user.is_banned());
        assert!(user.is_active());
    }

    #[test]
    fn test_whitelist_management() {
        let mut user = create_test_user();

        // Test whitelist
        user.whitelist_user();
        assert!(user.is_whitelisted());

        // Test remove whitelist
        user.remove_whitelist_user();
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
        assert_eq!(user.get_lifetime_yield_earned(), 1000); // 400 unclaimed + 600 claimed
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
        user.referrer = referrer;
        assert!(user.has_referrer());
    }

    // Time-based tests
    #[test]
    fn test_time_based_functions() {
        let mut user = create_test_user();
        let current_time = 2000;

        // Test update last activity
        user.update_last_activity(current_time);
        assert_eq!(user.updated_at, current_time);
        assert!(!user.is_idle());

        // Test idle status
        let idle_threshold = 1000;
        user.check_idle_status(current_time + 500, idle_threshold);
        assert!(!user.is_idle());

        user.check_idle_status(current_time + 1500, idle_threshold);
        assert!(user.is_idle());

        // Test days calculation
        assert_eq!(user.get_days_since_created(current_time), 0);
        assert_eq!(user.get_days_since_updated(current_time), 0);

        let future_time = current_time + 86400 * 2; // 2 days later
        assert_eq!(user.get_days_since_created(future_time), 2);
        assert_eq!(user.get_days_since_updated(future_time), 2);
    }

    // Name management tests
    #[test]
    fn test_name_management() {
        let mut user = create_test_user();

        let test_name = b"test_user_name";
        let mut name_array = [0u8; 32];
        name_array[..test_name.len()].copy_from_slice(test_name);

        user.set_name(name_array);
        assert_eq!(user.get_name(), name_array);
        assert_eq!(user.get_name_string(), "test_user_name");

        // Test with null bytes
        let name_with_nulls = [b't', b'e', b's', b't', 0, 0, 0];
        let mut name_array = [0u8; 32];
        name_array[..7].copy_from_slice(&name_with_nulls);

        user.set_name(name_array);
        assert_eq!(user.get_name_string(), "test");
    }

    // Delegate management tests
    #[test]
    fn test_delegate_management() {
        let mut user = create_test_user();

        assert!(!user.has_delegate());

        let delegate = Pubkey::new_unique();
        user.set_delegate(delegate);
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

        user.ban_user();
        assert!(!user.can_perform_actions());

        user.un_ban_user();
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
        assert_eq!(history.get_total_lifetime_value(), 1700); // 1000 + 500 + 200
        assert_eq!(history.get_roi_percentage(), 70); // (500 + 200) / 1000 * 100
        assert!(!history.is_profitable()); // 700 total return < 1000 investment
        assert_eq!(history.get_net_pnl(), -300); // 700 - 1000
        assert_eq!(history.get_profit_margin(), 0); // negative PnL returns 0

        // Test efficiency calculations
        assert_eq!(
            history.get_yield_efficiency(),
            500 * QUOTE_PRECISION_U64 / 1000
        );
        assert_eq!(
            history.get_referral_efficiency(),
            200 * QUOTE_PRECISION_U64 / 1000
        );
        assert_eq!(history.get_fee_ratio(), 100 * QUOTE_PRECISION_U64 / 1000);

        // Test summary
        let summary = history.get_summary();
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
        assert_eq!(history.get_roi_percentage(), 0);
        assert_eq!(history.get_yield_efficiency(), 0);
        assert_eq!(history.get_referral_efficiency(), 0);
        assert_eq!(history.get_fee_ratio(), 0);
        assert_eq!(history.get_profit_margin(), 0);
        assert!(!history.is_profitable());
        assert_eq!(history.get_net_pnl(), 0);
    }

    // ReferralRegistry tests
    #[test]
    fn test_referral_registry_basic() {
        let mut registry = ReferralRegistry::default();
        let referrer = Pubkey::new_unique();
        let user1 = Pubkey::new_unique();
        let user2 = Pubkey::new_unique();

        registry.referrer = referrer;
        registry.created_at = 1000;
        registry.updated_at = 1000;

        // Test adding users
        registry.add_referred_user(user1).unwrap();
        assert!(registry.is_user_referred(user1));
        assert_eq!(registry.total_referred_users, 1);

        registry.add_referred_user(user2).unwrap();
        assert!(registry.is_user_referred(user2));
        assert_eq!(registry.total_referred_users, 2);

        // Test adding duplicate
        let result = registry.add_referred_user(user1);
        assert!(result.is_err());

        // Test earnings
        registry.add_referral_earnings(1000).unwrap();
        assert_eq!(registry.total_referral_earnings, 1000);
        assert_eq!(registry.get_average_earnings_per_user(), 500);
    }

    #[test]
    fn test_referral_registry_removal() {
        let mut registry = ReferralRegistry::default();
        let user1 = Pubkey::new_unique();
        let user2 = Pubkey::new_unique();
        let user3 = Pubkey::new_unique();

        registry.add_referred_user(user1).unwrap();
        registry.add_referred_user(user2).unwrap();
        registry.add_referred_user(user3).unwrap();

        // Remove middle user
        registry.remove_referred_user(user2).unwrap();
        assert!(!registry.is_user_referred(user2));
        assert!(registry.is_user_referred(user1));
        assert!(registry.is_user_referred(user3));
        assert_eq!(registry.total_referred_users, 2);

        // Remove non-existent user
        let result = registry.remove_referred_user(user2);
        assert!(result.is_err());
    }

    #[test]
    fn test_referral_registry_limits() {
        let mut registry = ReferralRegistry::default();

        // Try to add more than 100 users
        for _i in 0..100 {
            let user = Pubkey::new_unique();
            registry.add_referred_user(user).unwrap();
        }

        assert!(registry.is_full());
        assert_eq!(registry.get_available_slots(), 0);

        // Try to add one more
        let user = Pubkey::new_unique();
        let result = registry.add_referred_user(user);
        assert!(result.is_err());
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

        // Too many users
        registry.updated_at = registry.created_at;
        registry.total_referred_users = 101;
        assert!(registry.validate_registry().is_err());
    }

    #[test]
    fn test_referral_registry_statistics() {
        let mut registry = ReferralRegistry::default();
        registry.referrer = Pubkey::new_unique();
        registry.created_at = 1000;
        registry.updated_at = 1000;

        // Add some users and earnings
        for _i in 0..5 {
            let user = Pubkey::new_unique();
            registry.add_referred_user(user).unwrap();
        }

        registry.add_referral_earnings(1000).unwrap();

        let stats = registry.get_referral_stats();
        assert_eq!(stats.0, 5); // total users
        assert_eq!(stats.1, 1000); // total earnings
        assert_eq!(stats.2, 200); // average earnings

        // Test time functions
        let current_time = 2000;
        assert_eq!(registry.get_days_since_created(current_time), 0);
        assert_eq!(registry.get_days_since_updated(current_time), 0);

        let future_time = current_time + 86400 * 3; // 3 days later
        assert_eq!(registry.get_days_since_created(future_time), 3);
        assert_eq!(registry.get_days_since_updated(future_time), 3);
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
        user.update_last_activity(2000);

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
        assert_eq!(user.get_yield_claim_rate(), expected_rate);

        // Test with zero agents
        let empty_user = create_test_user();
        assert_eq!(empty_user.get_yield_claim_rate(), 0);
    }

    #[test]
    fn test_status_combinations() {
        let mut user = create_test_user();

        // Clear the default banned status first
        user.remove_user_status(UserStatus::Banned);

        // Test multiple status combinations
        user.add_user_status(UserStatus::Active);
        user.add_user_status(UserStatus::WithListed);

        assert!(user.is_active());
        assert!(user.is_whitelisted());
        assert!(!user.is_banned());

        // Test status string with multiple flags
        let status_str = user.get_status_string();
        assert!(status_str.contains("Active"));
        assert!(status_str.contains("Whitelisted"));
        assert!(!status_str.contains("Banned"));

        // Test removing one status
        user.remove_user_status(UserStatus::Active);
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

        assert!(history.is_profitable()); // 1200 > 1000
        assert_eq!(history.get_net_pnl(), 200); // 1200 - 1000
        assert_eq!(history.get_profit_margin(), 20); // 200 / 1000 * 100
        assert_eq!(history.get_roi_percentage(), 120); // 1200 / 1000 * 100
    }

    #[test]
    fn test_user_comprehensive_scenario() {
        let mut user = create_test_user();

        // Set up a complete user profile
        user.remove_user_status(UserStatus::Banned);
        user.add_user_status(UserStatus::Active);
        user.add_user_status(UserStatus::WithListed);

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
        user.set_delegate(delegate);
        user.referrer = referrer;

        // Set name
        let name = b"test_user_123";
        let mut name_array = [0u8; 32];
        name_array[..name.len()].copy_from_slice(name);
        user.set_name(name_array);

        // Update activity
        user.update_last_activity(2000);

        // Verify all properties
        assert_eq!(user.get_agent_count(), 3);
        assert_eq!(user.get_total_agents_purchased(), 4500);
        assert_eq!(user.get_claimable_yield(), 400);
        assert_eq!(user.get_total_yield_ever_claimed(), 600);
        assert_eq!(user.get_total_fees_spent(), 200);
        assert_eq!(user.get_total_referral_earnings(), 300);
        assert_eq!(user.get_lifetime_yield_earned(), 1000);
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
        assert_eq!(user.get_yield_claim_rate(), expected_rate);
    }

    #[test]
    fn test_history_calculations_edge_cases() {
        let mut history = History::default();

        // Test with very large numbers
        history.add_agents_purchased(u64::MAX / 2).unwrap();
        history.add_yield_claimed(u64::MAX / 4).unwrap();
        history.add_referral_earnings(u64::MAX / 4).unwrap();

        // Should NOT be profitable (return == investment)
        assert!(!history.is_profitable());

        // Test with zero values
        let empty_history = History::default();
        assert_eq!(empty_history.get_total_lifetime_value(), 0);
        assert_eq!(empty_history.get_roi_percentage(), 0);
        assert!(!empty_history.is_profitable());
        assert_eq!(empty_history.get_net_pnl(), 0);
        assert_eq!(empty_history.get_profit_margin(), 0);
    }

    #[test]
    fn test_referral_registry_edge_cases() {
        let mut registry = ReferralRegistry::default();
        registry.referrer = Pubkey::new_unique();
        registry.created_at = 1000;
        registry.updated_at = 1000;

        // Test with no users
        assert_eq!(registry.get_average_earnings_per_user(), 0);
        assert_eq!(registry.get_referral_success_rate(), 0);
        assert_eq!(registry.get_available_slots(), 100);
        assert!(!registry.is_full());

        // Test with one user and earnings
        let user = Pubkey::new_unique();
        registry.add_referred_user(user).unwrap();
        registry.add_referral_earnings(500).unwrap();

        assert_eq!(registry.get_average_earnings_per_user(), 500);
        assert_eq!(registry.get_referral_success_rate(), 100); // 1 user with earnings

        // Test top referred users
        let top_users = registry.get_top_referred_users(5);
        assert_eq!(top_users.len(), 1);
        assert_eq!(top_users[0], user);
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
        assert_eq!(user.get_days_since_created(user.created_at), 0);
        assert_eq!(user.get_days_since_updated(user.updated_at), 0);

        // Test future time
        let future_time = user.created_at + 86400 * 10; // 10 days later
        assert_eq!(user.get_days_since_created(future_time), 10);
        assert_eq!(user.get_days_since_updated(future_time), 10);

        // Test idle status edge cases
        user.update_last_activity(1000);
        user.check_idle_status(1000, 0); // No idle threshold
        assert!(!user.is_idle());

        user.check_idle_status(1002, 1); // 2 > 1, should be idle
        assert!(user.is_idle());
    }
}
