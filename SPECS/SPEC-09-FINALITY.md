# SPECIFICATION: FINALITY MECHANISM

**Version:** 2.3  
**Subsystem ID:** 9  
**Bounded Context:** Finality & Economic Guarantees  
**Crate Name:** `crates/finality`  
**Author:** Systems Architecture Team  
**Date:** 2024-12-01  
**Architecture Compliance:** Architecture.md v2.3, IPC-MATRIX.md v2.3, System.md v2.3

---

## 1. ABSTRACT

### 1.1 Purpose

The **Finality** subsystem provides economic finality guarantees using Casper FFG (Friendly Finality Gadget). It ensures that once a block is finalized, it cannot be reverted without burning at least 1/3 of the total stake—providing strong security guarantees for high-value transactions.

### 1.2 Responsibility Boundaries

**In Scope:**
- Track validator attestations for checkpoints
- Determine block justification (2/3+ attestations)
- Finalize blocks (two consecutive justified checkpoints)
- Send `MarkFinalizedRequest` to Block Storage
- Implement circuit breaker for livelock prevention

**Out of Scope:**
- Block validation (Subsystem 8)
- Block storage (Subsystem 2)
- Signature verification (done via Subsystem 10, with zero-trust re-verification)
- Slashing execution (separate enforcement subsystem)

### 1.3 Critical Design Constraint (Deterministic Circuit Breaker)

**Architecture Mandate (Architecture.md v2.3, Section 5.4.1):**

The Finality subsystem uses a **deterministic circuit breaker** to prevent livelock:

```
STATE MACHINE:
                                                                        
  [RUNNING] ──finality achieved──→ [RUNNING]                           
      │                                                                 
      └── finality failed ──→ [SYNC]                                   
                                   │                                    
                                   └── sync success ──→ [RUNNING]      
                                   │                                    
                                   └── sync failed (3x) ──→ [HALTED]   
                                                              │        
                                   ← manual intervention ─────┘        
```

**Why Circuit Breaker:**
- Prevents infinite retry loops
- Provides deterministic, testable behavior
- Enables manual intervention for true network failures
- Distinguishes transient failures from systemic issues

### 1.4 Trust Model

```
┌─────────────────────────────────────────────────────────────────┐
│                    TRUST BOUNDARY (V2.3)                        │
├─────────────────────────────────────────────────────────────────┤
│  TRUSTED INPUTS:                                                │
│  ├─ Validated blocks from Subsystem 8 (Consensus)               │
│  └─ Attestation hints from validator network                    │
│                                                                 │
│  ZERO-TRUST (independently re-verify):                          │
│  └─ All attestation signatures (even if pre-validated)          │
│                                                                 │
│  SECURITY (Envelope-Only Identity - V2.2 Amendment):            │
│  Identity from AuthenticatedMessage.sender_id only.             │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. DOMAIN MODEL

### 2.1 Core Entities

```rust
/// A finality checkpoint
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Epoch number
    pub epoch: u64,
    /// Block hash at epoch boundary
    pub block_hash: Hash,
    /// Block height
    pub block_height: u64,
    /// State of this checkpoint
    pub state: CheckpointState,
}

/// Checkpoint finality state
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckpointState {
    /// Not yet justified
    Pending,
    /// 2/3+ validators attested
    Justified,
    /// Two consecutive justified checkpoints
    Finalized,
}

/// Validator attestation for a checkpoint
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attestation {
    pub validator_id: ValidatorId,
    pub source_checkpoint: CheckpointId,
    pub target_checkpoint: CheckpointId,
    pub signature: BlsSignature,
    pub slot: u64,
}

/// Aggregated attestations for a checkpoint
#[derive(Clone, Debug)]
pub struct AggregatedAttestations {
    pub target_checkpoint: CheckpointId,
    pub attestations: Vec<Attestation>,
    pub participation_bitmap: BitVec,
    pub aggregate_signature: Option<BlsSignature>,
}

/// Circuit breaker state
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinalityState {
    /// Normal operation
    Running,
    /// Attempting to sync due to finality failure
    Sync { attempt: u8 },
    /// Halted after max sync failures
    HaltedAwaitingIntervention,
}

/// Finality configuration
#[derive(Clone, Debug)]
pub struct FinalityConfig {
    /// Blocks per epoch (checkpoint interval)
    pub epoch_length: u64,
    /// Required attestation percentage for justification
    pub justification_threshold_percent: u8,
    /// Maximum sync attempts before halt
    pub max_sync_attempts: u8,
    /// Sync attempt timeout (seconds)
    pub sync_timeout_secs: u64,
    /// Inactivity leak start (epochs without finality)
    pub inactivity_leak_epochs: u64,
}

