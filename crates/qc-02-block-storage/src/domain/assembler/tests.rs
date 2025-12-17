//! # Assembler Tests
//!
//! Unit tests for the block assembly buffer.

#[cfg(test)]
mod tests {
    use crate::domain::assembler::{AssemblyConfig, BlockAssemblyBuffer, PendingBlockAssembly};

    use crate::test_utils::make_test_block;

    #[test]
    fn test_assembly_completes_with_all_three() {
        let mut buffer = BlockAssemblyBuffer::with_defaults();
        let block_hash = [0xAB; 32];
        let now = 1000;

        // Add BlockValidated
        buffer.add_block_validated(block_hash, make_test_block(1, [0; 32]), now);
        assert!(!buffer.is_complete(&block_hash));

        // Add MerkleRootComputed
        buffer.add_merkle_root(block_hash, [0xCC; 32], now);
        assert!(!buffer.is_complete(&block_hash));

        // Add StateRootComputed
        buffer.add_state_root(block_hash, [0xDD; 32], now);
        assert!(buffer.is_complete(&block_hash));
    }

    #[test]
    fn test_assembly_works_any_order() {
        let mut buffer = BlockAssemblyBuffer::with_defaults();
        let block_hash = [0xBB; 32];
        let now = 1000;

        // Reverse order: State → Merkle → Block
        buffer.add_state_root(block_hash, [0xDD; 32], now);
        assert!(!buffer.is_complete(&block_hash));

        buffer.add_merkle_root(block_hash, [0xCC; 32], now);
        assert!(!buffer.is_complete(&block_hash));

        buffer.add_block_validated(block_hash, make_test_block(1, [0; 32]), now);
        assert!(buffer.is_complete(&block_hash));
    }

    #[test]
    fn test_gc_expired_assemblies() {
        let config = AssemblyConfig::new(30, 1000);
        let mut buffer = BlockAssemblyBuffer::new(config);

        // Add assemblies at time 1000
        for i in 0..10 {
            let block_hash = [i as u8; 32];
            buffer.add_block_validated(block_hash, make_test_block(i as u64, [0; 32]), 1000);
        }

        assert_eq!(buffer.len(), 10);

        // GC at time 1031 (31 seconds later, past 30s timeout)
        let expired = buffer.gc_expired(1031);
        assert_eq!(expired.len(), 10);
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_enforce_max_pending() {
        let config = AssemblyConfig::new(30, 5);
        let mut buffer = BlockAssemblyBuffer::new(config);

        // Add 10 assemblies with staggered timestamps
        for i in 0..10 {
            let block_hash = [i as u8; 32];
            buffer.add_block_validated(
                block_hash,
                make_test_block(i as u64, [0; 32]),
                1000 + i as u64,
            );
        }

        assert_eq!(buffer.len(), 10);

        // Enforce limit - INVARIANT-8: purges oldest first
        let purged = buffer.enforce_max_pending();
        assert_eq!(purged.len(), 5);
        assert_eq!(buffer.len(), 5);

        // Oldest entries (0-4) purged
        for i in 0..5 {
            let block_hash = [i as u8; 32];
            assert!(buffer.get(&block_hash).is_none());
        }

        // Newest entries (5-9) retained
        for i in 5..10 {
            let block_hash = [i as u8; 32];
            assert!(buffer.get(&block_hash).is_some());
        }
    }

    #[test]
    fn test_take_complete() {
        let mut buffer = BlockAssemblyBuffer::with_defaults();
        let block_hash = [0xCC; 32];
        let now = 1000;

        // Add all components
        buffer.add_block_validated(block_hash, make_test_block(1, [0; 32]), now);
        buffer.add_merkle_root(block_hash, [0x11; 32], now);
        buffer.add_state_root(block_hash, [0x22; 32], now);

        // Take complete
        let assembly = buffer.take_complete(&block_hash);
        assert!(assembly.is_some());
        assert!(buffer.get(&block_hash).is_none());

        // Verify components
        let (block, merkle, state) = assembly.unwrap().take_components().unwrap();
        assert_eq!(block.header.height, 1);
        assert_eq!(merkle, [0x11; 32]);
        assert_eq!(state, [0x22; 32]);
    }

    #[test]
    fn test_pending_assembly_age() {
        let assembly = PendingBlockAssembly::new([0xAA; 32], 1000);
        assert_eq!(assembly.age(1050), 50);
        assert_eq!(assembly.age(1000), 0);
    }
}
