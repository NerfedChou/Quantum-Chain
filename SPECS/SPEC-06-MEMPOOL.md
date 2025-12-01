# SPECIFICATION: TRANSACTION POOL (MEMPOOL)

**Version:** 2.3  
**Subsystem ID:** 6  
**Bounded Context:** Transaction Management  
**Crate Name:** `crates/mempool`  
**Author:** Systems Architecture Team  
**Date:** 2024-12-01  
**Architecture Compliance:** Architecture.md v2.3, IPC-MATRIX.md v2.3, System.md v2.3

---

## 1. ABSTRACT

### 1.1 Purpose

The **Transaction Pool (Mempool)** subsystem queues, validates, and prioritizes unconfirmed transactions awaiting inclusion in blocks. It provides transactions to Consensus for block building and implements a Two-Phase Commit protocol to prevent transaction loss during block storage.

### 1.2 Responsibility Boundaries

**In Scope:**
- Queue incoming transactions after signature verification
- Validate transactions against current state (balance, nonce)
- Prioritize transactions by gas price
- Provide transaction batches to Consensus for block building
- Implement Two-Phase Commit for safe transaction removal
- Evict low-priority transactions when pool is full
- Handle Replace-by-Fee (RBF) transactions

**Out of Scope:**
- Signature verification (Subsystem 10)
- State management (Subsystem 4)
- Block building logic (Subsystem 8)
- Transaction execution (Subsystem 11)
- Block storage (Subsystem 2)

### 1.3 Critical Design Constraint (Two-Phase Commit)

**Architecture Mandate (System.md v2.3, IPC-MATRIX.md v2.3):**

Transactions are NEVER deleted when proposed for a block. They are only deleted upon receiving `BlockStorageConfirmation` from Block Storage (Subsystem 2).

**Transaction States:**
```
[PENDING] ──propose──→ [PENDING_INCLUSION] ──confirm──→ [DELETED]
                              │
                              └── timeout/reject ──→ [PENDING] (rollback)
```

**Why Two-Phase Commit:**
- Prevents transaction loss if block storage fails
- Ensures atomicity between Consensus and Storage
- Enables automatic rollback on timeout

### 1.4 Trust Model

```
┌─────────────────────────────────────────────────────────────────┐
│                    TRUST BOUNDARY (V2.3)                        │
├─────────────────────────────────────────────────────────────────┤
│  TRUSTED INPUTS:                                                │
│  ├─ AddTransactionRequest from Subsystem 10 (pre-verified sigs)│
│  ├─ GetTransactionsRequest from Subsystem 8 (Consensus)         │
│  └─ BlockStorageConfirmation from Subsystem 2                   │
│                                                                 │
│  VERIFICATION REQUIRED:                                         │
│  ├─ Balance check via Subsystem 4 (State Management)            │
│  └─ Nonce validation via Subsystem 4                            │
│                                                                 │
│  SECURITY (Envelope-Only Identity - V2.2 Amendment):            │
│  Identity from AuthenticatedMessage.sender_id only.             │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. DOMAIN MODEL

### 2.1 Core Entities

```rust
/// A transaction in the mempool
#[derive(Clone, Debug)]
pub struct MempoolTransaction {
    /// The signed transaction
    pub transaction: SignedTransaction,
    /// Transaction hash
    pub hash: Hash,
    /// Current state in two-phase commit
    pub state: TransactionState,
    /// Gas price (for prioritization)
    pub gas_price: U256,
    /// Sender address
    pub sender: Address,
    /// Sender's nonce
    pub nonce: u64,
    /// Time transaction was added
    pub added_at: Instant,
    /// Target block (if in pending_inclusion)
    pub target_block: Option<u64>,
}

/// Transaction state in two-phase commit
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransactionState {
    /// Available for block inclusion
    Pending,
    /// Proposed for a block, awaiting storage confirmation
    PendingInclusion {
        block_height: u64,
        proposed_at: Instant,
    },
}

/// Per-account transaction queue
#[derive(Debug)]
pub struct AccountQueue {
    pub address: Address,
    /// Transactions ordered by nonce
    pub transactions: BTreeMap<u64, MempoolTransaction>,
    /// Expected next nonce (from state)
    pub expected_nonce: u64,
    /// Total gas in queue
    pub total_gas: u64,
}

/// Mempool configuration
#[derive(Clone, Debug)]
pub struct MempoolConfig {
    /// Maximum transactions in pool
    pub max_transactions: usize,
    /// Maximum transactions per account
    pub max_per_account: usize,
    /// Minimum gas price
    pub min_gas_price: U256,
    /// Maximum gas per transaction
    pub max_gas_per_tx: u64,
    /// Pending inclusion timeout (seconds)
    pub pending_inclusion_timeout_secs: u64,
    /// Nonce gap timeout (seconds)
    pub nonce_gap_timeout_secs: u64,
    /// Enable Replace-by-Fee
    pub enable_rbf: bool,
    /// Minimum fee bump for RBF (percentage)
    pub rbf_min_bump_percent: u64,
}