impl Default for FinalityConfig {
    fn default() -> Self {
        Self {
            epoch_length: 32,  // 32 blocks per epoch
            justification_threshold_percent: 67,  // 2/3
            max_sync_attempts: 3,
            sync_timeout_secs: 60,
            inactivity_leak_epochs: 4,
        }
    }
}
```

### 2.2 Invariants

```rust
/// INVARIANT-1: Finalization Requires Justification
/// A checkpoint can only be finalized if it is justified and the
/// previous checkpoint is also justified (Casper FFG rule).
fn invariant_finalization_requires_justification(
    checkpoint: &Checkpoint,
    previous: &Checkpoint,
) -> bool {
    if checkpoint.state == CheckpointState::Finalized {
        checkpoint.state >= CheckpointState::Justified &&
        previous.state >= CheckpointState::Justified
    } else {
        true
    }
}

/// INVARIANT-2: Justification Threshold
/// A checkpoint is justified only if >= 2/3 of validators attested.
fn invariant_justification_threshold(
    attestations: &AggregatedAttestations,
    validators: &ValidatorSet,
) -> bool {
    let participating = attestations.participation_bitmap.count_ones();
    let required = (validators.len() * 2) / 3 + 1;
    participating >= required
}

/// INVARIANT-3: No Conflicting Finality
/// Two finalized blocks at the same height is impossible without
/// slashing 1/3 of validators.
fn invariant_no_conflicting_finality(
    finalized_blocks: &HashMap<u64, Hash>,
) -> bool {
    // Each height has at most one finalized block
    true  // Guaranteed by data structure
}

/// INVARIANT-4: Circuit Breaker Determinism
/// State transitions are deterministic and testable.
fn invariant_circuit_breaker_determinism(
    current: FinalityState,
    event: FinalityEvent,
) -> FinalityState {
    match (current, event) {
        (FinalityState::Running, FinalityEvent::FinalityFailed) => 
            FinalityState::Sync { attempt: 1 },
        (FinalityState::Sync { attempt }, FinalityEvent::SyncSuccess) =>
            FinalityState::Running,
        (FinalityState::Sync { attempt }, FinalityEvent::SyncFailed) if attempt < 3 =>
            FinalityState::Sync { attempt: attempt + 1 },
        (FinalityState::Sync { attempt: 3 }, FinalityEvent::SyncFailed) =>
            FinalityState::HaltedAwaitingIntervention,
        (FinalityState::HaltedAwaitingIntervention, FinalityEvent::ManualIntervention) =>
            FinalityState::Running,
        (state, _) => state,
    }
}
```

---

## 3. PORTS (HEXAGONAL ARCHITECTURE)

### 3.1 Driving Ports (API - Inbound)

```rust
/// Primary Finality API
#[async_trait]
pub trait FinalityApi: Send + Sync {
    /// Process attestations for a checkpoint
    async fn process_attestations(
        &mut self,
        attestations: Vec<Attestation>,
    ) -> Result<AttestationResult, FinalityError>;
    
    /// Check if a block is finalized
    async fn is_finalized(&self, block_hash: Hash) -> bool;
    
    /// Get last finalized block
    async fn get_last_finalized(&self) -> Option<Checkpoint>;
    
    /// Get current finality state (for circuit breaker)
    async fn get_state(&self) -> FinalityState;
    
    /// Manual intervention to reset from HALTED state
    async fn reset_from_halted(&mut self) -> Result<(), FinalityError>;
    
    /// Get finality lag (blocks since last finalized)
    async fn get_finality_lag(&self) -> u64;
}

/// Result of processing attestations
#[derive(Clone, Debug)]
pub struct AttestationResult {
    pub accepted: usize,
    pub rejected: usize,
    pub new_justified: Option<Checkpoint>,
    pub new_finalized: Option<Checkpoint>,
}
```

### 3.2 Driven Ports (SPI - Outbound Dependencies)

```rust
/// Block Storage interface for marking finalized blocks
#[async_trait]
pub trait BlockStorageGateway: Send + Sync {
    /// Mark a block as finalized
    async fn mark_finalized(
        &self,
        request: MarkFinalizedRequest,
    ) -> Result<(), StorageError>;
}

