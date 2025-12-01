# SPECIFICATION: STATE MANAGEMENT

**Version:** 2.3  
**Subsystem ID:** 4  
**Bounded Context:** State & Account Management  
**Crate Name:** `crates/state-management`  
**Author:** Systems Architecture Team  
**Date:** 2024-12-01  
**Architecture Compliance:** Architecture.md v2.3, IPC-MATRIX.md v2.3, System.md v2.3

---

## 1. ABSTRACT

### 1.1 Purpose

The **State Management** subsystem maintains the authoritative current state of all accounts and smart contract storage on the blockchain. It provides efficient state lookups, cryptographic state proofs via Patricia Merkle Tries, and computes the state root for each validated block as part of the V2.3 choreography pattern.

### 1.2 Responsibility Boundaries

**In Scope:**
- Maintain account state (balances, nonces, code hashes, storage roots)
- Maintain smart contract storage (key-value mappings per contract)
- Compute state root after applying block transactions
- Generate cryptographic state proofs for light clients
- Provide balance/nonce checks for Mempool validation
- Provide state read/write interface for Smart Contract execution
- Detect transaction conflicts for ordering optimization

**Out of Scope:**
- Block validation or consensus logic (Subsystem 8)
- Transaction signature verification (Subsystem 10)
- Smart contract bytecode execution (Subsystem 11)
- Block storage or persistence (Subsystem 2)
- Merkle tree computation for transactions (Subsystem 3)

### 1.3 Critical Design Constraint (V2.3 Choreography)

This subsystem is a **Choreography Participant**, NOT an orchestration target.

**Architecture Mandate (Architecture.md v2.3):**
- Subscribes to `BlockValidated` events from the Event Bus
- Computes state transitions for all transactions in the block
- Publishes `StateRootComputed` event to the Event Bus
- Block Storage (Subsystem 2) assembles this with other components
- There is NO direct State Management → Block Storage communication path

```
EVENT BUS CHOREOGRAPHY:
                                                           
  [Consensus (8)] ──BlockValidated──→ [Event Bus]         
                                           │               
                    ┌──────────────────────┼──────────────────────┐
                    ↓                      ↓                      ↓
           [Tx Indexing (3)]    [State Management (4)]    [Block Storage (2)]
                    │                      │              (Stateful Assembler)
                    ↓                      ↓                      ↑
           MerkleRootComputed     StateRootComputed               │
                    │                      │                      │
                    └──────────→ [Event Bus] ←────────────────────┘
```

### 1.4 Key Design Principles

1. **Single Source of Truth:** This subsystem is the authoritative source for current account state.
2. **Cryptographic Integrity:** All state is organized in a Patricia Merkle Trie for verifiable proofs.
3. **Deterministic Transitions:** Given the same input state and transactions, output state is always identical.
4. **Isolation:** State changes are applied atomically per block; partial application is impossible.

### 1.5 Trust Model

```
┌─────────────────────────────────────────────────────────────────┐
│                    TRUST BOUNDARY (V2.3)                        │
├─────────────────────────────────────────────────────────────────┤
│  TRUSTED INPUTS (via AuthenticatedMessage envelope):            │
│  ├─ BlockValidated events from Subsystem 8 (Consensus)          │
│  ├─ StateWriteRequest from Subsystem 11 (Smart Contracts)       │
│  └─ StateReadRequest from Subsystems 6, 11, 12, 14              │
│                                                                 │
│  UNTRUSTED:                                                     │
│  ├─ Any message with invalid envelope signature                 │
│  ├─ Any message from unauthorized subsystem                     │
│  └─ Any request that would create invalid state                 │
│                                                                 │
│  SECURITY (Envelope-Only Identity - V2.2 Amendment):            │
│  Identity is derived SOLELY from AuthenticatedMessage.sender_id │
│  Payloads MUST NOT contain requester_id fields.                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. DOMAIN MODEL

### 2.1 Core Entities

```rust
/// Account state stored in the Patricia Merkle Trie
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountState {
    /// Account balance in base units
    pub balance: U256,
    /// Transaction count (nonce)
    pub nonce: u64,
    /// Hash of the account's contract code (empty for EOAs)
    pub code_hash: Hash,
    /// Root hash of the account's storage trie
    pub storage_root: Hash,
}

/// Address type (20 bytes for Ethereum compatibility)
pub type Address = [u8; 20];

/// Storage key (32 bytes)
pub type StorageKey = [u8; 32];

/// Storage value (32 bytes)
pub type StorageValue = [u8; 32];

/// State transition for a single account
#[derive(Clone, Debug)]
pub struct AccountTransition {
    pub address: Address,
    pub balance_delta: i128,  // Can be negative (spending)
    pub nonce_increment: u64,
    pub storage_changes: Vec<(StorageKey, Option<StorageValue>)>,
    pub code_change: Option<Vec<u8>>,  // Contract deployment
}

/// Complete state transition for a block
#[derive(Clone, Debug)]
pub struct BlockStateTransition {
    pub block_hash: Hash,
    pub block_height: u64,
    pub account_transitions: Vec<AccountTransition>,
    pub previous_state_root: Hash,
}

/// Cryptographic proof of account state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateProof {
    pub address: Address,
    pub account_state: Option<AccountState>,
    pub proof_nodes: Vec<Vec<u8>>,  // RLP-encoded trie nodes
    pub state_root: Hash,
}

