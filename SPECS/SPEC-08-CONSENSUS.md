# SPECIFICATION: CONSENSUS MECHANISM

**Version:** 2.3  
**Subsystem ID:** 8  
**Bounded Context:** Block Validation & Agreement  
**Crate Name:** `crates/consensus`  
**Author:** Systems Architecture Team  
**Date:** 2024-12-01  
**Architecture Compliance:** Architecture.md v2.3, IPC-MATRIX.md v2.3, System.md v2.3

---

## 1. ABSTRACT

### 1.1 Purpose

The **Consensus** subsystem achieves agreement on valid blocks across all network nodes. It validates blocks cryptographically and publishes `BlockValidated` events to the Event Bus, triggering the V2.3 choreography pattern.

### 1.2 Responsibility Boundaries

**In Scope:**
- Validate block structure and cryptographic proofs
- Verify block signatures (PoS attestations or PBFT votes)
- Publish `BlockValidated` events to Event Bus
- Request transactions from Mempool for block building
- Coordinate with Finality for checkpoint handling

**Out of Scope:**
- Block storage (Subsystem 2 - via choreography)
- Merkle root computation (Subsystem 3)
- State root computation (Subsystem 4)
- Block propagation (Subsystem 5)
- Transaction signature verification (Subsystem 10)

### 1.3 Critical Design Constraint (V2.3 Choreography - NOT Orchestrator)

**Architecture Mandate (Architecture.md v2.3):**

Consensus performs **validation only**. It does NOT orchestrate block storage writes.

```
V2.3 CHOREOGRAPHY PATTERN:
                                                           
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

**Why NOT Orchestrator (v2.2 Design Decision):**
- Single point of failure
- Performance bottleneck  
- Hidden latency sources
- Complex retry logic ("god object")

### 1.4 Trust Model

```
┌─────────────────────────────────────────────────────────────────┐
│                    TRUST BOUNDARY (V2.3)                        │
├─────────────────────────────────────────────────────────────────┤
│  TRUSTED INPUTS:                                                │
│  ├─ Blocks from Subsystem 5 (Block Propagation)                 │
│  ├─ Transactions from Subsystem 6 (Mempool)                     │
│  └─ Signature validation hints from Subsystem 10                │
│                                                                 │
│  ZERO-TRUST (re-verify independently):                          │
│  └─ Block signatures (even if pre-validated)                    │
│                                                                 │
│  SECURITY (Envelope-Only Identity - V2.2 Amendment):            │
│  Identity from AuthenticatedMessage.sender_id only.             │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. DOMAIN MODEL

### 2.1 Core Entities

```rust
/// A validated block ready for the choreography
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidatedBlock {
    pub header: BlockHeader,
    pub transactions: Vec<SignedTransaction>,
    pub validation_proof: ValidationProof,
}

/// Block header
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockHeader {
    pub version: u32,
    pub block_height: u64,
    pub parent_hash: Hash,
    pub timestamp: u64,
    pub proposer: ValidatorId,
    /// TBD - computed by Subsystem 3 in choreography
    pub transactions_root: Option<Hash>,
    /// TBD - computed by Subsystem 4 in choreography
    pub state_root: Option<Hash>,
    pub receipts_root: Hash,
    pub difficulty: U256,
    pub nonce: u64,
}

/// Validation proof (PoS or PBFT)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ValidationProof {
    PoS(PoSProof),
    PBFT(PBFTProof),
}

/// Proof of Stake validation proof
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoSProof {
    /// Aggregated BLS signatures from attesters
    pub aggregate_signature: BlsSignature,
    /// Bitmap of participating validators
    pub participation_bitmap: BitVec,
    /// Epoch number
    pub epoch: u64,
    /// Slot number within epoch
    pub slot: u64,
}

/// PBFT validation proof
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PBFTProof {
    /// Prepare messages (2f+1 required)
    pub prepares: Vec<PrepareMessage>,
    /// Commit messages (2f+1 required)
    pub commits: Vec<CommitMessage>,
    /// View number
    pub view: u64,
}

/// Consensus configuration
#[derive(Clone, Debug)]
pub struct ConsensusConfig {
    /// Consensus algorithm
    pub algorithm: ConsensusAlgorithm,
    /// Block time target (milliseconds)
    pub block_time_ms: u64,
    /// Maximum transactions per block
    pub max_txs_per_block: usize,
    /// Maximum block gas
    pub max_block_gas: u64,
    /// Minimum attestations for PoS (percentage)
    pub min_attestation_percent: u8,
    /// Byzantine fault tolerance (f in 3f+1)
    pub byzantine_threshold: usize,
}

#[derive(Clone, Debug)]
pub enum ConsensusAlgorithm {
    ProofOfStake,
    PBFT,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            algorithm: ConsensusAlgorithm::ProofOfStake,
            block_time_ms: 12_000,  // 12 seconds
            max_txs_per_block: 10_000,
            max_block_gas: 30_000_000,
            min_attestation_percent: 67,  // 2/3
            byzantine_threshold: 1,
        }
    }
}
```

