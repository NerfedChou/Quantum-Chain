//! Value objects for Transaction Ordering
//!
//! Reference: SPEC-12 Section 2.1 (Lines 81-141)

use primitive_types::{H160, H256};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Type aliases for clarity
pub type Hash = H256;
pub type Address = H160;
pub type StorageKey = H256;

/// Dependency type between transactions
/// Reference: SPEC-12 Lines 81-89
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DependencyKind {
    /// To reads what From writes (Read-After-Write)
    ReadAfterWrite,
    /// Both write to same location (Write-After-Write)
    WriteAfterWrite,
    /// Same sender, nonce ordering required
    NonceOrder,
}

/// Storage location (contract address + slot)
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StorageLocation {
    pub address: Address,
    pub key: StorageKey,
}

impl StorageLocation {
    pub fn new(address: Address, key: StorageKey) -> Self {
        Self { address, key }
    }
}

/// Access pattern for a transaction
/// Reference: SPEC-12 Lines 254-261
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AccessPattern {
    /// Storage keys read
    pub reads: HashSet<StorageLocation>,
    /// Storage keys written
    pub writes: HashSet<StorageLocation>,
    /// Balance reads
    pub balance_reads: HashSet<Address>,
    /// Balance writes
    pub balance_writes: HashSet<Address>,
}

impl AccessPattern {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_reads(mut self, reads: Vec<StorageLocation>) -> Self {
        self.reads = reads.into_iter().collect();
        self
    }

    pub fn with_writes(mut self, writes: Vec<StorageLocation>) -> Self {
        self.writes = writes.into_iter().collect();
        self
    }

    /// Check if this pattern conflicts with another
    pub fn conflicts_with(&self, other: &AccessPattern) -> Option<DependencyKind> {
        // Write-After-Write: both write to same location
        for loc in &self.writes {
            if other.writes.contains(loc) {
                return Some(DependencyKind::WriteAfterWrite);
            }
        }

        // Read-After-Write: we read what other writes
        for loc in &self.reads {
            if other.writes.contains(loc) {
                return Some(DependencyKind::ReadAfterWrite);
            }
        }

        // Write-After-Read: we write what other reads (also RAW from other's perspective)
        for loc in &self.writes {
            if other.reads.contains(loc) {
                return Some(DependencyKind::ReadAfterWrite);
            }
        }

        None
    }
}

/// Conflict between two transactions
/// Reference: SPEC-12 Lines 263-270
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub tx1: Hash,
    pub tx2: Hash,
    pub kind: DependencyKind,
    pub location: Option<StorageLocation>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc(addr: u8, key: u8) -> StorageLocation {
        StorageLocation::new(
            H160::from_low_u64_be(addr as u64),
            H256::from_low_u64_be(key as u64),
        )
    }

    #[test]
    fn test_dependency_kind_equality() {
        assert_eq!(
            DependencyKind::ReadAfterWrite,
            DependencyKind::ReadAfterWrite
        );
        assert_ne!(
            DependencyKind::ReadAfterWrite,
            DependencyKind::WriteAfterWrite
        );
    }

    #[test]
    fn test_access_pattern_no_conflict() {
        let p1 = AccessPattern::new().with_writes(vec![loc(1, 1)]);
        let p2 = AccessPattern::new().with_writes(vec![loc(2, 2)]);

        assert!(p1.conflicts_with(&p2).is_none());
    }

    #[test]
    fn test_access_pattern_write_after_write_conflict() {
        let p1 = AccessPattern::new().with_writes(vec![loc(1, 1)]);
        let p2 = AccessPattern::new().with_writes(vec![loc(1, 1)]);

        assert_eq!(
            p1.conflicts_with(&p2),
            Some(DependencyKind::WriteAfterWrite)
        );
    }

    #[test]
    fn test_access_pattern_read_after_write_conflict() {
        let p1 = AccessPattern::new().with_reads(vec![loc(1, 1)]);
        let p2 = AccessPattern::new().with_writes(vec![loc(1, 1)]);

        assert_eq!(p1.conflicts_with(&p2), Some(DependencyKind::ReadAfterWrite));
    }
}
