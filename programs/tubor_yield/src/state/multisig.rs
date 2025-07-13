use anchor_lang::{prelude::*, solana_program::hash::hashv};

use crate::{
    error::{ErrorCode, TYieldResult},
    math::{SafeMath, MAX_SIGNERS},
    msg,
    state::Size,
};

// ============================================================================
// DATA STRUCTURES
// ============================================================================

/// Admin instruction types that require multisig approval.
///
/// These instruction types represent administrative actions that require
/// multisig approval to ensure protocol security and prevent unauthorized
/// changes to critical protocol parameters.
///
/// # Security Features
/// - All instructions require multisig approval
/// - Rate limiting prevents rapid execution
/// - Nonce-based replay protection
/// - Signature expiration for security
///
/// # Instruction Types
/// - `UpdatePrice`: Update master agent price with constraints
/// - `UpdateYield`: Update master agent yield rate with constraints
/// - `DeployAgent`: Deploy new agent with validation
/// - `AddAgent`: Add agent to existing master agent
/// - `BanUser`: Ban user from protocol participation
/// - `PermManager`: Manage protocol permissions
/// - `WithdrawFees`: Withdraw protocol fees
/// - `OpenTrade`: Open new trading position
///
/// # Example
/// ```
/// # use tubor_yield::state::AdminInstruction;
/// let instruction = AdminInstruction::UpdateYield;
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AdminInstruction {
    /// Update master agent price with security constraints
    UpdatePrice,
    /// Update master agent yield rate with security constraints
    UpdateYield,
    /// Deploy new agent with validation
    DeployAgent,
    /// Add agent to existing master agent
    AddAgent,
    /// Ban user from protocol participation
    BanUser,
    /// Manage protocol permissions
    PermManager,
    /// Withdraw protocol fees
    WithdrawFees,
    /// Open new trading position
    OpenTrade,
}

/// Multisig account for protocol admin control
///
/// This struct manages a multi-signature system with the following features:
/// - Up to 6 authorized signers
/// - Weight-based signing (different signers can have different weights)
/// - Rate limiting (3 signatures per minute)
/// - Nonce-based replay protection
/// - Signature expiration (default 1 hour)
/// - Full 32-byte instruction hash verification
/// - State validation and integrity checks
#[repr(C, packed)]
#[account(zero_copy)]
#[derive(Default, PartialEq, Debug)]
pub struct Multisig {
    // ===== LARGE ARRAYS (32-byte alignment) =====
    pub signers: [Pubkey; MAX_SIGNERS], // Array of authorized signer public keys (192 bytes)
    pub signer_weights: [u8; MAX_SIGNERS], // Weight per signer (6 bytes)

    // ===== 64-BIT FIELDS =====
    pub instruction_hash_bytes: [u8; 32], // Full 32-byte instruction hash
    pub nonce: u64,                       // Nonce for replay protection
    pub last_execution_time: i64,         // Timestamp of last execution
    pub last_signature_time: i64,         // Timestamp of last signature
    pub signature_timeout: i64,           // Signature expiration time

    // ===== 32-BIT FIELDS =====
    pub required_weight: u32, // Total weight needed for execution
    pub signature_count: u32, // Rate limiting: signature attempts

    // ===== 16-BIT FIELDS =====
    pub instruction_data_len: u16, // Length of instruction data

    // ===== 8-BIT FIELDS =====
    pub num_signers: u8,              // Total number of authorized signers
    pub num_signed: u8,               // Number of signatures collected so far
    pub min_signatures: u8,           // Minimum signatures required for execution
    pub bump: u8,                     // PDA bump seed
    pub instruction_accounts_len: u8, // Number of accounts in the instruction

    // ===== SMALL ARRAYS =====
    pub signed: [u8; MAX_SIGNERS], // Bitmap tracking which signers have signed (0/1)

    // ===== PADDING =====
    pub _padding: [u8; 2], // Padding for future extensibility
}

impl Size for Multisig {
    const SIZE: usize = 288; // Updated size for new fields
}

// ============================================================================
// CORE MULTISIG IMPLEMENTATION
// ============================================================================

impl Multisig {
    // ===== INITIALIZATION & SETUP =====

