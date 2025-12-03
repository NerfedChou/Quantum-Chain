use crate::domain::{
    detect_conflicts, AccountState, Address, Hash, PatriciaMerkleTrie,
    StateConfig, StateError,
};
use crate::events::{
    BalanceCheckRequestPayload, BalanceCheckResponsePayload, BlockValidatedPayload,
    ConflictDetectionRequestPayload, ConflictDetectionResponsePayload,
    StateReadRequestPayload, StateReadResponsePayload, StateRootComputedPayload,
    StateWriteRequestPayload,
};
use shared_types::security::{KeyProvider, MessageVerifier, NonceCache};
use shared_types::AuthenticatedMessage;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

const SUBSYSTEM_ID: u8 = 4;

// Authorized senders
const CONSENSUS: u8 = 8;
const MEMPOOL: u8 = 6;
const SMART_CONTRACTS: u8 = 11;
const TX_ORDERING: u8 = 12;
const SHARDING: u8 = 14;

/// Simple key provider that uses the same secret for all subsystems
#[derive(Clone)]
pub struct StaticKeyProvider {
    secrets: HashMap<u8, Vec<u8>>,
}

impl StaticKeyProvider {
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

/// IPC Handler for State Management with centralized security
pub struct IpcHandler<K: KeyProvider> {
    verifier: MessageVerifier<K>,
    trie: RwLock<PatriciaMerkleTrie>,
    current_height: RwLock<u64>,
    state_roots: RwLock<HashMap<u64, Hash>>,
}

impl<K: KeyProvider> IpcHandler<K> {
    pub fn new(nonce_cache: Arc<NonceCache>, key_provider: K) -> Self {
        Self {
            verifier: MessageVerifier::new(SUBSYSTEM_ID, nonce_cache, key_provider),
            trie: RwLock::new(PatriciaMerkleTrie::new()),
            current_height: RwLock::new(0),
            state_roots: RwLock::new(HashMap::new()),
        }
    }

    pub fn with_config(nonce_cache: Arc<NonceCache>, key_provider: K, config: StateConfig) -> Self {
        Self {
            verifier: MessageVerifier::new(SUBSYSTEM_ID, nonce_cache, key_provider),
            trie: RwLock::new(PatriciaMerkleTrie::with_config(config)),
            current_height: RwLock::new(0),
            state_roots: RwLock::new(HashMap::new()),
        }
    }

    /// Handle BlockValidated event from Consensus (8)
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

        let mut trie = self.trie.write().unwrap();
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
            let mut heights = self.current_height.write().unwrap();
            *heights = payload.block_height;
        }
        {
            let mut roots = self.state_roots.write().unwrap();
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

    /// Handle state read request from authorized subsystems (6, 11, 12, 14)
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
        if !matches!(msg.sender_id, MEMPOOL | SMART_CONTRACTS | TX_ORDERING | SHARDING) {
            return Err(StateError::UnauthorizedSender(msg.sender_id));
        }

        let trie = self.trie.read().unwrap();
        let payload = &msg.payload;

        let value = if let Some(key) = payload.storage_key {
            trie.get_storage(payload.address, key)?
                .map(|v| v.to_vec())
        } else {
            trie.get_account(payload.address)?
                .map(|acc| {
                    let mut v = Vec::new();
                    v.extend_from_slice(&acc.balance.to_be_bytes());
                    v.extend_from_slice(&acc.nonce.to_be_bytes());
                    v
                })
        };

        let height = *self.current_height.read().unwrap();

        Ok(StateReadResponsePayload {
            address: payload.address,
            storage_key: payload.storage_key,
            value,
            block_number: height,
        })
    }

    /// Handle state write request from Smart Contracts (11) ONLY
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

        let mut trie = self.trie.write().unwrap();
        let payload = &msg.payload;

        trie.set_storage(payload.address, payload.storage_key, payload.value)?;

        Ok(())
    }

    /// Handle balance check from Mempool (6) ONLY
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

        let trie = self.trie.read().unwrap();
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

    /// Handle conflict detection from Transaction Ordering (12) ONLY
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

    // Direct API methods (for internal use)
    pub fn get_account(&self, address: Address) -> Result<Option<AccountState>, StateError> {
        let trie = self.trie.read().unwrap();
        trie.get_account(address)
    }

    pub fn get_balance(&self, address: Address) -> Result<u128, StateError> {
        let trie = self.trie.read().unwrap();
        trie.get_balance(address)
    }

    pub fn get_current_state_root(&self) -> Result<Hash, StateError> {
        let trie = self.trie.read().unwrap();
        Ok(trie.root_hash())
    }
}
