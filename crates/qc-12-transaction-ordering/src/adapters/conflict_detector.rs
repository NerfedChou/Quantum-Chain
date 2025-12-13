//! Conflict Detector Adapter
//!
//! Implements `ConflictDetector` port using balance-based conflict analysis.
//! Reference: SPEC-12 Section 3.2

use crate::domain::entities::AnnotatedTransaction;
use crate::domain::errors::ConflictError;
use crate::domain::value_objects::Conflict;
use crate::ports::outbound::ConflictDetector;
use async_trait::async_trait;
use primitive_types::H256;
use std::collections::{HashMap, HashSet};
use tracing::debug;

/// Detects conflicts between transactions based on overlapping access patterns.
///
/// Two transactions conflict if one writes to a slot that the other reads or writes.
pub struct BalanceBasedConflictDetector {
    /// Whether to perform strict conflict detection.
    strict_mode: bool,
}

impl BalanceBasedConflictDetector {
    /// Create a new detector.
    pub fn new() -> Self {
        Self { strict_mode: true }
    }

    /// Create with configurable strictness.
    pub fn with_strict_mode(strict: bool) -> Self {
        Self { strict_mode: strict }
    }

    /// Check if two transactions conflict based on their access patterns.
    fn check_conflict(
        &self,
        tx1: &AnnotatedTransaction,
        tx2: &AnnotatedTransaction,
    ) -> Option<Conflict> {
        let pattern1 = &tx1.access_pattern;
        let pattern2 = &tx2.access_pattern;

        // Write-Write conflicts
        let writes1: HashSet<_> = pattern1.writes.iter().collect();
        let writes2: HashSet<_> = pattern2.writes.iter().collect();
        let write_write: Vec<_> = writes1.intersection(&writes2).cloned().collect();

        if !write_write.is_empty() {
            return Some(Conflict {
                tx1_hash: tx1.tx_hash,
                tx2_hash: tx2.tx_hash,
                conflict_type: crate::domain::value_objects::ConflictType::WriteWrite,
                conflicting_slots: write_write.into_iter().cloned().collect(),
            });
        }

        // Read-Write conflicts (tx1 reads what tx2 writes)
        let reads1: HashSet<_> = pattern1.reads.iter().collect();
        let read_write: Vec<_> = reads1.intersection(&writes2).cloned().collect();

        if !read_write.is_empty() {
            return Some(Conflict {
                tx1_hash: tx1.tx_hash,
                tx2_hash: tx2.tx_hash,
                conflict_type: crate::domain::value_objects::ConflictType::ReadWrite,
                conflicting_slots: read_write.into_iter().cloned().collect(),
            });
        }

        // Write-Read conflicts (tx1 writes what tx2 reads)
        let reads2: HashSet<_> = pattern2.reads.iter().collect();
        let write_read: Vec<_> = writes1.intersection(&reads2).cloned().collect();

        if !write_read.is_empty() {
            return Some(Conflict {
                tx1_hash: tx1.tx_hash,
                tx2_hash: tx2.tx_hash,
                conflict_type: crate::domain::value_objects::ConflictType::WriteRead,
                conflicting_slots: write_read.into_iter().cloned().collect(),
            });
        }

        // Balance conflicts
        let balance_writes1: HashSet<_> = pattern1.balance_writes.iter().collect();
        let balance_writes2: HashSet<_> = pattern2.balance_writes.iter().collect();
        let balance_conflict: Vec<_> = balance_writes1
            .intersection(&balance_writes2)
            .cloned()
            .collect();

        if !balance_conflict.is_empty() {
            return Some(Conflict {
                tx1_hash: tx1.tx_hash,
                tx2_hash: tx2.tx_hash,
                conflict_type: crate::domain::value_objects::ConflictType::BalanceConflict,
                conflicting_slots: balance_conflict
                    .into_iter()
                    .map(|addr| H256::from_slice(&addr.0))
                    .collect(),
            });
        }

        None
    }
}

impl Default for BalanceBasedConflictDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConflictDetector for BalanceBasedConflictDetector {
    async fn detect_conflicts(
        &self,
        transactions: &[AnnotatedTransaction],
    ) -> Result<Vec<Conflict>, ConflictError> {
        let mut conflicts = Vec::new();

        debug!(
            "[qc-12] Detecting conflicts among {} transactions",
            transactions.len()
        );

        // O(n²) pairwise comparison - acceptable for typical block sizes
        for i in 0..transactions.len() {
            for j in (i + 1)..transactions.len() {
                if let Some(conflict) = self.check_conflict(&transactions[i], &transactions[j]) {
                    debug!(
                        "[qc-12] Found {:?} conflict between tx {} and tx {}",
                        conflict.conflict_type, i, j
                    );
                    conflicts.push(conflict);
                }
            }
        }

        debug!("[qc-12] Found {} total conflicts", conflicts.len());
        Ok(conflicts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{AccessPattern, ConflictType};
    use primitive_types::H160;

    fn make_tx(hash: u8, writes: Vec<H256>) -> AnnotatedTransaction {
        AnnotatedTransaction {
            tx_hash: H256::from([hash; 32]),
            sender: H160::from([hash; 20]),
            nonce: 0,
            gas_price: 1,
            access_pattern: AccessPattern {
                tx_hash: H256::from([hash; 32]),
                reads: vec![],
                writes,
                balance_reads: vec![],
                balance_writes: vec![],
            },
            priority: 0,
        }
    }

    #[tokio::test]
    async fn test_no_conflicts() {
        let detector = BalanceBasedConflictDetector::new();

        let tx1 = make_tx(1, vec![H256::from([1u8; 32])]);
        let tx2 = make_tx(2, vec![H256::from([2u8; 32])]);

        let conflicts = detector.detect_conflicts(&[tx1, tx2]).await.unwrap();
        assert!(conflicts.is_empty());
    }

    #[tokio::test]
    async fn test_write_write_conflict() {
        let detector = BalanceBasedConflictDetector::new();

        let shared_slot = H256::from([99u8; 32]);
        let tx1 = make_tx(1, vec![shared_slot]);
        let tx2 = make_tx(2, vec![shared_slot]);

        let conflicts = detector.detect_conflicts(&[tx1, tx2]).await.unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].conflict_type, ConflictType::WriteWrite);
    }
}
