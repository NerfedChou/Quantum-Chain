# TODO: Light Client Sync Subsystem (qc-13)

**Generated:** 2025-12-10  
**Spec Reference:** [SPEC-13-LIGHT-CLIENT.md](file:///home/chef/Github/Quantum-Chain/SPECS/SPEC-13-LIGHT-CLIENT.md)

---

## 📚 MASTER DOCUMENT REFERENCES

| Document | Path | Relevance |
|----------|------|-----------|
| **System.md** | [Documentation/System.md](file:///home/chef/Github/Quantum-Chain/Documentation/System.md) | Subsystem 13 definition (lines 620-670): SPV, Merkle proofs, multi-node consensus |
| **Architecture.md** | [Documentation/Architecture.md](file:///home/chef/Github/Quantum-Chain/Documentation/Architecture.md) | Section 3.2.1: Envelope-Only Identity; Section 2.2: Hexagonal Architecture |
| **IPC-MATRIX.md** | [Documentation/IPC-MATRIX.md](file:///home/chef/Github/Quantum-Chain/Documentation/IPC-MATRIX.md) | Subsystem 13: Light client communication with Subsystems 1, 3, 7 |

### Key References by Topic

| Topic | Document | Section/Line |
|-------|----------|--------------|
| SPV Algorithm | System.md | Lines 623-624 |
| Header Chain Sync | System.md | Line 627 |
| Merkle Proof Verification | System.md | Line 628 |
| Bloom Filter Setup | System.md | Line 629 |
| Checkpoint Verification | System.md | Line 630 |
| Dependencies (1, 3, 7) | System.md | Lines 632-635 |
| Attack Vectors | System.md | Lines 638-641 |
| Security Defenses | System.md | Lines 643-648 |
| Robustness Measures | System.md | Lines 650-653 |
| Future: Utreexo/ZK Proofs | System.md | Lines 655-669 |
| Domain Model | SPEC-13 | Section 2.1 (Lines 56-124) |
| Invariants | SPEC-13 | Section 2.2 (Lines 127-158) |
| Ports Definition | SPEC-13 | Section 3 (Lines 162-270) |
| Event Schema | SPEC-13 | Section 4 (Lines 274-315) |
| TDD Strategy | SPEC-13 | Section 5 (Lines 319-503) |
| Error Handling | SPEC-13 | Section 6 (Lines 508-533) |
| Multi-Node Consensus | SPEC-13 | Appendix B.2 (Lines 573-618) |
| Privacy Considerations | SPEC-13 | Appendix B.3 (Lines 620-629) |

---

## 🎯 OBJECTIVE

> **System.md Lines 620-624:**
> "SUBSYSTEM 13: LIGHT CLIENT SYNC
> Purpose: Verify blockchain without downloading full chain data
> Main Algorithm: SPV (Simplified Payment Verification)
> Why: Download only headers (~80 bytes/block), verify via Merkle proofs"

Implement SPV-based light client that enables mobile/desktop clients to verify blockchain state without downloading the full chain, using block headers, Merkle proofs, and Bloom filters.

---

## 🏛️ TEAMS AUDIT

### 1️⃣ MASTER ARCHITECT
> *"Structure the crate for long-term maintainability following DDD + Hexagonal + TDD"*
> 
> **Reference:** Architecture.md Section 2 (Lines 56-284)

#### How to Implement:

```
crates/qc-13-light-client-sync/
├── Cargo.toml                    # Dependencies: thiserror, async-trait, serde, tokio, shared-types
├── src/
│   ├── lib.rs                    # Re-exports, feature flags
│   ├── domain/
│   │   ├── mod.rs
│   │   ├── entities.rs           # SPEC-13 Section 2.1: HeaderChain, ProvenTransaction, Checkpoint
│   │   ├── value_objects.rs      # SPEC-13 Section 2.1: CheckpointSource, ChainTip, SyncResult
│   │   ├── invariants.rs         # SPEC-13 Section 2.2: Proof verification, multi-node, checkpoints
│   │   └── errors.rs             # SPEC-13 Section 6: LightClientError enum
│   ├── ports/
│   │   ├── mod.rs
│   │   ├── inbound.rs            # SPEC-13 Section 3.1: LightClientApi trait
│   │   └── outbound.rs           # SPEC-13 Section 3.2: FullNodeConnection, PeerDiscovery, MerkleProofProvider, BloomFilterProvider
│   ├── application/
│   │   ├── mod.rs
│   │   └── service.rs            # LightClientService
│   ├── algorithms/
│   │   ├── mod.rs
│   │   ├── merkle_verifier.rs    # System.md Line 628: Merkle proof verification
│   │   ├── header_sync.rs        # System.md Line 627: Header chain sync
│   │   ├── checkpoint.rs         # System.md Line 630: Checkpoint verification
│   │   └── multi_node.rs         # System.md Line 644: Multi-node consensus
│   └── config.rs                 # SPEC-13 Section 7: LightClientConfig
└── tests/
    ├── unit/
    │   ├── merkle_tests.rs
    │   ├── header_chain_tests.rs
    │   └── checkpoint_tests.rs
    └── integration/
        └── light_client_tests.rs
```

#### Expected Output:
- Clear separation: Domain (pure), Ports (traits), Application (orchestration), Algorithms (pure functions)
- Zero I/O in domain and algorithms - all async through ports
- Dependency inversion via traits for FullNodeConnection, PeerDiscovery, etc.

#### Debugging When Errors:
| Error | Likely Cause | Fix |
|-------|--------------|-----|
| Circular dependency in `use` | Wrong module structure | Check `mod.rs` exports match file structure |
| "trait not implemented" | Missing adapter | Ensure ports have mock implementations for tests |
| "cannot find type" | Missing re-export | Add `pub use` in parent `mod.rs` |

---

### 2️⃣ CYBERSECURITY ZERO-DAY EXPERT
> *"Light clients are high-value targets - protect against malicious full nodes"*
>
> **Reference:** System.md Lines 637-648 (Security & Robustness)

#### Security Analysis:

| Attack Vector | Risk Level | Mitigation | Reference |
|---------------|------------|------------|-----------|
| **Malicious Full Node** | 🔴 HIGH | Multi-node consensus (3+ nodes) | System.md Line 644 |
| **Eclipse Attack** | 🔴 HIGH | Random peer selection from diverse sources | System.md Line 648 |
| **Invalid Merkle Proof** | 🔴 HIGH | Cryptographic verification | System.md Line 645 |
| **Checkpoint Bypass** | 🟡 MEDIUM | Trust hardcoded + multi-source checkpoints | System.md Line 646 |
| **Privacy Leakage** | 🟡 MEDIUM | Bloom filter obfuscation, connection rotation | SPEC-13 Lines 620-629 |
| **Sender Spoofing** | 🔴 HIGH | Envelope-Only Identity | Architecture.md 3.2.1 |

#### How to Implement Security:

```rust
/// Multi-node consensus for light client security
/// Reference: System.md Line 644, SPEC-13 Lines 579-617
async fn verify_with_multi_node_consensus(
    &self,
    request: VerificationRequest,
) -> Result<VerifiedData, LightClientError> {
    // 1. Get multiple full nodes (System.md Line 644: "Query 3+ independent nodes")
    let nodes = self.peer_discovery.get_full_nodes(self.config.min_full_nodes).await?;
    
    if nodes.len() < self.config.min_full_nodes {
        return Err(LightClientError::InsufficientNodes {
            got: nodes.len(),
            required: self.config.min_full_nodes,
        });
    }
    
    // 2. Query all nodes in parallel (Robustness: System.md Line 651)
    let responses: Vec<_> = join_all(
        nodes.iter().map(|node| self.query_node(node, &request))
    ).await;
    
    // 3. Require 2/3 agreement (SPEC-13 Line 606)
    let valid_responses: Vec<_> = responses.iter()
        .filter_map(|r| r.as_ref().ok())
        .collect();
    
    if valid_responses.len() * 3 < nodes.len() * 2 {
        return Err(LightClientError::ConsensusFailed);
    }
    
    // 4. Verify all valid responses match (detect fork attacks)
    if !all_equal(&valid_responses) {
        return Err(LightClientError::ForkDetected);
    }
    
    Ok(valid_responses[0].clone())
}

/// Merkle proof verification - NEVER trust, always verify
/// Reference: System.md Line 645, SPEC-13 Lines 130-138
fn verify_merkle_proof(
    tx_hash: &Hash,
    proof_path: &[Hash],
    merkle_root: &Hash,
) -> bool {
    let mut computed = *tx_hash;
    
    for sibling in proof_path {
        computed = if computed < *sibling {
            hash_pair(&computed, sibling)
        } else {
            hash_pair(sibling, &computed)
        };
    }
    
    computed == *merkle_root
}
```

#### Expected Output:
- All critical data verified by multiple nodes
- Merkle proofs cryptographically verified before trusting
- Checkpoints validated against multiple sources
- Privacy features protect user addresses

#### Debugging When Errors:
| Error | Likely Cause | Fix |
|-------|--------------|-----|
| `InsufficientNodes` | Not enough full nodes available | Lower `min_full_nodes` or wait for network |
| `ConsensusFailed` | Nodes disagree or network partition | Retry with different nodes |
| `InvalidProof` | Tampered or malformed proof | Verify proof structure matches expected format |
| `ForkDetected` | Chain split or attack | Alert user, require manual intervention |

---

### 3️⃣ SCALABILITY & MAINTAINABILITY EXPERT
> *"Design for mobile/IoT with limited resources"*
>
> **Reference:** Architecture.md Sections 2.1-2.4, System.md Lines 655-669 (V2 Architecture)

#### Scalability Patterns:

**1. Minimal Data Footprint**
> Reference: System.md Line 624 "~80 bytes/block"
```rust
/// Light client stores only headers, not full blocks
pub struct HeaderChain {
    headers: HashMap<Hash, BlockHeader>,      // ~80 bytes per header
    by_height: BTreeMap<u64, Hash>,           // Height index
    tip: Hash,                                // Current tip
    height: u64,
    checkpoints: Vec<Checkpoint>,             // Trusted anchors
}

// For 1M blocks: ~80MB headers vs ~500GB full chain
```

**2. Parallel Header Download**
> Reference: System.md Line 651 "Header download parallelization"
```rust
async fn sync_headers_parallel(
    &self,
    from_height: u64,
    batch_size: usize,
) -> Result<Vec<BlockHeader>, LightClientError> {
    let nodes = self.get_diverse_full_nodes(3).await?;
    
    // Download different ranges from different nodes
    let tasks: Vec<_> = nodes.iter().enumerate().map(|(i, node)| {
        let start = from_height + (i as u64 * batch_size as u64);
        self.download_headers(node, start, batch_size)
    }).collect();
    
    let results = join_all(tasks).await;
    self.merge_and_verify(results)
}
```

**3. Proof Caching**
> Reference: System.md Line 653 "Local proof caching"
```rust
pub struct ProofCache {
    /// LRU cache for recently verified proofs
    cache: LruCache<Hash, MerkleProof>,
    /// Cache hit/miss metrics
    hits: AtomicU64,
    misses: AtomicU64,
}
```

**4. Future V2 Preparation**
> Reference: System.md Lines 661-668 (Utreexo, ZK-SNARK)
```rust
/// Trait for future proof systems (Utreexo, ZK-SNARK)
pub trait ProofSystem: Send + Sync {
    type Proof;
    fn verify(&self, proof: &Self::Proof) -> Result<bool, ProofError>;
}
```

#### Expected Output:
- Memory usage: O(n) with header count, not blockchain size
- Network bandwidth: Minimal (headers + proofs only)
- CPU: Light verification suitable for mobile
- Graceful degradation on poor network

#### Debugging When Errors:
| Error | Likely Cause | Fix |
|-------|--------------|-----|
| Out of memory on mobile | Too many headers cached | Implement header pruning, keep only recent |
| Slow sync | Network latency | Use parallel download, try different nodes |
| Battery drain | Too many network requests | Batch requests, implement exponential backoff |

---

### 4️⃣ QA EXPERT
> *"Light clients must be bulletproof - they protect user funds"*
>
> **Reference:** SPEC-13 Section 5 (Lines 319-503)

#### Test Strategy:

| Test Type | Location | Coverage Target | Reference |
|-----------|----------|-----------------|-----------|
| Unit Tests | `src/algorithms/*.rs` | 100% of Merkle verification | SPEC-13 Lines 330-361 |
| Unit Tests | `src/domain/*.rs` | 100% of invariants | SPEC-13 Lines 130-157 |
| Integration | `tests/integration/` | Network scenarios | SPEC-13 Lines 451-503 |
| Property Tests | `tests/property/` | Proof fuzzing | SPEC-13 Lines 130-138 |

#### Critical Test Cases (from SPEC-13):

```rust
// === MUST-HAVE UNIT TESTS (SPEC-13 Lines 328-447) ===

// Merkle Proof Tests (Lines 330-361)
#[test]
fn test_merkle_proof_verification_valid() { }       // Line 331

#[test]
fn test_merkle_proof_verification_invalid() { }     // Line 345

// Header Chain Tests (Lines 363-395)
#[test]
fn test_header_chain_append() { }                   // Line 365

#[test]
fn test_header_chain_fork_handling() { }            // Line 377

// Multi-Node Consensus Tests (Lines 397-428)
#[test]
fn test_multi_node_consensus_agreement() { }         // Line 399

#[test]
fn test_multi_node_consensus_disagreement() { }      // Line 415

// Checkpoint Tests (Lines 430-447)
#[test]
fn test_checkpoint_verification() { }                // Line 433

// === INTEGRATION TESTS (SPEC-13 Lines 451-503) ===

#[tokio::test]
async fn test_sync_headers_from_network() { }        // Line 458

#[tokio::test]
async fn test_get_proven_transaction() { }           // Line 470

#[tokio::test]
async fn test_filtered_transactions() { }            // Line 490
```

#### Expected Output:
- `cargo test` passes with 0 failures
- `cargo llvm-cov` shows >90% line coverage
- All invariants have property-based tests
- Network simulation tests for edge cases

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
# Edit src/algorithms/merkle_verifier.rs

# Phase 3: REFACTOR - Clean up
cargo fmt && cargo clippy -- -D warnings

# Verify
cargo test
```

#### Expected Output Per Phase:

**Phase 1 (RED):**
```
---- algorithms::merkle_verifier::test_merkle_proof_verification_valid stdout ----
thread '...' panicked at 'not yet implemented'
```

**Phase 2 (GREEN):**
```
running 1 test
test algorithms::merkle_verifier::test_merkle_proof_verification_valid ... ok
```

---

### 6️⃣ ALGORITHMIC PRECISION EXPERT
> *"SPV verification must be cryptographically correct"*
>
> **Reference:** System.md Lines 623-630, SPEC-13 Lines 130-138

#### SPV Algorithm Overview:

```
SIMPLIFIED PAYMENT VERIFICATION (SPV):

1. HEADER SYNC
   └── Download block headers (~80 bytes each)
   └── Verify: header.parent_hash == previous_header.hash
   └── Verify: header.difficulty meets target
   └── Build local header chain

2. TRANSACTION VERIFICATION
   └── Request Merkle proof for tx_hash
   └── Verify proof against header.merkle_root
   └── Check confirmations >= required_confirmations

3. BLOOM FILTER USAGE (Privacy)
   └── Create Bloom filter with watched addresses
   └── Add random addresses for obfuscation
   └── Send filter to full nodes
   └── Receive filtered transactions
   └── Verify each with Merkle proof
```

#### Merkle Proof Verification:

```rust
/// Merkle proof verification algorithm
/// Reference: System.md Line 628, SPEC-13 Lines 130-138
/// 
/// Time complexity: O(log n) where n = transaction count
/// Space complexity: O(log n) for proof path
pub fn verify_merkle_proof(
    tx_hash: &Hash,
    proof_path: &[ProofNode],
    expected_root: &Hash,
) -> bool {
    let mut current = *tx_hash;
    
    for node in proof_path {
        current = match node.position {
            Position::Left => hash_concat(&node.hash, &current),
            Position::Right => hash_concat(&current, &node.hash),
        };
    }
    
    current == *expected_root
}

#[derive(Clone, Debug)]
pub struct ProofNode {
    pub hash: Hash,
    pub position: Position,
}

#[derive(Clone, Copy, Debug)]
pub enum Position {
    Left,  // Sibling is on the left
    Right, // Sibling is on the right
}
```

#### Header Chain Validation:

```rust
/// Validate header chain continuity
/// Reference: System.md Line 627
pub fn validate_header_chain(headers: &[BlockHeader]) -> Result<(), ChainError> {
    for window in headers.windows(2) {
        let prev = &window[0];
        let curr = &window[1];
        
        // 1. Parent hash continuity
        if curr.parent_hash != prev.hash() {
            return Err(ChainError::BrokenChain {
                height: curr.height,
                expected: prev.hash(),
                got: curr.parent_hash,
            });
        }
        
        // 2. Height increment
        if curr.height != prev.height + 1 {
            return Err(ChainError::InvalidHeight {
                expected: prev.height + 1,
                got: curr.height,
            });
        }
        
        // 3. Timestamp progression
        if curr.timestamp <= prev.timestamp {
            return Err(ChainError::InvalidTimestamp);
        }
    }
    
    Ok(())
}
```

#### Expected Output:
- Merkle verification: O(log n) time
- Header sync: O(n) where n = headers to sync
- Memory: O(h) where h = chain height (headers only)

---

### 7️⃣ BENCHMARK EXPERT
> *"Light clients must be fast on mobile devices"*
>
> **Reference:** System.md Lines 650-653 (Robustness Measures)

#### Benchmark Targets:

| Operation | Target | Reference |
|-----------|--------|-----------|
| Merkle proof verification | <1ms | System.md Line 628 |
| Header validation (single) | <100µs | System.md Line 627 |
| Sync 1000 headers | <5s | System.md Line 651 |
| Multi-node consensus query | <2s | System.md Line 644 |

#### Benchmark Setup:

```rust
// benches/light_client_bench.rs
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn benchmark_merkle_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("merkle_verification");
    
    for depth in [5, 10, 15, 20].iter() {  // Tree depths
        group.bench_with_input(
            BenchmarkId::new("verify_proof", depth),
            depth,
            |b, &depth| {
                let (proof, root, tx_hash) = create_proof_with_depth(depth);
                b.iter(|| verify_merkle_proof(&tx_hash, &proof, &root))
            },
        );
    }
    
    group.finish();
}

fn benchmark_header_sync(c: &mut Criterion) {
    let mut group = c.benchmark_group("header_sync");
    
    for count in [100, 500, 1000, 2000].iter() {
        group.bench_with_input(
            BenchmarkId::new("validate_chain", count),
            count,
            |b, &count| {
                let headers = create_header_chain(count);
                b.iter(|| validate_header_chain(&headers))
            },
        );
    }
    
    group.finish();
}

criterion_group!(benches, benchmark_merkle_verification, benchmark_header_sync);
criterion_main!(benches);
```

---

## 📋 IMPLEMENTATION CHECKLIST (DETAILED)

### Phase 1: Domain Setup (TDD)
> **Reference:** SPEC-13 Section 2 (Lines 54-158), Architecture.md Section 2.1

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

#### Step 1.2: Implement Value Objects (SPEC-13 Lines 74-124)
- [ ] **1.2.1** Create `CheckpointSource` enum (SPEC-13 Lines 82-87)
  ```rust
  #[derive(Clone, Debug, Serialize, Deserialize)]
  pub enum CheckpointSource {
      Hardcoded,
      MultiNodeConsensus { node_count: usize },
      External { url: String },
  }
  ```
- [ ] **1.2.2** Create `Checkpoint` struct (SPEC-13 Lines 75-80)
- [ ] **1.2.3** Create `ChainTip` struct (SPEC-13 Lines 211-216)
- [ ] **1.2.4** Create `SyncResult` struct (SPEC-13 Lines 201-208)
- [ ] **1.2.5** Create `LightClientConfig` struct (SPEC-13 Lines 99-124)
- [ ] **1.2.6** Write unit tests FIRST for value objects
- [ ] **1.2.7** Implement value objects to pass tests

#### Step 1.3: Implement Entities (SPEC-13 Lines 58-97)
- [ ] **1.3.1** Write test for `HeaderChain` (SPEC-13 Lines 59-72)
- [ ] **1.3.2** Implement `HeaderChain` with methods: `new()`, `append()`, `get_header()`, `get_tip()`
- [ ] **1.3.3** Write test for `ProvenTransaction` (SPEC-13 Lines 89-97)
- [ ] **1.3.4** Implement `ProvenTransaction`
- [ ] **1.3.5** Write test for header chain append (SPEC-13 Line 365)
- [ ] **1.3.6** Implement header append logic
- [ ] **1.3.7** Write test for fork handling (SPEC-13 Line 377)
- [ ] **1.3.8** Implement fork detection/handling

#### Step 1.4: Implement Errors (SPEC-13 Lines 510-533)
- [ ] **1.4.1** Create `LightClientError` enum with all variants
  ```rust
  #[derive(Debug, thiserror::Error)]
  pub enum LightClientError {
      #[error("Not enough full nodes: {got} < {required}")]
      InsufficientNodes { got: usize, required: usize },
      #[error("Multi-node consensus failed")]
      ConsensusFailed,
      #[error("Merkle proof verification failed")]
      InvalidProof,
      #[error("Transaction not found: {0:?}")]
      TransactionNotFound(Hash),
      #[error("Checkpoint mismatch at height {height}")]
      CheckpointMismatch { height: u64 },
      #[error("Header chain fork detected")]
      ForkDetected,
      #[error("Network error: {0}")]
      NetworkError(#[from] NetworkError),
  }
  ```

#### Step 1.5: Implement Invariants (SPEC-13 Lines 127-158)
- [ ] **1.5.1** Write property test for `invariant_proof_verified` (SPEC-13 Lines 130-138)
- [ ] **1.5.2** Implement `invariant_proof_verified`
- [ ] **1.5.3** Write property test for `invariant_multi_node` (SPEC-13 Lines 140-147)
- [ ] **1.5.4** Implement `invariant_multi_node`
- [ ] **1.5.5** Write property test for `invariant_checkpoint_chain` (SPEC-13 Lines 149-157)
- [ ] **1.5.6** Implement `invariant_checkpoint_chain`

---

### Phase 2: Algorithm Implementation (TDD)
> **Reference:** System.md Lines 627-630, SPEC-13 Section 5.1

#### Step 2.1: Create Algorithm Module Structure
```bash
mkdir -p src/algorithms
touch src/algorithms/mod.rs src/algorithms/merkle_verifier.rs src/algorithms/header_sync.rs src/algorithms/checkpoint.rs src/algorithms/multi_node.rs
```

#### Step 2.2: Implement Merkle Proof Verification (System.md Line 628)
- [ ] **2.2.1** Write test `test_merkle_proof_verification_valid` (SPEC-13 Line 331)
  - Input: Valid proof path + tx_hash + root
  - Expected: `true`
- [ ] **2.2.2** Implement skeleton `verify_merkle_proof()` returning `todo!()`
- [ ] **2.2.3** Run test, verify RED
- [ ] **2.2.4** Implement hash concatenation logic
- [ ] **2.2.5** Implement proof path traversal
- [ ] **2.2.6** Run test, verify GREEN

- [ ] **2.2.7** Write test `test_merkle_proof_verification_invalid` (SPEC-13 Line 345)
  - Input: Tampered proof
  - Expected: `false`
- [ ] **2.2.8** Verify existing implementation passes

- [ ] **2.2.9** Write fuzz test for random inputs
- [ ] **2.2.10** Verify no panics on malformed input

#### Step 2.3: Implement Header Sync (System.md Line 627)
- [ ] **2.3.1** Write test `test_header_chain_append` (SPEC-13 Line 365)
- [ ] **2.3.2** Implement `HeaderChain::append()`
- [ ] **2.3.3** Write test for parent hash validation
- [ ] **2.3.4** Implement parent hash check
- [ ] **2.3.5** Write test for height increment validation
- [ ] **2.3.6** Implement height check
- [ ] **2.3.7** Write test for timestamp progression
- [ ] **2.3.8** Implement timestamp check

#### Step 2.4: Implement Checkpoint Verification (System.md Line 630)
- [ ] **2.4.1** Write test `test_checkpoint_verification` (SPEC-13 Line 433)
- [ ] **2.4.2** Implement `verify_against_checkpoints()`
- [ ] **2.4.3** Write test for checkpoint mismatch detection
- [ ] **2.4.4** Implement checkpoint mismatch error

#### Step 2.5: Implement Multi-Node Consensus (System.md Line 644)
- [ ] **2.5.1** Write test `test_multi_node_consensus_agreement` (SPEC-13 Line 399)
  - Input: 3 nodes all agree
  - Expected: Success
- [ ] **2.5.2** Implement `check_multi_node_consensus()`
- [ ] **2.5.3** Write test `test_multi_node_consensus_disagreement` (SPEC-13 Line 415)
  - Input: 1 of 3 nodes disagrees
  - Expected: Error
- [ ] **2.5.4** Implement disagreement detection
- [ ] **2.5.5** Write test for 2/3 threshold
- [ ] **2.5.6** Implement threshold logic

---

### Phase 3: Ports & Adapters
> **Reference:** SPEC-13 Section 3 (Lines 162-270), Architecture.md Section 2.2

#### Step 3.1: Create Port Module Structure
```bash
mkdir -p src/ports
touch src/ports/mod.rs src/ports/inbound.rs src/ports/outbound.rs
```

#### Step 3.2: Define Inbound Ports (SPEC-13 Lines 166-217)
- [ ] **3.2.1** Create `LightClientApi` trait
  ```rust
  #[async_trait]
  pub trait LightClientApi: Send + Sync {
      async fn sync_headers(&mut self) -> Result<SyncResult, LightClientError>;
      async fn get_proven_transaction(&self, tx_hash: Hash) -> Result<ProvenTransaction, LightClientError>;
      async fn verify_transaction(&self, tx_hash: Hash, block_hash: Hash) -> Result<bool, LightClientError>;
      async fn get_filtered_transactions(&self, addresses: &[Address], from: u64, to: u64) -> Result<Vec<ProvenTransaction>, LightClientError>;
      fn get_chain_tip(&self) -> ChainTip;
      fn is_synced(&self) -> bool;
  }
  ```

#### Step 3.3: Define Outbound Ports (SPEC-13 Lines 219-270)
- [ ] **3.3.1** Create `FullNodeConnection` trait (SPEC-13 Lines 223-245)
- [ ] **3.3.2** Create `PeerDiscovery` trait (SPEC-13 Lines 248-252)
- [ ] **3.3.3** Create `MerkleProofProvider` trait (SPEC-13 Lines 255-262)
- [ ] **3.3.4** Create `BloomFilterProvider` trait (SPEC-13 Lines 265-269)
- [ ] **3.3.5** Create mock implementations for all ports

---

### Phase 4: Application Service
> **Reference:** Architecture.md Section 2.1

#### Step 4.1: Create Application Module
```bash
mkdir -p src/application
touch src/application/mod.rs src/application/service.rs
```

#### Step 4.2: Implement Service
- [ ] **4.2.1** Create `LightClientService` struct
- [ ] **4.2.2** Implement `LightClientApi` trait
- [ ] **4.2.3** Implement `sync_headers()` with parallel download
- [ ] **4.2.4** Implement `get_proven_transaction()` with multi-node verification
- [ ] **4.2.5** Implement `verify_transaction()` with Merkle proof
- [ ] **4.2.6** Implement `get_filtered_transactions()` with Bloom filter
- [ ] **4.2.7** Add proof caching for performance

---

### Phase 5: Integration & Benchmarks
> **Reference:** SPEC-13 Section 5.2 (Lines 451-503)

#### Step 5.1: Create Integration Tests
```bash
mkdir -p tests/integration
touch tests/integration/light_client_tests.rs
```

- [ ] **5.1.1** Write `test_sync_headers_from_network` (SPEC-13 Line 458)
- [ ] **5.1.2** Write `test_get_proven_transaction` (SPEC-13 Line 470)
- [ ] **5.1.3** Write `test_filtered_transactions` (SPEC-13 Line 490)
- [ ] **5.1.4** Write `test_malicious_node_detection`
- [ ] **5.1.5** Write `test_checkpoint_enforcement`

#### Step 5.2: Create Benchmarks
```bash
mkdir -p benches
touch benches/light_client_bench.rs
```

- [ ] **5.2.1** Add criterion benchmark for Merkle verification
- [ ] **5.2.2** Add criterion benchmark for header chain validation
- [ ] **5.2.3** Verify <1ms for single proof verification
- [ ] **5.2.4** Verify <5s for 1000 header sync

---

### Phase 6: Security Hardening
> **Reference:** System.md Lines 643-648, SPEC-13 Appendix B

- [ ] **6.1** Implement multi-node consensus (3+ nodes) - System.md Line 644
- [ ] **6.2** Implement random peer selection - System.md Line 648
- [ ] **6.3** Implement checkpoint verification from multiple sources - System.md Line 646
- [ ] **6.4** Add Bloom filter obfuscation (add random addresses) - SPEC-13 Line 627
- [ ] **6.5** Implement connection rotation - SPEC-13 Line 629
- [ ] **6.6** Add tests for each security feature
- [ ] **6.7** Verify all nodes are from diverse sources (IP diversity)

---

## 🔧 VERIFICATION COMMANDS

```bash
# Build check
cargo build -p qc-13-light-client-sync

# Run tests
cargo test -p qc-13-light-client-sync

# Check test coverage
cargo llvm-cov --package qc-13-light-client-sync

# Run benchmarks
cargo bench --package qc-13-light-client-sync

# Lint check
cargo clippy -p qc-13-light-client-sync -- -D warnings

# Format check
cargo fmt -p qc-13-light-client-sync -- --check
```

---

## ⚠️ STRICT IMPLEMENTATION CONSTRAINTS

> These constraints are NON-NEGOTIABLE and enforced by the architecture.

| # | Constraint | Reference | Enforcement |
|---|------------|-----------|-------------|
| 1 | **TDD MANDATORY** | Architecture.md 2.4 | Never write implementation without tests first |
| 2 | **MULTI-NODE VERIFICATION** | System.md Line 644 | Query 3+ independent nodes for critical data |
| 3 | **ALWAYS VERIFY PROOFS** | System.md Line 645 | Cryptographically verify every Merkle proof |
| 4 | **CHECKPOINT ENFORCEMENT** | System.md Line 646 | Reject chains that don't include checkpoints |
| 5 | **PRIVACY MODE** | SPEC-13 Lines 620-629 | Add random addresses to Bloom filters |
| 6 | **DIVERSE PEERS** | System.md Line 648 | Don't rely on single full node |
| 7 | **GRACEFUL DEGRADATION** | System.md Line 652 | Fallback to full sync if SPV fails |
| 8 | **ENVELOPE IDENTITY** | Architecture.md 3.2.1 | Use `msg.sender_id`, never payload fields |

---

**END OF TODO**