impl Default for MempoolConfig {
    fn default() -> Self {
        Self {
            max_transactions: 5000,
            max_per_account: 16,
            min_gas_price: U256::from(1_000_000_000),  // 1 gwei
            max_gas_per_tx: 30_000_000,
            pending_inclusion_timeout_secs: 30,
            nonce_gap_timeout_secs: 600,  // 10 minutes
            enable_rbf: true,
            rbf_min_bump_percent: 10,
        }
    }
}
```

### 2.2 Priority Queue Structure

```rust
/// Priority queue for transaction ordering
pub struct TransactionPriorityQueue {
    /// All transactions indexed by hash
    by_hash: HashMap<Hash, MempoolTransaction>,
    /// Transactions ordered by gas price (descending)
    by_price: BTreeSet<PricedTransaction>,
    /// Transactions grouped by sender
    by_sender: HashMap<Address, AccountQueue>,
    /// Transactions in pending_inclusion state
    pending_inclusion: HashMap<Hash, PendingInclusionInfo>,
}

/// Transaction with price for ordering
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PricedTransaction {
    pub gas_price: U256,
    pub hash: Hash,
    pub added_at: Instant,
}

impl Ord for PricedTransaction {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher gas price = higher priority
        other.gas_price.cmp(&self.gas_price)
            .then_with(|| self.added_at.cmp(&other.added_at))
            .then_with(|| self.hash.cmp(&other.hash))
    }
}

/// Pending inclusion tracking
#[derive(Clone, Debug)]
pub struct PendingInclusionInfo {
    pub block_height: u64,
    pub block_hash: Option<Hash>,
    pub proposed_at: Instant,
    pub transaction_hashes: Vec<Hash>,
}
```

### 2.3 Invariants

```rust
/// INVARIANT-1: No Duplicate Transactions
/// The same transaction hash cannot exist twice in the pool.
fn invariant_no_duplicates(pool: &TransactionPriorityQueue) -> bool {
    let hashes: HashSet<_> = pool.by_hash.keys().collect();
    hashes.len() == pool.by_hash.len()
}

/// INVARIANT-2: Nonce Ordering
/// Transactions from same sender are ordered by nonce.
fn invariant_nonce_ordering(queue: &AccountQueue) -> bool {
    let mut prev_nonce = None;
    for (nonce, _) in &queue.transactions {
        if let Some(prev) = prev_nonce {
            if *nonce != prev + 1 {
                return false;  // Gap or out of order
            }
        }
        prev_nonce = Some(*nonce);
    }
    true
}

/// INVARIANT-3: Two-Phase Commit Safety
/// Transactions in PendingInclusion are NOT available for re-proposal.
fn invariant_pending_exclusion(pool: &TransactionPriorityQueue, tx: &Hash) -> bool {
    if let Some(tx) = pool.by_hash.get(tx) {
        !matches!(tx.state, TransactionState::PendingInclusion { .. })
            || !pool.by_price.iter().any(|p| p.hash == *tx)
    } else {
        true
    }
}

/// INVARIANT-4: Balance Sufficiency
/// All transactions in pool have sufficient sender balance.
/// (Checked at insertion, may become stale)

/// INVARIANT-5: Pending Inclusion Timeout
/// Transactions in PendingInclusion for > timeout are auto-rolled back.
```

---

## 3. PORTS (HEXAGONAL ARCHITECTURE)

### 3.1 Driving Ports (API - Inbound)

```rust
/// Primary mempool API
#[async_trait]
pub trait MempoolApi: Send + Sync {
    /// Add a pre-verified transaction to the pool
    /// SECURITY: Only accepts from Subsystem 10
    async fn add_transaction(
        &mut self,
        tx: SignedTransaction,
        signature_valid: bool,
    ) -> Result<Hash, MempoolError>;
    
    /// Get transactions for block building
    /// SECURITY: Only accepts from Subsystem 8
    async fn get_transactions_for_block(
        &self,
        max_count: usize,
        max_gas: u64,
    ) -> Result<Vec<SignedTransaction>, MempoolError>;
    
    /// Propose transactions for inclusion (Phase 1)
    /// Moves transactions to PendingInclusion state
    async fn propose_transactions(
        &mut self,
        tx_hashes: Vec<Hash>,
        target_block_height: u64,
    ) -> Result<ProposeResult, MempoolError>;
    
    /// Confirm transactions were stored (Phase 2a)
    /// Permanently deletes transactions
    async fn confirm_inclusion(
        &mut self,
        block_height: u64,
        block_hash: Hash,
        tx_hashes: Vec<Hash>,
    ) -> Result<(), MempoolError>;
    
    /// Rollback proposed transactions (Phase 2b)
    /// Returns transactions to Pending state
    async fn rollback_proposal(
        &mut self,
        block_height: u64,
        tx_hashes: Vec<Hash>,
    ) -> Result<(), MempoolError>;
    
    /// Get mempool status
    async fn get_status(&self) -> MempoolStatus;
    
    /// Get transaction by hash
    async fn get_transaction(&self, hash: Hash) -> Option<MempoolTransaction>;
    
