//! Domain invariants for Transaction Ordering
//!
//! Reference: SPEC-12 Section 2.2 (Lines 144-193)

use super::entities::{DependencyGraph, ExecutionSchedule, ParallelGroup};
use super::value_objects::Hash;
use std::collections::HashSet;

/// INVARIANT-1: Topological Order
/// Dependencies are respected: if A → B exists, A executes before B.
/// Reference: SPEC-12 Lines 148-162
pub fn invariant_topological_order(schedule: &ExecutionSchedule, graph: &DependencyGraph) -> bool {
    let mut executed: HashSet<Hash> = HashSet::new();

    for group in &schedule.parallel_groups {
        for tx_hash in &group.transactions {
            // Check all incoming edges: their sources must be already executed
            for edge in &graph.edges {
                if edge.to != *tx_hash {
                    continue;
                }
                if !executed.contains(&edge.from) {
                    // Dependency not yet executed - violation!
                    return false;
                }
            }
        }

        // Mark all transactions in this group as executed
        for tx_hash in &group.transactions {
            executed.insert(*tx_hash);
        }
    }

    true
}

/// INVARIANT-2: No Cycles
/// The dependency graph must be a DAG (no cycles).
/// Reference: SPEC-12 Lines 165-170
pub fn invariant_no_cycles(graph: &DependencyGraph) -> bool {
    // Use DFS to detect cycles
    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();

    for hash in graph.transactions.keys() {
        if has_cycle_dfs(graph, *hash, &mut visited, &mut rec_stack) {
            return false;
        }
    }

    true
}

fn has_cycle_dfs(
    graph: &DependencyGraph,
    node: Hash,
    visited: &mut HashSet<Hash>,
    rec_stack: &mut HashSet<Hash>,
) -> bool {
    if rec_stack.contains(&node) {
        return true; // Back edge found - cycle!
    }

    if visited.contains(&node) {
        return false; // Already fully explored
    }

    visited.insert(node);
    rec_stack.insert(node);

    if let Some(neighbors) = graph.adjacency.get(&node) {
        for &neighbor in neighbors {
            if has_cycle_dfs(graph, neighbor, visited, rec_stack) {
                return true;
            }
        }
    }

    rec_stack.remove(&node);
    false
}

/// INVARIANT-3: Parallel Safety
/// Transactions in the same parallel group have no conflicts (no edges between them).
/// Reference: SPEC-12 Lines 173-192
pub fn invariant_parallel_safety(group: &ParallelGroup, graph: &DependencyGraph) -> bool {
    let _tx_set: HashSet<&Hash> = group.transactions.iter().collect();

    for i in 0..group.transactions.len() {
        for j in (i + 1)..group.transactions.len() {
            let tx_i = &group.transactions[i];
            let tx_j = &group.transactions[j];

            // No edge between them in either direction
            if graph.has_edge(tx_i, tx_j) || graph.has_edge(tx_j, tx_i) {
                return false;
            }
        }
    }

    true
}

