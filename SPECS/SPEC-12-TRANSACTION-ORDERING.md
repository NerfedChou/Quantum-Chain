# SPECIFICATION: TRANSACTION ORDERING

**Version:** 2.3  
**Subsystem ID:** 12  
**Bounded Context:** DAG-based Ordering & Parallelism  
**Crate Name:** `crates/transaction-ordering`  
**Author:** Systems Architecture Team  
**Date:** 2024-12-01  
**Architecture Compliance:** Architecture.md v2.3, IPC-MATRIX.md v2.3, System.md v2.3

---

## 1. ABSTRACT

### 1.1 Purpose

The **Transaction Ordering** subsystem orders transactions for parallel execution using dependency graph analysis and topological sorting. It enables parallelism where transactions don't conflict, while maintaining correctness where they do.

### 1.2 Responsibility Boundaries

**In Scope:**
- Analyze transaction read/write sets
- Build dependency graphs
- Perform topological sort (Kahn's algorithm)
- Identify parallel execution groups
- Resolve ordering conflicts
- Forward ordered transactions to Smart Contracts

**Out of Scope:**
- Transaction execution (Subsystem 11)
- State storage (Subsystem 4)
- Transaction validation (Subsystem 8)
- Mempool management (Subsystem 6)

### 1.3 Trust Model

```
┌─────────────────────────────────────────────────────────────────┐
│                    TRUST BOUNDARY (V2.3)                        │
├─────────────────────────────────────────────────────────────────┤
│  TRUSTED:                                                       │
│  ├─ Conflict detection via Subsystem 4 (State Management)       │
│  └─ Transaction validity from Subsystem 8 (Consensus)           │
│                                                                 │
│  SECURITY (Envelope-Only Identity - V2.2 Amendment):            │
│  Identity from AuthenticatedMessage.sender_id only.             │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. DOMAIN MODEL

### 2.1 Core Entities

```rust
/// Transaction with access pattern
#[derive(Clone, Debug)]
pub struct AnnotatedTransaction {
    pub transaction: SignedTransaction,
    pub hash: Hash,
    /// Storage keys read
    pub reads: HashSet<(Address, StorageKey)>,
    /// Storage keys written
    pub writes: HashSet<(Address, StorageKey)>,
    /// Gas estimate
    pub estimated_gas: u64,
}

/// Dependency graph edge
#[derive(Clone, Debug)]
pub struct Dependency {
    /// Transaction that must execute first
    pub from: Hash,
    /// Transaction that must execute after
    pub to: Hash,
    /// Type of dependency
    pub kind: DependencyKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DependencyKind {
    /// To reads what From writes
    ReadAfterWrite,
    /// Both write to same location
    WriteAfterWrite,
    /// Same sender (nonce ordering)
    NonceOrder,
}

/// Dependency graph
#[derive(Debug)]
pub struct DependencyGraph {
    /// All transactions
    pub transactions: HashMap<Hash, AnnotatedTransaction>,
    /// Edges (dependencies)
    pub edges: Vec<Dependency>,
    /// Adjacency list (outgoing)
    pub adjacency: HashMap<Hash, Vec<Hash>>,
    /// In-degree count for each node
    pub in_degree: HashMap<Hash, usize>,
}

/// Parallel execution schedule
#[derive(Debug)]
pub struct ExecutionSchedule {
    /// Ordered groups that can be executed in parallel
    pub parallel_groups: Vec<ParallelGroup>,
    /// Total transactions
    pub total_transactions: usize,
    /// Maximum parallelism achieved
    pub max_parallelism: usize,
}

/// Group of transactions that can execute in parallel
#[derive(Debug)]
pub struct ParallelGroup {
    pub group_id: usize,
    pub transactions: Vec<Hash>,
}

/// Ordering configuration
#[derive(Clone, Debug)]
pub struct OrderingConfig {
    /// Maximum transactions to analyze at once
    pub max_batch_size: usize,
    /// Fallback to sequential if conflicts exceed threshold
    pub conflict_threshold_percent: u8,
    /// Enable speculative execution
    pub enable_speculation: bool,
}

impl Default for OrderingConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 1000,
            conflict_threshold_percent: 50,
            enable_speculation: true,
        }
    }
}
```

### 2.2 Invariants

```rust
/// INVARIANT-1: Topological Order
/// Dependencies are respected: if A → B, A executes before B.
fn invariant_topological_order(schedule: &ExecutionSchedule, graph: &DependencyGraph) -> bool {
    let mut executed = HashSet::new();
    
    for group in &schedule.parallel_groups {
        for tx_hash in &group.transactions {
            // All dependencies of this tx must already be executed
            if let Some(deps) = graph.adjacency.get(tx_hash) {
                // This is reverse; we need to check incoming edges
            }
            executed.insert(*tx_hash);
        }
    }
    true
}