/// Cryptographic proof of storage value
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorageProof {
    pub address: Address,
    pub storage_key: StorageKey,
    pub storage_value: Option<StorageValue>,
    pub account_proof: Vec<Vec<u8>>,
    pub storage_proof: Vec<Vec<u8>>,
    pub state_root: Hash,
}
```

### 2.2 Patricia Merkle Trie Structure

```rust
/// Node types in the Patricia Merkle Trie
#[derive(Clone, Debug)]
pub enum TrieNode {
    /// Empty node (null)
    Empty,
    /// Leaf node: remaining path + value
    Leaf {
        path: Nibbles,
        value: Vec<u8>,
    },
    /// Extension node: shared path prefix + child hash
    Extension {
        path: Nibbles,
        child: Hash,
    },
    /// Branch node: 16 children + optional value
    Branch {
        children: [Option<Hash>; 16],
        value: Option<Vec<u8>>,
    },
}

/// Nibble path (half-bytes for hex prefix encoding)
#[derive(Clone, Debug)]
pub struct Nibbles(Vec<u8>);

/// The main Patricia Merkle Trie
pub struct PatriciaMerkleTrie {
    /// Root hash of the trie
    root: Hash,
    /// Node storage backend
    db: Arc<dyn TrieDatabase>,
    /// Dirty nodes pending commit
    dirty_nodes: HashMap<Hash, TrieNode>,
}

/// Configuration for the state trie
#[derive(Clone, Debug)]
pub struct StateConfig {
    /// Maximum trie depth (for DoS prevention)
    pub max_depth: usize,
    /// Cache size for hot nodes
    pub cache_size_mb: usize,
    /// Enable state snapshots
    pub enable_snapshots: bool,
    /// Snapshot interval (blocks)
    pub snapshot_interval: u64,
    /// Pruning depth (keep last N states)
    pub pruning_depth: u64,
}

impl Default for StateConfig {
    fn default() -> Self {
        Self {
            max_depth: 64,           // Sufficient for any address path
            cache_size_mb: 512,      // 512MB node cache
            enable_snapshots: true,
            snapshot_interval: 128,  // Snapshot every 128 blocks
            pruning_depth: 1000,     // Keep last 1000 block states
        }
    }
}
```

### 2.3 Invariants

```rust
/// INVARIANT-1: Balance Non-Negativity
/// Account balances can NEVER be negative.
/// Checked on every state transition.
fn invariant_balance_non_negative(account: &AccountState) -> bool {
    // U256 is unsigned, so this is always true by type
    // But we check for underflow during transitions
    true
}

/// INVARIANT-2: Strict Nonce Monotonicity
/// 
/// Reference: IPC-MATRIX.md, Subsystem 4 Security Boundaries - "Reject: Nonce decrements"
/// Reference: System.md, Subsystem 4 - Transaction replay prevention
/// 
/// For any PROCESSED TRANSACTION, the sender's account nonce MUST:
/// 1. Equal the expected nonce at transaction start (validation)
/// 2. Increment by exactly 1 after successful execution (state transition)
/// 
/// ALLOWED: Nonce stays same when NO transaction processed for account in block
/// FORBIDDEN: Nonce decrement (always)
/// FORBIDDEN: Nonce increment > 1 per transaction
/// FORBIDDEN: Nonce skip (processing tx with nonce > expected)
fn invariant_nonce_strictly_monotonic(
    old: &AccountState, 
    new: &AccountState,
    tx_processed: bool,
) -> bool {
    if tx_processed {
        // Exactly +1 per processed transaction
        new.nonce == old.nonce + 1
    } else {
        // No change if no transaction processed
        new.nonce == old.nonce
    }
}

/// INVARIANT-3: Deterministic State Root
/// Given identical inputs, state root computation is always identical.
/// Achieved via canonical ordering and deterministic hashing.

/// INVARIANT-4: Proof Validity
/// Any generated state proof can be verified against the state root.
fn invariant_proof_valid(proof: &StateProof) -> bool {
    verify_merkle_proof(&proof.proof_nodes, &proof.address, &proof.state_root)
}

/// INVARIANT-5: Atomic Transitions
/// State transitions for a block are all-or-nothing.
/// Partial block application is impossible.
```

---

## 3. PORTS (HEXAGONAL ARCHITECTURE)

### 3.1 Driving Ports (API - Inbound)

```rust
/// Primary API for state operations
#[async_trait]
pub trait StateManagementApi: Send + Sync {
    // === State Reads ===
    
    /// Get account state at a specific block
    async fn get_account_state(
        &self,
        address: Address,
        block_number: Option<u64>,
    ) -> Result<Option<AccountState>, StateError>;
    
    /// Get storage value for a contract
    async fn get_storage(
        &self,
        address: Address,
        key: StorageKey,
        block_number: Option<u64>,
    ) -> Result<Option<StorageValue>, StateError>;
    
    /// Get account balance (convenience method)
    async fn get_balance(
        &self,
        address: Address,
        block_number: Option<u64>,
    ) -> Result<U256, StateError>;
    
    /// Get account nonce (convenience method)
    async fn get_nonce(
        &self,
        address: Address,
        block_number: Option<u64>,
    ) -> Result<u64, StateError>;
    
