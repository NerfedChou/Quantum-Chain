# SPECIFICATION: SHARDING

**Version:** 2.3  
**Subsystem ID:** 14  
**Bounded Context:** Horizontal Scaling  
**Crate Name:** `crates/sharding`  
**Author:** Systems Architecture Team  
**Date:** 2024-12-01  
**Architecture Compliance:** Architecture.md v2.3, IPC-MATRIX.md v2.3, System.md v2.3

---

## 1. ABSTRACT

### 1.1 Purpose

The **Sharding** subsystem enables horizontal scaling by partitioning blockchain state across multiple shards. Each shard processes transactions independently, with cross-shard communication handled via atomic protocols.

### 1.2 Responsibility Boundaries

**In Scope:**
- Shard assignment (address → shard mapping)
- Shard-local transaction routing
- Cross-shard transaction coordination
- Shard state root aggregation
- Validator assignment to shards

**Out of Scope:**
- Intra-shard consensus (Subsystem 8)
- State storage per shard (Subsystem 4)
- Transaction execution (Subsystem 11)

### 1.3 Trust Model

```
┌─────────────────────────────────────────────────────────────────┐
│                    TRUST BOUNDARY (V2.3)                        │
├─────────────────────────────────────────────────────────────────┤
│  TRUSTED:                                                       │
│  ├─ Shard consensus from Subsystem 8                            │
│  └─ Partitioned state from Subsystem 4                          │
│                                                                 │
│  SECURITY (Envelope-Only Identity - V2.2 Amendment):            │
│  Identity from AuthenticatedMessage.sender_id only.             │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. DOMAIN MODEL

### 2.1 Core Entities

```rust
/// Shard identifier
pub type ShardId = u16;

/// Shard configuration
#[derive(Clone, Debug)]
pub struct ShardConfig {
    /// Total number of shards
    pub shard_count: u16,
    /// Validators per shard
    pub validators_per_shard: usize,
    /// Cross-shard message timeout
    pub cross_shard_timeout_secs: u64,
}

impl Default for ShardConfig {
    fn default() -> Self {
        Self {
            shard_count: 64,
            validators_per_shard: 128,
            cross_shard_timeout_secs: 30,
        }
    }
}

/// Shard assignment for an address
#[derive(Clone, Debug)]
pub struct ShardAssignment {
    pub address: Address,
    pub shard_id: ShardId,
}

/// Cross-shard transaction
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrossShardTransaction {
    pub transaction: SignedTransaction,
    pub source_shard: ShardId,
    pub target_shards: Vec<ShardId>,
    pub state: CrossShardState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrossShardState {
    Pending,
    Locked,      // Source shard locked funds
    Committed,   // All shards committed
    Aborted,     // Transaction aborted
}

/// Shard state root (for global state)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShardStateRoot {
    pub shard_id: ShardId,
    pub state_root: Hash,
    pub block_height: u64,
}

/// Global state root (Merkle tree of shard roots)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateRoot {
    pub root: Hash,
    pub shard_roots: Vec<ShardStateRoot>,
    pub block_height: u64,
}
```

### 2.2 Shard Assignment Algorithm

```rust
/// Consistent hashing for shard assignment
pub fn assign_shard(address: &Address, shard_count: u16) -> ShardId {
    // Use first 2 bytes of address hash
    let hash = keccak256(address);
    let value = u16::from_be_bytes([hash[0], hash[1]]);
    value % shard_count
}

/// Rendezvous hashing for minimal reassignment
pub fn rendezvous_assign(address: &Address, shards: &[ShardId]) -> ShardId {
    shards.iter()
        .map(|shard| {
            let combined = keccak256(&[address.as_slice(), &shard.to_be_bytes()].concat());
            (*shard, combined)
        })
        .max_by_key(|(_, hash)| *hash)
        .map(|(shard, _)| shard)
        .unwrap()
}
```

### 2.3 Invariants

```rust
/// INVARIANT-1: Deterministic Assignment
/// Same address always maps to same shard.
fn invariant_deterministic_assignment(address: &Address, config: &ShardConfig) -> bool {
    let shard1 = assign_shard(address, config.shard_count);
    let shard2 = assign_shard(address, config.shard_count);
    shard1 == shard2
}

