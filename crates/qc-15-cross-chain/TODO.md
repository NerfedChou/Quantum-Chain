# TODO: Cross-Chain Communication Subsystem (qc-15)

**Generated:** 2025-12-10  
**Spec Reference:** [SPEC-15-CROSS-CHAIN.md](file:///home/chef/Github/Quantum-Chain/SPECS/SPEC-15-CROSS-CHAIN.md)

---

## 📚 MASTER DOCUMENT REFERENCES

| Document | Path | Relevance |
|----------|------|-----------|
| **System.md** | [Documentation/System.md](file:///home/chef/Github/Quantum-Chain/Documentation/System.md) | Subsystem 15 definition (lines 729-762): HTLC, atomic swaps, timelock margins |
| **Architecture.md** | [Documentation/Architecture.md](file:///home/chef/Github/Quantum-Chain/Documentation/Architecture.md) | Section 3.2.1: Envelope-Only Identity; Section 2.2: Hexagonal Architecture |
| **IPC-MATRIX.md** | [Documentation/IPC-MATRIX.md](file:///home/chef/Github/Quantum-Chain/Documentation/IPC-MATRIX.md) | Subsystem 15: Cross-chain message types, relayer authorization |

### Key References by Topic

| Topic | Document | Section/Line |
|-------|----------|--------------|
| HTLC Algorithm | System.md | Lines 732-733 |
| Hashlock Creation | System.md | Line 736 |
| Timelock Setup | System.md | Line 737 |
| Atomic Swap Protocol | System.md | Line 738 |
| Secret Reveal | System.md | Line 739 |
| Dependencies (11, 8) | System.md | Lines 741-743 |
| Attack Vectors | System.md | Lines 746-749 |
| Security Defenses | System.md | Lines 751-756 |
| Robustness Measures | System.md | Lines 758-761 |
| Domain Model | SPEC-15 | Section 2.1 (Lines 54-175) |
| Invariants | SPEC-15 | Section 2.2 (Lines 177-211) |
| Ports Definition | SPEC-15 | Section 3 (Lines 215-291) |
| Event Schema | SPEC-15 | Section 4 (Lines 295-333) |
| TDD Strategy | SPEC-15 | Section 5 (Lines 337-493) |
| Error Handling | SPEC-15 | Section 6 (Lines 498-530) |
| HTLC Security | SPEC-15 | Appendix B.2 (Lines 574-634) |
| Chain-Specific Finality | SPEC-15 | Appendix B.3 (Lines 636-662) |

---

## 🎯 OBJECTIVE

> **System.md Lines 729-733:**
> "SUBSYSTEM 15: CROSS-CHAIN COMMUNICATION
> Purpose: Enable asset transfers between independent blockchains
> Main Algorithm: HTLC (Hash Time-Locked Contracts)
> Why: Trustless atomic swaps, no third-party escrow"

Implement trustless cross-chain asset transfers using Hash Time-Locked Contracts (HTLCs), enabling atomic swaps between QuantumChain and external blockchains (Ethereum, Bitcoin, Polygon, Arbitrum) without trusted intermediaries.

---

## 🏛️ TEAMS AUDIT

### 1️⃣ MASTER ARCHITECT
> *"Structure the crate for long-term maintainability following DDD + Hexagonal + TDD"*
> 
> **Reference:** Architecture.md Section 2 (Lines 56-284)

#### How to Implement:

```
crates/qc-15-cross-chain/
├── Cargo.toml                    # Dependencies: thiserror, async-trait, serde, tokio, sha2, shared-types
├── src/
│   ├── lib.rs                    # Re-exports, feature flags
│   ├── domain/
│   │   ├── mod.rs
│   │   ├── entities.rs           # SPEC-15 Section 2.1: HTLC, AtomicSwap, CrossChainMessage
│   │   ├── value_objects.rs      # SPEC-15 Section 2.1: ChainId, HTLCState, SwapState, ChainAddress
│   │   ├── invariants.rs         # SPEC-15 Section 2.2: HTLC atomicity, timelock ordering, hashlock match
│   │   └── errors.rs             # SPEC-15 Section 6: CrossChainError enum
│   ├── ports/
│   │   ├── mod.rs
│   │   ├── inbound.rs            # SPEC-15 Section 3.1: CrossChainApi trait
│   │   └── outbound.rs           # SPEC-15 Section 3.2: HTLCContract, ExternalChainClient, FinalityChecker
│   ├── application/
│   │   ├── mod.rs
│   │   └── service.rs            # CrossChainService
│   ├── algorithms/
│   │   ├── mod.rs
│   │   ├── htlc.rs               # System.md Lines 736-739: HTLC creation, verification
│   │   ├── atomic_swap.rs        # System.md Line 738: Atomic swap state machine
│   │   ├── proof_verifier.rs     # SPEC-15 Lines 140-147: Cross-chain proof verification
│   │   └── secret.rs             # System.md Lines 736, 739: Secret generation and hashing
│   ├── chains/
│   │   ├── mod.rs
│   │   ├── ethereum.rs           # Ethereum light client adapter
│   │   ├── bitcoin.rs            # Bitcoin light client adapter
│   │   └── finality.rs           # Chain-specific finality rules (SPEC-15 Lines 650-654)
│   └── config.rs                 # SPEC-15 Section 7: CrossChainConfig
└── tests/
    ├── unit/
    │   ├── htlc_tests.rs
    │   ├── swap_tests.rs
    │   └── proof_tests.rs
    └── integration/
        └── atomic_swap_tests.rs
```

#### Expected Output:
- Clear separation: Domain (pure), Ports (traits), Application (orchestration), Algorithms (crypto)
- Chain-specific adapters in `chains/` module
- Dependency inversion via traits for HTLCContract, ExternalChainClient

#### Debugging When Errors:
| Error | Likely Cause | Fix |
|-------|--------------|-----|
| Circular dependency in `use` | Wrong module structure | Check `mod.rs` exports match file structure |
| "trait not implemented" | Missing adapter | Ensure ports have mock implementations for tests |
| "cannot find type" | Missing re-export | Add `pub use` in parent `mod.rs` |

---

### 2️⃣ CYBERSECURITY ZERO-DAY EXPERT
> *"Cross-chain bridges are prime attack targets - HTLC security is critical"*
>
> **Reference:** System.md Lines 745-756 (Security & Robustness)

#### Security Analysis:

| Attack Vector | Risk Level | Mitigation | Reference |
|---------------|------------|------------|-----------|
| **Timing Attack** (claim one, timeout other) | 🔴 CRITICAL | Timelock margins (6+ hours) | System.md Line 752 |
| **Hash Collision** | 🔴 HIGH | Use SHA-256, never MD5/SHA-1 | System.md Line 753 |
| **Relay Censorship** | 🟡 MEDIUM | Multiple relay paths | System.md Line 755 |
| **Front-Running** | 🟡 MEDIUM | Mempool privacy, commit-reveal | N/A |
| **Secret Leakage** | 🔴 HIGH | Secure memory handling, zeroize | N/A |
| **Finality Bypass** | 🔴 CRITICAL | Wait for chain-specific confirmations | SPEC-15 Lines 650-654 |
| **Sender Spoofing** | 🔴 HIGH | Envelope-Only Identity | Architecture.md 3.2.1 |

#### How to Implement Security:

```rust
/// CRITICAL: Timelock margin enforcement
/// Reference: System.md Line 752 - "Chain A timeout > Chain B timeout + 6 hours"
fn validate_timelock_ordering(
    source_htlc: &HTLC,
    target_htlc: &HTLC,
    min_margin_hours: u64,
) -> Result<(), CrossChainError> {
    let min_margin_secs = min_margin_hours * 3600;
    
    if source_htlc.time_lock <= target_htlc.time_lock + min_margin_secs {
        return Err(CrossChainError::InvalidTimelockMargin {
            source: source_htlc.time_lock,
            target: target_htlc.time_lock,
            required_margin: min_margin_secs,
        });
    }
    
    Ok(())
}

/// Secret hashing - ONLY use SHA-256
/// Reference: System.md Line 753 - "Use SHA-256, avoid weak hashes"
fn create_hash_lock(secret: &[u8; 32]) -> Hash {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(secret);
    hasher.finalize().into()
}

/// Secret verification
fn verify_secret(htlc: &HTLC, secret: &[u8; 32]) -> bool {
    let computed_hash = create_hash_lock(secret);
    computed_hash == htlc.hash_lock
}

/// Chain-specific finality verification
/// Reference: SPEC-15 Lines 650-654
fn get_required_confirmations(chain: ChainId) -> u64 {
    match chain {
        ChainId::Bitcoin => 6,      // PoW, 1 hour
        ChainId::Ethereum => 12,    // PoS, 2 epochs
        ChainId::Polygon => 128,    // Fast finality
        ChainId::Arbitrum => 1,     // L2, verified by L1
        ChainId::QuantumChain => 6, // Our chain
    }
}

/// HTLC claim verification
/// Reference: SPEC-15 Lines 578-610
fn verify_htlc_claim(
    htlc: &HTLC,
    secret: &[u8; 32],
    claimer: &Address,
    current_time: u64,
) -> Result<(), CrossChainError> {
    // 1. Not expired
    if current_time > htlc.time_lock {
        return Err(CrossChainError::HTLCExpired);
    }
    
    // 2. Secret matches hashlock
    if !verify_secret(htlc, secret) {
        return Err(CrossChainError::InvalidSecret);
    }
    
    // 3. Claimer is authorized recipient
    if claimer != &htlc.recipient.address {
        return Err(CrossChainError::UnauthorizedClaimer);
    }
    
    Ok(())
}
```

#### Expected Output:
- All HTLCs use SHA-256 for hash locks
- Source timelock > Target timelock + 6 hours
- Chain-specific finality enforced before accepting proofs
- Secrets zeroized after use

#### Debugging When Errors:
| Error | Likely Cause | Fix |
|-------|--------------|-----|
| `InvalidTimelockMargin` | Insufficient time buffer | Increase source timelock |
| `InvalidSecret` | Wrong secret or hash mismatch | Verify secret generation is consistent |
| `HTLCExpired` | Claim too late | Improve monitoring, claim earlier |
| `NotFinalized` | Insufficient confirmations | Wait for chain-specific finality |

---

### 3️⃣ SCALABILITY & MAINTAINABILITY EXPERT
> *"Multi-chain support requires clean abstractions"*
>
> **Reference:** Architecture.md Sections 2.1-2.4

#### Scalability Patterns:

**1. Chain-Agnostic Interface**
```rust
/// Abstract over chain differences
pub trait ExternalChainClient: Send + Sync {
    async fn get_header(&self, chain: ChainId, height: u64) -> Result<BlockHeader, ChainError>;
    async fn verify_proof(&self, chain: ChainId, proof: &CrossChainProof) -> Result<bool, ChainError>;
    async fn is_finalized(&self, chain: ChainId, block_hash: Hash) -> Result<bool, ChainError>;
}

/// Chain-specific implementations behind the trait
struct EthereumClient { /* ... */ }
struct BitcoinClient { /* ... */ }
struct PolygonClient { /* ... */ }

impl ExternalChainClient for EthereumClient { /* ... */ }
impl ExternalChainClient for BitcoinClient { /* ... */ }
```

**2. Parallel Proof Verification**
```rust
/// Verify proofs from multiple chains in parallel
async fn verify_all_proofs(
    proofs: Vec<(ChainId, CrossChainProof)>,
    client: &impl ExternalChainClient,
) -> Vec<Result<bool, ChainError>> {
    let tasks: Vec<_> = proofs.iter()
        .map(|(chain, proof)| client.verify_proof(*chain, proof))
        .collect();
    
    join_all(tasks).await
}
```

**3. Swap State Machine**
```rust
/// Clean state transitions for atomic swaps
impl AtomicSwap {
    pub fn transition(&mut self, event: SwapEvent) -> Result<(), SwapError> {
        let new_state = match (&self.state, event) {
            (SwapState::Initiated, SwapEvent::SourceLocked) => SwapState::SourceLocked,
            (SwapState::SourceLocked, SwapEvent::TargetLocked) => SwapState::TargetLocked,
            (SwapState::TargetLocked, SwapEvent::BothClaimed) => SwapState::Completed,
            (_, SwapEvent::Timeout) => SwapState::Refunded,
            _ => return Err(SwapError::InvalidTransition),
        };
        self.state = new_state;
        Ok(())
    }
}
```

#### Expected Output:
- Adding new chain = implement one trait
- State machine prevents invalid transitions
- Parallel processing for multi-chain verification

#### Debugging When Errors:
| Error | Likely Cause | Fix |
|-------|--------------|-----|
| `UnsupportedChain` | Chain not implemented | Add chain client impl |
| `InvalidTransition` | State machine violation | Check event ordering |
| Slow proof verification | Sequential processing | Use `join_all` for parallelism |

---

### 4️⃣ QA EXPERT
> *"Cross-chain atomicity must be bulletproof - test all failure paths"*
>
> **Reference:** SPEC-15 Section 5 (Lines 337-493)

#### Test Strategy:

| Test Type | Location | Coverage Target | Reference |
|-----------|----------|-----------------|-----------|
| Unit Tests | `src/algorithms/*.rs` | 100% of HTLC + secret logic | SPEC-15 Lines 346-418 |
| Unit Tests | `src/domain/*.rs` | 100% of invariants | SPEC-15 Lines 177-211 |
| Integration | `tests/integration/` | Full swap + timeout/refund | SPEC-15 Lines 424-493 |
| Property Tests | `tests/property/` | Timelock ordering, atomicity | SPEC-15 Lines 194-210 |

#### Critical Test Cases (from SPEC-15):

```rust
// === HTLC TESTS (SPEC-15 Lines 346-375) ===

#[test]
fn test_htlc_secret_verification() { }              // Line 348

#[test]
fn test_htlc_expiry() { }                           // Line 359

#[test]
fn test_htlc_claim_before_expiry() { }              // Line 368

// === ATOMIC SWAP TESTS (SPEC-15 Lines 377-401) ===

#[test]
fn test_timelock_ordering() { }                     // Line 379

#[test]
fn test_swap_state_transitions() { }                // Line 387

// === PROOF VERIFICATION TESTS (SPEC-15 Lines 403-418) ===

#[test]
fn test_merkle_proof_verification() { }             // Line 405

#[test]
fn test_invalid_merkle_proof() { }                  // Line 412

// === INTEGRATION TESTS (SPEC-15 Lines 424-493) ===

#[tokio::test]
async fn test_complete_atomic_swap() { }            // Line 429

#[tokio::test]
async fn test_swap_timeout_refund() { }             // Line 469

// === CRITICAL FAILURE TESTS ===

#[tokio::test]
async fn test_secret_revealed_but_source_not_claimed() { }

#[tokio::test]
async fn test_target_locks_before_source() { }

#[tokio::test]
async fn test_insufficient_finality_rejects_claim() { }
```

#### Expected Output:
- `cargo test` passes with 0 failures
- `cargo llvm-cov` shows >90% line coverage
- All HTLC state transitions tested
- Timeout/refund paths fully tested

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
# Edit src/algorithms/htlc.rs

# Phase 3: REFACTOR - Clean up
cargo fmt && cargo clippy -- -D warnings

# Verify
cargo test
```

#### Expected Output Per Phase:

**Phase 1 (RED):**
```
---- algorithms::htlc::test_htlc_secret_verification stdout ----
thread '...' panicked at 'not yet implemented'
```

**Phase 2 (GREEN):**
```
running 1 test
test algorithms::htlc::test_htlc_secret_verification ... ok
```

---

### 6️⃣ ALGORITHMIC PRECISION EXPERT
> *"HTLC protocol must be cryptographically correct"*
>
> **Reference:** System.md Lines 736-739, SPEC-15 Lines 177-211

#### HTLC Protocol Overview:

```
HASH TIME-LOCKED CONTRACT (HTLC) PROTOCOL
Reference: System.md Lines 736-739

PARTICIPANTS:
- Alice: Initiator, has asset on Chain A, wants asset on Chain B
- Bob: Counterparty, has asset on Chain B, wants asset on Chain A

SETUP:
1. Alice generates random secret S (32 bytes)
2. Alice computes H = SHA-256(S)
3. Alice shares H (NOT S) with Bob

EXECUTION:
┌─────────────────────────────────────────────────────────────────┐
│ STEP 1: Alice locks on Chain A                                  │
│   - Deploy HTLC: lock(amount, Bob, H, timeout_A)                │
│   - timeout_A = now + 24 hours                                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ STEP 2: Bob verifies Alice's HTLC on Chain A                    │
│   - Get proof of HTLC from Chain A                              │
│   - Wait for finality (6 confirmations Bitcoin, 12 Ethereum)    │
│   - Verify: hashlock == H, receiver == Bob                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ STEP 3: Bob locks on Chain B                                    │
│   - Deploy HTLC: lock(amount, Alice, H, timeout_B)              │
│   - timeout_B = now + 18 hours (MUST be < timeout_A - 6 hours)  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ STEP 4: Alice claims on Chain B (REVEALS SECRET)                │
│   - Call claim(S) on Bob's HTLC                                 │
│   - Contract verifies SHA-256(S) == H                           │
│   - Alice receives Bob's asset on Chain B                       │
│   - SECRET S IS NOW PUBLIC ON CHAIN B                           │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ STEP 5: Bob claims on Chain A (USES REVEALED SECRET)            │
│   - Monitor Chain B for Alice's claim                           │
│   - Extract secret S from claim transaction                     │
│   - Call claim(S) on Alice's HTLC on Chain A                    │
│   - Bob receives Alice's asset on Chain A                       │
└─────────────────────────────────────────────────────────────────┘

TIMEOUT/REFUND (if either party fails):
- If Alice doesn't claim Bob's HTLC before timeout_B:
  - Bob calls refund() after timeout_B, gets assets back
  - Alice calls refund() after timeout_A, gets assets back
- ATOMICITY: Either BOTH claim, or BOTH refund
```

#### Timelock Invariant:

```rust
/// CRITICAL INVARIANT: Timelock Ordering
/// Reference: System.md Line 752, SPEC-15 Lines 194-198
/// 
/// Target HTLC MUST expire before source HTLC with sufficient margin.
/// This gives the recipient time to extract the secret and claim.
/// 
/// FORMULA: source_timeout > target_timeout + MIN_MARGIN
/// WHERE: MIN_MARGIN >= 6 hours (time to detect + claim)
fn invariant_timelock_ordering(source: &HTLC, target: &HTLC) -> bool {
    const MIN_MARGIN_SECS: u64 = 6 * 3600; // 6 hours
    target.time_lock + MIN_MARGIN_SECS < source.time_lock
}
```

#### State Machine:

```rust
/// HTLC State Machine (SPEC-15 Lines 96-103)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HTLCState {
    Pending,   // Created but not locked
    Locked,    // Funds locked, awaiting claim/expiry
    Claimed,   // Secret revealed, funds transferred to recipient
    Refunded,  // Expired, funds returned to sender
    Expired,   // Past timelock, awaiting refund
}

impl HTLCState {
    pub fn can_transition_to(&self, next: HTLCState, current_time: u64, timelock: u64) -> bool {
        match (self, next) {
            (Self::Pending, Self::Locked) => true,
            (Self::Locked, Self::Claimed) => current_time <= timelock,
            (Self::Locked, Self::Expired) => current_time > timelock,
            (Self::Expired, Self::Refunded) => true,
            _ => false,
        }
    }
}

/// Atomic Swap State Machine (SPEC-15 Lines 119-126)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwapState {
    Initiated,     // Swap created
    SourceLocked,  // Source HTLC locked
    TargetLocked,  // Target HTLC locked (both locked)
    Completed,     // Both HTLCs claimed
    Refunded,      // Both HTLCs refunded
}
```

#### Expected Output:
- Secret generation: 32 bytes of cryptographically secure randomness
- Hash function: SHA-256 only (32 bytes output)
- Timelock margin: ≥ 6 hours between target and source

---

### 7️⃣ BENCHMARK EXPERT
> *"Cross-chain operations must be efficient"*
>
> **Reference:** System.md Lines 758-761 (Robustness Measures)

#### Benchmark Targets:

| Operation | Target | Reference |
|-----------|--------|-----------|
| Secret generation | <1ms | N/A |
| Hash computation (SHA-256) | <1µs | System.md Line 753 |
| HTLC state validation | <10µs | N/A |
| Proof verification | <10ms | Chain-dependent |
| Full swap (happy path) | <10s + finality | Depends on chains |

#### Benchmark Setup:

```rust
// benches/cross_chain_bench.rs
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn benchmark_secret_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("secret_operations");
    
    group.bench_function("generate_secret", |b| {
        b.iter(|| generate_random_secret())
    });
    
    group.bench_function("hash_secret", |b| {
        let secret = [0xAB; 32];
        b.iter(|| create_hash_lock(&secret))
    });
    
    group.bench_function("verify_secret", |b| {
        let secret = [0xAB; 32];
        let hash_lock = create_hash_lock(&secret);
        let htlc = create_htlc_with_hashlock(hash_lock);
        b.iter(|| verify_secret(&htlc, &secret))
    });
    
    group.finish();
}

fn benchmark_htlc_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("htlc_operations");
    
    group.bench_function("create_htlc", |b| {
        b.iter(|| create_htlc_default())
    });
    
    group.bench_function("validate_claim", |b| {
        let htlc = create_locked_htlc();
        let secret = [0xAB; 32];
        let claimer = htlc.recipient.clone();
        b.iter(|| verify_htlc_claim(&htlc, &secret, &claimer.address, now()))
    });
    
    group.finish();
}