### 2.2 Invariants

```rust
/// INVARIANT-1: Valid Parent
/// Block parent_hash must reference an existing validated block.
fn invariant_valid_parent(block: &BlockHeader, chain: &ChainState) -> bool {
    block.block_height == 0 || chain.has_block(&block.parent_hash)
}

/// INVARIANT-2: Sufficient Attestations (PoS)
/// At least 2/3 of validators must attest for block validity.
fn invariant_sufficient_attestations(proof: &PoSProof, validators: &ValidatorSet) -> bool {
    let participating = proof.participation_bitmap.count_ones();
    let required = (validators.len() * 2) / 3 + 1;
    participating >= required
}

/// INVARIANT-3: Valid Signatures
/// All signatures in proof must be valid (independently verified).
fn invariant_valid_signatures(proof: &ValidationProof) -> bool {
    // Zero-trust: re-verify all signatures
    match proof {
        ValidationProof::PoS(p) => verify_aggregate_bls(&p.aggregate_signature),
        ValidationProof::PBFT(p) => p.prepares.iter().all(|m| verify_signature(m)),
    }
}

/// INVARIANT-4: Sequential Height
/// Block height must be parent height + 1.
fn invariant_sequential_height(block: &BlockHeader, parent: &BlockHeader) -> bool {
    block.block_height == parent.block_height + 1
}

/// INVARIANT-5: Timestamp Ordering
/// Block timestamp must be > parent timestamp.
fn invariant_timestamp_ordering(block: &BlockHeader, parent: &BlockHeader) -> bool {
    block.timestamp > parent.timestamp
}
```

---

## 3. PORTS (HEXAGONAL ARCHITECTURE)

### 3.1 Driving Ports (API - Inbound)

```rust
/// Primary Consensus API
#[async_trait]
pub trait ConsensusApi: Send + Sync {
    /// Validate a block received from the network
    async fn validate_block(
        &self,
        block: Block,
        source_peer: Option<PeerId>,
    ) -> Result<ValidatedBlock, ConsensusError>;
    
    /// Build a new block (for validators)
    async fn build_block(&self) -> Result<Block, ConsensusError>;
    
    /// Get current chain head
    async fn get_chain_head(&self) -> ChainHead;
    
    /// Check if a block is validated
    async fn is_validated(&self, block_hash: Hash) -> bool;
}

/// Chain head information
#[derive(Clone, Debug)]
pub struct ChainHead {
    pub block_hash: Hash,
    pub block_height: u64,
    pub timestamp: u64,
}
```

### 3.2 Driven Ports (SPI - Outbound Dependencies)