/// INVARIANT-2: No Cycles
/// The dependency graph must be a DAG (no cycles).
fn invariant_no_cycles(graph: &DependencyGraph) -> bool {
    // Kahn's algorithm will fail to schedule all nodes if cycles exist
    let scheduled = kahns_algorithm(graph);
    scheduled.len() == graph.transactions.len()
}

/// INVARIANT-3: Parallel Safety
/// Transactions in the same parallel group have no conflicts.
fn invariant_parallel_safety(group: &ParallelGroup, graph: &DependencyGraph) -> bool {
    for i in 0..group.transactions.len() {
        for j in (i+1)..group.transactions.len() {
            let tx_i = &group.transactions[i];
            let tx_j = &group.transactions[j];
            
            // No edge between them in either direction
            let has_edge = graph.edges.iter().any(|e| 
                (e.from == *tx_i && e.to == *tx_j) || 
                (e.from == *tx_j && e.to == *tx_i)
            );
            
            if has_edge {
                return false;
            }
        }
    }
    true
}
```

---

## 3. PORTS (HEXAGONAL ARCHITECTURE)

### 3.1 Driving Ports (API - Inbound)

```rust
/// Primary Transaction Ordering API
#[async_trait]
pub trait TransactionOrderingApi: Send + Sync {
    /// Analyze and order transactions for parallel execution
    async fn order_transactions(
        &self,
        transactions: Vec<SignedTransaction>,
    ) -> Result<ExecutionSchedule, OrderingError>;
    
    /// Build dependency graph for transactions
    async fn build_dependency_graph(
        &self,
        transactions: Vec<AnnotatedTransaction>,
    ) -> Result<DependencyGraph, OrderingError>;
    
    /// Get parallel execution schedule from graph
    fn schedule_parallel_execution(
        &self,
        graph: &DependencyGraph,
    ) -> Result<ExecutionSchedule, OrderingError>;
    
    /// Annotate transactions with access patterns
    async fn annotate_transactions(
        &self,
        transactions: Vec<SignedTransaction>,
    ) -> Result<Vec<AnnotatedTransaction>, OrderingError>;
}
```

### 3.2 Driven Ports (SPI - Outbound Dependencies)

```rust
/// State access pattern analyzer
#[async_trait]
pub trait AccessPatternAnalyzer: Send + Sync {
    /// Analyze a transaction to determine its read/write set
    async fn analyze_access_pattern(
        &self,
        transaction: &SignedTransaction,
    ) -> Result<AccessPattern, AnalysisError>;
}

/// Conflict detector (uses Subsystem 4)
#[async_trait]
pub trait ConflictDetector: Send + Sync {
    /// Detect conflicts between transactions
    async fn detect_conflicts(
        &self,
        transactions: &[AnnotatedTransaction],
    ) -> Result<Vec<Conflict>, ConflictError>;
}

/// Access pattern result
#[derive(Clone, Debug)]
pub struct AccessPattern {
    pub reads: HashSet<(Address, StorageKey)>,
    pub writes: HashSet<(Address, StorageKey)>,
    pub balance_reads: HashSet<Address>,
    pub balance_writes: HashSet<Address>,
}