/// INVARIANT-2: Cross-Shard Atomicity
/// Cross-shard transactions are all-or-nothing.
fn invariant_cross_shard_atomic(tx: &CrossShardTransaction) -> bool {
    match tx.state {
        CrossShardState::Committed => true,  // All shards committed
        CrossShardState::Aborted => true,    // All shards rolled back
        _ => false,                          // Intermediate state
    }
}

/// INVARIANT-3: Global State Consistency
/// Global root is Merkle tree of all shard roots.
fn invariant_global_consistency(global: &GlobalStateRoot) -> bool {
    let computed = compute_merkle_root(
        &global.shard_roots.iter().map(|s| s.state_root).collect::<Vec<_>>()
    );
    global.root == computed
}
```

---

## 3. PORTS (HEXAGONAL ARCHITECTURE)

### 3.1 Driving Ports (API - Inbound)

```rust
/// Primary Sharding API
#[async_trait]
pub trait ShardingApi: Send + Sync {
    /// Get shard for an address
    fn get_shard(&self, address: &Address) -> ShardId;
    
    /// Route transaction to appropriate shard(s)
    async fn route_transaction(
        &self,
        transaction: SignedTransaction,
    ) -> Result<RoutingResult, ShardError>;
    
    /// Get global state root
    async fn get_global_state_root(&self) -> Result<GlobalStateRoot, ShardError>;
    
    /// Get shard validators at current epoch
    /// 
    /// Reference: System.md, Subsystem 14 - "Validator Shuffling - Random rotation every epoch"
    /// 
    /// SECURITY: Validator assignments are queried from the BeaconChainProvider,
    /// which is the SOLE source of truth for shard assignments.
    async fn get_shard_validators(
        &self,
        shard_id: ShardId,
    ) -> Result<Vec<ValidatorInfo>, ShardError>;
    
    /// Process cross-shard message
    async fn process_cross_shard_message(
        &self,
        message: CrossShardMessage,
    ) -> Result<(), ShardError>;
}

/// Transaction routing result
#[derive(Clone, Debug)]
pub struct RoutingResult {
    pub transaction_hash: Hash,
    pub is_cross_shard: bool,
    pub source_shard: ShardId,
    pub target_shards: Vec<ShardId>,
}
```

### 3.2 Driven Ports (SPI - Outbound Dependencies)

```rust
/// Shard consensus interface
#[async_trait]
pub trait ShardConsensus: Send + Sync {
    /// Submit transaction to shard
    async fn submit_to_shard(
        &self,
        shard_id: ShardId,
        transaction: SignedTransaction,
    ) -> Result<(), ConsensusError>;
    
    /// Get shard block
    async fn get_shard_block(
        &self,
        shard_id: ShardId,
        height: u64,
    ) -> Result<ShardBlock, ConsensusError>;
}

/// Partitioned state access
#[async_trait]
pub trait PartitionedState: Send + Sync {
    /// Get shard state root
    async fn get_shard_state_root(
        &self,
        shard_id: ShardId,
    ) -> Result<Hash, StateError>;
    
    /// Access state within shard
    async fn get_shard_state(
        &self,
        shard_id: ShardId,
        address: Address,
    ) -> Result<AccountState, StateError>;
}

/// Beacon Chain provider for shard coordination
/// 
/// Reference: System.md, Subsystem 14 - "Beacon Chain Coordination"
/// Reference: IPC-MATRIX.md, Subsystem 14 - "Depends on: Subsystem 8 (Consensus)"
/// 
/// The Beacon Chain (or main consensus chain) is the authoritative source for:
/// 1. Validator-to-shard assignments (updated per epoch)
/// 2. Shard committee composition
/// 3. Cross-shard receipt verification keys
#[async_trait]
pub trait BeaconChainProvider: Send + Sync {
    /// Get validator assignments for all shards at a specific epoch
    /// 
    /// Returns: Map of ShardId -> Vec<ValidatorId>
    /// This data is from the Beacon Chain state at epoch boundary.
    async fn get_shard_assignments(
        &self,
        epoch: u64,
    ) -> Result<HashMap<ShardId, Vec<ValidatorId>>, BeaconError>;
    