```rust
/// Event bus for choreography
#[async_trait]
pub trait EventBus: Send + Sync {
    /// Publish BlockValidated event
    async fn publish_block_validated(
        &self,
        event: BlockValidatedEvent,
    ) -> Result<(), EventError>;
}

/// Mempool interface for block building
#[async_trait]
pub trait MempoolGateway: Send + Sync {
    /// Get transactions for block building
    async fn get_transactions_for_block(
        &self,
        max_count: usize,
        max_gas: u64,
    ) -> Result<Vec<SignedTransaction>, MempoolError>;
    
    /// Propose transactions (triggers two-phase commit)
    async fn propose_transactions(
        &self,
        tx_hashes: Vec<Hash>,
        target_block_height: u64,
    ) -> Result<(), MempoolError>;
}

/// Signature verification (for zero-trust re-verification)
#[async_trait]
pub trait SignatureVerifier: Send + Sync {
    /// Verify a single signature
    fn verify_signature(
        &self,
        message: &[u8],
        signature: &Signature,
        public_key: &PublicKey,
    ) -> bool;
    
    /// Verify aggregate BLS signature
    fn verify_aggregate_bls(
        &self,
        message: &[u8],
        signature: &BlsSignature,
        public_keys: &[BlsPublicKey],
    ) -> bool;
}

/// State provider for validator set queries
/// 
/// Reference: IPC-MATRIX.md, Subsystem 4 - "StateReadRequest from Subsystems 6, 11, 12, 14"
/// Reference: System.md, Subsystem 8 - "Stake-Weighted Randomness"
/// 
/// SECURITY: Consensus queries State Management (4) for the validator set
/// at the EPOCH BOUNDARY state root, not the current state root.
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
    ) -> Result<ValidatorSet, StateError>;
    
    /// Get total active stake at epoch
    async fn get_total_stake_at_epoch(
        &self,
        epoch: u64,
        state_root: Hash,
    ) -> Result<u128, StateError>;
}

/// Validator set with stake information
#[derive(Clone, Debug)]
pub struct ValidatorSet {
    pub epoch: u64,
    pub validators: Vec<ValidatorInfo>,
    pub total_stake: u128,
}

#[derive(Clone, Debug)]
pub struct ValidatorInfo {
    pub id: ValidatorId,
    pub stake: u128,
    pub pubkey: BlsPublicKey,
}
```

---

## 4. EVENT SCHEMA

### 4.1 Events Published (Outgoing)

```rust
/// V2.3: Published to Event Bus after validating a block
/// Triggers choreography: Subsystems 2, 3, 4 all subscribe
/// 
/// SECURITY (Envelope-Only Identity): No requester_id in payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockValidatedEvent {
    /// Block hash (correlation key for assembly)
    pub block_hash: Hash,
    /// Block height
    pub block_height: u64,
    /// The validated block with transactions
    pub block: ValidatedBlock,
    /// Consensus proof (PoS attestations or PBFT votes)
    pub consensus_proof: ValidationProof,
    /// Validation timestamp
    pub validated_at: u64,
}
```

### 4.2 Messages Received (Incoming)

```rust
/// Block received from network for validation
/// SECURITY: Envelope sender_id MUST be 5 (Block Propagation)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidateBlockRequest {
    pub correlation_id: CorrelationId,
    pub block: Block,
    pub source_peer: Option<PeerId>,
}

/// Transaction batch from Mempool
/// SECURITY: Envelope sender_id MUST be 6 (Mempool)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionBatchResponse {
    pub correlation_id: CorrelationId,
    pub transactions: Vec<SignedTransaction>,
    pub total_gas: u64,
}
```

### 4.3 Message Flow