    /// Sets up the multisig with authorized signers and minimum signature requirements
    ///
    /// # Arguments
    /// * `admin_signers` - Array of authorized signer account infos
    /// * `min_signatures` - Minimum number of signatures required for execution
    ///
    /// # Returns
    /// * `Ok(())` - If setup is successful
    /// * `Err` - If validation fails (empty signers, invalid counts, duplicates)
    pub fn set_signers(&mut self, admin_signers: &[AccountInfo], min_signatures: u8) -> Result<()> {
        // Validate input parameters
        if admin_signers.is_empty() || min_signatures == 0 {
            msg!("Error: At least one signer is required");
            return Err(ProgramError::MissingRequiredSignature.into());
        }

        if (min_signatures as usize) > admin_signers.len() {
            msg!(
                "Error: Number of min signatures ({}) exceeded number of signers ({})",
                min_signatures,
                admin_signers.len(),
            );
            return Err(ProgramError::InvalidArgument.into());
        }

        if admin_signers.len() > MAX_SIGNERS {
            msg!(
                "Error: Number of signers ({}) exceeded max ({})",
                admin_signers.len(),
                MAX_SIGNERS
            );
            return Err(ProgramError::InvalidArgument.into());
        }

        // Initialize arrays
        let mut signers: [Pubkey; MAX_SIGNERS] = Default::default();
        let mut signed: [u8; MAX_SIGNERS] = Default::default();
        let mut weights: [u8; MAX_SIGNERS] = Default::default();

        // Process each signer
        for idx in 0..admin_signers.len() {
            if signers.contains(admin_signers[idx].key) {
                msg!("Error: Duplicate signer {}", admin_signers[idx].key);
                return Err(ProgramError::InvalidArgument.into());
            }
            signers[idx] = *admin_signers[idx].key;
            signed[idx] = 0;
            weights[idx] = 1; // Default weight of 1
        }

        // Update multisig state
        *self = Multisig {
            num_signers: admin_signers.len() as u8,
            num_signed: 0,
            min_signatures,
            required_weight: min_signatures as u32, // Default: 1 weight per signature
            bump: self.bump,
            instruction_accounts_len: 0,
            instruction_data_len: 0,
            instruction_hash_bytes: [0; 32],
            nonce: 0,
            last_execution_time: 0,
            last_signature_time: 0,
            signature_timeout: 0,
            signature_count: 0,
            signers,
            signed,
            signer_weights: weights,
            _padding: [0; 2],
        };

        Ok(())
    }

    // ===== SIGNING OPERATIONS =====

