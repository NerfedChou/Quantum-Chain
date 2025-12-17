//! # Storage Tests
//!
//! Unit tests for storage entities.

#[cfg(test)]
mod tests {
    use crate::domain::storage::{BlockIndex, StorageMetadata};

    #[test]
    fn test_block_index_insert_and_get() {
        let mut index = BlockIndex::new();

        index.insert(0, [0x00; 32]);
        index.insert(1, [0x01; 32]);
        index.insert(2, [0x02; 32]);

        assert_eq!(index.get(0), Some([0x00; 32]));
        assert_eq!(index.get(1), Some([0x01; 32]));
        assert_eq!(index.get(2), Some([0x02; 32]));
        assert_eq!(index.get(3), None);
    }

    #[test]
    fn test_block_index_maintains_order() {
        let mut index = BlockIndex::new();

        // Insert out of order
        index.insert(5, [0x05; 32]);
        index.insert(1, [0x01; 32]);
        index.insert(3, [0x03; 32]);

        assert_eq!(index.latest_height(), Some(5));
        assert_eq!(index.len(), 3);
    }

    #[test]
    fn test_storage_metadata_genesis_immutability() {
        let mut meta = StorageMetadata::default();

        // First block at height 0 sets genesis
        meta.on_block_stored(0, [0x01; 32]);
        assert_eq!(meta.genesis_hash, Some([0x01; 32]));

        // Subsequent height 0 blocks don't change genesis
        meta.on_block_stored(0, [0x02; 32]);
        assert_eq!(meta.genesis_hash, Some([0x01; 32]));
    }

    #[test]
    fn test_storage_metadata_finalization_monotonicity() {
        let mut meta = StorageMetadata::default();

        // Finalize height 5
        assert!(meta.on_finalized(5));
        assert_eq!(meta.finalized_height, 5);

        // Cannot regress to height 3
        assert!(!meta.on_finalized(3));
        assert_eq!(meta.finalized_height, 5);

        // Cannot re-finalize same height
        assert!(!meta.on_finalized(5));
        assert_eq!(meta.finalized_height, 5);

        // Can finalize higher
        assert!(meta.on_finalized(7));
        assert_eq!(meta.finalized_height, 7);
    }
}
