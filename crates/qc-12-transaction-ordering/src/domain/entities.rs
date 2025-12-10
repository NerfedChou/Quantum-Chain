//! Core entities for Transaction Ordering
//!
//! Reference: SPEC-12 Section 2.1 (Lines 54-121)

use super::value_objects::{AccessPattern, DependencyKind, Hash, StorageLocation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Transaction with access pattern annotations
/// Reference: SPEC-12 Lines 57-68
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnnotatedTransaction {
    /// Original transaction hash
    pub hash: Hash,
    /// Sender address (for nonce ordering)
    pub sender: primitive_types::H160,
    /// Sender nonce
    pub nonce: u64,
    /// Access pattern (reads/writes)
    pub access_pattern: AccessPattern,
    /// Gas estimate
    pub estimated_gas: u64,
    /// Timestamp for ordering determinism
    pub timestamp: u64,
}

impl AnnotatedTransaction {
    pub fn new(
        hash: Hash,
        sender: primitive_types::H160,
        nonce: u64,
        access_pattern: AccessPattern,
    ) -> Self {
        Self {
            hash,
            sender,
            nonce,
            access_pattern,
            estimated_gas: 21_000,
            timestamp: 0,
        }
    }

    pub fn with_gas(mut self, gas: u64) -> Self {
        self.estimated_gas = gas;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }
}

/// Dependency graph edge
/// Reference: SPEC-12 Lines 71-79
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    /// Transaction that must execute first
    pub from: Hash,
    /// Transaction that must execute after
    pub to: Hash,
    /// Type of dependency
    pub kind: DependencyKind,
    /// Conflicting location (if applicable)
    pub location: Option<StorageLocation>,
}

impl Dependency {
    pub fn new(from: Hash, to: Hash, kind: DependencyKind) -> Self {
        Self {
            from,
            to,
            kind,
            location: None,
        }
    }

    pub fn with_location(mut self, loc: StorageLocation) -> Self {
        self.location = Some(loc);
        self
    }
}

/// Dependency graph for transaction ordering
/// Reference: SPEC-12 Lines 91-102
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    /// All transactions by hash
    pub transactions: HashMap<Hash, AnnotatedTransaction>,
    /// All edges (dependencies)
    pub edges: Vec<Dependency>,
    /// Adjacency list: from -> [to, to, ...]
    pub adjacency: HashMap<Hash, Vec<Hash>>,
    /// In-degree count for each node
    pub in_degree: HashMap<Hash, usize>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            transactions: HashMap::new(),
            edges: Vec::new(),
            adjacency: HashMap::new(),
            in_degree: HashMap::new(),
        }
    }

    /// Add a transaction node to the graph
    pub fn add_node(&mut self, tx: AnnotatedTransaction) {
        let hash = tx.hash;
        self.transactions.insert(hash, tx);
        self.adjacency.entry(hash).or_default();
        self.in_degree.entry(hash).or_insert(0);
    }

    /// Add a dependency edge
    pub fn add_edge(&mut self, dep: Dependency) {
        // Update adjacency list
        self.adjacency.entry(dep.from).or_default().push(dep.to);

        // Update in-degree
        *self.in_degree.entry(dep.to).or_insert(0) += 1;

        // Store edge
        self.edges.push(dep);
    }

    /// Check if an edge exists from -> to
    pub fn has_edge(&self, from: &Hash, to: &Hash) -> bool {
        self.adjacency
            .get(from)
            .map(|neighbors| neighbors.contains(to))
            .unwrap_or(false)
    }

    /// Get all zero in-degree nodes (can start execution)
    pub fn get_zero_degree_nodes(&self) -> Vec<Hash> {
        self.in_degree
            .iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(hash, _)| *hash)
            .collect()
    }

    /// Number of nodes
    pub fn node_count(&self) -> usize {
        self.transactions.len()
    }

    /// Number of edges
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// A group of transactions that can execute in parallel
/// Reference: SPEC-12 Lines 116-120
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelGroup {
    /// Group sequence number
    pub group_id: usize,
    /// Transactions in this group (can run concurrently)
    pub transactions: Vec<Hash>,
}

impl ParallelGroup {
    pub fn new(group_id: usize, transactions: Vec<Hash>) -> Self {
        Self {
            group_id,
            transactions,
        }
    }

    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }
}