    // === Proofs ===
    
    /// Generate state proof for an account
    async fn get_state_proof(
        &self,
        address: Address,
        block_number: Option<u64>,
    ) -> Result<StateProof, StateError>;
    
    /// Generate storage proof for a contract slot
    async fn get_storage_proof(
        &self,
        address: Address,
        keys: Vec<StorageKey>,
        block_number: Option<u64>,
    ) -> Result<StorageProof, StateError>;
    
    // === Validation ===
    
    /// Check if account has sufficient balance
    async fn check_balance(
        &self,
        address: Address,
        required: U256,
    ) -> Result<bool, StateError>;
    
    /// Get expected nonce for next transaction
    async fn get_expected_nonce(
        &self,
        address: Address,
    ) -> Result<u64, StateError>;
    
    // === Conflict Detection (for Subsystem 12) ===
    
    /// Detect read/write conflicts between transactions
    async fn detect_conflicts(
        &self,
        access_patterns: Vec<TransactionAccessPattern>,
    ) -> Result<Vec<ConflictInfo>, StateError>;
    
    // === State Root ===
    
    /// Get state root at a specific block
    async fn get_state_root(
        &self,
        block_number: u64,
    ) -> Result<Hash, StateError>;
    
    /// Get current (latest) state root
    async fn get_current_state_root(&self) -> Result<Hash, StateError>;
}

/// Transaction access pattern for conflict detection
#[derive(Clone, Debug)]
pub struct TransactionAccessPattern {
    pub tx_hash: Hash,
    pub reads: Vec<(Address, Option<StorageKey>)>,
    pub writes: Vec<(Address, Option<StorageKey>)>,
}

/// Conflict information
#[derive(Clone, Debug)]
pub struct ConflictInfo {
    pub tx1_index: usize,
    pub tx2_index: usize,
    pub conflict_type: ConflictType,
    pub conflicting_address: Address,
    pub conflicting_key: Option<StorageKey>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConflictType {
    ReadWrite,
    WriteWrite,
    NonceConflict,
}
```

### 3.2 Driven Ports (SPI - Outbound Dependencies)

```rust
/// Trie database abstraction
#[async_trait]
pub trait TrieDatabase: Send + Sync {
    /// Get a trie node by hash
    async fn get_node(&self, hash: &Hash) -> Result<Option<Vec<u8>>, DbError>;
    
    /// Store a trie node
    async fn put_node(&self, hash: Hash, data: Vec<u8>) -> Result<(), DbError>;
    
    /// Batch write nodes atomically
    async fn batch_put(&self, nodes: Vec<(Hash, Vec<u8>)>) -> Result<(), DbError>;
    
    /// Delete a trie node (for pruning)
    async fn delete_node(&self, hash: &Hash) -> Result<(), DbError>;
}

/// State snapshot storage
#[async_trait]
pub trait SnapshotStorage: Send + Sync {
    /// Create a snapshot at a block height
    async fn create_snapshot(
        &self,
        block_height: u64,
        state_root: Hash,
    ) -> Result<(), SnapshotError>;
    
    /// Get nearest snapshot before a block height
    async fn get_nearest_snapshot(
        &self,
        block_height: u64,
    ) -> Result<Option<(u64, Hash)>, SnapshotError>;
    
    /// Delete old snapshots (pruning)
    async fn prune_snapshots(
        &self,
        keep_after: u64,
    ) -> Result<u64, SnapshotError>;
}

/// Event bus interface for choreography
#[async_trait]
pub trait EventBus: Send + Sync {
    /// Subscribe to BlockValidated events
    async fn subscribe_block_validated(&self) -> Result<Receiver<BlockValidatedEvent>, EventError>;
    
    /// Publish StateRootComputed event
    async fn publish_state_root_computed(
        &self,
        event: StateRootComputedEvent,
    ) -> Result<(), EventError>;
}
```

---

## 4. EVENT SCHEMA (V2.3 CHOREOGRAPHY)

### 4.1 Events Subscribed (Incoming)

```rust
/// V2.3: Subscribed from Event Bus (published by Consensus, Subsystem 8)
/// This triggers state root computation as part of the choreography
/// 
/// SECURITY (Envelope-Only Identity): Identity from envelope only.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockValidatedEvent {
    /// Block hash (correlation key for assembly)
    pub block_hash: Hash,
    /// Block height
    pub block_height: u64,
    /// The validated block with all transactions
    pub block: ValidatedBlock,
    /// Consensus proof for the block
    pub consensus_proof: ConsensusProof,
}
```

### 4.2 Events Published (Outgoing)

```rust
/// V2.3: Published to Event Bus after computing state root
/// Block Storage (Subsystem 2) subscribes as part of Stateful Assembler
/// 
/// SECURITY (Envelope-Only Identity): No requester_id in payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateRootComputedPayload {
    /// Block hash this state root corresponds to (correlation key)
    pub block_hash: Hash,
    /// Block height for ordering
    pub block_height: u64,
    /// The computed state root
    pub state_root: Hash,
    /// Previous state root (for verification)
    pub previous_state_root: Hash,
    /// Number of accounts modified
    pub accounts_modified: u32,
    /// Number of storage slots modified
    pub storage_slots_modified: u32,
    /// Computation time in milliseconds (observability)
    pub computation_time_ms: u64,
}
```

### 4.3 Request/Response Messages

```rust
/// State read request from Subsystems 6, 11, 12, 14
/// 
/// SECURITY: Envelope sender_id must be 6, 11, 12, or 14
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateReadRequest {
    pub correlation_id: CorrelationId,
    pub reply_to: Topic,
    pub address: Address,
    pub storage_key: Option<StorageKey>,
    pub block_number: Option<u64>,
}