```
BLOCK VALIDATION FLOW (V2.3 Choreography):
┌─────────────────────────────────────────────────────────────────────────────┐
│  [Block Propagation (5)] ──ValidateBlockRequest──→ [Consensus (8)]          │
│                                                          │                   │
│                                                          ↓ validate block    │
│                                                          │                   │
│                                                  ┌───────┴───────┐           │
│                                               [Valid]        [Invalid]       │
│                                                  │               │           │
│                                                  ↓               ↓           │
│                                         BlockValidatedEvent   Reject        │
│                                                  │                           │
│                                                  ↓                           │
│                                            [Event Bus]                       │
│                                                  │                           │
│                    ┌─────────────────────────────┼─────────────────────────┐ │
│                    ↓                             ↓                         ↓ │
│           [Tx Indexing (3)]          [State Management (4)]    [Block Storage (2)]│
└─────────────────────────────────────────────────────────────────────────────┘

BLOCK BUILDING FLOW (Validators):
┌─────────────────────────────────────────────────────────────────────────────┐
│  [Consensus (8)] ──GetTransactionsRequest──→ [Mempool (6)]                  │
│                                                    │                         │
│                                                    ↓                         │
│  [Consensus (8)] ←──TransactionBatchResponse──── [Mempool (6)]              │
│        │                                                                     │
│        ↓ build block, collect attestations                                  │
│        │                                                                     │
│        ↓                                                                    │
│  BlockValidatedEvent ──→ [Event Bus]                                        │
└─────────────────────────────────────────────────────────────────────────────┘
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
    fn test_invariant_valid_parent() {
        let mut chain = MockChainState::new();
        let parent = create_test_block(0);
        chain.add_block(parent.clone());
        
        let child = create_child_block(&parent);
        assert!(invariant_valid_parent(&child.header, &chain));
        
        let orphan = create_test_block(1);  // No parent
        assert!(!invariant_valid_parent(&orphan.header, &chain));
    }
    
    #[tokio::test]
    async fn test_invariant_sufficient_attestations() {
        // Reference: System.md, Subsystem 8 - "2/3 validators must attest"
        
        // Setup: Mock state provider with known validator set
        let state_provider = MockStateProvider::new();
        state_provider.set_validator_set(EPOCH_1, vec![
            ValidatorInfo { id: V1, stake: 100, pubkey: pk1.clone() },
            ValidatorInfo { id: V2, stake: 100, pubkey: pk2.clone() },
            ValidatorInfo { id: V3, stake: 100, pubkey: pk3.clone() },
        ]);
        
        let service = ConsensusService::new(state_provider);
        
        // Case 1: 3/3 attestations (100% > 67%) - MUST succeed
        let proof_all = create_pos_proof(vec![V1, V2, V3]);
        assert!(service.verify_attestation_threshold(proof_all, EPOCH_1).await.is_ok());
        
        // Case 2: 2/3 attestations (67% = 67%) - MUST succeed
        let proof_two = create_pos_proof(vec![V1, V2]);
        assert!(service.verify_attestation_threshold(proof_two, EPOCH_1).await.is_ok());
        
        // Case 3: 1/3 attestations (33% < 67%) - MUST fail
        let proof_one = create_pos_proof(vec![V1]);
        assert!(service.verify_attestation_threshold(proof_one, EPOCH_1).await.is_err());
    }
    
    #[test]
    fn test_invariant_sequential_height() {
        let parent = create_test_block(10);
        
        let valid_child = create_block_at_height(11, &parent.header.hash());
        assert!(invariant_sequential_height(&valid_child.header, &parent.header));
        
        let invalid_child = create_block_at_height(13, &parent.header.hash());  // Skip
        assert!(!invariant_sequential_height(&invalid_child.header, &parent.header));
    }
    
    // === Validation Tests ===
    
    #[test]
    fn test_validate_block_success() {
        let consensus = create_test_consensus();
        let block = create_valid_block();
        
        let result = consensus.validate_block_sync(block);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_validate_block_invalid_signature() {
        let consensus = create_test_consensus();
        let mut block = create_valid_block();
        
        // Corrupt the signature
        match &mut block.proof {
            ValidationProof::PoS(p) => p.aggregate_signature = BlsSignature::invalid(),
            ValidationProof::PBFT(p) => p.prepares[0].signature = Signature::invalid(),
        }
        
        let result = consensus.validate_block_sync(block);
        assert!(matches!(result, Err(ConsensusError::InvalidSignature)));
    }
    
    #[test]
    fn test_validate_block_wrong_parent() {
        let consensus = create_test_consensus();
        let mut block = create_valid_block();
        
        block.header.parent_hash = [0xFF; 32];  // Non-existent parent
        
        let result = consensus.validate_block_sync(block);
        assert!(matches!(result, Err(ConsensusError::UnknownParent)));
    }
    
    // === Zero-Trust Tests ===
    
    #[test]
    fn test_zero_trust_re_verifies_signatures() {
        let consensus = create_test_consensus();
        let block = create_valid_block();
        
        // Even if block claims to be "pre-validated", we re-verify
        let verified = consensus.verify_all_signatures(&block);
        assert!(verified);
    }
}
```