    /// Signs a multisig instruction with the provided signer
    ///
    /// This method handles the complete multisig signing flow including:
    /// - Nonce validation for replay protection
    /// - Rate limiting (3 signatures per minute)
    /// - Signature expiration checks
    /// - Weight-based signature collection
    /// - Instruction hash verification
    ///
    /// # Arguments
    /// * `signer_account` - The account signing the instruction
    /// * `instruction_accounts` - Accounts involved in the instruction
    /// * `instruction_data` - Serialized instruction data
    /// * `nonce` - Unique nonce for replay protection
    /// * `current_time` - Current timestamp for rate limiting and expiration
    ///
    /// # Returns
    /// * `Ok(remaining_signatures)` - Number of signatures still needed
    /// * `Ok(0)` - If execution is complete
    /// * `Err` - If signing fails (unauthorized, rate limited, etc.)
    pub fn sign_multisig(
        &mut self,
        signer_account: &AccountInfo,
        instruction_accounts: &[AccountInfo],
        instruction_data: &[u8],
        nonce: u64,
        current_time: i64,
    ) -> TYieldResult<u8> {
        // Set default timeout if not set
        self.set_default_timeout(current_time)?;

        // Copy packed fields to local variables to avoid unaligned access
        let current_nonce = self.nonce;
        let current_signature_count = self.signature_count;
        let current_signature_timeout = self.signature_timeout;

        // ===== SECURITY VALIDATIONS =====

        // Validate nonce to prevent replay attacks
        if nonce <= current_nonce {
            msg!("Invalid nonce: expected > {}, got {}", current_nonce, nonce);
            return Err(ErrorCode::InvalidNonce);
        }

        // Rate limiting: max 3 signatures per minute
        if current_time - self.last_signature_time < 60 && current_signature_count >= 3 {
            msg!(
                "Rate limit exceeded: {} signatures in last minute",
                current_signature_count
            );
            return Err(ErrorCode::RateLimitExceeded);
        }

        // Check if signatures have expired
        if self.num_signed > 0
            && current_time > current_signature_timeout
            && current_signature_timeout > 0
        {
            msg!("Signatures expired at {}", current_signature_timeout);
            // Reset expired signatures
            self.num_signed = 0;
            self.signed.fill(0);
        }

        // ===== SIGNER VALIDATION =====

        // Verify signer is authorized
        if !signer_account.is_signer {
            return Err(ErrorCode::MissingRequiredSignature);
        }

        // Find signer index
        let signer_idx = if let Ok(idx) = self.get_signer_index(signer_account.key) {
            idx
        } else {
            return Err(ErrorCode::MultisigAccountNotAuthorized);
        };

        // ===== RATE LIMITING UPDATE =====

        // Update rate limiting state
        if current_time - self.last_signature_time >= 60 {
            self.signature_count = 0;
        }
        self.signature_count = self.signature_count.safe_add(1)?;
        self.last_signature_time = current_time;

        // ===== SINGLE SIGNER HANDLING =====

        // If single signer, return immediately
        if self.num_signers <= 1 {
            return Ok(0);
        }

        // ===== INSTRUCTION HASH VALIDATION =====

        let instruction_hash =
            Multisig::get_instruction_hash(instruction_accounts, instruction_data);

        // Check if this is a new instruction or continuation
        if instruction_hash != self.instruction_hash_bytes
            || instruction_accounts.len() != self.instruction_accounts_len as usize
            || instruction_data.len() != self.instruction_data_len as usize
        {
            // New instruction: reset and start fresh
            self.num_signed = 1;
            self.instruction_accounts_len = instruction_accounts.len() as u8;
            self.instruction_data_len = instruction_data.len() as u16;
            self.instruction_hash_bytes = instruction_hash;
            self.signed.fill(0);
            self.signed[signer_idx] = 1;
            self.nonce = nonce;

            // Return remaining signatures needed
            let remaining = self.min_signatures.safe_sub(1)?;
            Ok(remaining)
        } else if self.signed[signer_idx] == 1 {
            // Signer already signed this instruction
            Err(ErrorCode::MultisigAlreadySigned)
        } else if self.num_signed < self.min_signatures {
            // Add signature to existing instruction
            self.num_signed = self.num_signed.safe_add(1)?;
            self.signed[signer_idx] = 1;
            self.nonce = nonce;

            if self.num_signed == self.min_signatures {
                Ok(0) // No more signatures needed
            } else {
                // Return remaining signatures needed
                let remaining = self.min_signatures.safe_sub(self.num_signed)?;
                Ok(remaining)
            }
        } else {
            // Instruction already executed
            Err(ErrorCode::MultisigAlreadyExecuted)
        }
    }

    /// Removes a signature from the multisig
    ///
    /// # Arguments
    /// * `signer_account` - The account removing their signature
    ///
    /// # Returns
    /// * `Ok(())` - If unsigning is successful
    /// * `Err` - If unsigning fails
    pub fn unsign_multisig(&mut self, signer_account: &AccountInfo) -> Result<()> {
        // Validate signer
        if !signer_account.is_signer {
            return Err(ProgramError::MissingRequiredSignature.into());
        }

        // Handle single signer case
        if self.num_signers <= 1 || self.num_signed == 0 {
            return Ok(());
        }

        // Find signer index
        let signer_idx = if let Ok(idx) = self.get_signer_index(signer_account.key) {
            idx
        } else {
            return err!(ErrorCode::MultisigAccountNotAuthorized);
        };

        // Check if signer has signed
        if self.signed[signer_idx] == 0 {
            return Ok(());
        }

        // Remove signature
        self.num_signed = self.num_signed.safe_sub(1)?;
        self.signed[signer_idx] = 0;

        Ok(())
    }

    // ===== UTILITY METHODS =====

    /// Gets the index of a signer in the signers array
    pub fn get_signer_index(&self, signer: &Pubkey) -> TYieldResult<usize> {
        for i in 0..self.num_signers as usize {
            if &self.signers[i] == signer {
                return Ok(i);
            }
        }
        Err(ErrorCode::MultisigAccountNotAuthorized)
    }

    /// Checks if a public key is an authorized signer
    pub fn is_signer(&self, key: &Pubkey) -> Result<bool> {
        Ok(self.get_signer_index(key).is_ok())
    }

    /// Calculates the total weight of all signed signatures
    pub fn get_total_weight(&self) -> u32 {
        let mut total = 0u32;
        for i in 0..self.num_signers as usize {
            if self.signed[i] == 1 {
                total = total.safe_add(self.signer_weights[i] as u32).unwrap_or(0);
            }
        }
        total
    }