/// State read response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateReadResponse {
    pub correlation_id: CorrelationId,
    pub address: Address,
    pub storage_key: Option<StorageKey>,
    pub value: Option<Vec<u8>>,
    pub proof: Option<Vec<Vec<u8>>>,  // Include proof for verification
    pub block_number: u64,
}

/// State write request from Subsystem 11 ONLY
/// 
/// SECURITY: Envelope sender_id MUST be 11 (Smart Contracts)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateWriteRequest {
    pub correlation_id: CorrelationId,
    pub reply_to: Topic,
    pub address: Address,
    pub storage_key: StorageKey,
    pub value: StorageValue,
    pub execution_context: ExecutionContext,
}

/// Execution context for state writes
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub block_height: u64,
    pub tx_hash: Hash,
    pub tx_index: u32,
    pub gas_used: u64,
}

/// Balance check request from Subsystem 6 (Mempool)
/// 
/// SECURITY: Envelope sender_id MUST be 6
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BalanceCheckRequest {
    pub correlation_id: CorrelationId,
    pub reply_to: Topic,
    pub address: Address,
    pub required_balance: U256,
}

/// Balance check response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BalanceCheckResponse {
    pub correlation_id: CorrelationId,
    pub address: Address,
    pub has_sufficient_balance: bool,
    pub current_balance: U256,
    pub required_balance: U256,
}

/// Conflict detection request from Subsystem 12
/// 
/// SECURITY: Envelope sender_id MUST be 12
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConflictDetectionRequest {
    pub correlation_id: CorrelationId,
    pub reply_to: Topic,
    pub transactions: Vec<TransactionAccessPattern>,
}

/// Conflict detection response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConflictDetectionResponse {
    pub correlation_id: CorrelationId,
    pub conflicts: Vec<ConflictInfo>,
    pub total_transactions: usize,
    pub conflicting_pairs: usize,
}
```

### 4.4 Message Handler Implementation

```rust
impl StateManagementService {
    /// V2.3 Choreography: Handle BlockValidated event
    pub async fn handle_block_validated(
        &mut self,
        envelope: AuthenticatedMessage<BlockValidatedEvent>,
    ) -> Result<(), StateError> {
        // 1. Verify envelope
        if envelope.sender_id != SubsystemId::Consensus {
            return Err(StateError::UnauthorizedSender(envelope.sender_id));
        }
        
        let event = envelope.payload;
        let start_time = Instant::now();
        
        // 2. Apply all transactions to compute new state
        let previous_state_root = self.get_current_state_root().await?;
        let mut accounts_modified = 0u32;
        let mut storage_slots_modified = 0u32;
        
        for tx in &event.block.transactions {
            let transition = self.compute_transition(tx)?;
            accounts_modified += transition.account_transitions.len() as u32;
            for acc in &transition.account_transitions {
                storage_slots_modified += acc.storage_changes.len() as u32;
            }
            self.apply_transition(transition).await?;
        }
        
        // 3. Compute new state root
        let state_root = self.trie.root_hash();
        
        // 4. Commit the state changes
        self.commit_block(event.block_height).await?;
        
        // 5. Publish StateRootComputed to Event Bus
        let payload = StateRootComputedPayload {
            block_hash: event.block_hash,
            block_height: event.block_height,
            state_root,
            previous_state_root,
            accounts_modified,
            storage_slots_modified,
            computation_time_ms: start_time.elapsed().as_millis() as u64,
        };
        
        self.event_bus.publish_state_root_computed(
            self.sign_payload(payload)?
        ).await?;
        
        Ok(())
    }
}
```

---

## 5. TEST-DRIVEN DEVELOPMENT STRATEGY

### 5.1 Unit Tests

```rust
#[cfg(test)]
mod unit_tests {
    use super::*;
    
    // === Invariant Tests ===
    