    /// Remove invalid/expired transactions
    async fn remove_transactions(
        &mut self,
        hashes: Vec<Hash>,
        reason: RemovalReason,
    ) -> Result<usize, MempoolError>;
    
    /// Get transactions by hashes for compact block reconstruction
    /// 
    /// Reference: System.md, Subsystem 5 - Compact Block Relay
    /// Reference: IPC-MATRIX.md, Subsystem 6 - Provides to Block Propagation
    /// 
    /// SECURITY: Each returned transaction's signature is RE-VERIFIED via
    /// Subsystem 10 before return to prevent stale/tampered transaction injection.
    async fn get_transactions_for_compact_block(
        &self,
        tx_hashes: Vec<Hash>,
    ) -> Result<Vec<Option<SignedTransaction>>, MempoolError>;

    /// Calculate short transaction IDs for compact block relay
    /// 
    /// Reference: System.md, Subsystem 5 - "short_txids: first 6 bytes XOR'd with salt"
    fn calculate_short_ids(
        &self,
        tx_hashes: &[Hash],
        nonce: u64,
    ) -> Vec<ShortTxId>;
}

/// Result of proposing transactions
#[derive(Clone, Debug)]
pub struct ProposeResult {
    pub proposed_count: usize,
    pub already_pending: Vec<Hash>,
    pub not_found: Vec<Hash>,
}

/// Mempool status
#[derive(Clone, Debug)]
pub struct MempoolStatus {
    pub pending_count: u32,
    pub pending_inclusion_count: u32,
    pub total_gas: u64,
    pub memory_usage_bytes: u64,
    pub oldest_transaction_age_secs: u64,
}

#[derive(Clone, Copy, Debug)]
pub enum RemovalReason {
    Invalid,
    Expired,
    Replaced,  // RBF
}
```

### 3.2 Driven Ports (SPI - Outbound Dependencies)

```rust
/// State management interface for validation
#[async_trait]
pub trait StateProvider: Send + Sync {
    /// Check if sender has sufficient balance for transaction
    async fn check_balance(
        &self,
        address: Address,
        required: U256,
    ) -> Result<bool, StateError>;
    
    /// Get expected nonce for sender
    async fn get_nonce(&self, address: Address) -> Result<u64, StateError>;
}
```

---

## 4. EVENT SCHEMA

### 4.1 Incoming Messages

```rust
/// Add transaction request from Signature Verification (Subsystem 10)
/// SECURITY: Envelope sender_id MUST be 10
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddTransactionRequest {
    pub correlation_id: CorrelationId,
    pub transaction: SignedTransaction,
    pub signature_valid: bool,
}

/// Get transactions request from Consensus (Subsystem 8)
/// SECURITY: Envelope sender_id MUST be 8
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetTransactionsRequest {
    pub correlation_id: CorrelationId,
    pub reply_to: Topic,
    pub max_count: u32,
    pub max_gas: u64,
}

/// Block storage confirmation (Phase 2a) from Block Storage (Subsystem 2)
/// SECURITY: Envelope sender_id MUST be 2
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockStorageConfirmation {
    pub correlation_id: CorrelationId,
    pub block_hash: Hash,
    pub block_height: u64,
    pub included_transactions: Vec<Hash>,
    pub storage_timestamp: u64,
}

/// Block rejected notification (Phase 2b)
/// SECURITY: Envelope sender_id MUST be 2 or 8
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockRejectedNotification {
    pub correlation_id: CorrelationId,
    pub block_hash: Hash,
    pub block_height: u64,
    pub affected_transactions: Vec<Hash>,
    pub rejection_reason: BlockRejectionReason,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum BlockRejectionReason {
    ConsensusRejected,
    StorageFailure,
    Timeout,
    Reorg,
}
```

### 4.2 Outgoing Messages

```rust
/// Transaction batch for block proposal
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProposeTransactionBatch {
    pub correlation_id: CorrelationId,
    pub transactions: Vec<SignedTransaction>,
    pub total_gas: u64,
    pub target_block_height: u64,
    pub proposal_timestamp: u64,
}

/// Balance check request to State Management (Subsystem 4)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BalanceCheckRequest {
    pub correlation_id: CorrelationId,
    pub reply_to: Topic,
    pub address: Address,
    pub required_balance: U256,
}
```

### 4.3 Message Flow: Two-Phase Commit

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    TWO-PHASE TRANSACTION COMMIT                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  PHASE 1: PROPOSE                                                           │
│  ─────────────────                                                          │
│  [Consensus (8)] ──GetTransactionsRequest──→ [Mempool (6)]                  │
│                                                    │                         │
│                                                    ↓                         │
│  [Mempool (6)] ──ProposeTransactionBatch──→ [Consensus (8)]                 │
│       │                                                                      │
│       └── transactions moved to PENDING_INCLUSION                           │
│           (NOT deleted, still in pool)                                       │
│                                                                             │
│  PHASE 2a: CONFIRM (Success Path)                                           │
│  ────────────────────────────────                                           │
│  [Block Storage (2)] ──BlockStorageConfirmation──→ [Mempool (6)]            │
│                                                         │                    │
│                                                         ↓                    │
│                               transactions PERMANENTLY DELETED               │
│                                                                             │
│  PHASE 2b: ROLLBACK (Failure Path)                                          │
│  ─────────────────────────────────                                          │
│  [Consensus/Storage] ──BlockRejectedNotification──→ [Mempool (6)]           │
│                                                          │                   │
│                                                          ↓                   │
│                        transactions moved back to PENDING                    │
│                        (available for next block)                            │
│                                                                             │
│  TIMEOUT HANDLING (30 seconds):                                             │
│  ──────────────────────────────                                             │
│  If no confirm/reject within timeout, auto-rollback to PENDING              │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. TEST-DRIVEN DEVELOPMENT STRATEGY

### 5.1 Unit Tests

```rust
#[cfg(test)]
mod unit_tests {
    use super::*;
    