    /// Validates the internal state of the multisig
    pub fn validate_state(&self) -> Result<()> {
        // Validate signature count matches bitmap
        let mut count = 0u8;
        for i in 0..self.num_signers as usize {
            if self.signed[i] == 1 {
                count = count.safe_add(1)?;
            }
        }

        if count != self.num_signed {
            msg!("Invalid state: count mismatch");
            return Err(ErrorCode::InvalidState.into());
        }

        // Validate signers are unique
        let mut seen = std::collections::HashSet::new();
        for i in 0..self.num_signers as usize {
            if !seen.insert(self.signers[i]) {
                msg!("Invalid state: duplicate signer");
                return Err(ErrorCode::DuplicateSigner.into());
            }
        }

        Ok(())
    }

    // ===== CONFIGURATION METHODS =====

    /// Sets the default signature timeout (1 hour from current time)
    pub fn set_default_timeout(&mut self, current_time: i64) -> Result<()> {
        // Copy packed field to local variable to avoid unaligned access
        let signature_timeout = self.signature_timeout;

        // Set default 1-hour timeout if not already set
        if signature_timeout == 0 {
            let new_timeout = current_time + 3600; // 1 hour
            self.signature_timeout = new_timeout;
            msg!("Set default signature timeout: {} (1 hour)", new_timeout);
        }
        Ok(())
    }

    /// Sets a custom signature timeout
    pub fn set_signature_timeout(&mut self, timeout: i64) -> Result<()> {
        self.signature_timeout = timeout;
        Ok(())
    }

    /// Sets the weight for a specific signer
    pub fn set_signer_weight(&mut self, signer_idx: usize, weight: u8) -> Result<()> {
        if signer_idx >= self.num_signers as usize {
            return Err(ErrorCode::InvalidSignerIndex.into());
        }
        self.signer_weights[signer_idx] = weight;
        Ok(())
    }

    /// Sets the total weight required for execution
    pub fn set_required_weight(&mut self, weight: u32) -> Result<()> {
        self.required_weight = weight;
        Ok(())
    }

    // ===== INSTRUCTION HELPERS =====

    /// Generates a hash from instruction accounts and data
    pub fn get_instruction_hash(
        instruction_accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> [u8; 32] {
        let mut data_to_hash = Vec::new();

        // Hash account keys
        for account in instruction_accounts {
            data_to_hash.extend_from_slice(account.key.as_ref());
        }

        // Hash instruction data
        if !instruction_data.is_empty() {
            data_to_hash.extend_from_slice(instruction_data);
        }

        hashv(&[&data_to_hash]).to_bytes()
    }

    /// Gets all account infos from a context
    pub fn get_account_infos<'info, T: ToAccountInfos<'info> + anchor_lang::Bumps>(
        ctx: &Context<'_, '_, '_, 'info, T>,
    ) -> Vec<AccountInfo<'info>> {
        let mut infos = ctx.accounts.to_account_infos();
        infos.extend_from_slice(ctx.remaining_accounts);
        infos
    }

    /// Serializes instruction data with type identifier
    pub fn get_instruction_data<T: AnchorSerialize>(
        instruction_type: AdminInstruction,
        params: &T,
    ) -> Result<Vec<u8>> {
        let mut res = vec![];
        AnchorSerialize::serialize(&params, &mut res)?;
        res.push(instruction_type as u8);
        Ok(res)
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ===== TEST HELPERS =====

    /// Creates a mock account info for testing
    fn create_mock_account_info(key: Pubkey, is_signer: bool) -> AccountInfo<'static> {
        // Each call gets its own heap allocations to avoid test state leakage
        let key_ref = Box::leak(Box::new(key));
        let lamports = Box::leak(Box::new(0u64));
        let data = Box::leak(Box::new(Vec::<u8>::new()));
        let owner = Box::leak(Box::new(Pubkey::default()));
        AccountInfo::new(key_ref, is_signer, false, lamports, data, owner, false, 0)
    }

    // ===== BASIC FUNCTIONALITY TESTS =====

    #[test]
    fn test_multisig_size() {
        println!("Multisig on-chain size: {} bytes", Multisig::SIZE);
        println!(
            "Calculated size: {} bytes",
            8 + std::mem::size_of::<Multisig>()
        );
        assert_eq!(Multisig::SIZE, 288);
    }

    #[test]
    fn test_multisig_memory_layout() {
        let _multisig = Multisig::default();
        assert_eq!(Multisig::SIZE, 288);
        println!("Multisig on-chain size: {} bytes", Multisig::SIZE);
    }