    /// Get validators for a specific shard
    async fn get_shard_validators(
        &self,
        shard_id: ShardId,
        epoch: u64,
    ) -> Result<Vec<ValidatorInfo>, BeaconError>;
    
    /// Verify a cross-shard receipt
    /// 
    /// Reference: System.md, Subsystem 14 - "Cross-Links: Beacon validates shard headers"
    /// 
    /// SECURITY: Cross-shard receipts must be signed by validators assigned
    /// to the SOURCE shard at the receipt's epoch.
    async fn verify_cross_shard_receipt(
        &self,
        receipt: &CrossShardReceipt,
    ) -> Result<bool, BeaconError>;
}

/// Cross-shard receipt with proof
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrossShardReceipt {
    pub source_shard: ShardId,
    pub target_shard: ShardId,
    pub tx_hash: Hash,
    pub state_proof: Vec<Vec<u8>>,
    /// Signatures from source shard validators
    pub signatures: Vec<(ValidatorId, Signature)>,
    pub epoch: u64,
}
```

---

## 4. EVENT SCHEMA

### 4.1 Cross-Shard Messages

```rust
/// Cross-shard communication message
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CrossShardMessage {
    /// Lock request (2PC phase 1)
    LockRequest {
        tx_hash: Hash,
        source_shard: ShardId,
        target_shard: ShardId,
        lock_data: LockData,
    },
    /// Lock response
    LockResponse {
        tx_hash: Hash,
        shard_id: ShardId,
        success: bool,
        lock_proof: Option<LockProof>,
    },
    /// Commit request (2PC phase 2)
    CommitRequest {
        tx_hash: Hash,
        source_shard: ShardId,
        lock_proofs: Vec<LockProof>,
    },
    /// Commit acknowledgment
    CommitAck {
        tx_hash: Hash,
        shard_id: ShardId,
        success: bool,
    },
    /// Abort request
    AbortRequest {
        tx_hash: Hash,
        reason: AbortReason,
    },
}

/// Lock data for cross-shard transaction
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LockData {
    pub address: Address,
    pub amount: U256,
    pub nonce: u64,
}