    // === Two-Phase Commit Tests ===
    
    #[test]
    fn test_propose_moves_to_pending_inclusion() {
        let mut pool = create_test_pool();
        let tx = create_test_transaction(100);
        
        pool.add_transaction(tx.clone(), true).unwrap();
        assert!(matches!(pool.get_state(&tx.hash()), Some(TransactionState::Pending)));
        
        pool.propose_transactions(vec![tx.hash()], 1).unwrap();
        assert!(matches!(
            pool.get_state(&tx.hash()),
            Some(TransactionState::PendingInclusion { block_height: 1, .. })
        ));
        
        // Transaction still exists in pool
        assert!(pool.get_transaction(tx.hash()).is_some());
    }
    
    #[test]
    fn test_confirm_deletes_transaction() {
        let mut pool = create_test_pool();
        let tx = create_test_transaction(100);
        
        pool.add_transaction(tx.clone(), true).unwrap();
        pool.propose_transactions(vec![tx.hash()], 1).unwrap();
        
        pool.confirm_inclusion(1, [0xAB; 32], vec![tx.hash()]).unwrap();
        
        // Transaction is now deleted
        assert!(pool.get_transaction(tx.hash()).is_none());
    }
    
    #[test]
    fn test_rollback_returns_to_pending() {
        let mut pool = create_test_pool();
        let tx = create_test_transaction(100);
        
        pool.add_transaction(tx.clone(), true).unwrap();
        pool.propose_transactions(vec![tx.hash()], 1).unwrap();
        
        pool.rollback_proposal(1, vec![tx.hash()]).unwrap();
        
        // Transaction is back to Pending
        assert!(matches!(pool.get_state(&tx.hash()), Some(TransactionState::Pending)));
        
        // Transaction is available for next proposal
        let available = pool.get_transactions_for_block(10, u64::MAX).unwrap();
        assert!(available.iter().any(|t| t.hash() == tx.hash()));
    }
    
    #[test]
    fn test_pending_inclusion_excluded_from_proposal() {
        let mut pool = create_test_pool();
        let tx1 = create_test_transaction(100);
        let tx2 = create_test_transaction(200);
        
        pool.add_transaction(tx1.clone(), true).unwrap();
        pool.add_transaction(tx2.clone(), true).unwrap();
        
        // Propose tx1
        pool.propose_transactions(vec![tx1.hash()], 1).unwrap();
        
        // Get transactions for next block
        let available = pool.get_transactions_for_block(10, u64::MAX).unwrap();
        
        // tx1 should NOT be in available (it's pending inclusion)
        assert!(!available.iter().any(|t| t.hash() == tx1.hash()));
        // tx2 should be available
        assert!(available.iter().any(|t| t.hash() == tx2.hash()));
    }
    
    // === Priority Tests ===
    
    #[test]
    fn test_higher_gas_price_priority() {
        let mut pool = create_test_pool();
        
        let tx_low = create_transaction_with_gas_price(U256::from(1));
        let tx_high = create_transaction_with_gas_price(U256::from(100));
        
        pool.add_transaction(tx_low.clone(), true).unwrap();
        pool.add_transaction(tx_high.clone(), true).unwrap();
        
        let batch = pool.get_transactions_for_block(1, u64::MAX).unwrap();
        
        // Higher gas price should be first
        assert_eq!(batch[0].hash(), tx_high.hash());
    }
    
    #[test]
    fn test_nonce_ordering_per_account() {
        let mut pool = create_test_pool();
        let sender = Address::from([0xAB; 20]);
        
        // Add out of order
        let tx2 = create_transaction_with_nonce(sender, 2);
        let tx0 = create_transaction_with_nonce(sender, 0);
        let tx1 = create_transaction_with_nonce(sender, 1);
        
        pool.add_transaction(tx2.clone(), true).unwrap();
        pool.add_transaction(tx0.clone(), true).unwrap();
        pool.add_transaction(tx1.clone(), true).unwrap();
        
        let batch = pool.get_transactions_for_block(10, u64::MAX).unwrap();
        
        // Should be ordered by nonce
        let sender_txs: Vec<_> = batch.iter()
            .filter(|t| t.sender() == sender)
            .collect();
        
        assert_eq!(sender_txs[0].nonce(), 0);
        assert_eq!(sender_txs[1].nonce(), 1);
        assert_eq!(sender_txs[2].nonce(), 2);
    }
    