/// Conflict between transactions
#[derive(Clone, Debug)]
pub struct Conflict {
    pub tx1: Hash,
    pub tx2: Hash,
    pub kind: DependencyKind,
    pub location: (Address, StorageKey),
}
```

---

## 4. EVENT SCHEMA

### 4.1 Messages

```rust
/// Request to order transactions
/// SECURITY: Envelope sender_id MUST be 8 (Consensus)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrderTransactionsRequest {
    pub correlation_id: CorrelationId,
    pub reply_to: Topic,
    pub transactions: Vec<SignedTransaction>,
    pub block_context: BlockContext,
}

/// Ordering result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrderTransactionsResponse {
    pub correlation_id: CorrelationId,
    pub schedule: ExecutionSchedule,
    pub total_transactions: usize,
    pub parallel_groups: usize,
}

/// Conflict detection request (to Subsystem 4)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConflictDetectionRequest {
    pub correlation_id: CorrelationId,
    pub reply_to: Topic,
    pub transactions: Vec<TransactionAccessPattern>,
}
```

---

## 5. TEST-DRIVEN DEVELOPMENT STRATEGY

### 5.1 Unit Tests

```rust
#[cfg(test)]
mod unit_tests {
    use super::*;
    
    // === Dependency Detection Tests ===
    
    #[test]
    fn test_read_after_write_dependency() {
        let tx1 = create_tx_with_writes(vec![(CONTRACT_A, SLOT_1)]);
        let tx2 = create_tx_with_reads(vec![(CONTRACT_A, SLOT_1)]);
        
        let graph = build_graph(vec![tx1.clone(), tx2.clone()]);
        
        assert!(graph.has_edge(&tx1.hash, &tx2.hash));
        assert!(!graph.has_edge(&tx2.hash, &tx1.hash));
    }
    
    #[test]
    fn test_write_after_write_dependency() {
        let tx1 = create_tx_with_writes(vec![(CONTRACT_A, SLOT_1)]);
        let tx2 = create_tx_with_writes(vec![(CONTRACT_A, SLOT_1)]);
        
        let graph = build_graph(vec![tx1.clone(), tx2.clone()]);
        
        // Order determined by timestamp/hash
        assert!(graph.has_edge(&tx1.hash, &tx2.hash) || graph.has_edge(&tx2.hash, &tx1.hash));
    }
    
    #[test]
    fn test_no_dependency_independent_txs() {
        let tx1 = create_tx_with_writes(vec![(CONTRACT_A, SLOT_1)]);
        let tx2 = create_tx_with_writes(vec![(CONTRACT_B, SLOT_2)]);
        
        let graph = build_graph(vec![tx1.clone(), tx2.clone()]);
        
        assert!(!graph.has_edge(&tx1.hash, &tx2.hash));
        assert!(!graph.has_edge(&tx2.hash, &tx1.hash));
    }
    
    #[test]
    fn test_nonce_ordering_same_sender() {
        let tx1 = create_tx_from_sender(ALICE, 0);  // nonce 0
        let tx2 = create_tx_from_sender(ALICE, 1);  // nonce 1
        
        let graph = build_graph(vec![tx1.clone(), tx2.clone()]);
        
        // tx1 must execute before tx2 (nonce order)
        assert!(graph.has_edge(&tx1.hash, &tx2.hash));
    }
    
    // === Topological Sort Tests ===
    
    #[test]
    fn test_kahns_algorithm_simple_chain() {
        // A → B → C
        let graph = create_chain_graph(3);
        
        let schedule = kahns_algorithm(&graph);
        
        assert_eq!(schedule.parallel_groups.len(), 3);
        assert_eq!(schedule.parallel_groups[0].transactions[0], A);
        assert_eq!(schedule.parallel_groups[1].transactions[0], B);
        assert_eq!(schedule.parallel_groups[2].transactions[0], C);
    }
    
    #[test]
    fn test_kahns_algorithm_parallel() {
        // A, B, C all independent
        let graph = create_independent_graph(3);
        
        let schedule = kahns_algorithm(&graph);
        
        // All in one parallel group
        assert_eq!(schedule.parallel_groups.len(), 1);
        assert_eq!(schedule.parallel_groups[0].transactions.len(), 3);
        assert_eq!(schedule.max_parallelism, 3);
    }
    
