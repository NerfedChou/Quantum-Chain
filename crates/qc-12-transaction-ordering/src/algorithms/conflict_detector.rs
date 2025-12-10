//! Conflict Detector
//!
//! Detects conflicts between transactions for dependency analysis.
//! Reference: System.md Line 593

use crate::domain::entities::AnnotatedTransaction;
use crate::domain::value_objects::{Conflict, DependencyKind};

/// Detect all conflicts between a set of transactions.
///
/// Returns a list of conflicts ordered by transaction pair.
pub fn detect_conflicts(transactions: &[AnnotatedTransaction]) -> Vec<Conflict> {
    let mut conflicts = Vec::new();

    for i in 0..transactions.len() {
        for j in (i + 1)..transactions.len() {
            let tx1 = &transactions[i];
            let tx2 = &transactions[j];

            // Check nonce conflict (same sender)
            if tx1.sender == tx2.sender {
                conflicts.push(Conflict {
                    tx1: tx1.hash,
                    tx2: tx2.hash,
                    kind: DependencyKind::NonceOrder,
                    location: None,
                });
                continue;
            }

            // Check data conflicts
            if let Some(conflict) = detect_data_conflict(tx1, tx2) {
                conflicts.push(conflict);
            }
        }
    }

    conflicts
}

/// Detect data conflict between two transactions
fn detect_data_conflict(tx1: &AnnotatedTransaction, tx2: &AnnotatedTransaction) -> Option<Conflict> {
    let p1 = &tx1.access_pattern;
    let p2 = &tx2.access_pattern;

    // Write-After-Write
    for loc in &p1.writes {
        if p2.writes.contains(loc) {
            return Some(Conflict {
                tx1: tx1.hash,
                tx2: tx2.hash,
                kind: DependencyKind::WriteAfterWrite,
                location: Some(loc.clone()),
            });
        }
    }

    // Read-After-Write (either direction)
    for loc in &p1.writes {
        if p2.reads.contains(loc) {
            return Some(Conflict {
                tx1: tx1.hash,
                tx2: tx2.hash,
                kind: DependencyKind::ReadAfterWrite,
                location: Some(loc.clone()),
            });
        }
    }

    for loc in &p1.reads {
        if p2.writes.contains(loc) {
            return Some(Conflict {
                tx1: tx1.hash,
                tx2: tx2.hash,
                kind: DependencyKind::ReadAfterWrite,
                location: Some(loc.clone()),
            });
        }
    }

    None
}

/// Calculate conflict percentage for fallback decision
pub fn conflict_percentage(conflicts: &[Conflict], tx_count: usize) -> u8 {
    if tx_count <= 1 {
        return 0;
    }

    // Max possible pairs
    let max_pairs = tx_count * (tx_count - 1) / 2;
    if max_pairs == 0 {
        return 0;
    }

    let percent = (conflicts.len() * 100) / max_pairs;
    percent.min(100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{AccessPattern, StorageLocation};
    use primitive_types::{H160, H256};

    fn make_hash(val: u8) -> H256 {
        H256::from_low_u64_be(val as u64)
    }

    fn make_addr(val: u8) -> H160 {
        H160::from_low_u64_be(val as u64)
    }

    fn loc(addr: u8, key: u8) -> StorageLocation {
        StorageLocation::new(make_addr(addr), make_hash(key))
    }

    #[test]
    fn test_detect_no_conflicts() {
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

        let conflicts = detect_conflicts(&[tx1, tx2]);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_detect_waw_conflict() {
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

        let conflicts = detect_conflicts(&[tx1, tx2]);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].kind, DependencyKind::WriteAfterWrite);
    }

    #[test]
    fn test_detect_nonce_conflict() {
        let sender = make_addr(10);
        let tx1 = AnnotatedTransaction::new(make_hash(1), sender, 0, AccessPattern::new());
        let tx2 = AnnotatedTransaction::new(make_hash(2), sender, 1, AccessPattern::new());

        let conflicts = detect_conflicts(&[tx1, tx2]);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].kind, DependencyKind::NonceOrder);
    }

    #[test]
    fn test_conflict_percentage() {
        let conflicts = vec![
            Conflict {
                tx1: make_hash(1),
                tx2: make_hash(2),
                kind: DependencyKind::WriteAfterWrite,
                location: None,
            },
        ];

        // 3 transactions = 3 pairs max, 1 conflict = 33%
        assert_eq!(conflict_percentage(&conflicts, 3), 33);

        // 2 transactions = 1 pair max, 1 conflict = 100%
        assert_eq!(conflict_percentage(&conflicts, 2), 100);
    }
}