    #[test]
    fn test_invariant_balance_non_negative() {
        // Balance underflow should be caught during transition
        let account = AccountState {
            balance: U256::from(100),
            nonce: 0,
            code_hash: EMPTY_CODE_HASH,
            storage_root: EMPTY_TRIE_ROOT,
        };
        
        let transition = AccountTransition {
            address: [0u8; 20],
            balance_delta: -101,  // Attempt to overdraw
            nonce_increment: 1,
            storage_changes: vec![],
            code_change: None,
        };
        
        let result = apply_transition(&account, &transition);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), StateError::InsufficientBalance { .. }));
    }
    
    #[test]
    fn test_invariant_nonce_monotonic() {
        // Reference: IPC-MATRIX.md, Subsystem 4 - "Reject: Nonce decrements"
        
        let old_state = AccountState {
            balance: U256::from(1000),
            nonce: 5,
            code_hash: EMPTY_CODE_HASH,
            storage_root: EMPTY_TRIE_ROOT,
        };
        
        // CASE 1: Nonce decrement MUST fail
        let transition_decrement = AccountTransition {
            address: [0u8; 20],
            balance_delta: 0,
            nonce_increment: 0,  // Would result in nonce staying same when tx processed
            storage_changes: vec![],
            code_change: None,
        };
        
        let result = apply_transition_with_tx(&old_state, &transition_decrement);
        assert!(result.is_err(), "Nonce must increment for processed transaction");
        assert!(matches!(result.unwrap_err(), StateError::InvalidNonce { .. }));
        
        // CASE 2: Nonce increment of exactly 1 MUST succeed
        let transition_valid = AccountTransition {
            address: [0u8; 20],
            balance_delta: -100,
            nonce_increment: 1,
            storage_changes: vec![],
            code_change: None,
        };
        
        let result = apply_transition_with_tx(&old_state, &transition_valid);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().nonce, 6);
        
        // CASE 3: Nonce increment > 1 MUST fail (gap)
        let transition_gap = AccountTransition {
            address: [0u8; 20],
            balance_delta: 0,
            nonce_increment: 2,  // Skip nonce 6, go to 7
            storage_changes: vec![],
            code_change: None,
        };
        
        let result = apply_transition_with_tx(&old_state, &transition_gap);
        assert!(result.is_err(), "Nonce gap must be rejected");
    }
    
    #[test]
    fn test_invariant_deterministic_state_root() {
        let mut trie1 = PatriciaMerkleTrie::new();
        let mut trie2 = PatriciaMerkleTrie::new();
        
        // Apply same transitions in same order
        let transitions = vec![
            (Address::from([1u8; 20]), U256::from(100)),
            (Address::from([2u8; 20]), U256::from(200)),
            (Address::from([3u8; 20]), U256::from(300)),
        ];
        
        for (addr, balance) in &transitions {
            trie1.set_balance(*addr, *balance).unwrap();
            trie2.set_balance(*addr, *balance).unwrap();
        }
        
        assert_eq!(trie1.root_hash(), trie2.root_hash());
    }
    
    // === Trie Operation Tests ===
    
    #[test]
    fn test_trie_insert_and_get() {
        let mut trie = PatriciaMerkleTrie::new();
        let address = Address::from([0xAB; 20]);
        
        let account = AccountState {
            balance: U256::from(1_000_000),
            nonce: 42,
            code_hash: EMPTY_CODE_HASH,
            storage_root: EMPTY_TRIE_ROOT,
        };
        
        trie.insert_account(address, &account).unwrap();
        let retrieved = trie.get_account(address).unwrap();
        
        assert_eq!(retrieved, Some(account));
    }
    
    #[test]
    fn test_trie_proof_generation() {
        let mut trie = PatriciaMerkleTrie::new();
        let address = Address::from([0xCD; 20]);
        
        let account = AccountState {
            balance: U256::from(500),
            nonce: 1,
            code_hash: EMPTY_CODE_HASH,
            storage_root: EMPTY_TRIE_ROOT,
        };
        
        trie.insert_account(address, &account).unwrap();
        
        let proof = trie.generate_proof(address).unwrap();
        let verified = verify_proof(&proof, &address, &trie.root_hash());
        
        assert!(verified);
    }
    
    #[test]
    fn test_trie_nonexistent_account_proof() {
        let trie = PatriciaMerkleTrie::new();
        let address = Address::from([0xFF; 20]);
        
        // Should generate exclusion proof
        let proof = trie.generate_proof(address).unwrap();
        assert!(proof.account_state.is_none());
        
        // Proof should still be verifiable
        let verified = verify_proof(&proof, &address, &trie.root_hash());
        assert!(verified);
    }
    
    // === Storage Tests ===
    
    #[test]
    fn test_contract_storage() {
        let mut trie = PatriciaMerkleTrie::new();
        let contract = Address::from([0x42; 20]);
        let key = StorageKey::from([0x01; 32]);
        let value = StorageValue::from([0xFF; 32]);
        
        trie.set_storage(contract, key, value).unwrap();
        let retrieved = trie.get_storage(contract, key).unwrap();
        
        assert_eq!(retrieved, Some(value));
    }
    
    #[test]
    fn test_storage_deletion() {
        let mut trie = PatriciaMerkleTrie::new();
        let contract = Address::from([0x42; 20]);
        let key = StorageKey::from([0x01; 32]);
        let value = StorageValue::from([0xFF; 32]);
        
        trie.set_storage(contract, key, value).unwrap();
        trie.delete_storage(contract, key).unwrap();
        
        let retrieved = trie.get_storage(contract, key).unwrap();
        assert_eq!(retrieved, None);
    }
}
```

### 5.2 Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_choreography_block_validated_to_state_root() {
        // Setup
        let (event_bus, mut rx) = create_mock_event_bus();
        let db = create_mock_trie_db();
        let mut service = StateManagementService::new(db, event_bus);
        
        // Create a BlockValidated event
        let block = create_test_block_with_transfers(vec![
            (ALICE, BOB, 100),
            (BOB, CHARLIE, 50),
        ]);
        
        let event = BlockValidatedEvent {
            block_hash: block.hash(),
            block_height: 1,
            block: block.clone(),
            consensus_proof: ConsensusProof::mock(),
        };
        
        let envelope = create_authenticated_message(
            SubsystemId::Consensus,
            event,
        );
        
        // Act
        service.handle_block_validated(envelope).await.unwrap();
        
        // Assert: StateRootComputed was published
        let published = rx.recv().await.unwrap();
        assert_eq!(published.block_hash, block.hash());
        assert_eq!(published.block_height, 1);
        assert_ne!(published.state_root, EMPTY_TRIE_ROOT);
        assert_eq!(published.accounts_modified, 3); // ALICE, BOB, CHARLIE
    }
    
    #[tokio::test]
    async fn test_reject_unauthorized_block_validated() {
        let (event_bus, _) = create_mock_event_bus();
        let db = create_mock_trie_db();
        let mut service = StateManagementService::new(db, event_bus);
        
        let event = BlockValidatedEvent {
            block_hash: [0u8; 32],
            block_height: 1,
            block: ValidatedBlock::empty(),
            consensus_proof: ConsensusProof::mock(),
        };
        
        // Attempt from wrong sender
        let envelope = create_authenticated_message(
            SubsystemId::Mempool,  // Wrong sender!
            event,
        );
        
        let result = service.handle_block_validated(envelope).await;
        assert!(matches!(result, Err(StateError::UnauthorizedSender(_))));
    }
    
    #[tokio::test]
    async fn test_balance_check_for_mempool() {
        let (event_bus, _) = create_mock_event_bus();
        let db = create_mock_trie_db();
        let mut service = StateManagementService::new(db, event_bus);
        
        // Setup: Give ALICE 1000 tokens
        service.set_balance(ALICE, U256::from(1000)).await.unwrap();
        
        // Check: ALICE has enough for 500
        let request = BalanceCheckRequest {
            correlation_id: random_correlation_id(),
            reply_to: Topic::from("mempool.responses"),
            address: ALICE,
            required_balance: U256::from(500),
        };
        
        let envelope = create_authenticated_message(SubsystemId::Mempool, request);
        let response = service.handle_balance_check(envelope).await.unwrap();
        
        assert!(response.has_sufficient_balance);
        assert_eq!(response.current_balance, U256::from(1000));
    }
    
    #[tokio::test]
    async fn test_conflict_detection() {
        let (event_bus, _) = create_mock_event_bus();
        let db = create_mock_trie_db();
        let service = StateManagementService::new(db, event_bus);
        
        // Two transactions that both write to same storage slot
        let patterns = vec![
            TransactionAccessPattern {
                tx_hash: [1u8; 32],
                reads: vec![],
                writes: vec![(CONTRACT_A, Some(SLOT_1))],
            },
            TransactionAccessPattern {
                tx_hash: [2u8; 32],
                reads: vec![],
                writes: vec![(CONTRACT_A, Some(SLOT_1))],  // Conflict!
            },
        ];
        
        let request = ConflictDetectionRequest {
            correlation_id: random_correlation_id(),
            reply_to: Topic::from("ordering.responses"),
            transactions: patterns,
        };
        
        let envelope = create_authenticated_message(SubsystemId::TransactionOrdering, request);
        let response = service.handle_conflict_detection(envelope).await.unwrap();
        
        assert_eq!(response.conflicts.len(), 1);
        assert_eq!(response.conflicts[0].conflict_type, ConflictType::WriteWrite);
    }
}
```