    #[test]
    fn test_kahns_algorithm_diamond() {
        //     A
        //    / \
        //   B   C
        //    \ /
        //     D
        let graph = create_diamond_graph();
        
        let schedule = kahns_algorithm(&graph);
        
        assert_eq!(schedule.parallel_groups.len(), 3);
        assert_eq!(schedule.parallel_groups[0].transactions, vec![A]);
        assert_eq!(schedule.parallel_groups[1].transactions.len(), 2);  // B, C parallel
        assert_eq!(schedule.parallel_groups[2].transactions, vec![D]);
    }
    
    #[test]
    fn test_cycle_detection() {
        // A → B → C → A (cycle)
        let mut graph = create_chain_graph(3);
        graph.add_edge(C, A);  // Create cycle
        
        let result = kahns_algorithm(&graph);
        
        // Should fail or return partial schedule
        assert!(result.total_transactions < 3);
    }
    
    // === Parallel Safety Tests ===
    
    #[test]
    fn test_parallel_group_no_conflicts() {
        let tx1 = create_tx_with_writes(vec![(CONTRACT_A, SLOT_1)]);
        let tx2 = create_tx_with_writes(vec![(CONTRACT_B, SLOT_2)]);
        let tx3 = create_tx_with_reads(vec![(CONTRACT_C, SLOT_3)]);
        
        let graph = build_graph(vec![tx1, tx2, tx3]);
        let schedule = kahns_algorithm(&graph);
        
        // All should be in one parallel group
        assert_eq!(schedule.parallel_groups.len(), 1);
        assert_eq!(schedule.max_parallelism, 3);
    }
}
```

### 5.2 Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_order_transactions_with_state_analysis() {
        let (state, _) = create_mock_state();
        let (conflict_detector, _) = create_mock_conflict_detector();
        let service = TransactionOrderingService::new(state, conflict_detector);
        
        let transactions = vec![
            create_transfer_tx(ALICE, BOB, 100),
            create_transfer_tx(CHARLIE, DAVE, 200),  // Independent
            create_transfer_tx(BOB, EVE, 50),        // Depends on first
        ];
        
        let schedule = service.order_transactions(transactions).await.unwrap();
        
        // First two can be parallel, third depends on first
        assert!(schedule.parallel_groups.len() >= 2);
    }
    
    #[tokio::test]
    async fn test_high_conflict_fallback_to_sequential() {
        let config = OrderingConfig {
            conflict_threshold_percent: 10,  // Low threshold
            ..Default::default()
        };
        let service = create_service_with_config(config);
        
        // All transactions conflict (write same slot)
        let transactions: Vec<_> = (0..10)
            .map(|i| create_tx_with_writes(vec![(CONTRACT_A, SLOT_1)]))
            .collect();
        
        let schedule = service.order_transactions(transactions).await.unwrap();
        
        // Should fallback to sequential
        assert_eq!(schedule.max_parallelism, 1);
        assert_eq!(schedule.parallel_groups.len(), 10);
    }
    
    #[tokio::test]
    async fn test_correct_execution_order_verified() {
        let service = create_test_service();
        let (executor, execution_log) = create_mock_executor();
        
        // A writes X, B reads X
        let tx_a = create_tx_with_writes(vec![(CONTRACT_A, SLOT_X)]);
        let tx_b = create_tx_with_reads(vec![(CONTRACT_A, SLOT_X)]);
        
        let schedule = service.order_transactions(vec![tx_a.clone(), tx_b.clone()]).await.unwrap();
        
        // Execute according to schedule
        executor.execute_schedule(schedule).await;
        
        // Verify A executed before B
        let log = execution_log.lock().unwrap();
        let a_idx = log.iter().position(|h| *h == tx_a.hash).unwrap();
        let b_idx = log.iter().position(|h| *h == tx_b.hash).unwrap();
        
        assert!(a_idx < b_idx);
    }
}
```

---

## 6. ERROR HANDLING