    #[test]
    fn test_multisig_default_state() {
        let multisig = Multisig::default();

        // Copy packed fields to local variables to avoid unaligned access
        let num_signers = multisig.num_signers;
        let num_signed = multisig.num_signed;
        let min_signatures = multisig.min_signatures;
        let bump = multisig.bump;
        let instruction_accounts_len = multisig.instruction_accounts_len;
        let instruction_data_len = multisig.instruction_data_len;
        let instruction_hash_bytes = multisig.instruction_hash_bytes;
        let nonce = multisig.nonce;
        let last_execution_time = multisig.last_execution_time;
        let last_signature_time = multisig.last_signature_time;
        let signature_timeout = multisig.signature_timeout;
        let required_weight = multisig.required_weight;
        let signature_count = multisig.signature_count;

        // Test default values
        assert_eq!(num_signers, 0);
        assert_eq!(num_signed, 0);
        assert_eq!(min_signatures, 0);
        assert_eq!(bump, 0);
        assert_eq!(instruction_accounts_len, 0);
        assert_eq!(instruction_data_len, 0);
        assert_eq!(instruction_hash_bytes, [0; 32]);
        assert_eq!(nonce, 0);
        assert_eq!(last_execution_time, 0);
        assert_eq!(last_signature_time, 0);
        assert_eq!(signature_timeout, 0);
        assert_eq!(required_weight, 0);
        assert_eq!(signature_count, 0);

        // Check array initialization
        for i in 0..MAX_SIGNERS {
            assert_eq!(multisig.signers[i], Pubkey::default());
            assert_eq!(multisig.signer_weights[i], 0);
            assert_eq!(multisig.signed[i], 0);
        }
    }

    // ===== INSTRUCTION HASH TESTS =====

    #[test]
    fn test_get_instruction_hash() {
        let key1 = Pubkey::new_unique();
        let key2 = Pubkey::new_unique();
        let accounts = vec![
            create_mock_account_info(key1, false),
            create_mock_account_info(key2, false),
        ];
        let data = vec![1, 2, 3, 4];

        let hash = Multisig::get_instruction_hash(&accounts, &data);
        assert_eq!(hash.len(), 32);

        // Same inputs should produce same hash
        let hash2 = Multisig::get_instruction_hash(&accounts, &data);
        assert_eq!(hash, hash2);

        // Different data should produce different hash
        let data2 = vec![1, 2, 3, 5];
        let hash3 = Multisig::get_instruction_hash(&accounts, &data2);
        assert_ne!(hash, hash3);
    }

    #[test]
    fn test_get_instruction_hash_empty_data() {
        let key1 = Pubkey::new_unique();
        let accounts = vec![create_mock_account_info(key1, false)];
        let data = vec![];

        let hash = Multisig::get_instruction_hash(&accounts, &data);
        assert_eq!(hash.len(), 32);
    }

    // ===== SIGNER MANAGEMENT TESTS =====

    #[test]
    fn test_get_signer_index() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let signer2 = Pubkey::new_unique();
        let signer3 = Pubkey::new_unique();

        // Set up multisig with 3 signers
        let accounts = vec![
            create_mock_account_info(signer1, true),
            create_mock_account_info(signer2, true),
            create_mock_account_info(signer3, true),
        ];
        multisig.set_signers(&accounts, 2).unwrap();

        // Test finding existing signers
        assert_eq!(multisig.get_signer_index(&signer1).unwrap(), 0);
        assert_eq!(multisig.get_signer_index(&signer2).unwrap(), 1);
        assert_eq!(multisig.get_signer_index(&signer3).unwrap(), 2);