### 5.2 Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_choreography_block_validated_published() {
        // Setup
        let (event_bus, mut rx) = create_mock_event_bus();
        let consensus = ConsensusService::new(event_bus);
        
        // Create and validate block
        let block = create_valid_block();
        let validated = consensus.validate_block(block.clone(), None).await.unwrap();
        
        // Verify BlockValidated was published
        let event = rx.recv().await.unwrap();
        assert_eq!(event.block_hash, block.hash());
        assert_eq!(event.block_height, block.header.block_height);
    }
    
    #[tokio::test]
    async fn test_block_building_from_mempool() {
        // Setup
        let transactions = vec![
            create_test_transaction(100),
            create_test_transaction(200),
        ];
        let (mempool, _) = create_mock_mempool_with_txs(transactions.clone());
        let (event_bus, _) = create_mock_event_bus();
        let consensus = ConsensusService::new(mempool, event_bus);
        
        // Build block
        let block = consensus.build_block().await.unwrap();
        
        // Verify transactions included
        assert_eq!(block.transactions.len(), 2);
    }
    
    #[tokio::test]
    async fn test_reject_block_from_wrong_sender() {
        let consensus = create_test_consensus();
        
        let request = ValidateBlockRequest {
            correlation_id: random_id(),
            block: create_valid_block(),
            source_peer: None,
        };
        
        // From wrong sender (should be Block Propagation)
        let envelope = create_envelope(SubsystemId::Mempool, request);
        let result = consensus.handle_validate_request(envelope).await;
        
        assert!(matches!(result, Err(ConsensusError::UnauthorizedSender(_))));
    }
}
```

---

## 6. ERROR HANDLING

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConsensusError {
    #[error("Unknown parent block: {0:?}")]
    UnknownParent(Hash),
    
    #[error("Invalid block signature")]
    InvalidSignature,
    
    #[error("Insufficient attestations: {got}%, required {required}%")]
    InsufficientAttestations { got: u8, required: u8 },
    
    #[error("Invalid block height: expected {expected}, got {actual}")]
    InvalidHeight { expected: u64, actual: u64 },
    
    #[error("Invalid timestamp: block {block} <= parent {parent}")]
    InvalidTimestamp { block: u64, parent: u64 },
    
    #[error("Block gas exceeds limit: {used} > {limit}")]
    GasLimitExceeded { used: u64, limit: u64 },
    
    #[error("Unauthorized sender: {0:?}")]
    UnauthorizedSender(SubsystemId),
    
    #[error("PBFT view mismatch")]
    ViewMismatch,
    
    #[error("Event bus error: {0}")]
    EventBusError(#[from] EventError),
}
```

---

## 7. CONFIGURATION

```toml
[consensus]
algorithm = "proof_of_stake"  # or "pbft"
block_time_ms = 12000
max_txs_per_block = 10000
max_block_gas = 30000000
min_attestation_percent = 67
byzantine_threshold = 1

# Zero-trust settings
always_reverify_signatures = true
```

---

## APPENDIX A: DEPENDENCY MATRIX

**Reference:** System.md V2.3 Unified Dependency Graph, IPC-MATRIX.md Subsystem 8