/// Execution schedule with parallel groups
/// Reference: SPEC-12 Lines 105-113
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSchedule {
    /// Ordered groups for parallel execution
    pub parallel_groups: Vec<ParallelGroup>,
    /// Total transactions scheduled
    pub total_transactions: usize,
    /// Maximum parallelism achieved
    pub max_parallelism: usize,
}

impl ExecutionSchedule {
    pub fn new(groups: Vec<ParallelGroup>) -> Self {
        let total = groups.iter().map(|g| g.len()).sum();
        let max_par = groups.iter().map(|g| g.len()).max().unwrap_or(0);

        Self {
            parallel_groups: groups,
            total_transactions: total,
            max_parallelism: max_par,
        }
    }

    /// Create a sequential schedule (all transactions in order)
    pub fn sequential(transactions: Vec<Hash>) -> Self {
        let groups: Vec<ParallelGroup> = transactions
            .into_iter()
            .enumerate()
            .map(|(i, hash)| ParallelGroup::new(i, vec![hash]))
            .collect();

        Self::new(groups)
    }

    /// Get a flattened list of transactions in execution order
    pub fn flatten(&self) -> Vec<Hash> {
        self.parallel_groups
            .iter()
            .flat_map(|g| g.transactions.iter().copied())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitive_types::{H160, H256};

    fn make_hash(val: u8) -> Hash {
        H256::from_low_u64_be(val as u64)
    }

    fn make_tx(val: u8) -> AnnotatedTransaction {
        AnnotatedTransaction::new(
            make_hash(val),
            H160::zero(),
            0,
            AccessPattern::default(),
        )
    }

    #[test]
    fn test_dependency_graph_add_node() {
        let mut graph = DependencyGraph::new();
        let tx = make_tx(1);

        graph.add_node(tx.clone());

        assert_eq!(graph.node_count(), 1);
        assert!(graph.transactions.contains_key(&tx.hash));
    }

    #[test]
    fn test_dependency_graph_add_edge() {
        let mut graph = DependencyGraph::new();
        let tx1 = make_tx(1);
        let tx2 = make_tx(2);

        graph.add_node(tx1.clone());
        graph.add_node(tx2.clone());
        graph.add_edge(Dependency::new(tx1.hash, tx2.hash, DependencyKind::ReadAfterWrite));

        assert!(graph.has_edge(&tx1.hash, &tx2.hash));
        assert!(!graph.has_edge(&tx2.hash, &tx1.hash));
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn test_zero_degree_nodes() {
        let mut graph = DependencyGraph::new();
        let tx1 = make_tx(1);
        let tx2 = make_tx(2);
        let tx3 = make_tx(3);

        graph.add_node(tx1.clone());
        graph.add_node(tx2.clone());
        graph.add_node(tx3.clone());

        // tx1 -> tx2, tx1 -> tx3
        graph.add_edge(Dependency::new(tx1.hash, tx2.hash, DependencyKind::ReadAfterWrite));
        graph.add_edge(Dependency::new(tx1.hash, tx3.hash, DependencyKind::ReadAfterWrite));

        let zero_nodes = graph.get_zero_degree_nodes();
        assert_eq!(zero_nodes.len(), 1);
        assert!(zero_nodes.contains(&tx1.hash));
    }

    #[test]
    fn test_execution_schedule_sequential() {
        let hashes = vec![make_hash(1), make_hash(2), make_hash(3)];
        let schedule = ExecutionSchedule::sequential(hashes.clone());

        assert_eq!(schedule.total_transactions, 3);
        assert_eq!(schedule.max_parallelism, 1);
        assert_eq!(schedule.parallel_groups.len(), 3);
    }

    #[test]
    fn test_execution_schedule_flatten() {
        let group1 = ParallelGroup::new(0, vec![make_hash(1), make_hash(2)]);
        let group2 = ParallelGroup::new(1, vec![make_hash(3)]);

        let schedule = ExecutionSchedule::new(vec![group1, group2]);
        let flat = schedule.flatten();

        assert_eq!(flat.len(), 3);
        assert_eq!(flat[0], make_hash(1));
        assert_eq!(flat[1], make_hash(2));
        assert_eq!(flat[2], make_hash(3));
    }
}
