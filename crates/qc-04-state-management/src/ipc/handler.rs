//! # IPC Handler for State Management
//!
//! Authenticated message handler for direct IPC communication.
//! This is an alternative to the choreography pattern when direct
//! request/response semantics are needed.
//!
//! ## Security Model
//!
//! Uses centralized `MessageVerifier` from shared-types to enforce
//! IPC-MATRIX.md authorization rules. All handlers verify:
//! 1. Message signature (HMAC)
//! 2. Nonce freshness (replay protection)
//! 3. Sender authorization (per-operation ACL)
//!
//! ## Usage
//!
//! For event-based choreography, use node-runtime's StateAdapter.
//! Use IpcHandler when you need direct request/response semantics.

use crate::domain::{
    detect_conflicts, AccountState, Address, Hash, PatriciaMerkleTrie, StateConfig, StateError,
};
use crate::events::{
    BalanceCheckRequestPayload, BalanceCheckResponsePayload, BlockValidatedPayload,
    ConflictDetectionRequestPayload, ConflictDetectionResponsePayload, StateReadRequestPayload,
    StateReadResponsePayload, StateRootComputedPayload, StateWriteRequestPayload,
};
use shared_types::security::{KeyProvider, MessageVerifier, NonceCache};
use shared_types::AuthenticatedMessage;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Subsystem identifier for State Management.
const SUBSYSTEM_ID: u8 = 4;

// =============================================================================
// AUTHORIZED SENDERS (per IPC-MATRIX.md)
// =============================================================================

/// Consensus subsystem (8) - BlockValidated events.
const CONSENSUS: u8 = 8;
/// Mempool subsystem (6) - Balance checks.
const MEMPOOL: u8 = 6;
/// Smart Contracts subsystem (11) - State writes and reads.
const SMART_CONTRACTS: u8 = 11;
/// Transaction Ordering subsystem (12) - Conflict detection.
const TX_ORDERING: u8 = 12;
/// Sharding subsystem (14) - Cross-shard state reads.
const SHARDING: u8 = 14;

// =============================================================================
// KEY PROVIDER
// =============================================================================

/// Static key provider for testing and development.
///
/// In production, use a key provider that retrieves secrets from
/// a secure key management system (HSM, Vault, etc.).
#[derive(Clone)]
pub struct StaticKeyProvider {
    secrets: HashMap<u8, Vec<u8>>,
}

impl StaticKeyProvider {
    /// Create with a default secret for all subsystems.
    pub fn new(default_secret: &[u8]) -> Self {
        let mut secrets = HashMap::new();
        for id in 1..=15 {
            secrets.insert(id, default_secret.to_vec());
        }
        Self { secrets }
    }
}

impl KeyProvider for StaticKeyProvider {
    fn get_shared_secret(&self, sender_id: u8) -> Option<Vec<u8>> {
        self.secrets.get(&sender_id).cloned()
    }
}

// =============================================================================
// IPC HANDLER
// =============================================================================

/// IPC Handler for State Management.
///
/// Provides authenticated message handling for all state operations.
/// Each handler method verifies sender authorization before processing.
///
/// ## Thread Safety
///
/// Uses RwLock for concurrent read access to state trie.
/// Write operations acquire exclusive lock.
pub struct IpcHandler<K: KeyProvider> {
    /// Message verifier for authentication.
    verifier: MessageVerifier<K>,
    /// Patricia Merkle Trie (state storage).
    trie: RwLock<PatriciaMerkleTrie>,
    /// Current block height.
    current_height: RwLock<u64>,
    /// State roots by block height (for historical queries).
    state_roots: RwLock<HashMap<u64, Hash>>,
}