| This Subsystem | Depends On | Dependency Type | Purpose | Reference |
|----------------|------------|-----------------|---------|-----------|
| Consensus (8) | Subsystem 5 (Block Propagation) | Accepts from | Blocks to validate | System.md Subsystem 8 |
| Consensus (8) | Subsystem 6 (Mempool) | Query | Transactions for block building | IPC-MATRIX.md Subsystem 8 |
| Consensus (8) | Subsystem 10 (Sig Verify) | Uses | Signature verification (Zero-Trust) | System.md Subsystem 8 |
| Consensus (8) | Event Bus | Publishes | BlockValidated events (Choreography) | Architecture.md Section 5.1 |
| Consensus (8) | Subsystem 9 (Finality) | Provides to | Attestations for finality | System.md Subsystem 8 |
| Consensus (8) | Subsystem 4 (State Mgmt) | Query | ValidatorSet at epoch boundary | IPC-MATRIX.md §4 |

---

## APPENDIX B: SECURITY BOUNDARIES (IPC-MATRIX.md Compliance)

**Reference:** IPC-MATRIX.md, Subsystem 8 Section

### B.1 Authorized Message Senders

| Message Type | Authorized Sender(s) | Reference |
|--------------|---------------------|-----------|
| `ValidateBlockRequest` | Subsystem 5 (Block Propagation) ONLY | IPC-MATRIX.md Security Boundaries |
| `AttestationReceived` | Subsystem 10 (Sig Verify) ONLY | IPC-MATRIX.md Security Boundaries |
| `PBFTMessage` | Subsystem 10 (Sig Verify) ONLY | IPC-MATRIX.md Security Boundaries |

### B.2 Zero-Trust Signature Re-Verification

**Reference:** IPC-MATRIX.md, "Zero-Trust Signature Re-Verification (CRITICAL)"

```rust
/// MANDATORY: Consensus MUST NOT trust pre-validation flags
/// 
/// Reference: IPC-MATRIX.md, Subsystem 8 Security Boundaries
/// 
/// Even if Subsystem 10 says signature_valid=true, we re-verify
/// because if Subsystem 10 is compromised, attackers could inject
/// fake attestations.
fn handle_attestation(attestation: AttestationReceived) -> Result<(), ConsensusError> {
    // Step 1: Verify envelope (standard checks)
    verify_envelope(&attestation)?;
    
    // Step 2: INDEPENDENTLY re-verify signature (ZERO TRUST)
    let message = keccak256(&encode_attestation_message(&attestation));
    let recovered_signer = ecrecover(message, &attestation.signature)?;
    
    if recovered_signer != attestation.validator {
        log::warn!(
            "SECURITY: Attestation signature mismatch. Claimed: {:?}, Recovered: {:?}",
            attestation.validator, recovered_signer
        );
        return Err(ConsensusError::SignatureVerificationFailed);
    }
    
    // Step 3: Verify validator is in active set
    if !self.validator_set.contains(&recovered_signer) {
        return Err(ConsensusError::UnknownValidator(recovered_signer));
    }
    
    // Now safe to process
    self.process_attestation(attestation)?;
    Ok(())
}
```

**Rationale (System.md):**
> "If Subsystem 10 is compromised, an attacker could inject attestations 
> with signature_valid=true for signatures they never verified. By 
> re-verifying, Consensus becomes independently secure."

### B.3 Choreography Pattern (NOT Orchestrator)

**Reference:** Architecture.md Section 5.1.1, System.md Subsystem 8

```
V2.3 CHOREOGRAPHY PATTERN:

[WRONG - V2.0/V2.1 Orchestrator Anti-Pattern]:
  Consensus requests roots via RPC → waits for responses → writes block
  PROBLEMS: Single point of failure, bottleneck, complex retry logic

[CORRECT - V2.3 Choreography Pattern]:
  Consensus validates block → publishes BlockValidated to Event Bus → DONE
  
  Subsystems 3, 4, 2 independently react:
  - Subsystem 3: Computes MerkleRoot, publishes MerkleRootComputed
  - Subsystem 4: Computes StateRoot, publishes StateRootComputed  
  - Subsystem 2: Assembles all 3, writes atomically
```