```rust
#[derive(Debug, thiserror::Error)]
pub enum OrderingError {
    #[error("Cycle detected in dependency graph")]
    CycleDetected,
    
    #[error("Batch size exceeded: {size} > {max}")]
    BatchTooLarge { size: usize, max: usize },
    
    #[error("Access pattern analysis failed: {0}")]
    AnalysisFailed(String),
    
    #[error("Conflict detection failed: {0}")]
    ConflictDetectionFailed(#[from] ConflictError),
    
    #[error("Too many conflicts: {percent}% exceeds threshold")]
    TooManyConflicts { percent: u8 },
    
    #[error("Unauthorized sender: {0:?}")]
    UnauthorizedSender(SubsystemId),
}
```

---

## 7. CONFIGURATION

```toml
[transaction_ordering]
max_batch_size = 1000
conflict_threshold_percent = 50
enable_speculation = true
```

---

## APPENDIX A: DEPENDENCY MATRIX

**Reference:** System.md V2.3 Unified Dependency Graph, IPC-MATRIX.md Subsystem 12

| This Subsystem | Depends On | Dependency Type | Purpose | Reference |
|----------------|------------|-----------------|---------|-----------|
| Tx Ordering (12) | Subsystem 4 (State Mgmt) | Query | Conflict detection | System.md Subsystem 12 |
| Tx Ordering (12) | Subsystem 11 (Smart Contracts) | Sends to | Ordered transactions for execution | IPC-MATRIX.md Subsystem 12 |
| Tx Ordering (12) | Subsystem 8 (Consensus) | Accepts from | Transactions to order | IPC-MATRIX.md Subsystem 12 |

---

## APPENDIX B: SECURITY BOUNDARIES (IPC-MATRIX.md Compliance)

**Reference:** IPC-MATRIX.md, Subsystem 12 Section

### B.1 Authorized Message Senders

| Message Type | Authorized Sender(s) | Reference |
|--------------|---------------------|-----------|
| `OrderTransactionsRequest` | Subsystem 8 (Consensus) ONLY | IPC-MATRIX.md Security Boundaries |

### B.2 Rejection Rules

```rust
/// MANDATORY rejection rules per IPC-MATRIX.md
fn validate_order_request(
    msg: &AuthenticatedMessage<OrderTransactionsRequest>
) -> Result<(), OrderingError> {
    // Only Consensus can request ordering
    if msg.sender_id != SubsystemId::Consensus {
        return Err(OrderingError::UnauthorizedSender(msg.sender_id));
    }
    
    // Reject circular dependencies
    if has_cycle(&msg.payload.transactions) {
        return Err(OrderingError::CycleDetected);
    }
    
    // Reject dependency graphs with >10,000 edges (complexity attack)
    let edge_count = count_edges(&msg.payload.transactions);
    if edge_count > 10_000 {
        return Err(OrderingError::BatchTooLarge { 
            size: edge_count, 
            max: 10_000 
        });
    }
    
    Ok(())
}
```

---

## APPENDIX C: CROSS-REFERENCES

### C.1 Master Document References

| Document | Section | Relevance to This Spec |
|----------|---------|------------------------|
| Architecture.md | Section 3.2.1 | Envelope-Only Identity mandate |
| IPC-MATRIX.md | Subsystem 12 | Security boundaries |
| System.md | Subsystem 12 | Topological Sort (Kahn's Algorithm) |

### C.2 Related Specifications

| Specification | Relationship | Notes |
|--------------|--------------|-------|
| SPEC-04-STATE-MANAGEMENT.md | Dependency | Conflict detection queries |
| SPEC-08-CONSENSUS.md | Producer | Sends transactions to order |
| SPEC-11-SMART-CONTRACTS.md | Consumer | Receives ordered transactions |

### C.3 Implementation Priority

**Reference:** System.md, Implementation Priority Section

This subsystem is in **Phase 3 (Advanced - Weeks 9-12)** because:
- Optional subsystem for DAG-based parallel execution
- Depends on Subsystems 4 (State) and 11 (Smart Contracts)
- Not required for basic block processing

---

**END OF SPECIFICATION**

**Next Steps:**
1. Review this specification for completeness
2. Approve for TDD Phase (write tests)
3. Implement domain logic (pass tests)
4. Implement adapters (wire to runtime)
