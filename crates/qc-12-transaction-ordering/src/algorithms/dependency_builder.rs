//! Dependency Graph Builder
//!
//! Builds dependency graph from annotated transactions.
//! Reference: System.md Line 591

use crate::domain::entities::{AnnotatedTransaction, Dependency, DependencyGraph};
use crate::domain::value_objects::DependencyKind;
use std::collections::HashMap;

/// Build a dependency graph from annotated transactions.
///
/// Detects three types of dependencies:
/// 1. Read-After-Write: tx2 reads what tx1 writes
/// 2. Write-After-Write: both write to same location
/// 3. Nonce Order: same sender, must respect nonce ordering
pub fn build_dependency_graph(transactions: Vec<AnnotatedTransaction>) -> DependencyGraph {
    let mut graph = DependencyGraph::new();

    // Add all transactions as nodes
    for tx in &transactions {
        graph.add_node(tx.clone());
    }

    // Group transactions by sender for nonce ordering
    let mut by_sender: HashMap<primitive_types::H160, Vec<&AnnotatedTransaction>> = HashMap::new();
    for tx in &transactions {
        by_sender.entry(tx.sender).or_default().push(tx);
    }

    // Add nonce ordering dependencies (same sender)
    for txs in by_sender.values() {
        let mut sorted: Vec<_> = txs.iter().collect();
        sorted.sort_by_key(|tx| tx.nonce);

        for window in sorted.windows(2) {
            let tx1 = window[0];
            let tx2 = window[1];
            graph.add_edge(Dependency::new(tx1.hash, tx2.hash, DependencyKind::NonceOrder));
        }
    }

    // Detect data dependencies (RAW, WAW)
    let tx_list: Vec<_> = transactions.iter().collect();
    for i in 0..tx_list.len() {
        for j in (i + 1)..tx_list.len() {
            let tx1 = tx_list[i];
            let tx2 = tx_list[j];

            // Skip if already have nonce dependency
            if tx1.sender == tx2.sender {
                continue;
            }

            // Check for data conflicts
            if let Some(dep) = detect_data_dependency(tx1, tx2) {
                graph.add_edge(dep);
            }
        }
    }

    graph
}

/// Detect data dependency between two transactions
fn detect_data_dependency(tx1: &AnnotatedTransaction, tx2: &AnnotatedTransaction) -> Option<Dependency> {
    let p1 = &tx1.access_pattern;
    let p2 = &tx2.access_pattern;

    // Write-After-Write: both write to same location
    for loc in &p1.writes {
        if p2.writes.contains(loc) {
            return Some(
                Dependency::new(tx1.hash, tx2.hash, DependencyKind::WriteAfterWrite)
                    .with_location(loc.clone()),
            );
        }
    }

    // Read-After-Write: tx2 reads what tx1 writes
    for loc in &p1.writes {
        if p2.reads.contains(loc) {
            return Some(
                Dependency::new(tx1.hash, tx2.hash, DependencyKind::ReadAfterWrite)
                    .with_location(loc.clone()),
            );
        }
    }

    // Write-After-Read: tx2 writes what tx1 reads (tx1 must complete first)
    for loc in &p1.reads {
        if p2.writes.contains(loc) {
            return Some(
                Dependency::new(tx1.hash, tx2.hash, DependencyKind::ReadAfterWrite)
                    .with_location(loc.clone()),
            );
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{AccessPattern, StorageLocation};
    use primitive_types::{H160, H256};

    fn make_hash(val: u8) -> primitive_types::H256 {
        H256::from_low_u64_be(val as u64)
    }

    fn make_addr(val: u8) -> H160 {
        H160::from_low_u64_be(val as u64)
    }

    fn loc(addr: u8, key: u8) -> StorageLocation {
        StorageLocation::new(make_addr(addr), make_hash(key))
    }

    #[test]
    fn test_read_after_write_dependency() {
        let tx1 = AnnotatedTransaction::new(
            make_hash(1),
            make_addr(10),
            0,
            AccessPattern::new().with_writes(vec![loc(1, 1)]),
        );
        let tx2 = AnnotatedTransaction::new(
            make_hash(2),
            make_addr(20), // Different sender
            0,
            AccessPattern::new().with_reads(vec![loc(1, 1)]),
        );

        let graph = build_dependency_graph(vec![tx1.clone(), tx2.clone()]);

        assert!(graph.has_edge(&tx1.hash, &tx2.hash));
        assert!(!graph.has_edge(&tx2.hash, &tx1.hash));
    }

    #[test]
    fn test_write_after_write_dependency() {
        let tx1 = AnnotatedTransaction::new(
            make_hash(1),
            make_addr(10),
            0,
            AccessPattern::new().with_writes(vec![loc(1, 1)]),
        );
        let tx2 = AnnotatedTransaction::new(
            make_hash(2),
            make_addr(20),
            0,
            AccessPattern::new().with_writes(vec![loc(1, 1)]),
        );

        let graph = build_dependency_graph(vec![tx1.clone(), tx2.clone()]);

        // tx1 -> tx2 (WAW)
        assert!(graph.has_edge(&tx1.hash, &tx2.hash));
    }

    #[test]
    fn test_nonce_ordering() {
        let sender = make_addr(10);
        let tx1 = AnnotatedTransaction::new(make_hash(1), sender, 0, AccessPattern::new());
        let tx2 = AnnotatedTransaction::new(make_hash(2), sender, 1, AccessPattern::new());
        let tx3 = AnnotatedTransaction::new(make_hash(3), sender, 2, AccessPattern::new());

        let graph = build_dependency_graph(vec![tx1.clone(), tx2.clone(), tx3.clone()]);

        // Nonce order: tx1 -> tx2 -> tx3
        assert!(graph.has_edge(&tx1.hash, &tx2.hash));
        assert!(graph.has_edge(&tx2.hash, &tx3.hash));
        assert!(!graph.has_edge(&tx1.hash, &tx3.hash)); // Not direct
    }

    #[test]
    fn test_no_dependency_independent_txs() {
        let tx1 = AnnotatedTransaction::new(
            make_hash(1),
            make_addr(10),
            0,
            AccessPattern::new().with_writes(vec![loc(1, 1)]),
        );
        let tx2 = AnnotatedTransaction::new(
            make_hash(2),
            make_addr(20),
            0,
            AccessPattern::new().with_writes(vec![loc(2, 2)]),
        );

        let graph = build_dependency_graph(vec![tx1.clone(), tx2.clone()]);

        assert!(!graph.has_edge(&tx1.hash, &tx2.hash));
        assert!(!graph.has_edge(&tx2.hash, &tx1.hash));
    }
}