### 5.3 Security Tests

```rust
#[cfg(test)]
mod security_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_reject_state_write_from_non_smart_contracts() {
        let service = create_test_service();
        
        let request = StateWriteRequest {
            correlation_id: random_correlation_id(),
            reply_to: Topic::from("test"),
            address: CONTRACT_A,
            storage_key: SLOT_1,
            value: [0xFF; 32],
            execution_context: ExecutionContext::mock(),
        };
        
        // Try from Mempool (should fail)
        let envelope = create_authenticated_message(SubsystemId::Mempool, request.clone());
        let result = service.handle_state_write(envelope).await;
        assert!(matches!(result, Err(StateError::UnauthorizedSender(_))));
        
        // Try from Consensus (should fail)
        let envelope = create_authenticated_message(SubsystemId::Consensus, request.clone());
        let result = service.handle_state_write(envelope).await;
        assert!(matches!(result, Err(StateError::UnauthorizedSender(_))));
        
        // Try from SmartContracts (should succeed)
        let envelope = create_authenticated_message(SubsystemId::SmartContracts, request);
        let result = service.handle_state_write(envelope).await;
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_proof_tampering_detection() {
        let mut trie = PatriciaMerkleTrie::new();
        let address = Address::from([0xAB; 20]);
        
        trie.insert_account(address, &AccountState {
            balance: U256::from(1000),
            nonce: 1,
            code_hash: EMPTY_CODE_HASH,
            storage_root: EMPTY_TRIE_ROOT,
        }).unwrap();
        
        let mut proof = trie.generate_proof(address).unwrap();
        
        // Tamper with the proof
        if let Some(ref mut node) = proof.proof_nodes.first_mut() {
            node[0] ^= 0xFF;
        }
        
        // Verification should fail
        let verified = verify_proof(&proof, &address, &trie.root_hash());
        assert!(!verified);
    }
    
    #[test]
    fn test_state_bloat_protection() {
        let mut trie = PatriciaMerkleTrie::new();
        let contract = Address::from([0x42; 20]);
        
        // Attempt to create excessive storage slots
        let max_slots = 10_000;  // Configurable limit
        
        for i in 0..max_slots + 1 {
            let key = StorageKey::from([i as u8; 32]);
            let value = StorageValue::from([0xFF; 32]);
            
            let result = trie.set_storage(contract, key, value);
            
            if i >= max_slots {
                // Should be rejected after limit
                assert!(matches!(result, Err(StateError::StorageLimitExceeded)));
            }
        }
    }
}
```

