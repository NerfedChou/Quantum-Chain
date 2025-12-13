//! Kahn's Topological Sort Algorithm
//!
//! Reference: System.md Lines 587-594
//! O(V + E) complexity, detects cycles, enables parallelism

use crate::domain::entities::{DependencyGraph, ExecutionSchedule, ParallelGroup};
use crate::domain::errors::OrderingError;
use crate::domain::value_objects::Hash;
use std::collections::HashMap;

/// Perform Kahn's topological sort on the dependency graph.
///
/// Returns an ExecutionSchedule with parallel groups.
/// Transactions in the same group can execute concurrently.
///
/// Reference: System.md Lines 587-588
/// "Topological Sort (Kahn's Algorithm) - O(V + E) complexity, detects cycles, enables parallelism"
pub fn kahns_topological_sort(graph: &DependencyGraph) -> Result<ExecutionSchedule, OrderingError> {
    if graph.transactions.is_empty() {
        return Ok(ExecutionSchedule::new(vec![]));
    }

    // 1. Copy in-degree map (we'll modify it)
    let mut in_degree: HashMap<Hash, usize> = graph.in_degree.clone();

    // 2. Initialize queue with zero in-degree nodes
    let mut queue: Vec<Hash> = in_degree
        .iter()
        .filter(|(_, &degree)| degree == 0)
        .map(|(hash, _)| *hash)
        .collect();

    // 3. Sort for determinism (System.md Line 608: "Deterministic Ordering")
    queue.sort();

    // 4. Process queue, building parallel groups
    let mut groups: Vec<ParallelGroup> = Vec::new();
    let mut scheduled_count = 0;

    while !queue.is_empty() {
        // All nodes in current queue have zero in-degree - they can run in parallel
        let current_group: Vec<Hash> = std::mem::take(&mut queue);
        let group_size = current_group.len();

        // Create parallel group
        groups.push(ParallelGroup::new(groups.len(), current_group.clone()));
        scheduled_count += group_size;

        // Find next batch of zero in-degree nodes
        let mut next_queue: Vec<Hash> = Vec::new();

        for node in &current_group {
            let Some(neighbors) = graph.adjacency.get(node) else {
                continue;
            };
            for neighbor in neighbors {
                let Some(degree) = in_degree.get_mut(neighbor) else {
                    continue;
                };
                *degree = degree.saturating_sub(1);
                if *degree == 0 {
                    next_queue.push(*neighbor);
                }
            }
        }

        // Sort next queue for determinism
        next_queue.sort();
        queue = next_queue;
    }

    // 5. Cycle detection: if not all nodes scheduled, there's a cycle
    if scheduled_count < graph.transactions.len() {
        return Err(OrderingError::CycleDetected);
    }

    Ok(ExecutionSchedule::new(groups))
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

    /// Test: A → B → C (simple chain)
    /// Expected: 3 groups, each with 1 tx
    #[test]
    fn test_kahns_simple_chain() {
        let mut graph = DependencyGraph::new();
        let tx_a = make_tx(1);
        let tx_b = make_tx(2);
        let tx_c = make_tx(3);

        graph.add_node(tx_a.clone());
        graph.add_node(tx_b.clone());
        graph.add_node(tx_c.clone());
        graph.add_edge(Dependency::new(
            tx_a.hash,
            tx_b.hash,
            DependencyKind::ReadAfterWrite,
        ));
        graph.add_edge(Dependency::new(
            tx_b.hash,
            tx_c.hash,
            DependencyKind::ReadAfterWrite,
        ));

        let schedule = kahns_topological_sort(&graph).unwrap();

        assert_eq!(schedule.parallel_groups.len(), 3);
        assert_eq!(schedule.max_parallelism, 1);
        assert_eq!(schedule.total_transactions, 3);

        // Verify order
        let flat = schedule.flatten();
        assert_eq!(flat[0], tx_a.hash);
        assert_eq!(flat[1], tx_b.hash);
        assert_eq!(flat[2], tx_c.hash);
    }

    /// Test: A, B, C independent
    /// Expected: 1 group with 3 txs
    #[test]
    fn test_kahns_fully_parallel() {
        let mut graph = DependencyGraph::new();
        let tx_a = make_tx(1);
        let tx_b = make_tx(2);
        let tx_c = make_tx(3);

        graph.add_node(tx_a.clone());
        graph.add_node(tx_b.clone());
        graph.add_node(tx_c.clone());
        // No edges - all independent

        let schedule = kahns_topological_sort(&graph).unwrap();

        assert_eq!(schedule.parallel_groups.len(), 1);
        assert_eq!(schedule.max_parallelism, 3);
        assert_eq!(schedule.total_transactions, 3);
    }

    /// Test: Diamond graph
    ///     A
    ///    / \
    ///   B   C
    ///    \ /
    ///     D
    /// Expected: 3 groups: [A], [B,C], [D]
    #[test]
    fn test_kahns_diamond_graph() {
        let mut graph = DependencyGraph::new();
        let tx_a = make_tx(1);
        let tx_b = make_tx(2);
        let tx_c = make_tx(3);
        let tx_d = make_tx(4);

        graph.add_node(tx_a.clone());
        graph.add_node(tx_b.clone());
        graph.add_node(tx_c.clone());
        graph.add_node(tx_d.clone());

        graph.add_edge(Dependency::new(
            tx_a.hash,
            tx_b.hash,
            DependencyKind::ReadAfterWrite,
        ));
        graph.add_edge(Dependency::new(
            tx_a.hash,
            tx_c.hash,
            DependencyKind::ReadAfterWrite,
        ));
        graph.add_edge(Dependency::new(
            tx_b.hash,
            tx_d.hash,
            DependencyKind::ReadAfterWrite,
        ));
        graph.add_edge(Dependency::new(
            tx_c.hash,
            tx_d.hash,
            DependencyKind::ReadAfterWrite,
        ));

        let schedule = kahns_topological_sort(&graph).unwrap();

        assert_eq!(schedule.parallel_groups.len(), 3);
        assert_eq!(schedule.parallel_groups[0].transactions, vec![tx_a.hash]);
        assert_eq!(schedule.parallel_groups[1].transactions.len(), 2); // B and C
        assert_eq!(schedule.parallel_groups[2].transactions, vec![tx_d.hash]);
        assert_eq!(schedule.max_parallelism, 2); // B and C in parallel
    }

    /// Test: A → B → C → A (cycle)
    /// Expected: Err(CycleDetected)
    #[test]
    fn test_cycle_detected() {
        let mut graph = DependencyGraph::new();
        let tx_a = make_tx(1);
        let tx_b = make_tx(2);
        let tx_c = make_tx(3);

        graph.add_node(tx_a.clone());
        graph.add_node(tx_b.clone());
        graph.add_node(tx_c.clone());

        graph.add_edge(Dependency::new(
            tx_a.hash,
            tx_b.hash,
            DependencyKind::ReadAfterWrite,
        ));
        graph.add_edge(Dependency::new(
            tx_b.hash,
            tx_c.hash,
            DependencyKind::ReadAfterWrite,
        ));
        graph.add_edge(Dependency::new(
            tx_c.hash,
            tx_a.hash,
            DependencyKind::ReadAfterWrite,
        )); // Cycle!

        let result = kahns_topological_sort(&graph);

        assert!(matches!(result, Err(OrderingError::CycleDetected)));
    }

    /// Test: Empty graph
    #[test]
    fn test_empty_graph() {
        let graph = DependencyGraph::new();
        let schedule = kahns_topological_sort(&graph).unwrap();

        assert_eq!(schedule.parallel_groups.len(), 0);
        assert_eq!(schedule.total_transactions, 0);
    }

    /// Test: Deterministic output
    /// Same input should always produce same output
    #[test]
    fn test_deterministic_output() {
        let mut graph = DependencyGraph::new();
        let tx_a = make_tx(1);
        let tx_b = make_tx(2);
        let tx_c = make_tx(3);

        graph.add_node(tx_a.clone());
        graph.add_node(tx_b.clone());
        graph.add_node(tx_c.clone());

        let schedule1 = kahns_topological_sort(&graph).unwrap();
        let schedule2 = kahns_topological_sort(&graph).unwrap();

        assert_eq!(schedule1.flatten(), schedule2.flatten());
    }
}