    // === RBF Tests ===
    
    #[test]
    fn test_replace_by_fee() {
        let mut pool = create_test_pool_with_rbf();
        let sender = Address::from([0xAB; 20]);
        
        let tx1 = create_transaction_with_nonce_and_price(sender, 0, U256::from(100));
        let tx2 = create_transaction_with_nonce_and_price(sender, 0, U256::from(115));  // 15% higher
        
        pool.add_transaction(tx1.clone(), true).unwrap();
        pool.add_transaction(tx2.clone(), true).unwrap();
        
        // tx1 should be replaced
        assert!(pool.get_transaction(tx1.hash()).is_none());
        assert!(pool.get_transaction(tx2.hash()).is_some());
    }
    
    #[test]
    fn test_rbf_requires_minimum_bump() {
        let mut pool = create_test_pool_with_rbf();
        let sender = Address::from([0xAB; 20]);
        
        let tx1 = create_transaction_with_nonce_and_price(sender, 0, U256::from(100));
        let tx2 = create_transaction_with_nonce_and_price(sender, 0, U256::from(105));  // Only 5% higher
        
        pool.add_transaction(tx1.clone(), true).unwrap();
        let result = pool.add_transaction(tx2.clone(), true);
        
        // Should fail - bump too small
        assert!(matches!(result, Err(MempoolError::InsufficientFeeBump)));
        
        // tx1 should still exist
        assert!(pool.get_transaction(tx1.hash()).is_some());
    }
    
    // === Eviction Tests ===
    
    #[test]
    fn test_evict_lowest_fee_when_full() {
        let config = MempoolConfig {
            max_transactions: 3,
            ..Default::default()
        };
        let mut pool = create_test_pool_with_config(config);
        
        let tx_low = create_transaction_with_gas_price(U256::from(1));
        let tx_med = create_transaction_with_gas_price(U256::from(50));
        let tx_high = create_transaction_with_gas_price(U256::from(100));
        let tx_higher = create_transaction_with_gas_price(U256::from(200));
        
        pool.add_transaction(tx_low.clone(), true).unwrap();
        pool.add_transaction(tx_med.clone(), true).unwrap();
        pool.add_transaction(tx_high.clone(), true).unwrap();
        
        // Pool is now full, adding tx_higher should evict tx_low
        pool.add_transaction(tx_higher.clone(), true).unwrap();
        
        assert!(pool.get_transaction(tx_low.hash()).is_none());
        assert!(pool.get_transaction(tx_higher.hash()).is_some());
    }
}
```

### 5.2 Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_full_two_phase_commit_success() {
        // Setup
        let (state_provider, _) = create_mock_state_provider();
        let (event_bus, mut storage_rx) = create_mock_event_bus();
        let mut service = MempoolService::new(state_provider, event_bus);
        
        // Add transactions
        let tx1 = create_test_transaction(100);
        let tx2 = create_test_transaction(200);
        
        service.handle_add_transaction(
            create_envelope(SubsystemId::SignatureVerification,
                AddTransactionRequest { transaction: tx1.clone(), signature_valid: true, correlation_id: random_id() })
        ).await.unwrap();
        
        service.handle_add_transaction(
            create_envelope(SubsystemId::SignatureVerification,
                AddTransactionRequest { transaction: tx2.clone(), signature_valid: true, correlation_id: random_id() })
        ).await.unwrap();
        
        // Consensus requests transactions
        let request = GetTransactionsRequest {
            correlation_id: random_id(),
            reply_to: Topic::from("consensus.responses"),
            max_count: 10,
            max_gas: u64::MAX,
        };
        
        let response = service.handle_get_transactions(
            create_envelope(SubsystemId::Consensus, request)
        ).await.unwrap();
        
        assert_eq!(response.transactions.len(), 2);
        
        // Simulate block storage confirmation
        let confirmation = BlockStorageConfirmation {
            correlation_id: random_id(),
            block_hash: [0xAB; 32],
            block_height: 1,
            included_transactions: vec![tx1.hash(), tx2.hash()],
            storage_timestamp: now(),
        };
        
        service.handle_storage_confirmation(
            create_envelope(SubsystemId::BlockStorage, confirmation)
        ).await.unwrap();
        
        // Transactions should be deleted
        assert!(service.get_transaction(tx1.hash()).is_none());
        assert!(service.get_transaction(tx2.hash()).is_none());
    }
    
    #[tokio::test]
    async fn test_two_phase_commit_rollback() {
        let mut service = create_test_service();
        
        let tx = create_test_transaction(100);
        service.add_transaction(tx.clone(), true).await.unwrap();
        
        // Propose
        service.propose_transactions(vec![tx.hash()], 1).await.unwrap();
        
        // Block rejected
        let notification = BlockRejectedNotification {
            correlation_id: random_id(),
            block_hash: [0xAB; 32],
            block_height: 1,
            affected_transactions: vec![tx.hash()],
            rejection_reason: BlockRejectionReason::ConsensusRejected,
        };
        
        service.handle_block_rejected(
            create_envelope(SubsystemId::Consensus, notification)
        ).await.unwrap();
        
        // Transaction should be back in pending
        let status = service.get_transaction(tx.hash()).unwrap();
        assert!(matches!(status.state, TransactionState::Pending));
    }
    
    #[tokio::test]
    async fn test_pending_inclusion_timeout() {
        let config = MempoolConfig {
            pending_inclusion_timeout_secs: 1,  // 1 second for testing
            ..Default::default()
        };
        let mut service = create_test_service_with_config(config);
        
        let tx = create_test_transaction(100);
        service.add_transaction(tx.clone(), true).await.unwrap();
        service.propose_transactions(vec![tx.hash()], 1).await.unwrap();
        
        // Wait for timeout
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // Run timeout cleanup
        service.cleanup_expired().await;
        
        // Transaction should be rolled back to pending
        let status = service.get_transaction(tx.hash()).unwrap();
        assert!(matches!(status.state, TransactionState::Pending));
    }
    
    #[tokio::test]
    async fn test_balance_validation() {
        let (state_provider, _) = create_mock_state_provider_with_balance(
            ALICE, U256::from(1000)
        );
        let service = MempoolService::new(state_provider, create_mock_event_bus().0);
        
        // Transaction requiring more than balance
        let tx = create_transaction_with_value(ALICE, U256::from(2000));
        
        let result = service.add_transaction(tx, true).await;
        assert!(matches!(result, Err(MempoolError::InsufficientBalance)));
    }
}
```