/// Signature verification for attestations
#[async_trait]
pub trait AttestationVerifier: Send + Sync {
    /// Verify a single attestation signature
    fn verify_attestation(&self, attestation: &Attestation) -> bool;
    
    /// Verify aggregate BLS signature
    fn verify_aggregate(
        &self,
        attestations: &AggregatedAttestations,
        validators: &ValidatorSet,
    ) -> bool;
}

/// Validator set provider with stake information
/// 
/// Reference: IPC-MATRIX.md, Subsystem 4 - State Management is authoritative for stake
/// Reference: System.md, Subsystem 9 - "2/3+ validators attest to checkpoint"
/// 
/// CRITICAL: Finality calculations require ACCURATE stake information.
/// Stale stake data could lead to incorrect finalization.
#[async_trait]
pub trait ValidatorSetProvider: Send + Sync {
    /// Get validator set at a specific epoch
    /// 
    /// Reference: Architecture.md §5.4.1 - Deterministic state queries
    async fn get_validator_set_at_epoch(
        &self,
        epoch: u64,
    ) -> Result<ValidatorSet, StateError>;
    
    /// Get individual validator stake
    /// 
    /// Used during zero-trust signature verification to weight attestations.
    async fn get_validator_stake(
        &self,
        validator_id: &ValidatorId,
        epoch: u64,
    ) -> Result<u128, StateError>;
    
    /// Get total active stake at epoch
    /// 
    /// Used to calculate 2/3 threshold for justification.
    async fn get_total_active_stake(
        &self,
        epoch: u64,
    ) -> Result<u128, StateError>;
}
```

---

## 4. EVENT SCHEMA

### 4.1 Outgoing Messages

```rust
/// Request to mark a block as finalized
/// Sent to Block Storage (Subsystem 2)
/// SECURITY (Envelope-Only Identity): No requester_id in payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarkFinalizedRequest {
    pub correlation_id: CorrelationId,
    /// The finalized block hash
    pub block_hash: Hash,
    /// The finalized block height
    pub block_height: u64,
    /// Epoch in which finalization occurred
    pub finalized_epoch: u64,
    /// Proof of finalization (attestations)
    pub finality_proof: FinalityProof,
}

/// Proof of block finalization
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinalityProof {
    /// Source checkpoint (previous justified)
    pub source_checkpoint: Checkpoint,
    /// Target checkpoint (newly finalized)
    pub target_checkpoint: Checkpoint,
    /// Aggregated attestations
    pub aggregate_signature: BlsSignature,
    /// Participation bitmap
    pub participation_bitmap: BitVec,
}
```

### 4.2 Incoming Messages

```rust
/// Attestations from Consensus for finality processing
/// SECURITY: Envelope sender_id MUST be 8 (Consensus)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttestationBatch {
    pub attestations: Vec<Attestation>,
    pub epoch: u64,
    pub slot: u64,
}
```

---

## 5. TEST-DRIVEN DEVELOPMENT STRATEGY

### 5.1 Unit Tests

```rust
#[cfg(test)]
mod unit_tests {
    use super::*;
    
    // === Circuit Breaker Tests ===
    
    #[test]
    fn test_circuit_breaker_running_to_sync() {
        let mut service = FinalityService::new(Default::default());
        assert_eq!(service.get_state_sync(), FinalityState::Running);
        
        service.on_finality_failed();
        assert_eq!(service.get_state_sync(), FinalityState::Sync { attempt: 1 });
    }
    
    #[test]
    fn test_circuit_breaker_sync_to_running() {
        let mut service = create_service_in_sync_state(1);
        
        service.on_sync_success();
        assert_eq!(service.get_state_sync(), FinalityState::Running);
    }
    
    #[test]
    fn test_circuit_breaker_max_attempts_to_halted() {
        let mut service = create_service_in_sync_state(3);
        
        service.on_sync_failed();
        assert_eq!(service.get_state_sync(), FinalityState::HaltedAwaitingIntervention);
    }
    
    #[test]
    fn test_circuit_breaker_halted_blocks_finalization() {
        let mut service = FinalityService::new(Default::default());
        service.force_state(FinalityState::HaltedAwaitingIntervention);
        
        let attestations = create_valid_attestations(100);
        let result = service.process_attestations_sync(attestations);
        
        assert!(matches!(result, Err(FinalityError::SystemHalted)));
    }
    