criterion_group!(benches, benchmark_secret_ops, benchmark_htlc_ops);
criterion_main!(benches);
```

---

## 📋 IMPLEMENTATION CHECKLIST (DETAILED)

### Phase 1: Domain Setup (TDD)
> **Reference:** SPEC-15 Section 2 (Lines 52-211), Architecture.md Section 2.1

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

#### Step 1.2: Implement Value Objects (SPEC-15 Lines 56-126)
- [ ] **1.2.1** Create `ChainId` enum (SPEC-15 Lines 57-65)
  ```rust
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
  pub enum ChainId {
      QuantumChain,
      Ethereum,
      Bitcoin,
      Polygon,
      Arbitrum,
  }
  ```
- [ ] **1.2.2** Create `HTLCState` enum (SPEC-15 Lines 96-103)
- [ ] **1.2.3** Create `SwapState` enum (SPEC-15 Lines 119-126)
- [ ] **1.2.4** Create `ChainAddress` struct (SPEC-15 Lines 89-94)
- [ ] **1.2.5** Write unit tests FIRST for value objects
- [ ] **1.2.6** Implement value objects to pass tests

#### Step 1.3: Implement Entities (SPEC-15 Lines 67-174)
- [ ] **1.3.1** Write test for `HTLC` struct (SPEC-15 Lines 67-87)
- [ ] **1.3.2** Implement `HTLC` with methods: `is_expired()`, `can_claim()`, `can_refund()`
- [ ] **1.3.3** Write test for `AtomicSwap` struct (SPEC-15 Lines 105-117)
- [ ] **1.3.4** Implement `AtomicSwap` with state machine
- [ ] **1.3.5** Write test for `CrossChainMessage` (SPEC-15 Lines 128-138)
- [ ] **1.3.6** Implement `CrossChainMessage`
- [ ] **1.3.7** Write test for `CrossChainProof` (SPEC-15 Lines 140-147)
- [ ] **1.3.8** Implement `CrossChainProof`
- [ ] **1.3.9** Write test for `CrossChainConfig` (SPEC-15 Lines 149-174)
- [ ] **1.3.10** Implement `CrossChainConfig` with Default impl

#### Step 1.4: Implement Errors (SPEC-15 Lines 500-530)
- [ ] **1.4.1** Create `CrossChainError` enum with all variants
  ```rust
  #[derive(Debug, thiserror::Error)]
  pub enum CrossChainError {
      #[error("Unsupported chain: {0:?}")]
      UnsupportedChain(ChainId),
      #[error("HTLC not found: {0:?}")]
      HTLCNotFound(Hash),
      #[error("Invalid secret")]
      InvalidSecret,
      #[error("HTLC expired")]
      HTLCExpired,
      #[error("HTLC not expired (cannot refund)")]
      HTLCNotExpired,
      // ... rest from SPEC-15
  }
  ```

#### Step 1.5: Implement Invariants (SPEC-15 Lines 177-211)
- [ ] **1.5.1** Write property test for `invariant_atomic_swap` (SPEC-15 Lines 180-192)
- [ ] **1.5.2** Implement `invariant_atomic_swap`
- [ ] **1.5.3** Write property test for `invariant_timelock_ordering` (SPEC-15 Lines 194-198)
- [ ] **1.5.4** Implement `invariant_timelock_ordering`
- [ ] **1.5.5** Write property test for `invariant_hashlock_match` (SPEC-15 Lines 200-204)
- [ ] **1.5.6** Implement `invariant_hashlock_match`
- [ ] **1.5.7** Write property test for `invariant_secret_atomic` (SPEC-15 Lines 206-210)
- [ ] **1.5.8** Implement `invariant_secret_atomic`

---

### Phase 2: Algorithm Implementation (TDD)
> **Reference:** System.md Lines 736-739, SPEC-15 Section 5.1

#### Step 2.1: Create Algorithm Module Structure
```bash
mkdir -p src/algorithms
touch src/algorithms/mod.rs src/algorithms/htlc.rs src/algorithms/atomic_swap.rs src/algorithms/proof_verifier.rs src/algorithms/secret.rs
```

#### Step 2.2: Implement Secret Generation (System.md Lines 736, 739)
- [ ] **2.2.1** Write test for `generate_random_secret()` (32 bytes)
- [ ] **2.2.2** Implement `generate_random_secret()` using CSPRNG
- [ ] **2.2.3** Write test for `create_hash_lock()` (SHA-256)
- [ ] **2.2.4** Implement `create_hash_lock()` using sha2 crate
- [ ] **2.2.5** Write test `test_htlc_secret_verification` (SPEC-15 Line 348)
- [ ] **2.2.6** Implement `verify_secret()`

#### Step 2.3: Implement HTLC Logic (System.md Lines 737-738)
- [ ] **2.3.1** Write test `test_htlc_expiry` (SPEC-15 Line 359)
- [ ] **2.3.2** Implement `HTLC::is_expired()`
- [ ] **2.3.3** Write test `test_htlc_claim_before_expiry` (SPEC-15 Line 368)
- [ ] **2.3.4** Implement `HTLC::can_claim()`
- [ ] **2.3.5** Write test for refund after expiry
- [ ] **2.3.6** Implement `HTLC::can_refund()`
- [ ] **2.3.7** Write test for claim verification (SPEC-15 Lines 578-610)
- [ ] **2.3.8** Implement `verify_htlc_claim()`
- [ ] **2.3.9** Write test for refund verification (SPEC-15 Lines 612-633)
- [ ] **2.3.10** Implement `verify_htlc_refund()`

#### Step 2.4: Implement Atomic Swap State Machine (System.md Line 738)
- [ ] **2.4.1** Write test `test_swap_state_transitions` (SPEC-15 Line 387)
- [ ] **2.4.2** Implement `AtomicSwap::transition()`
- [ ] **2.4.3** Write test `test_timelock_ordering` (SPEC-15 Line 379)
- [ ] **2.4.4** Implement `validate_timelock_ordering()`

#### Step 2.5: Implement Proof Verification (SPEC-15 Lines 403-418)
- [ ] **2.5.1** Write test `test_merkle_proof_verification` (SPEC-15 Line 405)
- [ ] **2.5.2** Implement `verify_merkle_proof_external()`
- [ ] **2.5.3** Write test `test_invalid_merkle_proof` (SPEC-15 Line 412)
- [ ] **2.5.4** Verify tampered proofs rejected

---

### Phase 3: Chain-Specific Adapters
> **Reference:** SPEC-15 Appendix B.3 (Lines 636-662)

#### Step 3.1: Create Chains Module Structure
```bash
mkdir -p src/chains
touch src/chains/mod.rs src/chains/ethereum.rs src/chains/bitcoin.rs src/chains/finality.rs
```

#### Step 3.2: Define Chain-Specific Finality (SPEC-15 Lines 650-654)
- [ ] **3.2.1** Create `get_required_confirmations(ChainId) -> u64`
- [ ] **3.2.2** Bitcoin: 6 confirmations
- [ ] **3.2.3** Ethereum: 12 confirmations
- [ ] **3.2.4** Polygon: 128 confirmations
- [ ] **3.2.5** Arbitrum: 1 confirmation

#### Step 3.3: Implement Chain Client Stubs
- [ ] **3.3.1** Create `EthereumClient` struct (stub)
- [ ] **3.3.2** Create `BitcoinClient` struct (stub)
- [ ] **3.3.3** Implement `ExternalChainClient` trait for each

---

### Phase 4: Ports & Adapters
> **Reference:** SPEC-15 Section 3 (Lines 215-291), Architecture.md Section 2.2

#### Step 4.1: Create Port Module Structure
```bash
mkdir -p src/ports
touch src/ports/mod.rs src/ports/inbound.rs src/ports/outbound.rs
```

#### Step 4.2: Define Inbound Ports (SPEC-15 Lines 219-250)
- [ ] **4.2.1** Create `CrossChainApi` trait
  ```rust
  #[async_trait]
  pub trait CrossChainApi: Send + Sync {
      async fn initiate_swap(&self, ...) -> Result<AtomicSwap, CrossChainError>;
      async fn lock_source(&self, swap_id: Hash, secret_hash: Hash) -> Result<HTLC, CrossChainError>;
      async fn lock_target(&self, swap_id: Hash, proof: CrossChainProof) -> Result<HTLC, CrossChainError>;
      async fn claim(&self, htlc_id: Hash, secret: [u8; 32]) -> Result<(), CrossChainError>;
      async fn refund(&self, htlc_id: Hash) -> Result<(), CrossChainError>;
      async fn relay_message(&self, message: CrossChainMessage) -> Result<(), CrossChainError>;
      async fn verify_proof(&self, proof: CrossChainProof, chain: ChainId) -> Result<bool, CrossChainError>;
  }
  ```

#### Step 4.3: Define Outbound Ports (SPEC-15 Lines 253-291)
- [ ] **4.3.1** Create `HTLCContract` trait (SPEC-15 Lines 257-270)
- [ ] **4.3.2** Create `ExternalChainClient` trait (SPEC-15 Lines 273-283)
- [ ] **4.3.3** Create `FinalityChecker` trait (SPEC-15 Lines 286-290)
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
- [ ] **5.2.1** Create `CrossChainService` struct
- [ ] **5.2.2** Implement `CrossChainApi` trait
- [ ] **5.2.3** Implement `initiate_swap()` with secret generation
- [ ] **5.2.4** Implement `lock_source()` with HTLC deployment
- [ ] **5.2.5** Implement `lock_target()` with proof verification + timelock check
- [ ] **5.2.6** Implement `claim()` with secret verification
- [ ] **5.2.7** Implement `refund()` with expiry verification

---

### Phase 6: Integration & Benchmarks
> **Reference:** SPEC-15 Section 5.2 (Lines 424-493)

#### Step 6.1: Create Integration Tests
```bash
mkdir -p tests/integration
touch tests/integration/atomic_swap_tests.rs
```

- [ ] **6.1.1** Write `test_complete_atomic_swap` (SPEC-15 Line 429)
- [ ] **6.1.2** Write `test_swap_timeout_refund` (SPEC-15 Line 469)
- [ ] **6.1.3** Write `test_secret_extraction_from_claim`
- [ ] **6.1.4** Write `test_insufficient_finality_blocks_lock`

#### Step 6.2: Create Benchmarks
```bash
mkdir -p benches
touch benches/cross_chain_bench.rs
```

- [ ] **6.2.1** Add criterion benchmark for secret operations
- [ ] **6.2.2** Add criterion benchmark for HTLC operations
- [ ] **6.2.3** Verify <1ms for secret generation
- [ ] **6.2.4** Verify <1µs for SHA-256 hash

---

### Phase 7: Security Hardening
> **Reference:** System.md Lines 751-756, SPEC-15 Appendix B

- [ ] **7.1** Enforce timelock margin (6+ hours) - System.md Line 752
- [ ] **7.2** Use only SHA-256 for hashlocks - System.md Line 753
- [ ] **7.3** Implement timeout monitoring service - System.md Line 754
- [ ] **7.4** Support multiple relay paths - System.md Line 755
- [ ] **7.5** Add chain-specific finality checks - SPEC-15 Lines 650-654
- [ ] **7.6** Zeroize secrets after use
- [ ] **7.7** Add security-specific tests

---

## 🔧 VERIFICATION COMMANDS

```bash
# Build check
cargo build -p qc-15-cross-chain