### 5.3 Security Tests

```rust
#[cfg(test)]
mod security_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_reject_add_from_non_signature_verification() {
        let mut service = create_test_service();
        
        let request = AddTransactionRequest {
            correlation_id: random_id(),
            transaction: create_test_transaction(100),
            signature_valid: true,
        };
        
        // From Consensus (wrong sender)
        let envelope = create_envelope(SubsystemId::Consensus, request.clone());
        let result = service.handle_add_transaction(envelope).await;
        assert!(matches!(result, Err(MempoolError::UnauthorizedSender(_))));
        
        // From correct sender
        let envelope = create_envelope(SubsystemId::SignatureVerification, request);
        let result = service.handle_add_transaction(envelope).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_reject_get_from_non_consensus() {
        let service = create_test_service();
        
        let request = GetTransactionsRequest {
            correlation_id: random_id(),
            reply_to: Topic::from("test"),
            max_count: 10,
            max_gas: u64::MAX,
        };
        
        let envelope = create_envelope(SubsystemId::BlockStorage, request);
        let result = service.handle_get_transactions(envelope).await;
        assert!(matches!(result, Err(MempoolError::UnauthorizedSender(_))));
    }
    
    #[tokio::test]
    async fn test_reject_confirmation_from_non_storage() {
        let mut service = create_test_service();
        
        let confirmation = BlockStorageConfirmation {
            correlation_id: random_id(),
            block_hash: [0xAB; 32],
            block_height: 1,
            included_transactions: vec![],
            storage_timestamp: now(),
        };
        
        let envelope = create_envelope(SubsystemId::Consensus, confirmation);
        let result = service.handle_storage_confirmation(envelope).await;
        assert!(matches!(result, Err(MempoolError::UnauthorizedSender(_))));
    }
    
    #[test]
    fn test_per_account_limit() {
        let config = MempoolConfig {
            max_per_account: 3,
            ..Default::default()
        };
        let mut pool = create_test_pool_with_config(config);
        let sender = Address::from([0xAB; 20]);
        
        for i in 0..3 {
            let tx = create_transaction_with_nonce(sender, i);
            pool.add_transaction(tx, true).unwrap();
        }
        
        // 4th transaction should fail
        let tx = create_transaction_with_nonce(sender, 3);
        let result = pool.add_transaction(tx, true);
        assert!(matches!(result, Err(MempoolError::AccountLimitReached)));
    }
    
    #[test]
    fn test_minimum_gas_price() {
        let config = MempoolConfig {
            min_gas_price: U256::from(1_000_000_000),  // 1 gwei
            ..Default::default()
        };
        let mut pool = create_test_pool_with_config(config);
        
        let tx_low = create_transaction_with_gas_price(U256::from(500_000_000));  // 0.5 gwei
        let result = pool.add_transaction(tx_low, true);
        
        assert!(matches!(result, Err(MempoolError::GasPriceTooLow)));
    }
}
```

---

## 6. ERROR HANDLING