/// INVARIANT-4: Completeness
/// All transactions are scheduled exactly once.
pub fn invariant_completeness(schedule: &ExecutionSchedule, graph: &DependencyGraph) -> bool {
    let scheduled: HashSet<Hash> = schedule
        .parallel_groups
        .iter()
        .flat_map(|g| g.transactions.iter().copied())
        .collect();

    let all_tx: HashSet<Hash> = graph.transactions.keys().copied().collect();

    scheduled == all_tx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::{AnnotatedTransaction, Dependency};
    use crate::domain::value_objects::{AccessPattern, DependencyKind};
    use primitive_types::{H160, H256};

    fn make_hash(val: u8) -> Hash {
        H256::from_low_u64_be(val as u64)
    }

    fn make_tx(val: u8) -> AnnotatedTransaction {
        AnnotatedTransaction::new(make_hash(val), H160::zero(), 0, AccessPattern::default())
    }

    #[test]
    fn test_invariant_no_cycles_acyclic() {
        let mut graph = DependencyGraph::new();
        let tx1 = make_tx(1);
        let tx2 = make_tx(2);
        let tx3 = make_tx(3);

        graph.add_node(tx1.clone());
        graph.add_node(tx2.clone());
        graph.add_node(tx3.clone());
        graph.add_edge(Dependency::new(
            tx1.hash,
            tx2.hash,
            DependencyKind::ReadAfterWrite,
        ));
        graph.add_edge(Dependency::new(
            tx2.hash,
            tx3.hash,
            DependencyKind::ReadAfterWrite,
        ));

        assert!(invariant_no_cycles(&graph));
    }

    #[test]
    fn test_invariant_no_cycles_cyclic() {
        let mut graph = DependencyGraph::new();
        let tx1 = make_tx(1);
        let tx2 = make_tx(2);
        let tx3 = make_tx(3);

        graph.add_node(tx1.clone());
        graph.add_node(tx2.clone());
        graph.add_node(tx3.clone());
        graph.add_edge(Dependency::new(
            tx1.hash,
            tx2.hash,
            DependencyKind::ReadAfterWrite,
        ));
        graph.add_edge(Dependency::new(
            tx2.hash,
            tx3.hash,
            DependencyKind::ReadAfterWrite,
        ));
        graph.add_edge(Dependency::new(
            tx3.hash,
            tx1.hash,
            DependencyKind::ReadAfterWrite,
        )); // Cycle!

        assert!(!invariant_no_cycles(&graph));
    }

    #[test]
    fn test_invariant_parallel_safety_valid() {
        let mut graph = DependencyGraph::new();
        let tx1 = make_tx(1);
        let tx2 = make_tx(2);

        graph.add_node(tx1.clone());
        graph.add_node(tx2.clone());
        // No edges - safe to parallelize

        let group = ParallelGroup::new(0, vec![tx1.hash, tx2.hash]);
        assert!(invariant_parallel_safety(&group, &graph));
    }

    #[test]
    fn test_invariant_parallel_safety_invalid() {
        let mut graph = DependencyGraph::new();
        let tx1 = make_tx(1);
        let tx2 = make_tx(2);

        graph.add_node(tx1.clone());
        graph.add_node(tx2.clone());
        graph.add_edge(Dependency::new(
            tx1.hash,
            tx2.hash,
            DependencyKind::ReadAfterWrite,
        ));

        let group = ParallelGroup::new(0, vec![tx1.hash, tx2.hash]);
        assert!(!invariant_parallel_safety(&group, &graph)); // Should fail - they have a dependency
    }

    #[test]
    fn test_invariant_topological_order_valid() {
        let mut graph = DependencyGraph::new();
        let tx1 = make_tx(1);
        let tx2 = make_tx(2);

        graph.add_node(tx1.clone());
        graph.add_node(tx2.clone());
        graph.add_edge(Dependency::new(
            tx1.hash,
            tx2.hash,
            DependencyKind::ReadAfterWrite,
        ));

        let schedule = ExecutionSchedule::new(vec![
            ParallelGroup::new(0, vec![tx1.hash]),
            ParallelGroup::new(1, vec![tx2.hash]),
        ]);

        assert!(invariant_topological_order(&schedule, &graph));
    }

    #[test]
    fn test_invariant_topological_order_invalid() {
        let mut graph = DependencyGraph::new();
        let tx1 = make_tx(1);
        let tx2 = make_tx(2);

        graph.add_node(tx1.clone());
        graph.add_node(tx2.clone());
        graph.add_edge(Dependency::new(
            tx1.hash,
            tx2.hash,
            DependencyKind::ReadAfterWrite,
        ));

        // Wrong order: tx2 before tx1
        let schedule = ExecutionSchedule::new(vec![
            ParallelGroup::new(0, vec![tx2.hash]),
            ParallelGroup::new(1, vec![tx1.hash]),
        ]);

        assert!(!invariant_topological_order(&schedule, &graph));
    }
}