    #[test]
    fn test_circuit_breaker_manual_reset() {
        let mut service = FinalityService::new(Default::default());
        service.force_state(FinalityState::HaltedAwaitingIntervention);
        
        service.reset_from_halted_sync().unwrap();
        assert_eq!(service.get_state_sync(), FinalityState::Running);
    }
    
    // === Justification Tests ===
    
    #[test]
    fn test_justification_at_threshold() {
        let validators = create_validator_set(100);
        let checkpoint = create_pending_checkpoint(1);
        
        // Exactly 67 attestations (67% - meets threshold)
        let attestations = create_attestations_for_checkpoint(&checkpoint, 67, &validators);
        let result = check_justification(&attestations, &validators);
        
        assert!(result.is_justified);
    }
    
    #[test]
    fn test_justification_below_threshold() {
        let validators = create_validator_set(100);
        let checkpoint = create_pending_checkpoint(1);
        
        // Only 66 attestations (66% - below threshold)
        let attestations = create_attestations_for_checkpoint(&checkpoint, 66, &validators);
        let result = check_justification(&attestations, &validators);
        
        assert!(!result.is_justified);
    }
    
    // === Finalization Tests ===
    
    #[test]
    fn test_finalization_two_consecutive_justified() {
        let mut service = FinalityService::new(Default::default());
        
        let cp1 = create_and_justify_checkpoint(&mut service, 1);
        let cp2 = create_and_justify_checkpoint(&mut service, 2);
        
        // cp1 should now be finalized (two consecutive justified)
        assert!(service.is_finalized_sync(cp1.block_hash));
    }
    
    #[test]
    fn test_finalization_gap_blocks_finality() {
        let mut service = FinalityService::new(Default::default());
        
        let cp1 = create_and_justify_checkpoint(&mut service, 1);
        // Skip cp2
        let cp3 = create_and_justify_checkpoint(&mut service, 3);
        
        // cp1 should NOT be finalized (no consecutive justified)
        assert!(!service.is_finalized_sync(cp1.block_hash));
    }
    
    // === Zero-Trust Signature Tests ===
    
    #[test]
    fn test_zero_trust_reverifies_attestations() {
        let mut service = FinalityService::new(Default::default());
        
        // Create attestation with invalid signature
        let mut attestation = create_valid_attestation();
        attestation.signature = BlsSignature::invalid();
        
        let result = service.process_attestations_sync(vec![attestation]);
        
        // Should reject due to invalid signature
        assert_eq!(result.unwrap().rejected, 1);
    }
}
```

### 5.2 Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_full_finalization_flow() {
        // Setup
        let (storage, storage_rx) = create_mock_block_storage();
        let validators = create_validator_set(100);
        let mut service = FinalityService::new(storage, validators);
        
        // Create two epochs of attestations
        let attestations_epoch_1 = create_attestations_for_epoch(1, 80);  // 80%
        let attestations_epoch_2 = create_attestations_for_epoch(2, 80);
        
        // Process epoch 1
        let result1 = service.process_attestations(attestations_epoch_1).await.unwrap();
        assert!(result1.new_justified.is_some());
        assert!(result1.new_finalized.is_none());  // Need 2 consecutive
        
        // Process epoch 2
        let result2 = service.process_attestations(attestations_epoch_2).await.unwrap();
        assert!(result2.new_justified.is_some());
        assert!(result2.new_finalized.is_some());  // Now finalized!
        
        // Verify MarkFinalizedRequest was sent
        let request = storage_rx.recv().await.unwrap();
        assert!(request.finality_proof.participation_bitmap.count_ones() >= 67);
    }
    
    #[tokio::test]
    async fn test_halted_state_stops_finalization_events() {
        let (storage, storage_rx) = create_mock_block_storage();
        let mut service = FinalityService::new(storage, create_validator_set(100));
        
        // Force into HALTED state
        for _ in 0..4 {
            service.on_sync_failed();
        }
        assert_eq!(service.get_state().await, FinalityState::HaltedAwaitingIntervention);
        
        // Attempt finalization
        let attestations = create_attestations_for_epoch(1, 80);
        let result = service.process_attestations(attestations).await;
        
        assert!(matches!(result, Err(FinalityError::SystemHalted)));
        
        // No MarkFinalizedRequest should be sent
        assert!(storage_rx.try_recv().is_err());
    }
    
    #[tokio::test]
    async fn test_mark_finalized_sent_to_correct_subsystem() {
        let (storage, storage_rx) = create_mock_block_storage();
        let mut service = FinalityService::new(storage, create_validator_set(100));
        
        // Trigger finalization
        finalize_two_epochs(&mut service).await;
        
        // Verify request
        let request = storage_rx.recv().await.unwrap();
        assert!(request.block_height > 0);
        assert!(!request.finality_proof.aggregate_signature.is_empty());
    }
}
```