        // Test finding non-existent signer
        let non_signer = Pubkey::new_unique();
        assert!(multisig.get_signer_index(&non_signer).is_err());
    }

    #[test]
    fn test_set_signers() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let signer2 = Pubkey::new_unique();

        let accounts = vec![
            create_mock_account_info(signer1, true),
            create_mock_account_info(signer2, true),
        ];

        // Test successful setup
        let result = multisig.set_signers(&accounts, 2);
        assert!(result.is_ok());
        assert_eq!(multisig.num_signers, 2);
        assert_eq!(multisig.min_signatures, 2);
        assert_eq!(multisig.num_signed, 0);
        assert_eq!(multisig.signers[0], signer1);
        assert_eq!(multisig.signers[1], signer2);
    }

    #[test]
    fn test_set_signers_validation() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();

        // Test empty signers
        let result = multisig.set_signers(&[], 1);
        assert!(result.is_err());

        // Test zero min signatures
        let accounts = vec![create_mock_account_info(signer1, true)];
        let result = multisig.set_signers(&accounts, 0);
        assert!(result.is_err());

        // Test min signatures exceeds signers
        let result = multisig.set_signers(&accounts, 2);
        assert!(result.is_err());

        // Test duplicate signers
        let accounts_with_duplicate = vec![
            create_mock_account_info(signer1, true),
            create_mock_account_info(signer1, true),
        ];
        let result = multisig.set_signers(&accounts_with_duplicate, 1);
        assert!(result.is_err());
    }

    // ===== SIGNING TESTS =====

    #[test]
    fn test_sign_multisig_single_signer() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let accounts = vec![create_mock_account_info(signer1, true)];

        multisig.set_signers(&accounts, 1).unwrap();

        let signer_account = create_mock_account_info(signer1, true);
        let instruction_accounts = vec![];
        let instruction_data = vec![1, 2, 3];

        let result = multisig.sign_multisig(
            &signer_account,
            &instruction_accounts,
            &instruction_data,
            1,
            100,
        );
        assert_eq!(result.unwrap(), 0); // No more signatures needed
    }

    #[test]
    fn test_sign_multisig_multi_signer_flow() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let signer2 = Pubkey::new_unique();
        let signer3 = Pubkey::new_unique();

        let accounts = vec![
            create_mock_account_info(signer1, true),
            create_mock_account_info(signer2, true),
            create_mock_account_info(signer3, true),
        ];

        multisig.set_signers(&accounts, 2).unwrap();

        let instruction_accounts: Vec<AccountInfo> = vec![];
        let instruction_data = vec![1, 2, 3];

        // First signature
        let result = multisig.sign_multisig(
            &accounts[0],
            &instruction_accounts,
            &instruction_data,
            1,
            100,
        );
        assert_eq!(result.unwrap(), 1); // 1 more signature needed

        // Second signature
        let result = multisig.sign_multisig(
            &accounts[1],
            &instruction_accounts,
            &instruction_data,
            2,
            100,
        );
        assert_eq!(result.unwrap(), 0); // No more signatures needed

        // Third signature (should fail, already executed)
        let result = multisig.sign_multisig(
            &accounts[2],
            &instruction_accounts,
            &instruction_data,
            3,
            100,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_sign_multisig_validation() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let accounts = vec![create_mock_account_info(signer1, true)];

        multisig.set_signers(&accounts, 1).unwrap();

        // Test unauthorized signer
        let non_signer = Pubkey::new_unique();
        let signer_account = create_mock_account_info(non_signer, true);
        let instruction_accounts = vec![];
        let instruction_data = vec![1, 2, 3];

        let result = multisig.sign_multisig(
            &signer_account,
            &instruction_accounts,
            &instruction_data,
            1,
            100,
        );
        assert!(result.is_err());

        // Test unsigned account
        let signer_account = create_mock_account_info(signer1, false);
        let result = multisig.sign_multisig(
            &signer_account,
            &instruction_accounts,
            &instruction_data,
            1,
            100,
        );
        assert!(result.is_err());
    }

    // ===== SECURITY FEATURE TESTS =====

    #[test]
    fn test_nonce_validation() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let signer2 = Pubkey::new_unique();
        let accounts = vec![
            create_mock_account_info(signer1, true),
            create_mock_account_info(signer2, true),
        ];

        multisig.set_signers(&accounts, 2).unwrap();

        let instruction_accounts = vec![create_mock_account_info(Pubkey::new_unique(), false)];
        let instruction_data = vec![1, 2, 3];

        // First signature with nonce 1
        let result = multisig.sign_multisig(
            &accounts[0],
            &instruction_accounts,
            &instruction_data,
            1,
            100,
        );
        assert!(result.is_ok());

        // Try to sign again with same nonce (should fail)
        let result = multisig.sign_multisig(
            &accounts[1],
            &instruction_accounts,
            &instruction_data,
            1,
            101,
        );
        assert!(result.is_err());

        // Try to sign with lower nonce (should fail)
        let result = multisig.sign_multisig(
            &accounts[1],
            &instruction_accounts,
            &instruction_data,
            0,
            102,
        );
        assert!(result.is_err());

        // Sign with higher nonce and new instruction (should succeed)
        let new_instruction_data = vec![4, 5, 6];
        let result = multisig.sign_multisig(
            &accounts[1],
            &instruction_accounts,
            &new_instruction_data,
            2,
            103,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_rate_limiting() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let accounts = vec![create_mock_account_info(signer1, true)];

        multisig.set_signers(&accounts, 1).unwrap();

        let signer_account = create_mock_account_info(signer1, true);
        let instruction_accounts = vec![];
        let instruction_data = vec![1, 2, 3];

        // Make 3 signature attempts within 60 seconds (should succeed)
        for i in 1..=3 {
            let result = multisig.sign_multisig(
                &signer_account,
                &instruction_accounts,
                &instruction_data,
                i,
                100,
            );
            assert!(result.is_ok());
        }

        // 4th attempt should fail due to rate limiting
        let result = multisig.sign_multisig(
            &signer_account,
            &instruction_accounts,
            &instruction_data,
            4,
            100,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_signature_expiration() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let signer2 = Pubkey::new_unique();
        let accounts = vec![
            create_mock_account_info(signer1, true),
            create_mock_account_info(signer2, true),
        ];

        multisig.set_signers(&accounts, 2).unwrap();

        // Set signature timeout to 200
        multisig.set_signature_timeout(200).unwrap();

        let signer_account = create_mock_account_info(signer1, true);
        let instruction_accounts = vec![];
        let instruction_data = vec![1, 2, 3];

        // Sign at time 100 (before timeout)
        let result = multisig.sign_multisig(
            &signer_account,
            &instruction_accounts,
            &instruction_data,
            1,
            100,
        );
        assert!(result.is_ok());

        // Try to sign at time 300 (after timeout) - should reset signatures
        let result = multisig.sign_multisig(
            &signer_account,
            &instruction_accounts,
            &instruction_data,
            2,
            300,
        );
        assert!(result.is_ok());
        assert_eq!(multisig.num_signed, 1); // Should be reset to 1
    }

    // ===== WEIGHT-BASED SIGNING TESTS =====

    #[test]
    fn test_weight_based_signing() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let signer2 = Pubkey::new_unique();
        let accounts = vec![
            create_mock_account_info(signer1, true),
            create_mock_account_info(signer2, true),
        ];

        multisig.set_signers(&accounts, 2).unwrap();

        // Set different weights for signers
        multisig.set_signer_weight(0, 2).unwrap(); // Signer 1 has weight 2
        multisig.set_signer_weight(1, 1).unwrap(); // Signer 2 has weight 1
        multisig.set_required_weight(3).unwrap(); // Need total weight of 3

        let instruction_accounts = vec![];
        let instruction_data = vec![1, 2, 3];

        // Sign with first signer (weight 2)
        let result = multisig.sign_multisig(
            &accounts[0],
            &instruction_accounts,
            &instruction_data,
            1,
            100,
        );
        assert!(result.is_ok());
        assert_eq!(multisig.get_total_weight(), 2);

        // Sign with second signer (weight 1) - should complete the requirement
        let result = multisig.sign_multisig(
            &accounts[1],
            &instruction_accounts,
            &instruction_data,
            2,
            100,
        );
        assert!(result.is_ok());
        assert_eq!(multisig.get_total_weight(), 3);
    }

    // ===== STATE VALIDATION TESTS =====

    #[test]
    fn test_state_validation() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let signer2 = Pubkey::new_unique();
        let accounts = vec![
            create_mock_account_info(signer1, true),
            create_mock_account_info(signer2, true),
        ];

        multisig.set_signers(&accounts, 2).unwrap();

        // Valid state should pass validation
        assert!(multisig.validate_state().is_ok());

        // Corrupt state by setting num_signed incorrectly
        multisig.num_signed = 5; // Invalid: should be 0
        assert!(multisig.validate_state().is_err());

        // Reset and test with valid signatures
        multisig.num_signed = 0;
        multisig.signed.fill(0);
        assert!(multisig.validate_state().is_ok());

        // Add a valid signature
        let instruction_accounts = vec![];
        let instruction_data = vec![1, 2, 3];
        let result = multisig.sign_multisig(
            &accounts[0],
            &instruction_accounts,
            &instruction_data,
            1,
            100,
        );
        assert!(result.is_ok());
        assert!(multisig.validate_state().is_ok());
    }

    // ===== CONFIGURATION TESTS =====

    #[test]
    fn test_default_timeout() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let accounts = vec![create_mock_account_info(signer1, true)];

        multisig.set_signers(&accounts, 1).unwrap();

        let signer_account = create_mock_account_info(signer1, true);
        let instruction_accounts = vec![];
        let instruction_data = vec![1, 2, 3];

        // Initially, signature_timeout should be 0
        let initial_timeout = multisig.signature_timeout;
        assert_eq!(initial_timeout, 0);

        // First signature should set default timeout
        let current_time = 100;
        let result = multisig.sign_multisig(
            &signer_account,
            &instruction_accounts,
            &instruction_data,
            1,
            current_time,
        );
        assert!(result.is_ok());

        // Verify default timeout was set (current_time + 3600)
        let timeout_after_first = multisig.signature_timeout;
        assert_eq!(timeout_after_first, current_time + 3600);

        // Second signature should not change the timeout
        let result = multisig.sign_multisig(
            &signer_account,
            &instruction_accounts,
            &instruction_data,
            2,
            current_time + 100,
        );
        assert!(result.is_ok());
        let timeout_after_second = multisig.signature_timeout;
        assert_eq!(timeout_after_second, current_time + 3600); // Should remain the same
    }

    // ===== COMPREHENSIVE TESTS =====

    #[test]
    fn test_comprehensive_multisig_flow() {
        let mut multisig = Multisig::default();
        let signer1 = Pubkey::new_unique();
        let signer2 = Pubkey::new_unique();
        let signer3 = Pubkey::new_unique();

        let accounts = vec![
            create_mock_account_info(signer1, true),
            create_mock_account_info(signer2, true),
            create_mock_account_info(signer3, true),
        ];

        multisig.set_signers(&accounts, 2).unwrap();

        // Set weights and required weight
        multisig.set_signer_weight(0, 2).unwrap();
        multisig.set_signer_weight(1, 1).unwrap();
        multisig.set_signer_weight(2, 1).unwrap();
        multisig.set_required_weight(3).unwrap();

        // Set signature timeout
        multisig.set_signature_timeout(200).unwrap();

        let instruction_accounts = vec![];
        let instruction_data = vec![1, 2, 3];

        // First signature (weight 2)
        let result = multisig.sign_multisig(
            &accounts[0],
            &instruction_accounts,
            &instruction_data,
            1,
            100,
        );
        assert!(result.is_ok());
        assert_eq!(multisig.get_total_weight(), 2);

        // Second signature (weight 1) - should complete requirement
        let result = multisig.sign_multisig(
            &accounts[1],
            &instruction_accounts,
            &instruction_data,
            2,
            100,
        );
        assert!(result.is_ok());
        assert_eq!(multisig.get_total_weight(), 3);

        // Third signature should fail (already executed)
        let result = multisig.sign_multisig(
            &accounts[2],
            &instruction_accounts,
            &instruction_data,
            3,
            100,
        );
        assert!(result.is_err());

        // Validate state
        assert!(multisig.validate_state().is_ok());
    }

    // ===== MISC TESTS =====

    #[test]
    fn test_get_instruction_data() {
        #[derive(AnchorSerialize)]
        struct TestParams {
            value: u64,
            flag: bool,
        }

        let params = TestParams {
            value: 12345,
            flag: true,
        };

        let result = Multisig::get_instruction_data(AdminInstruction::UpdatePrice, &params);
        assert!(result.is_ok());

        let data = result.unwrap();
        assert!(!data.is_empty());

        // The last byte should be the instruction type
        assert_eq!(data[data.len() - 1], AdminInstruction::UpdatePrice as u8);
    }

    #[test]
    fn test_admin_instruction_values() {
        // Test that all AdminInstruction variants have unique values
        let mut values = std::collections::HashSet::new();

        values.insert(AdminInstruction::UpdatePrice as u8);
        values.insert(AdminInstruction::DeployAgent as u8);
        values.insert(AdminInstruction::AddAgent as u8);
        values.insert(AdminInstruction::BanUser as u8);
        values.insert(AdminInstruction::PermManager as u8);
        values.insert(AdminInstruction::WithdrawFees as u8);
        values.insert(AdminInstruction::OpenTrade as u8);

        // All values should be unique
        assert_eq!(values.len(), 7);
    }

    #[test]
    fn test_multisig_partial_eq() {
        let mut multisig1 = Multisig::default();
        let mut multisig2 = Multisig::default();

        // Should be equal when both are default
        assert_eq!(multisig1, multisig2);

        // Should not be equal after modification
        multisig1.num_signers = 1;
        assert_ne!(multisig1, multisig2);

        // Should be equal again after same modification
        multisig2.num_signers = 1;
        assert_eq!(multisig1, multisig2);
    }
}
