# TODO: Transaction Ordering Subsystem (qc-12)

**Generated:** 2025-12-10  
**Spec Reference:** [SPEC-12-TRANSACTION-ORDERING.md](file:///home/chef/Github/Quantum-Chain/SPECS/SPEC-12-TRANSACTION-ORDERING.md)

---

## 📚 MASTER DOCUMENT REFERENCES

| Document | Path | Relevance |
|----------|------|-----------|
| **System.md** | [Documentation/System.md](file:///home/chef/Github/Quantum-Chain/Documentation/System.md) | Subsystem 12 definition (lines 584-617): Kahn's Algorithm, dependency types, security defenses |
| **Architecture.md** | [Documentation/Architecture.md](file:///home/chef/Github/Quantum-Chain/Documentation/Architecture.md) | Section 3.2.1: Envelope-Only Identity; Section 3.5: Time-Bounded Replay Prevention |
| **IPC-MATRIX.md** | [Documentation/IPC-MATRIX.md](file:///home/chef/Github/Quantum-Chain/Documentation/IPC-MATRIX.md) | Subsystem 12 Security Boundaries: Only accepts from Consensus (8) |

### Key References by Topic

| Topic | Document | Section/Line |
|-------|----------|--------------|
| Kahn's Algorithm | System.md | Lines 587-594 |
| Dependency Types (RAW, WAW, Nonce) | System.md | Lines 591-594 |
| Conflict Resolution | System.md | Lines 593 |
| Subsystem Dependencies | System.md | Lines 597-598 |
| Security Defenses | System.md | Lines 600-616 |
| Envelope-Only Identity | Architecture.md | Section 3.2.1 (Lines 387-449) |
| Time-Bounded Nonce | Architecture.md | Section 3.5 (Lines 650-762) |
| Request/Response Pattern | Architecture.md | Section 3.3 (Lines 451-601) |
| Domain Model | SPEC-12 | Section 2.1 (Lines 54-141) |
| Invariants | SPEC-12 | Section 2.2 (Lines 144-193) |
| Ports Definition | SPEC-12 | Section 3 (Lines 197-271) |
| Event Schema | SPEC-12 | Section 4 (Lines 275-306) |
| TDD Strategy | SPEC-12 | Section 5 (Lines 310-506) |
| Error Handling | SPEC-12 | Section 6 (Lines 511-533) |

---

## 🎯 OBJECTIVE

> **System.md Lines 584-586:**
> "SUBSYSTEM 12: TRANSACTION ORDERING (DAG-based)
> Purpose: Order transactions correctly for parallel execution in DAG chains"

Implement DAG-based transaction ordering using Kahn's topological sort algorithm to enable parallel transaction execution while maintaining correctness for conflicting transactions.

---

## 🏛️ TEAMS AUDIT

### 1️⃣ MASTER ARCHITECT
> *"Structure the crate for long-term maintainability following DDD + Hexagonal + TDD"*
> 
> **Reference:** Architecture.md Section 2 (Lines 56-284)

#### How to Implement:

```
crates/qc-12-transaction-ordering/
├── Cargo.toml                    # Dependencies: thiserror, async-trait, serde, shared-types
├── src/
│   ├── lib.rs                    # Re-exports, feature flags
│   ├── domain/
│   │   ├── mod.rs
│   │   ├── entities.rs           # SPEC-12 Section 2.1: AnnotatedTransaction, Dependency, DependencyGraph
│   │   ├── value_objects.rs      # SPEC-12 Section 2.1: DependencyKind, AccessPattern
│   │   ├── invariants.rs         # SPEC-12 Section 2.2: Topological order, no-cycles, parallel safety
│   │   └── errors.rs             # SPEC-12 Section 6: OrderingError enum
│   ├── ports/
│   │   ├── mod.rs
│   │   ├── inbound.rs            # SPEC-12 Section 3.1: TransactionOrderingApi trait
│   │   └── outbound.rs           # SPEC-12 Section 3.2: AccessPatternAnalyzer, ConflictDetector
│   ├── application/
│   │   ├── mod.rs
│   │   └── service.rs            # TransactionOrderingService
│   ├── algorithms/
│   │   ├── mod.rs
│   │   ├── kahns.rs              # System.md Lines 587-588: Kahn's topological sort
│   │   ├── dependency_builder.rs # System.md Lines 591: Dependency Graph Construction
│   │   └── conflict_detector.rs  # System.md Lines 593: Conflict Resolution
│   └── config.rs                 # SPEC-12 Section 7: OrderingConfig
└── tests/
    ├── unit/
    │   ├── kahns_tests.rs
    │   ├── dependency_tests.rs
    │   └── invariant_tests.rs
    └── integration/
        └── ordering_service_tests.rs
```

#### Expected Output:
- Clear separation: Domain (pure), Ports (traits), Application (orchestration), Algorithms (pure functions)
- Zero I/O in domain and algorithms - all async through ports
- Dependency inversion via traits

#### Debugging When Errors:
| Error | Likely Cause | Fix |
|-------|--------------|-----|
| Circular dependency in `use` | Wrong module structure | Check `mod.rs` exports match file structure |
| "trait not implemented" | Missing adapter | Ensure ports have mock implementations for tests |
| "cannot find type" | Missing re-export | Add `pub use` in parent `mod.rs` |

---

### 2️⃣ CYBERSECURITY ZERO-DAY EXPERT
> *"Identify attack vectors before they become exploits"*
>
> **Reference:** System.md Lines 600-616 (Security & Robustness), Architecture.md Section 3.2.1

#### Security Analysis:

| Attack Vector | Risk Level | Mitigation | Reference |
|---------------|------------|------------|-----------|
| **Complexity DoS** | 🔴 HIGH | Limit edge count | System.md Line 608 |
| **Cycle Injection** | 🔴 HIGH | Kahn's detects cycles | System.md Line 609 |
| **Dependency Manipulation** | 🟡 MEDIUM | Verify access patterns from trusted source | System.md Line 607 |
| **Sender Spoofing** | 🔴 HIGH | Envelope-Only Identity | Architecture.md 3.2.1 |
| **Memory Exhaustion** | 🟡 MEDIUM | Cap batch size | SPEC-12 Lines 127-130 |
| **Replay Attack** | 🔴 HIGH | Nonce + timestamp | Architecture.md 3.5 |

#### How to Implement Security:

```rust
/// MANDATORY: Validate sender per IPC-MATRIX.md Subsystem 12
/// Reference: Architecture.md Section 3.2.1 (Envelope-Only Identity)
fn validate_request(msg: &AuthenticatedMessage<OrderTransactionsRequest>) -> Result<(), OrderingError> {
    // 1. ENVELOPE IDENTITY ONLY (Architecture.md 3.2.1, Lines 397-403)
    //    "The sender_id in the AuthenticatedMessage envelope is the ONLY source of truth"
    if msg.sender_id != SubsystemId::Consensus {
        return Err(OrderingError::UnauthorizedSender(msg.sender_id));
    }
    
    // 2. TIMESTAMP WINDOW (Architecture.md 3.5, Lines 784-800)
    //    "Allow 10s clock skew into future, 60s into past"
    let now = current_timestamp();
    if msg.timestamp < now.saturating_sub(60) || msg.timestamp > now.saturating_add(10) {
        return Err(OrderingError::StaleMessage);
    }
    
    // 3. NONCE REPLAY PREVENTION (Architecture.md Lines 726-762)
    if !nonce_cache.check_and_add(msg.nonce, msg.timestamp)? {
        return Err(OrderingError::ReplayDetected);
    }
    
    // 4. COMPLEXITY LIMIT (System.md Line 608: anti-DoS)
    //    "Reject dependency graphs with >10,000 edges (complexity attack)"
    if msg.payload.transactions.len() > config.max_batch_size {
        return Err(OrderingError::BatchTooLarge { 
            size: msg.payload.transactions.len(), 
            max: config.max_batch_size 
        });
    }
    
    Ok(())
}
```

#### Expected Output:
- All requests validated before processing
- Logging uses `msg.sender_id` (envelope), never payload identity
- Metrics track rejection reasons for forensics

#### Debugging When Errors:
| Error | Likely Cause | Fix |
|-------|--------------|-----|
| `UnauthorizedSender` in tests | Mock not setting sender_id | Set `sender_id: SubsystemId::Consensus` in test message |
| `ReplayDetected` false positive | Clock skew in tests | Use `#[cfg(test)]` to inject mock time |
| Memory keeps growing | Nonce cache not GC'd | Ensure `garbage_collect()` runs every 10s |

---

### 3️⃣ SCALABILITY & MAINTAINABILITY EXPERT
> *"DDD, EDA, TDD, Hexagonal - design for 10x growth"*
>
> **Reference:** Architecture.md Sections 2.1-2.4 (Lines 58-281)

#### Scalability Patterns:

**1. Event-Driven Integration (EDA)**
> Reference: Architecture.md Section 2.3 (Lines 162-225)
```
┌─────────────────┐        ┌─────────────────────┐        ┌──────────────────┐
│ Consensus (8)   │ ──────→│  Transaction        │ ──────→│ Smart Contracts  │
│                 │ Request│  Ordering (12)      │ Result │     (11)         │
└─────────────────┘        └─────────────────────┘        └──────────────────┘
         │                          │                              
         │                          ↓ Query                        
         │                  ┌─────────────────────┐                
         │                  │ State Management    │                
         │                  │       (4)           │                
         │                  └─────────────────────┘                
```

**2. Parallelism Scaling**
- Each `ParallelGroup` can be executed concurrently
- Use work-stealing scheduler (rayon) for CPU-bound graph analysis
- Cap parallelism to avoid context-switch overhead

**3. Memory Efficiency**
```rust
// Use Arc for transaction sharing between graph nodes
pub struct DependencyGraph {
    transactions: HashMap<Hash, Arc<AnnotatedTransaction>>,  // Shared ownership
    edges: Vec<Dependency>,
    adjacency: HashMap<Hash, Vec<Hash>>,
    in_degree: HashMap<Hash, usize>,
}
```

#### Expected Output:
- Service handles 1000 tx/batch in <100ms
- Memory usage O(n) with transaction count
- Clear async boundaries for I/O

#### Debugging When Errors:
| Error | Likely Cause | Fix |
|-------|--------------|-----|
| Slow performance on large batches | O(n²) conflict detection | Use hash-based lookup for access patterns |
| Deadlock in async code | Blocking in async context | Use `tokio::spawn_blocking` for CPU work |
| Memory leak | Arc cycle | Ensure no circular references in graph |

---

### 4️⃣ QA EXPERT
> *"Test coverage is non-negotiable"*
>
> **Reference:** SPEC-12 Section 5 (Lines 310-506), Architecture.md Section 2.4 (Lines 227-281)

#### Test Strategy:

| Test Type | Location | Coverage Target | Reference |
|-----------|----------|-----------------|-----------|
| Unit Tests | `src/algorithms/*.rs` | 100% of Kahn's algorithm | SPEC-12 Lines 314-437 |
| Unit Tests | `src/domain/*.rs` | 100% of invariants | SPEC-12 Lines 144-193 |
| Integration | `tests/integration/` | Happy path + error cases | SPEC-12 Lines 440-506 |
| Property Tests | `tests/property/` | Invariant fuzzing | SPEC-12 Lines 147-192 |
| Benchmark | `benches/` | Performance regression | System.md Line 614 |

#### Critical Test Cases (from SPEC-12):

```rust
// === MUST-HAVE UNIT TESTS (SPEC-12 Lines 319-436) ===

#[test]
fn test_kahns_simple_chain() { /* A → B → C */ }       // Line 368

#[test]
fn test_kahns_diamond_graph() { /* A → B,C → D */ }    // Line 394

#[test]
fn test_kahns_fully_parallel() { /* A, B, C independent */ }  // Line 381

#[test]
fn test_cycle_detected_and_rejected() { /* A → B → C → A */ } // Line 411

#[test]
fn test_read_after_write_creates_dependency() { }      // Line 322

#[test]
fn test_write_after_write_creates_dependency() { }     // Line 332

#[test]
fn test_nonce_ordering_enforced() { }                  // Line 354

#[test]
fn test_independent_txs_no_dependency() { }            // Line 343

// === SECURITY TESTS (SPEC-12 Lines 573-598) ===

#[test]
fn test_reject_unauthorized_sender() { }               // Line 579

#[test]
fn test_reject_oversized_batch() { }                   // Line 590
```

#### Expected Output:
- `cargo test` passes with 0 failures
- `cargo llvm-cov` shows >90% line coverage
- All invariants have property-based tests

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
# Edit src/algorithms/kahns.rs

# Phase 3: REFACTOR - Clean up
cargo fmt && cargo clippy -- -D warnings

# Verify
cargo test
```

#### Expected Output Per Phase:

**Phase 1 (RED):**
```
---- algorithms::kahns_tests::test_kahns_simple_chain stdout ----
thread 'algorithms::kahns_tests::test_kahns_simple_chain' panicked at 'not yet implemented'
```

**Phase 2 (GREEN):**
```
running 1 test
test algorithms::kahns_tests::test_kahns_simple_chain ... ok
```

---

### 6️⃣ ALGORITHMIC PRECISION EXPERT
> *"Kahn's Algorithm must be correct, optimal, and deterministic"*
>
> **Reference:** System.md Lines 587-588: "Topological Sort (Kahn's Algorithm) - O(V + E) complexity, detects cycles, enables parallelism"

#### Kahn's Algorithm Pseudocode:

> Reference: System.md Lines 591-594

```
FUNCTION kahns_topological_sort(graph):
    // 1. Calculate in-degree for all nodes
    in_degree = {}
    FOR each node in graph.nodes:
        in_degree[node] = count of incoming edges to node
    
    // 2. Initialize queue with zero in-degree nodes
    queue = []
    FOR each node where in_degree[node] == 0:
        queue.push(node)
    
    // 3. Sort by deterministic key (hash) for stability
    //    Reference: System.md Line 608 "Deterministic Ordering"
    queue.sort_by(|a, b| a.hash.cmp(b.hash))
    
    // 4. Process queue, building parallel groups
    //    Reference: System.md Line 594 "Parallel Execution Scheduling"
    result = []
    current_group = []
    
    WHILE queue is not empty:
        current_group = queue.drain()
        result.push(ParallelGroup { transactions: current_group })
        
        next_queue = []
        FOR each node in current_group:
            FOR each neighbor in graph.adjacency[node]:
                in_degree[neighbor] -= 1
                IF in_degree[neighbor] == 0:
                    next_queue.push(neighbor)
        
        queue = next_queue.sort_by(hash)
    
    // 5. Cycle detection (System.md Line 609)
    IF result.total_count < graph.nodes.count:
        RETURN Error(CycleDetected)
    
    RETURN result
```

#### Expected Output:
- O(V + E) time complexity
- Deterministic output for same input
- Cycle detection as error

---

### 7️⃣ BENCHMARK EXPERT
> *"Measure, don't guess"*
>
> **Reference:** System.md Lines 614-616 (Robustness Measures)

#### Benchmark Target:

> System.md Line 614: "Fallback to sequential execution if conflicts exceed threshold"

- **Target:** <100ms for 1000 transactions
- **Memory:** O(n) with transaction count

---

## 📋 IMPLEMENTATION CHECKLIST (DETAILED)

### Phase 1: Domain Setup (TDD)
> **Reference:** SPEC-12 Section 2 (Lines 52-193), Architecture.md Section 2.1 (Lines 58-103)

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

#### Step 1.2: Implement Value Objects (SPEC-12 Lines 81-141)
- [ ] **1.2.1** Create `DependencyKind` enum (SPEC-12 Lines 81-89)
  ```rust
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum DependencyKind {
      ReadAfterWrite,   // To reads what From writes
      WriteAfterWrite,  // Both write to same location
      NonceOrder,       // Same sender (nonce ordering)
  }
  ```
- [ ] **1.2.2** Create `AccessPattern` struct (SPEC-12 Lines 254-261)
- [ ] **1.2.3** Write unit tests FIRST for value objects
- [ ] **1.2.4** Implement value objects to pass tests

#### Step 1.3: Implement Entities (SPEC-12 Lines 57-121)
- [ ] **1.3.1** Write test for `AnnotatedTransaction` (SPEC-12 Lines 57-68)
- [ ] **1.3.2** Implement `AnnotatedTransaction`
- [ ] **1.3.3** Write test for `Dependency` (SPEC-12 Lines 71-79)
- [ ] **1.3.4** Implement `Dependency`
- [ ] **1.3.5** Write test for `DependencyGraph` (SPEC-12 Lines 92-102)
- [ ] **1.3.6** Implement `DependencyGraph` with methods: `add_node()`, `add_edge()`, `has_edge()`
- [ ] **1.3.7** Write test for `ExecutionSchedule` (SPEC-12 Lines 105-113)
- [ ] **1.3.8** Implement `ExecutionSchedule`
- [ ] **1.3.9** Write test for `ParallelGroup` (SPEC-12 Lines 116-120)
- [ ] **1.3.10** Implement `ParallelGroup`

#### Step 1.4: Implement Errors (SPEC-12 Lines 511-533)
- [ ] **1.4.1** Create `OrderingError` enum with all variants
  ```rust
  #[derive(Debug, thiserror::Error)]
  pub enum OrderingError {
      #[error("Cycle detected in dependency graph")]
      CycleDetected,
      #[error("Batch size exceeded: {size} > {max}")]
      BatchTooLarge { size: usize, max: usize },
      #[error("Access pattern analysis failed: {0}")]
      AnalysisFailed(String),
      #[error("Unauthorized sender: {0:?}")]
      UnauthorizedSender(SubsystemId),
      // ... rest from SPEC-12
  }
  ```

#### Step 1.5: Implement Invariants (SPEC-12 Lines 144-193)
- [ ] **1.5.1** Write property test for `invariant_topological_order` (SPEC-12 Lines 148-162)
- [ ] **1.5.2** Implement `invariant_topological_order`
- [ ] **1.5.3** Write property test for `invariant_no_cycles` (SPEC-12 Lines 165-170)
- [ ] **1.5.4** Implement `invariant_no_cycles`
- [ ] **1.5.5** Write property test for `invariant_parallel_safety` (SPEC-12 Lines 173-192)
- [ ] **1.5.6** Implement `invariant_parallel_safety`

---

### Phase 2: Algorithm Implementation (TDD)
> **Reference:** System.md Lines 587-594, SPEC-12 Section 5.1 (Lines 314-437)

#### Step 2.1: Create Algorithm Module Structure
```bash
mkdir -p src/algorithms
touch src/algorithms/mod.rs src/algorithms/kahns.rs src/algorithms/dependency_builder.rs src/algorithms/conflict_detector.rs
```

#### Step 2.2: Implement Kahn's Algorithm (System.md Lines 587-588)
- [ ] **2.2.1** Write test `test_kahns_simple_chain` (SPEC-12 Line 368)
  - Input: Graph A → B → C
  - Expected: 3 groups, each with 1 tx, order [A], [B], [C]
- [ ] **2.2.2** Implement skeleton `kahns_topological_sort()` returning `todo!()`
- [ ] **2.2.3** Run test, verify RED
- [ ] **2.2.4** Implement in-degree calculation
- [ ] **2.2.5** Implement zero-degree queue initialization
- [ ] **2.2.6** Implement main loop with parallel groups
- [ ] **2.2.7** Run test, verify GREEN

- [ ] **2.2.8** Write test `test_kahns_diamond_graph` (SPEC-12 Line 394)
  - Input: A → B,C → D
  - Expected: 3 groups: [A], [B,C], [D]
- [ ] **2.2.9** Verify existing implementation passes (should pass)

- [ ] **2.2.10** Write test `test_kahns_fully_parallel` (SPEC-12 Line 381)
  - Input: A, B, C independent
  - Expected: 1 group with 3 txs, max_parallelism = 3
- [ ] **2.2.11** Verify passes

- [ ] **2.2.12** Write test `test_cycle_detected` (SPEC-12 Line 411)
  - Input: A → B → C → A (cycle)
  - Expected: `Err(OrderingError::CycleDetected)`
- [ ] **2.2.13** Implement cycle detection (check if scheduled < total)
- [ ] **2.2.14** Verify GREEN

- [ ] **2.2.15** Write test for deterministic output
  - Same input twice should produce identical output
- [ ] **2.2.16** Add `queue.sort()` for determinism
- [ ] **2.2.17** Verify GREEN

#### Step 2.3: Implement Dependency Builder (System.md Line 591)
- [ ] **2.3.1** Write test `test_read_after_write_dependency` (SPEC-12 Line 322)
- [ ] **2.3.2** Implement `build_dependency_graph()`
- [ ] **2.3.3** Write test `test_write_after_write_dependency` (SPEC-12 Line 332)
- [ ] **2.3.4** Extend implementation for WAW
- [ ] **2.3.5** Write test `test_nonce_ordering` (SPEC-12 Line 354)
- [ ] **2.3.6** Extend implementation for nonce ordering
- [ ] **2.3.7** Write test `test_no_dependency_independent` (SPEC-12 Line 343)
- [ ] **2.3.8** Verify passes

#### Step 2.4: Implement Conflict Detector (System.md Line 593)
- [ ] **2.4.1** Write test for conflict detection between two transactions
- [ ] **2.4.2** Implement `detect_conflicts()` using hashsets for access patterns
- [ ] **2.4.3** Write test for nonce conflict
- [ ] **2.4.4** Extend implementation

---

### Phase 3: Ports & Adapters
> **Reference:** SPEC-12 Section 3 (Lines 197-271), Architecture.md Section 2.2 (Lines 106-158)

#### Step 3.1: Create Port Module Structure
```bash
mkdir -p src/ports
touch src/ports/mod.rs src/ports/inbound.rs src/ports/outbound.rs
```

#### Step 3.2: Define Inbound Ports (SPEC-12 Lines 201-228)
- [ ] **3.2.1** Create `TransactionOrderingApi` trait
  ```rust
  #[async_trait]
  pub trait TransactionOrderingApi: Send + Sync {
      async fn order_transactions(&self, transactions: Vec<SignedTransaction>) -> Result<ExecutionSchedule, OrderingError>;
      async fn build_dependency_graph(&self, transactions: Vec<AnnotatedTransaction>) -> Result<DependencyGraph, OrderingError>;
      fn schedule_parallel_execution(&self, graph: &DependencyGraph) -> Result<ExecutionSchedule, OrderingError>;
      async fn annotate_transactions(&self, transactions: Vec<SignedTransaction>) -> Result<Vec<AnnotatedTransaction>, OrderingError>;
  }
  ```

#### Step 3.3: Define Outbound Ports (SPEC-12 Lines 233-270)
- [ ] **3.3.1** Create `AccessPatternAnalyzer` trait (SPEC-12 Lines 235-242)
- [ ] **3.3.2** Create `ConflictDetector` trait (SPEC-12 Lines 245-251)
- [ ] **3.3.3** Create mock implementations for testing

---

### Phase 4: Application Service
> **Reference:** Architecture.md Section 2.1 (Lines 58-103)

#### Step 4.1: Create Application Module
```bash
mkdir -p src/application
touch src/application/mod.rs src/application/service.rs
```

#### Step 4.2: Implement Service
- [ ] **4.2.1** Create `TransactionOrderingService` struct
- [ ] **4.2.2** Implement `TransactionOrderingApi` trait
- [ ] **4.2.3** Wire domain algorithms through ports
- [ ] **4.2.4** Add security validation (see Security Expert section)

---

### Phase 5: Integration & Benchmarks
> **Reference:** SPEC-12 Section 5.2 (Lines 440-506)

#### Step 5.1: Create Integration Tests
```bash
mkdir -p tests/integration
touch tests/integration/ordering_service_tests.rs
```

- [ ] **5.1.1** Write `test_order_transactions_with_state_analysis` (SPEC-12 Line 448)
- [ ] **5.1.2** Write `test_high_conflict_fallback_to_sequential` (SPEC-12 Line 465)
- [ ] **5.1.3** Write `test_correct_execution_order_verified` (SPEC-12 Line 485)

#### Step 5.2: Create Benchmarks
```bash
mkdir -p benches
touch benches/ordering_bench.rs
```

- [ ] **5.2.1** Add criterion benchmark for chain graph
- [ ] **5.2.2** Add criterion benchmark for parallel graph
- [ ] **5.2.3** Add criterion benchmark for random DAG
- [ ] **5.2.4** Verify <100ms for 1000 tx

---

### Phase 6: Security Hardening
> **Reference:** Architecture.md Section 3.2.1 (Lines 387-449), IPC-MATRIX.md Subsystem 12

- [ ] **6.1** Implement sender validation (Subsystem 8 only) - IPC-MATRIX.md
- [ ] **6.2** Implement timestamp validation (±60s window) - Architecture.md 3.5
- [ ] **6.3** Implement nonce replay prevention - Architecture.md Lines 726-762
- [ ] **6.4** Implement batch size limits - SPEC-12 Line 127
- [ ] **6.5** Add security-specific tests (SPEC-12 Lines 573-598)
- [ ] **6.6** Verify all logging uses `msg.sender_id` (envelope identity)

---

## 🔧 VERIFICATION COMMANDS

```bash
# Build check
cargo build -p qc-12-transaction-ordering

# Run tests
cargo test -p qc-12-transaction-ordering

# Check test coverage
cargo llvm-cov --package qc-12-transaction-ordering

# Run benchmarks
cargo bench --package qc-12-transaction-ordering

# Lint check
cargo clippy -p qc-12-transaction-ordering -- -D warnings

# Format check
cargo fmt -p qc-12-transaction-ordering -- --check
```

---

## ⚠️ STRICT IMPLEMENTATION CONSTRAINTS

> These constraints are NON-NEGOTIABLE and enforced by the architecture.

| # | Constraint | Reference | Enforcement |
|---|------------|-----------|-------------|
| 1 | **TDD MANDATORY** | Architecture.md 2.4 | Never write implementation without tests first |
| 2 | **ENVELOPE IDENTITY** | Architecture.md 3.2.1 | Always use `msg.sender_id`, never `payload.requester_id` |
| 3 | **DETERMINISM** | System.md Line 608 | Always sort queues by hash for reproducible output |
| 4 | **NO BLOCKING** | Architecture.md 2.3 | Use `tokio::spawn_blocking` for CPU-heavy work in async context |
| 5 | **PORTS FIRST** | Architecture.md 2.2 | Define traits before implementations |
| 6 | **COMPLEXITY LIMITS** | SPEC-12 Line 590 | Reject batches > 1000 tx, graphs > 10,000 edges |
| 7 | **AUTHORIZED SENDERS** | IPC-MATRIX.md | Only accept `OrderTransactionsRequest` from Subsystem 8 (Consensus) |
| 8 | **TIME-BOUNDED NONCE** | Architecture.md 3.5 | Implement `TimeBoundedNonceCache` with 120s window |

---

**END OF TODO**