---

## 6. ERROR HANDLING

```rust
#[derive(Debug, thiserror::Error)]
pub enum FinalityError {
    #[error("System halted awaiting intervention")]
    SystemHalted,
    
    #[error("Invalid attestation signature from validator {0:?}")]
    InvalidSignature(ValidatorId),
    
    #[error("Unknown validator: {0:?}")]
    UnknownValidator(ValidatorId),
    
    #[error("Attestation for unknown checkpoint: {0:?}")]
    UnknownCheckpoint(CheckpointId),
    
    #[error("Conflicting attestation detected (slashable)")]
    ConflictingAttestation,
    
    #[error("Checkpoint not found: epoch {epoch}")]
    CheckpointNotFound { epoch: u64 },
    
    #[error("Unauthorized sender: {0:?}")]
    UnauthorizedSender(SubsystemId),
    
    #[error("Storage error: {0}")]
    StorageError(#[from] StorageError),
}
```

---

## 7. ARCHITECTURAL CONTEXT

### 7.1 Impact on Other Subsystems

**Reference:** SPEC-02-BLOCK-STORAGE.md, SPEC-04-STATE-MANAGEMENT.md

When this subsystem enters `HALTED_AWAITING_INTERVENTION`:
- `MarkFinalizedRequest` events STOP being emitted
- Block Storage continues to store blocks but they remain "unfinalized"
- State Management is NOT affected (continues computing state roots)
- Operators should monitor finality lag metrics

**Debugging Guide:**
1. Check `finality.state` metric
2. If HALTED, check sync failure logs
3. Investigate network connectivity or validator availability
4. Use `reset_from_halted()` API after root cause resolved

---

## 8. CONFIGURATION

```toml
[finality]
epoch_length = 32
justification_threshold_percent = 67
max_sync_attempts = 3
sync_timeout_secs = 60
inactivity_leak_epochs = 4

# Zero-trust settings  
always_reverify_signatures = true
```

---

## APPENDIX A: DEPENDENCY MATRIX

**Reference:** System.md V2.3 Unified Dependency Graph, IPC-MATRIX.md Subsystem 9

| This Subsystem | Depends On | Dependency Type | Purpose | Reference |
|----------------|------------|-----------------|---------|-----------|
| Finality (9) | Subsystem 8 (Consensus) | Accepts from | Attestations | System.md Subsystem 9 |
| Finality (9) | Subsystem 10 (Sig Verify) | Uses | Signature verification (Zero-Trust) | IPC-MATRIX.md Subsystem 9 |
| Finality (9) | Subsystem 2 (Block Storage) | Sends to | MarkFinalizedRequest | IPC-MATRIX.md Subsystem 9 |

---

## APPENDIX B: SECURITY BOUNDARIES (IPC-MATRIX.md Compliance)

**Reference:** IPC-MATRIX.md, Subsystem 9 Section

### B.1 Authorized Message Senders

| Message Type | Authorized Sender(s) | Reference |
|--------------|---------------------|-----------|
| `FinalityCheckRequest` | Subsystem 8 (Consensus) ONLY | IPC-MATRIX.md Security Boundaries |
| `FinalityProofRequest` | Subsystem 15 (Cross-Chain) ONLY | IPC-MATRIX.md Security Boundaries |
| `Attestation` | Subsystem 8 (via Subsystem 10 verification) | IPC-MATRIX.md Security Boundaries |

### B.2 Zero-Trust Signature Re-Verification

**Reference:** IPC-MATRIX.md, Subsystem 9 "Zero-Trust Signature Re-Verification"