### 5.4 Test Naming Conventions

| Test Category | Prefix | Example |
|---------------|--------|---------|
| Invariant | `test_invariant_*` | `test_invariant_balance_non_negative` |
| Unit | `test_*` | `test_trie_insert_and_get` |
| Integration | `test_*` (async) | `test_choreography_block_validated_to_state_root` |
| Security | `test_*` | `test_reject_state_write_from_non_smart_contracts` |
| Performance | `bench_*` | `bench_state_root_computation` |

---

## 6. ERROR HANDLING

```rust
#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("Account not found: {address:?}")]
    AccountNotFound { address: Address },
    
    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: U256, available: U256 },
    
    #[error("Invalid nonce: expected {expected}, got {actual}")]
    InvalidNonce { expected: u64, actual: u64 },
    
    #[error("Storage limit exceeded for contract {address:?}")]
    StorageLimitExceeded { address: Address },
    
    #[error("Block not found: height {height}")]
    BlockNotFound { height: u64 },
    
    #[error("Unauthorized sender: {0:?}")]
    UnauthorizedSender(SubsystemId),
    
    #[error("Trie database error: {0}")]
    DatabaseError(#[from] DbError),
    
    #[error("Proof verification failed")]
    ProofVerificationFailed,
    
    #[error("State root mismatch: expected {expected:?}, got {actual:?}")]
    StateRootMismatch { expected: Hash, actual: Hash },
    
    #[error("Snapshot not found for block {height}")]
    SnapshotNotFound { height: u64 },
}
```

---

## 7. ARCHITECTURAL CONTEXT

### 7.1 Finality Circuit Breaker Awareness

**Reference:** Architecture.md Section 5.4.1

This subsystem should be aware that the Finality subsystem (9) uses a deterministic circuit breaker. If Finality enters `HALTED_AWAITING_INTERVENTION` state after 3 failed sync attempts, it will stop emitting `MarkFinalizedRequest` events to Block Storage.

**Impact on State Management:**
- State Management is NOT directly affected by the circuit breaker
- However, operators debugging "why isn't state advancing" should check Finality status
- State continues to be computed for each `BlockValidated` event regardless of finality

### 7.2 Stateful Assembler Timeout

**Reference:** SPEC-02-BLOCK-STORAGE.md Section 1.2

Block Storage times out incomplete assemblies after 30 seconds. If State Management takes longer than 30 seconds to compute `StateRootComputed`, the block will be dropped.

**Performance Target:**
- State root computation MUST complete in < 10 seconds under normal load
- If approaching 30s, consider state computation optimizations or sharding

### 7.3 Future: State Sharding (V2 Architecture)

**Reference:** System.md, Subsystem 4, Future Scalability

The current single-trie architecture may become a bottleneck at scale. V2 plans include:
- Sharded state model with parallel access
- Merkle forest (root of shard roots)
- Cross-shard atomic commits

This SPEC is designed to allow future migration to sharded state.

---

## 8. OBSERVABILITY

### 8.1 Metrics

```rust
/// Key metrics to export
pub struct StateMetrics {
    /// Time to compute state root (histogram)
    pub state_root_computation_time_ms: Histogram,
    /// Number of accounts modified per block (gauge)
    pub accounts_modified_per_block: Gauge,
    /// Number of storage slots modified per block (gauge)
    pub storage_slots_modified_per_block: Gauge,
    /// Trie depth (gauge)
    pub trie_depth: Gauge,
    /// Cache hit rate (counter)
    pub cache_hits: Counter,
    pub cache_misses: Counter,
    /// State proof generation time (histogram)
    pub proof_generation_time_ms: Histogram,
}
```

### 8.2 Logging

```rust
// Key log events
tracing::info!(
    block_hash = %hex::encode(event.block_hash),
    block_height = event.block_height,
    accounts_modified = accounts_modified,
    computation_time_ms = elapsed,
    "StateRootComputed"
);

tracing::warn!(
    sender_id = ?envelope.sender_id,
    "Rejected message from unauthorized sender"
);
```

---

## APPENDIX A: DEPENDENCY MATRIX

| This Subsystem | Depends On | Dependency Type | Purpose |
|----------------|------------|-----------------|---------|
| State Mgmt (4) | Event Bus | Subscribe | BlockValidated events |
| State Mgmt (4) | Event Bus | Publish | StateRootComputed events |
| State Mgmt (4) | Subsystem 11 | Accepts from | State write requests |
| State Mgmt (4) | Subsystem 6 | Accepts from | Balance/nonce checks |
| State Mgmt (4) | Subsystem 12 | Accepts from | Conflict detection |
| State Mgmt (4) | Subsystem 14 | Accepts from | Sharded state access |

## APPENDIX B: CONFIGURATION