# Run tests
cargo test -p qc-15-cross-chain

# Check test coverage
cargo llvm-cov --package qc-15-cross-chain

# Run benchmarks
cargo bench --package qc-15-cross-chain

# Lint check
cargo clippy -p qc-15-cross-chain -- -D warnings

# Format check
cargo fmt -p qc-15-cross-chain -- --check
```

---

## ⚠️ STRICT IMPLEMENTATION CONSTRAINTS

> These constraints are NON-NEGOTIABLE and enforced by the architecture.

| # | Constraint | Reference | Enforcement |
|---|------------|-----------|-------------|
| 1 | **TDD MANDATORY** | Architecture.md 2.4 | Never write implementation without tests first |
| 2 | **SHA-256 ONLY** | System.md Line 753 | Never use MD5, SHA-1, or other weak hashes for hashlocks |
| 3 | **TIMELOCK MARGIN** | System.md Line 752 | Source timeout > Target timeout + 6 hours |
| 4 | **CHAIN FINALITY** | SPEC-15 Lines 650-654 | Wait for chain-specific confirmations before accepting proofs |
| 5 | **ATOMIC INVARIANT** | SPEC-15 Lines 180-192 | Both HTLCs claim OR both refund, never partial |
| 6 | **SECRET ZEROIZE** | Security best practice | Clear secret from memory after use |
| 7 | **PROOF VERIFICATION** | SPEC-15 Lines 636-662 | Never trust external chain data without cryptographic verification |
| 8 | **ENVELOPE IDENTITY** | Architecture.md 3.2.1 | Use `msg.sender_id`, never payload fields |

---

**END OF TODO**
