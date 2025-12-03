//! Driven ports (Outbound dependencies)
//!
//! Reference: SPEC-08-CONSENSUS.md Section 3.2

use crate::domain::{SignedTransaction, ValidatedBlock, ValidationProof, ValidatorSet};
use async_trait::async_trait;
use shared_types::Hash;

/// Event bus for choreography
///
/// Reference: SPEC-08 Section 3.2, Architecture.md Section 5.1
#[async_trait]
pub trait EventBus: Send + Sync {
    /// Publish BlockValidated event to trigger choreography
    ///
    /// # Choreography Pattern (V2.3)
    /// This event triggers:
    /// - Subsystem 3 (Tx Indexing) to compute MerkleRoot
    /// - Subsystem 4 (State Mgmt) to compute StateRoot
    /// - Subsystem 2 (Block Storage) to begin assembly
    async fn publish_block_validated(
        &self,
        block_hash: Hash,
        block_height: u64,
        block: ValidatedBlock,
        consensus_proof: ValidationProof,
        validated_at: u64,
    ) -> Result<(), String>;
}

/// Mempool interface for block building
///
/// Reference: SPEC-08 Section 3.2, IPC-MATRIX.md Subsystem 8
#[async_trait]
pub trait MempoolGateway: Send + Sync {
    /// Get transactions for block building
    ///
    /// Returns transactions sorted by gas price, up to limits
    async fn get_transactions_for_block(
        &self,
        max_count: usize,
        max_gas: u64,
    ) -> Result<Vec<SignedTransaction>, String>;

    /// Propose transactions for inclusion (triggers two-phase commit in mempool)
    async fn propose_transactions(
        &self,
        tx_hashes: Vec<Hash>,
        target_block_height: u64,
    ) -> Result<(), String>;
}

/// Signature verification for zero-trust re-verification
///
/// Reference: SPEC-08 Section 3.2, IPC-MATRIX.md "Zero-Trust Signature Re-Verification"
///
/// # Security
/// Consensus MUST NOT trust pre-validated flags from Subsystem 10.
/// All signatures are independently re-verified here.
pub trait SignatureVerifier: Send + Sync {
    /// Verify a single ECDSA signature (65 bytes with recovery id)
    ///
    /// # Zero-Trust
    /// Even if Subsystem 10 says signature_valid=true, we re-verify
    fn verify_ecdsa(&self, message: &[u8], signature: &[u8; 65], public_key: &[u8; 33]) -> bool;

    /// Verify aggregate BLS signature (for PoS attestations)
    fn verify_aggregate_bls(
        &self,
        message: &[u8],
        signature: &[u8; 96],
        public_keys: &[[u8; 48]],
    ) -> bool;

    /// Recover signer address from signature (65 bytes with recovery id)
    fn recover_signer(&self, message: &[u8], signature: &[u8; 65]) -> Option<[u8; 20]>;
}

/// Validator set provider (queries State Management)
///
/// Reference: SPEC-08 Section 3.2, IPC-MATRIX.md Subsystem 4
///
/// # Security
/// Queries validator set at EPOCH BOUNDARY state root, not current state
#[async_trait]
pub trait ValidatorSetProvider: Send + Sync {
    /// Get validator set at a specific epoch
    ///
    /// The state_root is the state at the BEGINNING of the epoch.
    /// This ensures consistent validator sets for the entire epoch.
    async fn get_validator_set_at_epoch(
        &self,
        epoch: u64,
        state_root: Hash,
    ) -> Result<ValidatorSet, String>;

    /// Get total active stake at epoch
    async fn get_total_stake_at_epoch(&self, epoch: u64, state_root: Hash) -> Result<u128, String>;

    /// Get current epoch number
    async fn current_epoch(&self) -> u64;

    /// Get epoch boundary state root
    async fn get_epoch_state_root(&self, epoch: u64) -> Result<Hash, String>;
}

/// Time source for timestamp validation
pub trait TimeSource: Send + Sync {
    /// Get current unix timestamp in seconds
    fn now(&self) -> u64;

    /// Get current epoch based on genesis time and epoch length
    fn current_epoch(&self, genesis_time: u64, epoch_length_secs: u64) -> u64;
}

/// Default time source using system time
pub struct SystemTimeSource;

impl TimeSource for SystemTimeSource {
    fn now(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn current_epoch(&self, genesis_time: u64, epoch_length_secs: u64) -> u64 {
        let now = self.now();
        if now < genesis_time {
            return 0;
        }
        (now - genesis_time) / epoch_length_secs
    }
}