```toml
[state_management]
# Patricia Merkle Trie configuration
max_trie_depth = 64
node_cache_size_mb = 512

# Snapshots
enable_snapshots = true
snapshot_interval_blocks = 128

# Pruning
enable_pruning = true
keep_last_n_states = 1000

# Performance
parallel_proof_generation = true
max_concurrent_reads = 100

# Limits (DoS protection)
max_storage_slots_per_contract = 10000
max_state_proof_depth = 64
```

---

## APPENDIX C: SECURITY BOUNDARIES (IPC-MATRIX.md Compliance)

**Reference:** IPC-MATRIX.md, Subsystem 4 Section

### C.1 Authorized Message Senders

| Message Type | Authorized Sender(s) | Reference |
|--------------|---------------------|-----------|
| `BlockValidatedEvent` | Subsystem 8 (Consensus) via Event Bus | IPC-MATRIX.md, V2.3 Choreography |
| `StateReadRequest` | Subsystems 6, 11, 12, 14 | IPC-MATRIX.md, Security Boundaries |
| `StateWriteRequest` | Subsystem 11 ONLY | IPC-MATRIX.md, Security Boundaries |
| `BalanceCheckRequest` | Subsystem 6 ONLY | IPC-MATRIX.md, Security Boundaries |
| `ConflictDetectionRequest` | Subsystem 12 ONLY | IPC-MATRIX.md, Security Boundaries |

### C.2 Rejection Rules

```rust
/// SECURITY: These rejection rules are MANDATORY per IPC-MATRIX.md
fn validate_sender(msg: &AuthenticatedMessage<T>) -> Result<(), StateError> {
    match msg.payload_type() {
        PayloadType::StateWriteRequest => {
            // ONLY Smart Contracts (11) can write state
            if msg.sender_id != SubsystemId::SmartContracts {
                return Err(StateError::UnauthorizedSender(msg.sender_id));
            }
        }
        PayloadType::BalanceCheckRequest => {
            // ONLY Mempool (6) can check balances
            if msg.sender_id != SubsystemId::Mempool {
                return Err(StateError::UnauthorizedSender(msg.sender_id));
            }
        }
        PayloadType::ConflictDetectionRequest => {
            // ONLY Transaction Ordering (12) can request conflict detection
            if msg.sender_id != SubsystemId::TransactionOrdering {
                return Err(StateError::UnauthorizedSender(msg.sender_id));
            }
        }
        PayloadType::StateReadRequest => {
            // Multiple subsystems can read: 6, 11, 12, 14
            if !matches!(
                msg.sender_id,
                SubsystemId::Mempool 
                | SubsystemId::SmartContracts 
                | SubsystemId::TransactionOrdering 
                | SubsystemId::Sharding
            ) {
                return Err(StateError::UnauthorizedSender(msg.sender_id));
            }
        }
        _ => {}
    }
    Ok(())
}
```

### C.3 Envelope-Only Identity (V2.2 Amendment)

**Reference:** Architecture.md Section 3.2.1

```
MANDATORY RULE: Payloads MUST NOT contain identity fields.
The sender_id in AuthenticatedMessage envelope is the SOLE source of truth.
This prevents the "Payload Impersonation" attack.
```

---

## APPENDIX D: CROSS-REFERENCES

### D.1 Master Document References

| Document | Section | Relevance to This Spec |
|----------|---------|------------------------|
| Architecture.md | Section 3.2.1 | Envelope-Only Identity mandate |
| Architecture.md | Section 5.1 | Choreography pattern (BlockValidated → StateRootComputed) |
| Architecture.md | Section 5.4.1 | Finality Circuit Breaker awareness |
| IPC-MATRIX.md | Subsystem 4 | Security boundaries and message types |
| System.md | Subsystem 4 | Patricia Merkle Trie algorithm, dependencies |
| System.md | V2.3 Dependency Graph | Event Bus choreography flow |

### D.2 Related Specifications

| Specification | Relationship | Notes |
|--------------|--------------|-------|
| SPEC-02-BLOCK-STORAGE.md | Consumer | Subscribes to our StateRootComputed event (30s timeout) |
| SPEC-03-TRANSACTION-INDEXING.md | Parallel participant | Both subscribe to BlockValidated, publish roots |
| SPEC-06-MEMPOOL.md | Client | Sends BalanceCheckRequest for transaction validation |
| SPEC-08-CONSENSUS.md | Producer | Publishes BlockValidated that triggers our computation |
| SPEC-11-SMART-CONTRACTS.md | Client | Sends StateReadRequest and StateWriteRequest |
| SPEC-12-TRANSACTION-ORDERING.md | Client | Sends ConflictDetectionRequest |
| SPEC-14-SHARDING.md | Client | Accesses partitioned state |

### D.3 Implementation Priority

**Reference:** System.md, Implementation Priority Section

This subsystem is in **Phase 2 (Consensus - Weeks 5-8)** because:
- Depends on Subsystem 11 (Smart Contracts) for state updates
- Participates in the BlockValidated choreography
- Must be ready before Block Storage can assemble complete blocks

---

**END OF SPECIFICATION**

**Next Steps:**
1. Review this specification for completeness
2. Approve for TDD Phase (write tests)
3. Implement domain logic (pass tests)
4. Implement adapters (wire to runtime)