```rust
#[derive(Debug, thiserror::Error)]
pub enum MempoolError {
    #[error("Transaction already in pool: {0:?}")]
    DuplicateTransaction(Hash),
    
    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: U256, available: U256 },
    
    #[error("Invalid nonce: expected {expected}, got {actual}")]
    InvalidNonce { expected: u64, actual: u64 },
    
    #[error("Gas price too low: {price}, minimum: {min}")]
    GasPriceTooLow { price: U256, min: U256 },
    
    #[error("Account transaction limit reached: {address:?}")]
    AccountLimitReached { address: Address },
    
    #[error("Pool is full")]
    PoolFull,
    
    #[error("Transaction not found: {0:?}")]
    TransactionNotFound(Hash),
    
    #[error("Insufficient fee bump for RBF: {bump_percent}% (min: {min_percent}%)")]
    InsufficientFeeBump { bump_percent: u64, min_percent: u64 },
    
    #[error("Unauthorized sender: {0:?}")]
    UnauthorizedSender(SubsystemId),
    
    #[error("State provider error: {0}")]
    StateError(#[from] StateError),
    
    #[error("Transaction in pending inclusion state")]
    TransactionPendingInclusion,
}
```

---

## 7. CONFIGURATION

```toml
[mempool]
# Pool limits
max_transactions = 5000
max_per_account = 16

# Gas settings
min_gas_price_gwei = 1
max_gas_per_tx = 30000000

# Two-Phase Commit
pending_inclusion_timeout_secs = 30

# Eviction
nonce_gap_timeout_secs = 600

# Replace-by-Fee
enable_rbf = true
rbf_min_bump_percent = 10

# Cleanup
cleanup_interval_secs = 60
```

---

## APPENDIX A: DEPENDENCY MATRIX

**Reference:** System.md V2.3 Unified Dependency Graph, IPC-MATRIX.md Subsystem 6

| This Subsystem | Depends On | Dependency Type | Purpose | Reference |
|----------------|------------|-----------------|---------|-----------|
| Mempool (6) | Subsystem 10 (Sig Verify) | Accepts from | Pre-verified transactions | IPC-MATRIX.md Security Boundaries |
| Mempool (6) | Subsystem 10 (Sig Verify) | Query | Re-verify tx sigs for compact block reconstruction | IPC-MATRIX.md §10 |
| Mempool (6) | Subsystem 8 (Consensus) | Accepts from | GetTransactionsRequest | IPC-MATRIX.md Subsystem 6 |
| Mempool (6) | Subsystem 2 (Block Storage) | Accepts from | BlockStorageConfirmation (Two-Phase Commit) | IPC-MATRIX.md Subsystem 6 |
| Mempool (6) | Subsystem 4 (State Mgmt) | Query | Balance/nonce validation | System.md Subsystem 6 |
| Mempool (6) | Subsystem 8 (Consensus) | Sends to | ProposeTransactionBatch | IPC-MATRIX.md Subsystem 8 |

---

## APPENDIX B: SECURITY BOUNDARIES (IPC-MATRIX.md Compliance)

**Reference:** IPC-MATRIX.md, Subsystem 6 Section

### B.1 Authorized Message Senders

| Message Type | Authorized Sender(s) | Reference |
|--------------|---------------------|-----------|
| `AddTransactionRequest` | Subsystem 10 (Signature Verification) ONLY | IPC-MATRIX.md: "Only pre-verified transactions" |
| `GetTransactionsRequest` | Subsystem 8 (Consensus) ONLY | IPC-MATRIX.md Security Boundaries |
| `RemoveTransactionsRequest` | Subsystem 8 (Consensus) ONLY (Invalid/Expired reasons) | IPC-MATRIX.md Security Boundaries |
| `BlockStorageConfirmation` | Subsystem 2 (Block Storage) ONLY | IPC-MATRIX.md Two-Phase Commit |
| `BlockRejectedNotification` | Subsystems 2, 8 ONLY | IPC-MATRIX.md Two-Phase Commit |

### B.2 Mandatory Rejection Rules

**Reference:** IPC-MATRIX.md, Subsystem 6 Security Boundaries

```rust
/// MANDATORY security checks per IPC-MATRIX.md
fn validate_add_transaction_request(
    msg: &AuthenticatedMessage<AddTransactionRequest>
) -> Result<(), MempoolError> {
    // Rule 1: ONLY Signature Verification (10) can add transactions
    if msg.sender_id != SubsystemId::SignatureVerification {
        log::warn!(
            "SECURITY: Rejected AddTransaction from {:?} (only Subsystem 10 allowed)",
            msg.sender_id
        );
        return Err(MempoolError::UnauthorizedSender(msg.sender_id));
    }
    
    // Rule 2: Transaction must be pre-verified
    if !msg.payload.signature_valid {
        return Err(MempoolError::UnverifiedTransaction);
    }
    
    // Rule 3: Gas price minimum
    if msg.payload.transaction.gas_price < MIN_GAS_PRICE {
        return Err(MempoolError::GasPriceTooLow);
    }
    
    Ok(())
}

fn validate_block_storage_confirmation(
    msg: &AuthenticatedMessage<BlockStorageConfirmation>
) -> Result<(), MempoolError> {
    // Rule 1: ONLY Block Storage (2) can confirm
    if msg.sender_id != SubsystemId::BlockStorage {
        return Err(MempoolError::UnauthorizedSender(msg.sender_id));
    }
    
    // Rule 2: Correlation ID must match a pending proposal
    // (prevents replay of old confirmations)
    if !pending_proposals.contains(&msg.payload.correlation_id) {
        return Err(MempoolError::UnknownCorrelationId);
    }
    
    Ok(())
}
```