```rust
/// Verify attestations for finality determination
/// 
/// Reference: IPC-MATRIX.md, Subsystem 9 - "Zero-Trust Signature Re-Verification"
/// Reference: Architecture.md §5.4.1 - Deterministic Trigger Conditions
/// 
/// SECURITY: Every signature is re-verified independently.
/// Stake is queried from State Management (4) for the correct epoch.
async fn verify_attestations_for_finality(
    &self,
    attestations: &[Attestation],
    checkpoint: &Checkpoint,
) -> Result<FinalityResult, FinalityError> {
    let epoch = checkpoint.epoch;
    
    // Get total stake for threshold calculation
    // Reference: System.md, Subsystem 9 - "2/3+ validators"
    let total_stake = self.validator_provider
        .get_total_active_stake(epoch)
        .await?;
    
    let mut valid_stake = 0u128;
    
    for attestation in attestations {
        // ZERO TRUST: Re-verify every signature independently
        // Reference: IPC-MATRIX.md, Subsystem 9 Security Mandate
        let message = keccak256(&encode_attestation(attestation));
        let recovered = bls_verify(&message, &attestation.signature)?;
        
        if recovered != attestation.validator_id {
            log::warn!(
                "SECURITY: Invalid attestation signature from {:?}",
                attestation.validator_id
            );
            continue;  // Skip invalid, don't fail entire batch
        }
        
        // Get stake from State Management for CORRECT EPOCH
        // Reference: IPC-MATRIX.md, Subsystem 4 - authoritative stake data
        let stake = self.validator_provider
            .get_validator_stake(&recovered, epoch)
            .await?;
        
        valid_stake += stake;
    }
    
    // Check supermajority (2/3 = 67%)
    // Reference: System.md, Subsystem 9 - "2/3+ validators attest"
    let required_stake = (total_stake * 2) / 3 + 1;
    
    if valid_stake >= required_stake {
        Ok(FinalityResult::Justified { 
            checkpoint: checkpoint.clone(),
            participating_stake: valid_stake,
            total_stake,
        })
    } else {
        Err(FinalityError::InsufficientAttestations {
            have: valid_stake,
            need: required_stake,
        })
    }
}
```

### B.3 Circuit Breaker with Livelock Prevention

**Reference:** Architecture.md Section 5.4.1, System.md Subsystem 9

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    FINALITY CIRCUIT BREAKER STATE MACHINE                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  [PRODUCING] ──finality failure──→ [SYNCING]                                │
│       ↑                                │                                    │
│       │                                ├── find better chain → [PRODUCING]  │
│       │                                │   (reset counter)                  │
│       │                                │                                    │
│       │                                └── no better chain                  │
│       │                                    (increment counter)              │
│       │                                         │                           │
│       │                                         ↓                           │
│       │                              counter < 3? ──yes──→ [SYNCING]        │
│       │                                         │          (retry)          │
│       │                                         no                          │
│       │                                         │                           │
│       │                                         ↓                           │
│       │                              [HALTED_AWAITING_INTERVENTION]         │
│       │                                         │                           │
│       └────────manual intervention──────────────┘                           │
│            OR network recovery with                                         │
│            significantly higher checkpoint                                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Why Livelock Prevention (System.md):**
> "Standard retry logic (Auto-Retry after 1 hour) is inappropriate for 
> mathematical impossibilities. If >33% of validators reject finalization, 
> consensus is mathematically impossible. Retrying wastes CPU and fills logs."

---

## APPENDIX C: CROSS-REFERENCES

### C.1 Master Document References

| Document | Section | Relevance to This Spec |
|----------|---------|------------------------|
| Architecture.md | Section 5.4 | DLQ strategy (Finality = special case) |
| Architecture.md | Section 5.4.1 | Circuit Breaker deterministic triggers |
| IPC-MATRIX.md | Subsystem 9 | Security boundaries, Zero-Trust mandate |
| System.md | Subsystem 9 | Casper FFG algorithm, livelock prevention |

### C.2 Related Specifications

| Specification | Relationship | Notes |
|--------------|--------------|-------|
| SPEC-02-BLOCK-STORAGE.md | Consumer | Receives MarkFinalizedRequest to mark blocks final |
| SPEC-08-CONSENSUS.md | Producer | Provides attestations for finality checking |
| SPEC-10-SIGNATURE-VERIFICATION.md | Dependency | Signature verification (but Zero-Trust re-verify) |
| SPEC-15-CROSS-CHAIN.md | Consumer | Requests FinalityProof for cross-chain operations |

### C.3 Implementation Priority

**Reference:** System.md, Implementation Priority Section

This subsystem is in **Phase 2 (Consensus - Weeks 5-8)** because:
- Depends on Subsystem 8 (Consensus) for attestations
- Critical for economic finality guarantees
- Circuit breaker must be implemented correctly to prevent livelock

---

**END OF SPECIFICATION**

**Next Steps:**
1. Review this specification for completeness
2. Approve for TDD Phase (write tests)
3. Implement domain logic (pass tests)
4. Implement adapters (wire to runtime)