---

## APPENDIX C: CROSS-REFERENCES

### C.1 Master Document References

| Document | Section | Relevance to This Spec |
|----------|---------|------------------------|
| Architecture.md | Section 3.2.1 | Envelope-Only Identity mandate |
| Architecture.md | Section 5.1 | Choreography pattern (Consensus → Event Bus) |
| IPC-MATRIX.md | Subsystem 8 | Security boundaries, Zero-Trust mandate |
| System.md | Subsystem 8 | PoS/PBFT algorithms, validation-only role |
| System.md | V2.3 Dependency Graph | Publishes BlockValidated; NOT orchestrator |

### C.2 Related Specifications

| Specification | Relationship | Notes |
|--------------|--------------|-------|
| SPEC-02-BLOCK-STORAGE.md | Consumer | Subscribes to our BlockValidated event (Stateful Assembler) |
| SPEC-03-TRANSACTION-INDEXING.md | Consumer | Subscribes to our BlockValidated event |
| SPEC-04-STATE-MANAGEMENT.md | Consumer | Subscribes to our BlockValidated event |
| SPEC-05-BLOCK-PROPAGATION.md | Bidirectional | Receives blocks; sends validated blocks to propagate |
| SPEC-06-MEMPOOL.md | Dependency | Source of transactions for block building |
| SPEC-09-FINALITY.md | Consumer | Receives attestations for finality checking |
| SPEC-10-SIGNATURE-VERIFICATION.md | Dependency | Signature verification (but Zero-Trust re-verify) |

### C.3 Implementation Priority

**Reference:** System.md, Implementation Priority Section

This subsystem is in **Phase 2 (Consensus - Weeks 5-8)** because:
- Depends on Subsystems 5, 6, 10
- Core to block validation - required for choreography
- Must publish BlockValidated before other subsystems can function

---

## APPENDIX D: BLOCK VALIDATION FLOW

**Reference:** Architecture.md Section 5.1

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         BLOCK VALIDATION FLOW                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. RECEIVE: Block arrives from Block Propagation (5)                       │
│     ├── ValidateBlockRequest { block, received_from, timestamp }            │
│     └── Validate envelope.sender_id == SubsystemId::BlockPropagation        │
│                                                                             │
│  2. VALIDATE STRUCTURE:                                                     │
│     ├── Check block size limits                                             │
│     ├── Check parent hash links to known block                              │
│     ├── Check timestamp is within acceptable range                          │
│     └── Check transaction count within limits                               │
│                                                                             │
│  3. VALIDATE SIGNATURES (Zero-Trust):                                       │
│     ├── For PoS: Re-verify all attestation signatures                       │
│     ├── For PBFT: Re-verify prepare/commit signatures                       │
│     └── Count participation; require 67%+ for PoS, 2f+1 for PBFT            │
│                                                                             │
│  4. PUBLISH (Choreography):                                                 │
│     ├── BlockValidatedPayload { block_hash, block, consensus_proof }        │
│     ├── Publish to Event Bus                                                │
│     └── DO NOT wait for responses (non-blocking)                            │
│                                                                             │
│  5. DOWNSTREAM (Independent - NOT orchestrated by Consensus):               │
│     ├── Subsystem 3: Subscribes → computes MerkleRootComputed               │
│     ├── Subsystem 4: Subscribes → computes StateRootComputed                │
│     └── Subsystem 2: Subscribes → assembles and writes atomically           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

**END OF SPECIFICATION**

**Next Steps:**
1. Review this specification for completeness
2. Approve for TDD Phase (write tests)
3. Implement domain logic (pass tests)
4. Implement adapters (wire to runtime)
