# TODO: Sharding Subsystem (qc-14)

**Generated:** 2025-12-10  
**Spec Reference:** [SPEC-14-SHARDING.md](file:///home/chef/Github/Quantum-Chain/SPECS/SPEC-14-SHARDING.md)

---

## 📚 MASTER DOCUMENT REFERENCES

| Document | Path | Relevance |
|----------|------|-----------|
| **System.md** | [Documentation/System.md](file:///home/chef/Github/Quantum-Chain/Documentation/System.md) | Subsystem 14 definition (lines 673-726): Consistent hashing, 2PC, validator rotation |
| **Architecture.md** | [Documentation/Architecture.md](file:///home/chef/Github/Quantum-Chain/Documentation/Architecture.md) | Section 3.2.1: Envelope-Only Identity; Section 2.2: Hexagonal Architecture |
| **IPC-MATRIX.md** | [Documentation/IPC-MATRIX.md](file:///home/chef/Github/Quantum-Chain/Documentation/IPC-MATRIX.md) | Subsystem 14: Cross-shard message types, security boundaries |

### Key References by Topic

| Topic | Document | Section/Line |
|-------|----------|--------------|
| Consistent Hashing (Rendezvous) | System.md | Lines 676-677 |
| Shard Assignment Algorithm | System.md | Line 680 |
| Cross-Shard 2PC Protocol | System.md | Line 681 |
| Beacon Chain Coordination | System.md | Line 682 |
| Validator Rotation | System.md | Line 683 |
| Dependencies (8, 4) | System.md | Lines 685-687 |
| Attack Vectors | System.md | Lines 689-693 |
| Security Defenses | System.md | Lines 695-700 |
| Robustness Measures | System.md | Lines 702-705 |
| V2 Async Receipt Protocol | System.md | Lines 707-725 |
| Domain Model | SPEC-14 | Section 2.1 (Lines 52-118) |
| Shard Assignment Algo | SPEC-14 | Section 2.2 (Lines 120-142) |
| Invariants | SPEC-14 | Section 2.3 (Lines 144-173) |
| Ports Definition | SPEC-14 | Section 3 (Lines 177-313) |
| Cross-Shard Messages | SPEC-14 | Section 4 (Lines 317-380) |
| TDD Strategy | SPEC-14 | Section 5 (Lines 384-527) |
| Error Handling | SPEC-14 | Section 6 (Lines 531-554) |
| 2PC Protocol Diagram | SPEC-14 | Appendix B.3 (Lines 626-655) |

---

## 🎯 OBJECTIVE

> **System.md Lines 673-677:**
> "SUBSYSTEM 14: SHARDING (ADVANCED)
> Purpose: Split blockchain state across multiple shards for horizontal scaling
> Main Algorithm: Consistent Hashing (Rendezvous Hashing)
> Why: Minimal data movement on shard addition/removal, load balancing"

Implement horizontal scaling via state sharding with deterministic address-to-shard assignment, cross-shard atomic transactions using Two-Phase Commit (2PC), and beacon chain coordination for validator management.

---

## 🏛️ TEAMS AUDIT

### 1️⃣ MASTER ARCHITECT
> *"Structure the crate for long-term maintainability following DDD + Hexagonal + TDD"*
> 
> **Reference:** Architecture.md Section 2 (Lines 56-284)

#### How to Implement:

```
crates/qc-14-sharding/
├── Cargo.toml                    # Dependencies: thiserror, async-trait, serde, tokio, shared-types
├── src/
│   ├── lib.rs                    # Re-exports, feature flags
│   ├── domain/
│   │   ├── mod.rs
│   │   ├── entities.rs           # SPEC-14 Section 2.1: ShardConfig, CrossShardTransaction, ShardStateRoot
│   │   ├── value_objects.rs      # SPEC-14 Section 2.1: ShardId, CrossShardState, ShardAssignment
│   │   ├── invariants.rs         # SPEC-14 Section 2.3: Deterministic assignment, atomicity, global consistency
│   │   └── errors.rs             # SPEC-14 Section 6: ShardError enum
│   ├── ports/
│   │   ├── mod.rs
│   │   ├── inbound.rs            # SPEC-14 Section 3.1: ShardingApi trait
│   │   └── outbound.rs           # SPEC-14 Section 3.2: ShardConsensus, PartitionedState, BeaconChainProvider
│   ├── application/
│   │   ├── mod.rs
│   │   └── service.rs            # ShardingService
│   ├── algorithms/
│   │   ├── mod.rs
│   │   ├── shard_assignment.rs   # System.md Lines 680: Consistent hashing, rendezvous hashing
│   │   ├── two_phase_commit.rs   # System.md Line 681: 2PC coordinator
│   │   ├── global_state.rs       # SPEC-14 Lines 166-172: Global state root computation
│   │   └── validator_shuffle.rs  # System.md Line 683: Validator rotation
│   ├── protocol/
│   │   ├── mod.rs
│   │   ├── messages.rs           # SPEC-14 Section 4: CrossShardMessage, LockData, LockProof
│   │   └── coordinator.rs        # 2PC coordinator state machine
│   └── config.rs                 # SPEC-14 Section 7: ShardConfig
└── tests/
    ├── unit/
    │   ├── assignment_tests.rs
    │   ├── two_phase_tests.rs
    │   └── global_state_tests.rs
    └── integration/
        └── cross_shard_tests.rs
```

#### Expected Output:
- Clear separation: Domain (pure), Ports (traits), Application (orchestration), Algorithms (pure functions)
- Protocol layer for cross-shard message handling
- Dependency inversion via traits for ShardConsensus, PartitionedState, BeaconChainProvider

#### Debugging When Errors:
| Error | Likely Cause | Fix |
|-------|--------------|-----|
| Circular dependency in `use` | Wrong module structure | Check `mod.rs` exports match file structure |
| "trait not implemented" | Missing adapter | Ensure ports have mock implementations for tests |
| "cannot find type" | Missing re-export | Add `pub use` in parent `mod.rs` |

---

### 2️⃣ CYBERSECURITY ZERO-DAY EXPERT
> *"Sharding reduces security budget per shard - 1% attack, not 51%"*
>
> **Reference:** System.md Lines 689-700 (Security & Robustness)

#### Security Analysis:

| Attack Vector | Risk Level | Mitigation | Reference |
|---------------|------------|------------|-----------|
| **Shard Takeover (1% Attack)** | 🔴 CRITICAL | Validator shuffling + minimum shard size | System.md Lines 696, 700 |
| **Cross-Shard Fraud** | 🔴 HIGH | Cross-links + fraud proofs | System.md Lines 697, 699 |
| **Data Availability Attack** | 🔴 HIGH | Data availability sampling | System.md Line 698 |
| **Lock Holding (DoS)** | 🟡 MEDIUM | 30s timeout with auto-abort | SPEC-14 Lines 650-652 |
| **Sender Spoofing** | 🔴 HIGH | Envelope-Only Identity | Architecture.md 3.2.1 |
| **Invalid Receipt Replay** | 🔴 HIGH | Verify epoch + validator signatures | SPEC-14 Lines 596-623 |

#### How to Implement Security:

```rust
/// Validator shuffling verification
/// Reference: System.md Line 696 - "Validator Shuffling - Random rotation every epoch"
fn verify_validator_assignment(
    validator: &ValidatorId,
    shard_id: ShardId,
    epoch: u64,
    beacon_provider: &impl BeaconChainProvider,
) -> Result<bool, ShardError> {
    // CRITICAL: Only trust beacon chain for validator assignments
    let assigned_validators = beacon_provider.get_shard_validators(shard_id, epoch).await?;
    
    Ok(assigned_validators.iter().any(|v| v.id == *validator))
}

/// Cross-shard receipt verification
/// Reference: System.md Line 697 - "Cross-Links: Beacon validates shard headers"
/// Reference: SPEC-14 Lines 596-623
fn verify_cross_shard_receipt(
    receipt: &CrossShardReceipt,
    beacon_provider: &impl BeaconChainProvider,
) -> Result<(), ShardError> {
    // 1. Get source shard validators at receipt's epoch
    let validators = beacon_provider
        .get_shard_validators(receipt.source_shard, receipt.epoch)
        .await?;
    
    // 2. Verify 67%+ of validators signed the receipt
    let valid_sigs = receipt.signatures.iter()
        .filter(|(vid, sig)| {
            validators.iter().any(|v| v.id == *vid) && 
            verify_signature(&receipt.hash(), vid, sig)
        })
        .count();
    
    if valid_sigs * 3 < validators.len() * 2 {
        return Err(ShardError::InvalidProof);
    }
    
    Ok(())
}

/// Two-Phase Commit timeout enforcement
/// Reference: SPEC-14 Lines 650-652
fn enforce_lock_timeout(
    lock_time: Instant,
    config: &ShardConfig,
) -> Result<(), ShardError> {
    if lock_time.elapsed() > Duration::from_secs(config.cross_shard_timeout_secs) {
        return Err(ShardError::Timeout);
    }
    Ok(())
}
```

#### Expected Output:
- Validator assignments verified against beacon chain
- Cross-shard receipts require 67%+ validator signatures
- Locks auto-abort after 30s timeout
- No single shard can be controlled without controlling 67%+ of its validators

#### Debugging When Errors:
| Error | Likely Cause | Fix |
|-------|--------------|-----|
| `InvalidProof` on valid receipt | Wrong epoch for validator lookup | Use receipt.epoch, not current epoch |
| `Timeout` too fast in tests | Clock skew or sync issues | Use mock time in tests |
| Shard assignment changes | Epoch boundary crossed | Re-query beacon chain for current assignments |

---

### 3️⃣ SCALABILITY & MAINTAINABILITY EXPERT
> *"Sharding is THE horizontal scaling solution - design for 64+ shards"*
>
> **Reference:** System.md Lines 707-725 (V2 Architecture)

#### Scalability Patterns:

**1. Consistent Hashing for Minimal Reassignment**
> Reference: System.md Lines 676-677 "Minimal data movement on shard addition/removal"
```rust
/// Rendezvous hashing - only 1/n addresses move when adding shard n
/// Reference: SPEC-14 Lines 131-141
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

**2. Parallel Shard Processing**
```rust
/// Each shard processes independently - true parallelism
async fn process_shards_parallel(
    shards: &[ShardId],
    transactions: HashMap<ShardId, Vec<Transaction>>,
) -> Vec<Result<ShardBlock, ShardError>> {
    join_all(
        shards.iter().map(|shard_id| {
            let txs = transactions.get(shard_id).cloned().unwrap_or_default();
            self.process_shard(*shard_id, txs)
        })
    ).await
}
```

**3. V2 Async Receipt Pattern (Future)**
> Reference: System.md Lines 714-721
```rust
/// V2 Pattern: Non-blocking cross-shard (future implementation)
// Source shard: Commit + emit receipt (non-blocking)
// Receipt relayed asynchronously
// Destination processes in next block
// Fraud proof window before finality
```

#### Expected Output:
- Linear scaling with shard count
- Cross-shard latency bounded by 2PC timeout (30s)
- Adding shard moves only 1/n addresses

#### Debugging When Errors:
| Error | Likely Cause | Fix |
|-------|--------------|-----|
| Uneven shard distribution | Poor hash function | Use keccak256, verify distribution in tests |
| Cross-shard bottleneck | Too many cross-shard txs | Consider shard locality hints |
| Memory growth | Unbounded pending cross-shard | Add pending queue limits |

---

### 4️⃣ QA EXPERT
> *"Cross-shard atomicity is critical - test all failure modes"*
>
> **Reference:** SPEC-14 Section 5 (Lines 384-527)

#### Test Strategy:

| Test Type | Location | Coverage Target | Reference |
|-----------|----------|-----------------|-----------|
| Unit Tests | `src/algorithms/*.rs` | 100% of assignment + 2PC logic | SPEC-14 Lines 388-465 |
| Unit Tests | `src/domain/*.rs` | 100% of invariants | SPEC-14 Lines 144-173 |
| Integration | `tests/integration/` | Cross-shard happy + failure paths | SPEC-14 Lines 468-527 |
| Property Tests | `tests/property/` | Uniform distribution, atomicity | SPEC-14 Lines 406-423 |

#### Critical Test Cases (from SPEC-14):

```rust
// === SHARD ASSIGNMENT TESTS (SPEC-14 Lines 393-423) ===

#[test]
fn test_deterministic_shard_assignment() { }         // Line 395

#[test]
fn test_uniform_distribution() { }                   // Line 406

// === CROSS-SHARD TESTS (SPEC-14 Lines 425-451) ===

#[test]
fn test_detect_cross_shard_transaction() { }         // Line 427

#[test]
fn test_same_shard_transaction() { }                 // Line 440

// === GLOBAL STATE TESTS (SPEC-14 Lines 453-465) ===

#[test]
fn test_global_state_root_computation() { }          // Line 455

// === INTEGRATION TESTS (SPEC-14 Lines 468-527) ===

#[tokio::test]
async fn test_cross_shard_transfer() { }             // Line 475

#[tokio::test]
async fn test_cross_shard_timeout_abort() { }        // Line 502

// === FAILURE MODE TESTS (CRITICAL) ===

#[tokio::test]
async fn test_lock_failure_aborts_transaction() { }

#[tokio::test]
async fn test_partial_commit_rollback() { }

#[tokio::test]
async fn test_network_partition_recovery() { }
```

#### Expected Output:
- `cargo test` passes with 0 failures
- `cargo llvm-cov` shows >90% line coverage
- All 2PC failure modes tested
- Property tests verify uniform distribution

---

### 5️⃣ TESTER (HANDS-ON)
> *"TDD: Red → Green → Refactor"*
>
> **Reference:** Architecture.md Section 2.4 (Lines 227-281)

#### Step-by-Step TDD Flow:

```bash
# Phase 1: RED - Write failing tests first
cargo test --lib -- --nocapture 2>&1 | head -50

# Phase 2: GREEN - Implement minimum code
# Edit src/algorithms/shard_assignment.rs

# Phase 3: REFACTOR - Clean up
cargo fmt && cargo clippy -- -D warnings

# Verify
cargo test
```

#### Expected Output Per Phase:

**Phase 1 (RED):**
```
---- algorithms::shard_assignment::test_deterministic_shard_assignment stdout ----
thread '...' panicked at 'not yet implemented'
```

**Phase 2 (GREEN):**
```
running 1 test
test algorithms::shard_assignment::test_deterministic_shard_assignment ... ok
```

---

### 6️⃣ ALGORITHMIC PRECISION EXPERT
> *"Consistent hashing and 2PC must be mathematically correct"*
>
> **Reference:** System.md Lines 676-683, SPEC-14 Lines 120-142

#### Shard Assignment Algorithm:

```rust
/// Simple modulo assignment (System.md Line 680)
/// "Hash(account) % num_shards determines shard"
pub fn assign_shard(address: &Address, shard_count: u16) -> ShardId {
    let hash = keccak256(address);
    let value = u16::from_be_bytes([hash[0], hash[1]]);
    value % shard_count
}

/// Rendezvous hashing for minimal reassignment (SPEC-14 Lines 131-141)
/// When adding shard N, only 1/N addresses move to new shard
pub fn rendezvous_assign(address: &Address, shards: &[ShardId]) -> ShardId {
    shards.iter()
        .map(|shard| {
            // Combine address with shard ID
            let combined = keccak256(&[address.as_slice(), &shard.to_be_bytes()].concat());
            (*shard, combined)
        })
        // Pick shard with highest hash
        .max_by_key(|(_, hash)| *hash)
        .map(|(shard, _)| shard)
        .unwrap()
}
```

#### Two-Phase Commit Protocol:

```
TWO-PHASE COMMIT FOR CROSS-SHARD TRANSACTIONS
Reference: System.md Line 681, SPEC-14 Lines 630-655

PHASE 1: LOCK (Prepare)
┌─────────────────────────────────────────────────────────────┐
│ Coordinator (Source Shard)                                   │
│   1. Lock sender's balance                                   │
│   2. Send LockRequest to all target shards                   │
│   3. Wait for LockResponse from all (or timeout)             │
└─────────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────┐
│ Participants (Target Shards)                                 │
│   1. Receive LockRequest                                     │
│   2. Lock recipient slot                                     │
│   3. Send LockResponse (success/failure)                     │
└─────────────────────────────────────────────────────────────┘

PHASE 2a: COMMIT (if all locks succeed)
┌─────────────────────────────────────────────────────────────┐
│ Coordinator                                                  │
│   1. Collect all LockProofs                                  │
│   2. Send CommitRequest to all shards (including self)       │
│   3. Apply deduction on source shard                         │
└─────────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────┐
│ Participants                                                 │
│   1. Receive CommitRequest                                   │
│   2. Apply credit on destination shard                       │
│   3. Release locks                                           │
│   4. Send CommitAck                                          │
└─────────────────────────────────────────────────────────────┘

PHASE 2b: ABORT (if any lock fails or timeout)
┌─────────────────────────────────────────────────────────────┐
│ All Shards                                                   │
│   1. Receive AbortRequest                                    │
│   2. Release all locks                                       │
│   3. No balance changes                                      │
│   4. Transaction marked as Aborted                           │
└─────────────────────────────────────────────────────────────┘
```

#### State Machine:

```rust
/// Cross-shard transaction state machine (SPEC-14 Lines 95-101)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrossShardState {
    Pending,     // Initial state
    Locked,      // Phase 1 complete - all locks acquired
    Committed,   // Phase 2a complete - all shards committed
    Aborted,     // Phase 2b - rolled back
}

impl CrossShardState {
    pub fn can_transition_to(&self, next: CrossShardState) -> bool {
        match (self, next) {
            (Self::Pending, Self::Locked) => true,
            (Self::Pending, Self::Aborted) => true,  // Lock failed
            (Self::Locked, Self::Committed) => true,
            (Self::Locked, Self::Aborted) => true,   // Commit failed or timeout
            _ => false,
        }
    }
}
```

#### Expected Output:
- Deterministic shard assignment (same input = same shard)
- Uniform distribution (each shard gets ~1/n addresses)
- Atomicity (Committed or Aborted, never partial)

---

### 7️⃣ BENCHMARK EXPERT
> *"Sharding must provide actual speedup"*
>
> **Reference:** System.md Line 725 "10x throughput increase for cross-shard transactions"

#### Benchmark Targets:

| Operation | Target | Reference |
|-----------|--------|-----------|
| Shard assignment | <1µs | System.md Line 680 |
| Intra-shard tx | Same as single-chain | System.md Line 686 |
| Cross-shard tx | <30s (2PC timeout) | SPEC-14 Line 650 |
| Global state root | <100ms for 64 shards | SPEC-14 Line 167 |

#### Benchmark Setup:

```rust
// benches/sharding_bench.rs
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn benchmark_shard_assignment(c: &mut Criterion) {
    let mut group = c.benchmark_group("shard_assignment");
    
    for shard_count in [4, 16, 64, 256].iter() {
        group.bench_with_input(
            BenchmarkId::new("assign", shard_count),
            shard_count,
            |b, &count| {
                let address = Address::random();
                b.iter(|| assign_shard(&address, count))
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("rendezvous", shard_count),
            shard_count,
            |b, &count| {
                let address = Address::random();
                let shards: Vec<_> = (0..count).collect();
                b.iter(|| rendezvous_assign(&address, &shards))
            },
        );
    }
    
    group.finish();
}

fn benchmark_global_state(c: &mut Criterion) {
    let mut group = c.benchmark_group("global_state");
    
    for shard_count in [16, 64, 256].iter() {
        group.bench_with_input(
            BenchmarkId::new("compute_root", shard_count),
            shard_count,
            |b, &count| {
                let shard_roots: Vec<Hash> = (0..count).map(|i| [i as u8; 32]).collect();
                b.iter(|| compute_global_state_root(&shard_roots))
            },
        );
    }
    
    group.finish();
}

criterion_group!(benches, benchmark_shard_assignment, benchmark_global_state);
criterion_main!(benches);
```

---

## 📋 IMPLEMENTATION CHECKLIST (DETAILED)

### Phase 1: Domain Setup (TDD)
> **Reference:** SPEC-14 Section 2 (Lines 50-173), Architecture.md Section 2.1

#### Step 1.1: Create Domain Module Structure
```bash
mkdir -p src/domain
touch src/domain/mod.rs src/domain/entities.rs src/domain/value_objects.rs src/domain/errors.rs src/domain/invariants.rs
```

**Sub-tasks:**
- [ ] **1.1.1** Create `src/domain/mod.rs`
  ```rust
  pub mod entities;
  pub mod value_objects;
  pub mod errors;
  pub mod invariants;
  
  pub use entities::*;
  pub use value_objects::*;
  pub use errors::*;
  ```

#### Step 1.2: Implement Value Objects (SPEC-14 Lines 54-101)
- [ ] **1.2.1** Create `ShardId` type alias (SPEC-14 Line 56)
  ```rust
  pub type ShardId = u16;
  ```
- [ ] **1.2.2** Create `CrossShardState` enum (SPEC-14 Lines 95-101)
- [ ] **1.2.3** Create `ShardAssignment` struct (SPEC-14 Lines 79-84)
- [ ] **1.2.4** Create `AbortReason` enum (SPEC-14 Lines 374-379)
- [ ] **1.2.5** Write unit tests FIRST for value objects
- [ ] **1.2.6** Implement value objects to pass tests

#### Step 1.3: Implement Entities (SPEC-14 Lines 58-117)
- [ ] **1.3.1** Write test for `ShardConfig` (SPEC-14 Lines 58-77)
- [ ] **1.3.2** Implement `ShardConfig` with Default impl
- [ ] **1.3.3** Write test for `CrossShardTransaction` (SPEC-14 Lines 86-93)
- [ ] **1.3.4** Implement `CrossShardTransaction`
- [ ] **1.3.5** Write test for `ShardStateRoot` (SPEC-14 Lines 103-109)
- [ ] **1.3.6** Implement `ShardStateRoot`
- [ ] **1.3.7** Write test for `GlobalStateRoot` (SPEC-14 Lines 111-117)
- [ ] **1.3.8** Implement `GlobalStateRoot`

#### Step 1.4: Implement Errors (SPEC-14 Lines 533-554)
- [ ] **1.4.1** Create `ShardError` enum with all variants
  ```rust
  #[derive(Debug, thiserror::Error)]
  pub enum ShardError {
      #[error("Unknown shard: {0}")]
      UnknownShard(ShardId),
      #[error("Cross-shard lock failed: {0}")]
      LockFailed(String),
      #[error("Cross-shard timeout")]
      Timeout,
      #[error("Invalid cross-shard proof")]
      InvalidProof,
      // ... rest from SPEC-14
  }
  ```

#### Step 1.5: Implement Invariants (SPEC-14 Lines 144-173)
- [ ] **1.5.1** Write property test for `invariant_deterministic_assignment` (SPEC-14 Lines 147-153)
- [ ] **1.5.2** Implement `invariant_deterministic_assignment`
- [ ] **1.5.3** Write property test for `invariant_cross_shard_atomic` (SPEC-14 Lines 155-163)
- [ ] **1.5.4** Implement `invariant_cross_shard_atomic`
- [ ] **1.5.5** Write property test for `invariant_global_consistency` (SPEC-14 Lines 165-172)
- [ ] **1.5.6** Implement `invariant_global_consistency`

---

### Phase 2: Algorithm Implementation (TDD)
> **Reference:** System.md Lines 680-683, SPEC-14 Section 2.2

#### Step 2.1: Create Algorithm Module Structure
```bash
mkdir -p src/algorithms
touch src/algorithms/mod.rs src/algorithms/shard_assignment.rs src/algorithms/two_phase_commit.rs src/algorithms/global_state.rs src/algorithms/validator_shuffle.rs
```

#### Step 2.2: Implement Shard Assignment (System.md Line 680)
- [ ] **2.2.1** Write test `test_deterministic_shard_assignment` (SPEC-14 Line 395)
  - Input: Same address twice
  - Expected: Same shard both times
- [ ] **2.2.2** Implement skeleton `assign_shard()` returning `todo!()`
- [ ] **2.2.3** Run test, verify RED
- [ ] **2.2.4** Implement `assign_shard()` with modulo hash
- [ ] **2.2.5** Run test, verify GREEN

- [ ] **2.2.6** Write test `test_uniform_distribution` (SPEC-14 Line 406)
  - Input: 10,000 random addresses
  - Expected: Each shard gets 80-120% of expected count
- [ ] **2.2.7** Verify existing implementation passes

- [ ] **2.2.8** Write test for `rendezvous_assign` (SPEC-14 Lines 131-141)
- [ ] **2.2.9** Implement `rendezvous_assign()`
- [ ] **2.2.10** Write test for minimal reassignment on shard addition
- [ ] **2.2.11** Verify rendezvous moves only 1/n addresses

#### Step 2.3: Implement Two-Phase Commit (System.md Line 681)
- [ ] **2.3.1** Write test for `is_cross_shard()` detection (SPEC-14 Line 427)
- [ ] **2.3.2** Implement `is_cross_shard()`
- [ ] **2.3.3** Write test `test_same_shard_transaction` (SPEC-14 Line 440)
- [ ] **2.3.4** Verify passes

- [ ] **2.3.5** Write test for lock acquisition
- [ ] **2.3.6** Implement `acquire_lock()`
- [ ] **2.3.7** Write test for lock release
- [ ] **2.3.8** Implement `release_lock()`

- [ ] **2.3.9** Write test for successful 2PC flow (SPEC-14 Line 475)
- [ ] **2.3.10** Implement `TwoPhaseCoordinator` state machine
- [ ] **2.3.11** Write test for timeout abort (SPEC-14 Line 502)
- [ ] **2.3.12** Implement timeout handling with auto-abort

#### Step 2.4: Implement Global State (SPEC-14 Lines 165-172)
- [ ] **2.4.1** Write test `test_global_state_root_computation` (SPEC-14 Line 455)
  - Input: 4 shard roots
  - Expected: Deterministic Merkle tree root
- [ ] **2.4.2** Implement `compute_global_state_root()`
- [ ] **2.4.3** Write test for re-computation consistency
- [ ] **2.4.4** Verify same input produces same output

---

### Phase 3: Protocol Implementation
> **Reference:** SPEC-14 Section 4 (Lines 317-380)

#### Step 3.1: Create Protocol Module Structure
```bash
mkdir -p src/protocol
touch src/protocol/mod.rs src/protocol/messages.rs src/protocol/coordinator.rs
```

#### Step 3.2: Define Cross-Shard Messages (SPEC-14 Lines 321-380)
- [ ] **3.2.1** Create `CrossShardMessage` enum (SPEC-14 Lines 323-356)
- [ ] **3.2.2** Create `LockData` struct (SPEC-14 Lines 358-364)
- [ ] **3.2.3** Create `LockProof` struct (SPEC-14 Lines 366-372)

#### Step 3.3: Implement Coordinator State Machine
- [ ] **3.3.1** Create `Coordinator` struct with state tracking
- [ ] **3.3.2** Implement `handle_lock_response()`
- [ ] **3.3.3** Implement `decide_commit_or_abort()`
- [ ] **3.3.4** Implement timeout handling

---

### Phase 4: Ports & Adapters
> **Reference:** SPEC-14 Section 3 (Lines 177-313), Architecture.md Section 2.2

#### Step 4.1: Create Port Module Structure
```bash
mkdir -p src/ports
touch src/ports/mod.rs src/ports/inbound.rs src/ports/outbound.rs
```

#### Step 4.2: Define Inbound Ports (SPEC-14 Lines 181-223)
- [ ] **4.2.1** Create `ShardingApi` trait
  ```rust
  #[async_trait]
  pub trait ShardingApi: Send + Sync {
      fn get_shard(&self, address: &Address) -> ShardId;
      async fn route_transaction(&self, tx: SignedTransaction) -> Result<RoutingResult, ShardError>;
      async fn get_global_state_root(&self) -> Result<GlobalStateRoot, ShardError>;
      async fn get_shard_validators(&self, shard_id: ShardId) -> Result<Vec<ValidatorInfo>, ShardError>;
      async fn process_cross_shard_message(&self, msg: CrossShardMessage) -> Result<(), ShardError>;
  }
  ```

#### Step 4.3: Define Outbound Ports (SPEC-14 Lines 225-313)
- [ ] **4.3.1** Create `ShardConsensus` trait (SPEC-14 Lines 228-244)
- [ ] **4.3.2** Create `PartitionedState` trait (SPEC-14 Lines 247-261)
- [ ] **4.3.3** Create `BeaconChainProvider` trait (SPEC-14 Lines 263-300)
- [ ] **4.3.4** Create mock implementations for all ports

---

### Phase 5: Application Service
> **Reference:** Architecture.md Section 2.1

#### Step 5.1: Create Application Module
```bash
mkdir -p src/application
touch src/application/mod.rs src/application/service.rs
```

#### Step 5.2: Implement Service
- [ ] **5.2.1** Create `ShardingService` struct
- [ ] **5.2.2** Implement `ShardingApi` trait
- [ ] **5.2.3** Implement `route_transaction()` with cross-shard detection
- [ ] **5.2.4** Implement `get_global_state_root()` with Merkle tree
- [ ] **5.2.5** Wire 2PC coordinator for cross-shard transactions

---

### Phase 6: Integration & Benchmarks
> **Reference:** SPEC-14 Section 5.2 (Lines 468-527)

#### Step 6.1: Create Integration Tests
```bash
mkdir -p tests/integration
touch tests/integration/cross_shard_tests.rs
```

- [ ] **6.1.1** Write `test_cross_shard_transfer` (SPEC-14 Line 475)
- [ ] **6.1.2** Write `test_cross_shard_timeout_abort` (SPEC-14 Line 502)
- [ ] **6.1.3** Write `test_multi_shard_transaction`
- [ ] **6.1.4** Write `test_network_partition_recovery`

#### Step 6.2: Create Benchmarks
```bash
mkdir -p benches
touch benches/sharding_bench.rs
```

- [ ] **6.2.1** Add criterion benchmark for shard assignment
- [ ] **6.2.2** Add criterion benchmark for global state root
- [ ] **6.2.3** Verify <1µs for assignment
- [ ] **6.2.4** Verify <100ms for 64-shard global root

---

### Phase 7: Security Hardening
> **Reference:** System.md Lines 695-700, SPEC-14 Appendix B

- [ ] **7.1** Implement validator shuffling verification - System.md Line 696
- [ ] **7.2** Implement cross-link verification - System.md Line 697
- [ ] **7.3** Implement data availability sampling (stub) - System.md Line 698
- [ ] **7.4** Implement fraud proof handling (stub) - System.md Line 699
- [ ] **7.5** Enforce minimum shard size (128 validators) - System.md Line 700
- [ ] **7.6** Implement 67%+ signature verification for receipts
- [ ] **7.7** Add security-specific tests

---

## 🔧 VERIFICATION COMMANDS

```bash
# Build check
cargo build -p qc-14-sharding

# Run tests
cargo test -p qc-14-sharding

# Check test coverage
cargo llvm-cov --package qc-14-sharding

# Run benchmarks
cargo bench --package qc-14-sharding

# Lint check
cargo clippy -p qc-14-sharding -- -D warnings

# Format check
cargo fmt -p qc-14-sharding -- --check
```

---

## ⚠️ STRICT IMPLEMENTATION CONSTRAINTS

> These constraints are NON-NEGOTIABLE and enforced by the architecture.

| # | Constraint | Reference | Enforcement |
|---|------------|-----------|-------------|
| 1 | **TDD MANDATORY** | Architecture.md 2.4 | Never write implementation without tests first |
| 2 | **DETERMINISTIC ASSIGNMENT** | SPEC-14 Lines 147-153 | Same address MUST map to same shard |
| 3 | **ATOMIC CROSS-SHARD** | SPEC-14 Lines 155-163 | Committed OR Aborted, never partial |
| 4 | **2PC TIMEOUT** | SPEC-14 Lines 650-652 | Auto-abort after 30s (configurable) |
| 5 | **VALIDATOR MINIMUM** | System.md Line 700 | Require 128+ validators per shard |
| 6 | **67% SIGNATURES** | SPEC-14 Lines 605-614 | Cross-shard receipts require 2/3 validator signatures |
| 7 | **BEACON AUTHORITY** | SPEC-14 Lines 268-272 | Only beacon chain assigns validators to shards |
| 8 | **ENVELOPE IDENTITY** | Architecture.md 3.2.1 | Use `msg.sender_id`, never payload fields |

---

**END OF TODO**
