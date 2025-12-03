use super::{Address, Hash, StorageKey};
use serde::{Deserialize, Serialize};

/// Transaction access pattern for conflict detection
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionAccessPattern {
    pub tx_hash: Hash,
    pub reads: Vec<(Address, Option<StorageKey>)>,
    pub writes: Vec<(Address, Option<StorageKey>)>,
}

impl TransactionAccessPattern {
    pub fn new(tx_hash: Hash) -> Self {
        Self {
            tx_hash,
            reads: vec![],
            writes: vec![],
        }
    }

    pub fn with_reads(mut self, reads: Vec<(Address, Option<StorageKey>)>) -> Self {
        self.reads = reads;
        self
    }

    pub fn with_writes(mut self, writes: Vec<(Address, Option<StorageKey>)>) -> Self {
        self.writes = writes;
        self
    }
}

/// Conflict type between transactions
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictType {
    ReadWrite,
    WriteWrite,
    NonceConflict,
}

/// Conflict information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConflictInfo {
    pub tx1_index: usize,
    pub tx2_index: usize,
    pub conflict_type: ConflictType,
    pub conflicting_address: Address,
    pub conflicting_key: Option<StorageKey>,
}

/// Detect conflicts between transaction access patterns
#[allow(clippy::excessive_nesting)]
pub fn detect_conflicts(patterns: &[TransactionAccessPattern]) -> Vec<ConflictInfo> {
    let mut conflicts = Vec::new();

    for i in 0..patterns.len() {
        for j in (i + 1)..patterns.len() {
            let p1 = &patterns[i];
            let p2 = &patterns[j];

            // Check Write-Write conflicts
            for (addr1, key1) in &p1.writes {
                for (addr2, key2) in &p2.writes {
                    if addr1 == addr2 && key1 == key2 {
                        conflicts.push(ConflictInfo {
                            tx1_index: i,
                            tx2_index: j,
                            conflict_type: ConflictType::WriteWrite,
                            conflicting_address: *addr1,
                            conflicting_key: *key1,
                        });
                    }
                }
            }

            // Check Read-Write conflicts (p1 reads, p2 writes)
            for (addr1, key1) in &p1.reads {
                for (addr2, key2) in &p2.writes {
                    if addr1 == addr2 && key1 == key2 {
                        conflicts.push(ConflictInfo {
                            tx1_index: i,
                            tx2_index: j,
                            conflict_type: ConflictType::ReadWrite,
                            conflicting_address: *addr1,
                            conflicting_key: *key1,
                        });
                    }
                }
            }

            // Check Read-Write conflicts (p1 writes, p2 reads)
            for (addr1, key1) in &p1.writes {
                for (addr2, key2) in &p2.reads {
                    if addr1 == addr2 && key1 == key2 {
                        conflicts.push(ConflictInfo {
                            tx1_index: i,
                            tx2_index: j,
                            conflict_type: ConflictType::ReadWrite,
                            conflicting_address: *addr1,
                            conflicting_key: *key1,
                        });
                    }
                }
            }
        }
    }

    conflicts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_write_write_conflict() {
        let contract = [0x42u8; 20];
        let slot = [0x01u8; 32];

        let patterns = vec![
            TransactionAccessPattern::new([1u8; 32])
                .with_writes(vec![(contract, Some(slot))]),
            TransactionAccessPattern::new([2u8; 32])
                .with_writes(vec![(contract, Some(slot))]),
        ];

        let conflicts = detect_conflicts(&patterns);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].conflict_type, ConflictType::WriteWrite);
    }

    #[test]
    fn test_detect_read_write_conflict() {
        let contract = [0x42u8; 20];
        let slot = [0x01u8; 32];

        let patterns = vec![
            TransactionAccessPattern::new([1u8; 32])
                .with_reads(vec![(contract, Some(slot))]),
            TransactionAccessPattern::new([2u8; 32])
                .with_writes(vec![(contract, Some(slot))]),
        ];

        let conflicts = detect_conflicts(&patterns);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].conflict_type, ConflictType::ReadWrite);
    }

    #[test]
    fn test_no_conflict_different_slots() {
        let contract = [0x42u8; 20];
        let slot1 = [0x01u8; 32];
        let slot2 = [0x02u8; 32];

        let patterns = vec![
            TransactionAccessPattern::new([1u8; 32])
                .with_writes(vec![(contract, Some(slot1))]),
            TransactionAccessPattern::new([2u8; 32])
                .with_writes(vec![(contract, Some(slot2))]),
        ];

        let conflicts = detect_conflicts(&patterns);
        assert!(conflicts.is_empty());
    }
}