/// Proof that funds were locked
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LockProof {
    pub shard_id: ShardId,
    pub block_hash: Hash,
    pub merkle_proof: Vec<Hash>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AbortReason {
    Timeout,
    LockFailed,
    ValidationFailed,
}
```

---

## 5. TEST-DRIVEN DEVELOPMENT STRATEGY

### 5.1 Unit Tests

```rust
#[cfg(test)]
mod unit_tests {
    use super::*;
    
    // === Shard Assignment Tests ===
    
    #[test]
    fn test_deterministic_shard_assignment() {
        let address = Address::from([0xAB; 20]);
        let config = ShardConfig { shard_count: 64, ..Default::default() };
        
        let shard1 = assign_shard(&address, config.shard_count);
        let shard2 = assign_shard(&address, config.shard_count);
        
        assert_eq!(shard1, shard2);
    }
    
    #[test]
    fn test_uniform_distribution() {
        let config = ShardConfig { shard_count: 16, ..Default::default() };
        let mut counts = vec![0usize; 16];
        
        for i in 0..10_000 {
            let address = Address::from([i as u8; 20]);
            let shard = assign_shard(&address, config.shard_count);
            counts[shard as usize] += 1;
        }
        
        // Check roughly uniform (within 20%)
        let expected = 10_000 / 16;
        for count in counts {
            assert!(count > expected * 80 / 100);
            assert!(count < expected * 120 / 100);
        }
    }
    
    // === Cross-Shard Tests ===
    
    #[test]
    fn test_detect_cross_shard_transaction() {
        let config = ShardConfig { shard_count: 4, ..Default::default() };
        
        // Transaction from shard 0 to shard 1
        let from = create_address_in_shard(0, &config);
        let to = create_address_in_shard(1, &config);
        let tx = create_transfer(from, to, 100);
        
        let is_cross = is_cross_shard(&tx, &config);
        assert!(is_cross);
    }
    
    #[test]
    fn test_same_shard_transaction() {
        let config = ShardConfig { shard_count: 4, ..Default::default() };
        
        // Both addresses in same shard
        let from = create_address_in_shard(2, &config);
        let to = create_address_in_shard(2, &config);
        let tx = create_transfer(from, to, 100);
        
        let is_cross = is_cross_shard(&tx, &config);
        assert!(!is_cross);
    }
    
    // === Global State Tests ===
    
    #[test]
    fn test_global_state_root_computation() {
        let shard_roots: Vec<Hash> = (0..4).map(|i| [i as u8; 32]).collect();
        
        let global = compute_global_state_root(&shard_roots);
        
        // Re-compute should be identical
        let global2 = compute_global_state_root(&shard_roots);
        assert_eq!(global, global2);
    }
}
```

### 5.2 Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_cross_shard_transfer() {
        let service = create_sharding_service(4);
        
        let from = create_funded_address(SHARD_0, 1000);
        let to = create_address_in_shard(SHARD_1);
        let tx = create_transfer(from, to, 100);
        
        // Route transaction
        let result = service.route_transaction(tx.clone()).await.unwrap();
        
        assert!(result.is_cross_shard);
        assert_eq!(result.source_shard, 0);
        assert_eq!(result.target_shards, vec![1]);
        
        // Wait for completion
        let status = wait_for_tx(&service, tx.hash()).await;
        assert_eq!(status, CrossShardState::Committed);
        
        // Verify balances
        let from_balance = get_balance(&service, from).await;
        let to_balance = get_balance(&service, to).await;
        
        assert_eq!(from_balance, U256::from(900));
        assert_eq!(to_balance, U256::from(100));
    }
    
    #[tokio::test]
    async fn test_cross_shard_timeout_abort() {
        let config = ShardConfig {
            cross_shard_timeout_secs: 1,  // Fast timeout for test
            ..Default::default()
        };
        let service = create_sharding_service_with_config(config);
        
        // Make target shard unresponsive
        service.make_shard_unresponsive(1);
        
        let tx = create_cross_shard_transfer(0, 1, 100);
        service.route_transaction(tx.clone()).await.unwrap();
        
        // Wait for timeout
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        let status = get_tx_status(&service, tx.hash()).await;
        assert_eq!(status, CrossShardState::Aborted);
        
        // Source shard should have rolled back
        let from_balance = get_balance(&service, tx.sender()).await;
        assert_eq!(from_balance, U256::from(1000));  // Unchanged
    }
}
```

---

## 6. ERROR HANDLING

```rust
#[derive(Debug, thiserror::Error)]
pub enum ShardError {
    #[error("Unknown shard: {0}")]
    UnknownShard(ShardId),
    
    #[error("Cross-shard lock failed: {0}")]
    LockFailed(String),
    
    #[error("Cross-shard timeout")]
    Timeout,
    
    #[error("Shard consensus error: {0}")]
    ConsensusError(#[from] ConsensusError),
    
    #[error("State error: {0}")]
    StateError(#[from] StateError),
    
    #[error("Invalid cross-shard proof")]
    InvalidProof,
}
```

---

## 7. CONFIGURATION

```toml
[sharding]
shard_count = 64
validators_per_shard = 128
cross_shard_timeout_secs = 30
```

---

## APPENDIX A: DEPENDENCY MATRIX

**Reference:** System.md V2.3 Unified Dependency Graph, IPC-MATRIX.md Subsystem 14

| This Subsystem | Depends On | Dependency Type | Purpose | Reference |
|----------------|------------|-----------------|---------|-----------|
| Sharding (14) | Subsystem 8 (Consensus) | Uses | Per-shard consensus | System.md Subsystem 14 |
| Sharding (14) | Subsystem 4 (State Mgmt) | Uses | Partitioned state | System.md Subsystem 14 |
| Sharding (14) | Beacon Chain (via 8) | Query | Validator-to-shard assignments | System.md §14 |

---

## APPENDIX B: SECURITY BOUNDARIES (IPC-MATRIX.md Compliance)

**Reference:** IPC-MATRIX.md, Subsystem 14 Section

### B.1 Authorized Message Senders

| Message Type | Authorized Sender(s) | Reference |
|--------------|---------------------|-----------|
| `CrossShardTransactionRequest` | Subsystem 8 (Consensus) ONLY | IPC-MATRIX.md Security Boundaries |
| `ShardAssignmentUpdate` | Beacon Chain (trusted) | System.md Subsystem 14 |
| `CrossShardReceipt` | Subsystem 4 (State Mgmt) of destination shard | IPC-MATRIX.md Subsystem 14 |

### B.2 Cross-Shard Security

**Reference:** System.md, Subsystem 14 Security Defenses

```rust
/// Cross-shard transaction verification
/// 
/// Reference: System.md, Subsystem 14 Two-Phase Locking
fn verify_cross_shard_receipt(
    receipt: &CrossShardReceipt,
    source_shard_validators: &[ValidatorId],
) -> Result<(), ShardError> {
    // Step 1: Verify receipt has 67%+ of source shard validators
    let valid_sigs = receipt.signatures.iter()
        .filter(|sig| {
            let recovered = ecrecover(&receipt.message_hash, sig);
            source_shard_validators.contains(&recovered.ok())
        })
        .count();
    
    if valid_sigs * 3 < source_shard_validators.len() * 2 {
        return Err(ShardError::InvalidProof);
    }
    
    // Step 2: Verify receipt matches expected transaction
    if receipt.tx_hash != expected_tx_hash {
        return Err(ShardError::InvalidProof);
    }
    
    Ok(())
}
```

### B.3 Two-Phase Locking for Atomicity

**Reference:** System.md, Subsystem 14 Cross-Shard Protocol

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    CROSS-SHARD TRANSACTION PROTOCOL                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  PHASE 1: LOCK                                                              │
│  ├─ Source shard locks sender's balance                                     │
│  ├─ Destination shard locks recipient slot                                  │
│  └─ Both shards prepare transaction                                         │
│                                                                             │
│  PHASE 2a: COMMIT (if both locks succeed)                                   │
│  ├─ Source shard deducts balance                                            │
│  ├─ Destination shard credits balance                                       │
│  └─ Both shards release locks                                               │
│                                                                             │
│  PHASE 2b: ABORT (if any lock fails or timeout)                             │
│  ├─ Source shard releases lock (no deduction)                               │
│  ├─ Destination shard releases lock (no credit)                             │
│  └─ Transaction marked as failed                                            │
│                                                                             │
│  TIMEOUT: 30 seconds (configurable)                                         │
│  ├─ Prevents indefinite lock holding                                        │
│  └─ Auto-abort on timeout                                                   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## APPENDIX C: CROSS-REFERENCES

### C.1 Master Document References

| Document | Section | Relevance to This Spec |
|----------|---------|------------------------|
| IPC-MATRIX.md | Subsystem 14 | Cross-shard message types |
| System.md | Subsystem 14 | Sharding algorithm, Two-Phase Locking |

### C.2 Related Specifications

| Specification | Relationship | Notes |
|--------------|--------------|-------|
| SPEC-04-STATE-MANAGEMENT.md | Dependency | Partitioned state |
| SPEC-08-CONSENSUS.md | Dependency | Per-shard consensus |

### C.3 Implementation Priority

**Reference:** System.md, Implementation Priority Section

This subsystem is in **Phase 4 (Future - Post V1)** because:
- Complex distributed system
- Requires proven single-chain operation first
- High research risk

---

**END OF SPECIFICATION**

**Next Steps:**
1. Review this specification for completeness
2. Approve for TDD Phase (write tests)
3. Implement domain logic (pass tests)
4. Implement adapters (wire to runtime)