### B.3 Two-Phase Commit Protocol

**Reference:** IPC-MATRIX.md, "Two-Phase Transaction Removal Protocol"

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    TWO-PHASE TRANSACTION REMOVAL                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  PHASE 1: PROPOSE                                                           │
│  ├─ Mempool receives GetTransactionsRequest from Consensus (8)              │
│  ├─ Mempool moves transactions from 'pending' to 'pending_inclusion'        │
│  ├─ Mempool sends ProposeTransactionBatch to Consensus                      │
│  └─ Transactions are NOT deleted                                            │
│                                                                             │
│  PHASE 2a: CONFIRM (Success Path)                                           │
│  ├─ Block Storage (2) sends BlockStorageConfirmation                        │
│  ├─ Mempool permanently deletes transactions in included_transactions       │
│  └─ Space is freed in mempool                                               │
│                                                                             │
│  PHASE 2b: ROLLBACK (Failure Path)                                          │
│  ├─ Consensus/Storage sends BlockRejectedNotification OR timeout            │
│  ├─ Mempool moves transactions back to 'pending'                            │
│  └─ Transactions available for next block                                   │
│                                                                             │
│  TIMEOUT HANDLING:                                                          │
│  ├─ If no confirmation/rejection within 30 seconds → auto-rollback          │
│  └─ Prevents transactions stuck in limbo                                    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Security Rationale (System.md):**
- Prevents "Transaction Loss" vulnerability
- Ensures atomicity between Consensus and Storage
- Enables automatic rollback on failure

---

## APPENDIX C: CROSS-REFERENCES

### C.1 Master Document References

| Document | Section | Relevance to This Spec |
|----------|---------|------------------------|
| Architecture.md | Section 3.2.1 | Envelope-Only Identity mandate |
| IPC-MATRIX.md | Subsystem 6 | Two-Phase Commit protocol, security boundaries |
| System.md | Subsystem 6 | Priority Queue algorithm, Two-Phase Commit |
| System.md | V2.3 Dependency Graph | Dependencies on 4, 10; provides to 8 |

### C.2 Related Specifications

| Specification | Relationship | Notes |
|--------------|--------------|-------|
| SPEC-02-BLOCK-STORAGE.md | Bidirectional | Sends BlockStorageConfirmation to confirm tx deletion |
| SPEC-04-STATE-MANAGEMENT.md | Dependency | Balance/nonce validation queries |
| SPEC-08-CONSENSUS.md | Bidirectional | Receives GetTransactionsRequest; sends ProposeTransactionBatch |
| SPEC-10-SIGNATURE-VERIFICATION.md | Receives from | Pre-verified transactions |

### C.3 Implementation Priority

**Reference:** System.md, Implementation Priority Section

This subsystem is in **Phase 1 (Core - Weeks 1-4)** because:
- Depends only on Subsystems 4 (State) and 10 (Signatures)
- Required for Consensus to build blocks
- Two-Phase Commit is critical for data integrity

---

## APPENDIX D: TRANSACTION STATE MACHINE

```rust
/// Transaction state transitions (MUST be enforced)
/// 
/// Reference: IPC-MATRIX.md, Two-Phase Transaction Removal Protocol
impl TransactionState {
    /// Valid transitions per the protocol
    fn transition(&self, event: StateEvent) -> Result<TransactionState, InvalidTransition> {
        match (self, event) {
            // PENDING can be proposed for a block
            (TransactionState::Pending, StateEvent::Proposed { block_height }) => {
                Ok(TransactionState::PendingInclusion {
                    block_height,
                    proposed_at: Instant::now(),
                })
            }
            
            // PENDING can be evicted (low priority, expired)
            (TransactionState::Pending, StateEvent::Evicted) => {
                Ok(TransactionState::Deleted)
            }
            
            // PENDING_INCLUSION can be confirmed → permanent delete
            (TransactionState::PendingInclusion { .. }, StateEvent::Confirmed) => {
                Ok(TransactionState::Deleted)
            }
            
            // PENDING_INCLUSION can be rolled back → back to pending
            (TransactionState::PendingInclusion { .. }, StateEvent::Rollback) => {
                Ok(TransactionState::Pending)
            }
            
            // PENDING_INCLUSION times out → auto-rollback
            (TransactionState::PendingInclusion { proposed_at, .. }, StateEvent::Timeout) => {
                if proposed_at.elapsed() > PENDING_INCLUSION_TIMEOUT {
                    Ok(TransactionState::Pending)
                } else {
                    Err(InvalidTransition::NotTimedOut)
                }
            }
            
            // All other transitions are invalid
            _ => Err(InvalidTransition::InvalidStateTransition),
        }
    }
}
```

---

**END OF SPECIFICATION**

**Next Steps:**
1. Review this specification for completeness
2. Approve for TDD Phase (write tests)
3. Implement domain logic (pass tests)
4. Implement adapters (wire to runtime)