impl<K: KeyProvider> IpcHandler<K> {
    /// Create a new IPC handler with default configuration.
    pub fn new(nonce_cache: Arc<NonceCache>, key_provider: K) -> Self {
        Self {
            verifier: MessageVerifier::new(SUBSYSTEM_ID, nonce_cache, key_provider),
            trie: RwLock::new(PatriciaMerkleTrie::new()),
            current_height: RwLock::new(0),
            state_roots: RwLock::new(HashMap::new()),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(nonce_cache: Arc<NonceCache>, key_provider: K, config: StateConfig) -> Self {
        Self {
            verifier: MessageVerifier::new(SUBSYSTEM_ID, nonce_cache, key_provider),
            trie: RwLock::new(PatriciaMerkleTrie::with_config(config)),
            current_height: RwLock::new(0),
            state_roots: RwLock::new(HashMap::new()),
        }
    }

    /// Handle BlockValidated event from Consensus (8).
    ///
    /// ## Authorization
    ///
    /// Only Consensus (Subsystem 8) can trigger state root computation.
    ///
    /// ## Process
    ///
    /// 1. Verify message signature and sender
    /// 2. Apply all transactions to state trie
    /// 3. Compute new state root
    /// 4. Return StateRootComputedPayload
    pub fn handle_block_validated(
        &self,
        msg: &AuthenticatedMessage<BlockValidatedPayload>,
        msg_bytes: &[u8],
    ) -> Result<StateRootComputedPayload, StateError> {
        // Verify message using centralized security
        let result = self.verifier.verify(msg, msg_bytes);
        if result.is_error() {
            return Err(StateError::UnauthorizedSender(msg.sender_id));
        }

        // Check sender is Consensus (8)
        if msg.sender_id != CONSENSUS {
            return Err(StateError::UnauthorizedSender(msg.sender_id));
        }

        let start_time = Instant::now();
        let payload = &msg.payload;

        let mut trie = self.trie.write().map_err(|_| StateError::LockPoisoned)?;
        let previous_root = trie.root_hash();
        let mut accounts_modified = 0u32;
        let storage_modified = 0u32;

        // Apply all transactions
        for tx in &payload.transactions {
            // Debit sender
            if tx.value > 0 {
                trie.apply_balance_change(tx.from, -(tx.value as i128))?;
                accounts_modified += 1;

                // Credit recipient
                if let Some(to) = tx.to {
                    trie.apply_balance_change(to, tx.value as i128)?;
                    accounts_modified += 1;
                }
            }

            // Increment sender nonce
            trie.apply_nonce_increment(tx.from, tx.nonce)?;
        }

        let new_root = trie.root_hash();

        // Store state root for this height
        {
            let mut heights = self
                .current_height
                .write()
                .map_err(|_| StateError::LockPoisoned)?;
            *heights = payload.block_height;
        }
        {
            let mut roots = self
                .state_roots
                .write()
                .map_err(|_| StateError::LockPoisoned)?;
            roots.insert(payload.block_height, new_root);
        }

        Ok(StateRootComputedPayload {
            block_hash: payload.block_hash,
            block_height: payload.block_height,
            state_root: new_root,
            previous_state_root: previous_root,
            accounts_modified,
            storage_slots_modified: storage_modified,
            computation_time_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    /// Handle state read request.
    ///
    /// ## Authorization
    ///
    /// Allowed sources: Subsystems 6, 11, 12, 14
    /// - Mempool (6): Balance checks for tx validation
    /// - Smart Contracts (11): Contract state reads
    /// - Tx Ordering (12): Conflict detection
    /// - Sharding (14): Cross-shard state queries
    pub fn handle_state_read(
        &self,
        msg: &AuthenticatedMessage<StateReadRequestPayload>,
        msg_bytes: &[u8],
    ) -> Result<StateReadResponsePayload, StateError> {
        let result = self.verifier.verify(msg, msg_bytes);
        if result.is_error() {
            return Err(StateError::UnauthorizedSender(msg.sender_id));
        }

        // Check authorized senders
        if !matches!(
            msg.sender_id,
            MEMPOOL | SMART_CONTRACTS | TX_ORDERING | SHARDING
        ) {
            return Err(StateError::UnauthorizedSender(msg.sender_id));
        }

        let trie = self.trie.read().map_err(|_| StateError::LockPoisoned)?;
        let payload = &msg.payload;

        let value = if let Some(key) = payload.storage_key {
            trie.get_storage(payload.address, key)?.map(|v| v.to_vec())
        } else {
            trie.get_account(payload.address)?.map(|acc| {
                let mut v = Vec::new();
                v.extend_from_slice(&acc.balance.to_be_bytes());
                v.extend_from_slice(&acc.nonce.to_be_bytes());
                v
            })
        };

        let height = *self
            .current_height
            .read()
            .map_err(|_| StateError::LockPoisoned)?;

        Ok(StateReadResponsePayload {
            address: payload.address,
            storage_key: payload.storage_key,
            value,
            block_number: height,
        })
    }

    /// Handle state write request.
    ///
    /// ## Authorization
    ///
    /// Allowed source: Subsystem 11 (Smart Contracts) ONLY.
    /// All other sources are rejected with UnauthorizedSender error.
    ///
    /// ## Security Rationale
    ///
    /// Only the Smart Contracts VM should modify contract storage.
    /// Direct writes from other subsystems would bypass execution validation.
    pub fn handle_state_write(
        &self,
        msg: &AuthenticatedMessage<StateWriteRequestPayload>,
        msg_bytes: &[u8],
    ) -> Result<(), StateError> {
        let result = self.verifier.verify(msg, msg_bytes);
        if result.is_error() {
            return Err(StateError::UnauthorizedSender(msg.sender_id));
        }

        // ONLY Smart Contracts (11) can write state
        if msg.sender_id != SMART_CONTRACTS {
            return Err(StateError::UnauthorizedSender(msg.sender_id));
        }

        let mut trie = self.trie.write().map_err(|_| StateError::LockPoisoned)?;
        let payload = &msg.payload;

        trie.set_storage(payload.address, payload.storage_key, payload.value)?;

        Ok(())
    }

    /// Handle balance check request.
    ///
    /// ## Authorization
    ///
    /// Allowed source: Subsystem 6 (Mempool) ONLY.
    ///
    /// ## Purpose
    ///
    /// Used by Mempool to validate that transaction senders have
    /// sufficient balance before admitting transactions to the pool.
    pub fn handle_balance_check(
        &self,
        msg: &AuthenticatedMessage<BalanceCheckRequestPayload>,
        msg_bytes: &[u8],
    ) -> Result<BalanceCheckResponsePayload, StateError> {
        let result = self.verifier.verify(msg, msg_bytes);
        if result.is_error() {
            return Err(StateError::UnauthorizedSender(msg.sender_id));
        }

        // ONLY Mempool (6) can check balances
        if msg.sender_id != MEMPOOL {
            return Err(StateError::UnauthorizedSender(msg.sender_id));
        }

        let trie = self.trie.read().map_err(|_| StateError::LockPoisoned)?;
        let payload = &msg.payload;

        let current_balance = trie.get_balance(payload.address)?;
        let has_sufficient = current_balance >= payload.required_balance;

        Ok(BalanceCheckResponsePayload {
            address: payload.address,
            has_sufficient_balance: has_sufficient,
            current_balance,
            required_balance: payload.required_balance,
        })
    }

    /// Handle conflict detection request.
    ///
    /// ## Authorization
    ///
    /// Allowed source: Subsystem 12 (Transaction Ordering) ONLY.
    ///
    /// ## Purpose
    ///
    /// Used by Transaction Ordering to identify read-write and write-write
    /// conflicts between transactions for parallel execution scheduling.
    pub fn handle_conflict_detection(
        &self,
        msg: &AuthenticatedMessage<ConflictDetectionRequestPayload>,
        msg_bytes: &[u8],
    ) -> Result<ConflictDetectionResponsePayload, StateError> {
        let result = self.verifier.verify(msg, msg_bytes);
        if result.is_error() {
            return Err(StateError::UnauthorizedSender(msg.sender_id));
        }

        // ONLY Transaction Ordering (12) can request conflict detection
        if msg.sender_id != TX_ORDERING {
            return Err(StateError::UnauthorizedSender(msg.sender_id));
        }

        let payload = &msg.payload;
        let conflicts = detect_conflicts(&payload.transactions);

        Ok(ConflictDetectionResponsePayload {
            conflicts: conflicts.clone(),
            total_transactions: payload.transactions.len(),
            conflicting_pairs: conflicts.len(),
        })
    }

    // =========================================================================
    // DIRECT API METHODS (for internal use)
    // =========================================================================

    /// Get account state directly (bypasses IPC authentication).
    ///
    /// For internal use by node-runtime when it already owns the handler.
    pub fn get_account(&self, address: Address) -> Result<Option<AccountState>, StateError> {
        let trie = self.trie.read().map_err(|_| StateError::LockPoisoned)?;
        trie.get_account(address)
    }

    /// Get account balance directly (bypasses IPC authentication).
    pub fn get_balance(&self, address: Address) -> Result<u128, StateError> {
        let trie = self.trie.read().map_err(|_| StateError::LockPoisoned)?;
        trie.get_balance(address)
    }

    /// Get current state root directly (bypasses IPC authentication).
    pub fn get_current_state_root(&self) -> Result<Hash, StateError> {
        let trie = self.trie.read().map_err(|_| StateError::LockPoisoned)?;
        Ok(trie.root_hash())
    }
}

// =============================================================================
// IPC AUTHORIZATION TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{BalanceCheckRequestPayload, StateWriteRequestPayload};
    use uuid::Uuid;

    /// Create a test IPC handler with default configuration.
    fn create_test_handler() -> IpcHandler<StaticKeyProvider> {
        let nonce_cache = NonceCache::new_shared();
        let key_provider = StaticKeyProvider::new(&[0x42; 32]);
        IpcHandler::new(nonce_cache, key_provider)
    }

    /// Create a test message with the specified sender_id.
    /// Uses all-zero signature which the MessageVerifier should reject
    /// unless explicitly bypassed in test mode.
    fn create_test_message<T: Clone>(sender_id: u8, payload: T) -> AuthenticatedMessage<T> {
        AuthenticatedMessage {
            version: 1,
            correlation_id: Uuid::new_v4(),
            sender_id,
            recipient_id: 4, // State Management
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            nonce: Uuid::new_v4(),
            signature: [0u8; 64],
            reply_to: None,
            payload,
        }
    }

    // =========================================================================
    // BlockValidated Authorization Tests
    // =========================================================================

    #[test]
    fn test_block_validated_rejects_non_consensus() {
        let handler = create_test_handler();

        let payload = BlockValidatedPayload {
            block_hash: [0u8; 32],
            block_height: 1,
            transactions: vec![],
        };

        // Create message from Mempool (6) - should be rejected
        let msg = create_test_message(MEMPOOL, payload.clone());
        let msg_bytes = bincode::serialize(&msg).unwrap_or_default();

        let result = handler.handle_block_validated(&msg, &msg_bytes);
        assert!(
            matches!(result, Err(StateError::UnauthorizedSender(6))),
            "BlockValidated should reject sender 6 (Mempool)"
        );

        // Create message from Smart Contracts (11) - should be rejected
        let msg = create_test_message(SMART_CONTRACTS, payload);
        let msg_bytes = bincode::serialize(&msg).unwrap_or_default();

        let result = handler.handle_block_validated(&msg, &msg_bytes);
        assert!(
            matches!(result, Err(StateError::UnauthorizedSender(11))),
            "BlockValidated should reject sender 11 (Smart Contracts)"
        );
    }

    // =========================================================================
    // StateWriteRequest Authorization Tests
    // =========================================================================

    #[test]
    fn test_state_write_rejects_non_smart_contracts() {
        let handler = create_test_handler();

        let payload = StateWriteRequestPayload {
            address: [0x42u8; 20],
            storage_key: [0x01u8; 32],
            value: [0xFFu8; 32],
            block_height: 1,
            tx_hash: [0u8; 32],
        };

        // Create message from Mempool (6) - should be rejected
        let msg = create_test_message(MEMPOOL, payload.clone());
        let msg_bytes = bincode::serialize(&msg).unwrap_or_default();

        let result = handler.handle_state_write(&msg, &msg_bytes);
        assert!(
            matches!(result, Err(StateError::UnauthorizedSender(6))),
            "StateWrite should reject sender 6 (Mempool)"
        );

        // Create message from Consensus (8) - should be rejected
        let msg = create_test_message(CONSENSUS, payload.clone());
        let msg_bytes = bincode::serialize(&msg).unwrap_or_default();

        let result = handler.handle_state_write(&msg, &msg_bytes);
        assert!(
            matches!(result, Err(StateError::UnauthorizedSender(8))),
            "StateWrite should reject sender 8 (Consensus)"
        );

        // Create message from Tx Ordering (12) - should be rejected
        let msg = create_test_message(TX_ORDERING, payload);
        let msg_bytes = bincode::serialize(&msg).unwrap_or_default();

        let result = handler.handle_state_write(&msg, &msg_bytes);
        assert!(
            matches!(result, Err(StateError::UnauthorizedSender(12))),
            "StateWrite should reject sender 12 (Tx Ordering)"
        );
    }

    // =========================================================================
    // BalanceCheckRequest Authorization Tests
    // =========================================================================

    #[test]
    fn test_balance_check_rejects_non_mempool() {
        let handler = create_test_handler();

        let payload = BalanceCheckRequestPayload {
            address: [0x42u8; 20],
            required_balance: 1000,
        };

        // Create message from Consensus (8) - should be rejected
        let msg = create_test_message(CONSENSUS, payload.clone());
        let msg_bytes = bincode::serialize(&msg).unwrap_or_default();

        let result = handler.handle_balance_check(&msg, &msg_bytes);
        assert!(
            matches!(result, Err(StateError::UnauthorizedSender(8))),
            "BalanceCheck should reject sender 8 (Consensus)"
        );

        // Create message from Smart Contracts (11) - should be rejected
        let msg = create_test_message(SMART_CONTRACTS, payload.clone());
        let msg_bytes = bincode::serialize(&msg).unwrap_or_default();

        let result = handler.handle_balance_check(&msg, &msg_bytes);
        assert!(
            matches!(result, Err(StateError::UnauthorizedSender(11))),
            "BalanceCheck should reject sender 11 (Smart Contracts)"
        );

        // Create message from Tx Ordering (12) - should be rejected
        let msg = create_test_message(TX_ORDERING, payload);
        let msg_bytes = bincode::serialize(&msg).unwrap_or_default();

        let result = handler.handle_balance_check(&msg, &msg_bytes);
        assert!(
            matches!(result, Err(StateError::UnauthorizedSender(12))),
            "BalanceCheck should reject sender 12 (Tx Ordering)"
        );
    }

    // =========================================================================
    // ConflictDetectionRequest Authorization Tests
    // =========================================================================

    #[test]
    fn test_conflict_detection_rejects_non_tx_ordering() {
        let handler = create_test_handler();

        let payload = ConflictDetectionRequestPayload {
            transactions: vec![],
        };

        // Create message from Mempool (6) - should be rejected
        let msg = create_test_message(MEMPOOL, payload.clone());
        let msg_bytes = bincode::serialize(&msg).unwrap_or_default();

        let result = handler.handle_conflict_detection(&msg, &msg_bytes);
        assert!(
            matches!(result, Err(StateError::UnauthorizedSender(6))),
            "ConflictDetection should reject sender 6 (Mempool)"
        );

        // Create message from Consensus (8) - should be rejected
        let msg = create_test_message(CONSENSUS, payload.clone());
        let msg_bytes = bincode::serialize(&msg).unwrap_or_default();

        let result = handler.handle_conflict_detection(&msg, &msg_bytes);
        assert!(
            matches!(result, Err(StateError::UnauthorizedSender(8))),
            "ConflictDetection should reject sender 8 (Consensus)"
        );

        // Create message from Smart Contracts (11) - should be rejected
        let msg = create_test_message(SMART_CONTRACTS, payload);
        let msg_bytes = bincode::serialize(&msg).unwrap_or_default();

        let result = handler.handle_conflict_detection(&msg, &msg_bytes);
        assert!(
            matches!(result, Err(StateError::UnauthorizedSender(11))),
            "ConflictDetection should reject sender 11 (Smart Contracts)"
        );
    }
}
