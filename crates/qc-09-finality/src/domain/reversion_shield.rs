//! # Finality Reversion Shield
//!
//! Protects finalized blocks from being reverted by consensus reorgs.
//!
//! ## Threat
//!
//! A 51% attack on block production tries to reorg a finalized block.
//!
//! ## Solution: Immutable Barrier
//!
//! Any block that doesn't descend from the last finalized block is INVALID,
//! regardless of chain weight or accumulated work.
//!
//! Reference: SPEC-09-FINALITY.md

use shared_types::Hash;
use std::collections::{HashMap, HashSet};

/// Finality reversion shield.
///
/// Prevents accepting any chain that conflicts with finalized checkpoints.
#[derive(Debug)]
pub struct ReversionShield {
    /// Last finalized block hash
    last_finalized: Option<Hash>,
    /// Last finalized block height
    last_finalized_height: u64,
    /// Known ancestors of finalized block (for quick lookups)
    finalized_ancestors: HashSet<Hash>,
    /// Parent -> Child mapping for ancestor checks
    block_tree: HashMap<Hash, Hash>,
}

impl Default for ReversionShield {
    fn default() -> Self {
        Self::new()
    }
}

impl ReversionShield {
    pub fn new() -> Self {
        Self {
            last_finalized: None,
            last_finalized_height: 0,
            finalized_ancestors: HashSet::new(),
            block_tree: HashMap::new(),
        }
    }

    /// Update the last finalized block.
    pub fn set_finalized(&mut self, block_hash: Hash, height: u64) {
        self.last_finalized = Some(block_hash);
        self.last_finalized_height = height;
        self.finalized_ancestors.insert(block_hash);
    }

    /// Record a block in the tree (for ancestry checks).
    pub fn record_block(&mut self, block_hash: Hash, parent_hash: Hash) {
        self.block_tree.insert(block_hash, parent_hash);
    }

    /// Check if a block is valid according to the reversion shield.
    ///
    /// A block is valid iff:
    /// 1. No finalized checkpoint exists (genesis), OR
    /// 2. The block is the finalized block itself, OR
    /// 3. The block descends from the finalized block
    pub fn is_valid_block(&self, block_hash: &Hash, block_height: u64) -> bool {
        match &self.last_finalized {
            None => true, // No finality yet
            Some(finalized) => {
                if block_hash == finalized {
                    return true;
                }

                // Block must be at height > finalized to be a descendant
                if block_height <= self.last_finalized_height {
                    // Could be an ancestor of finalized, which is valid
                    return self.finalized_ancestors.contains(block_hash);
                }

                // Check if block descends from finalized
                self.is_descendant(block_hash, finalized)
            }
        }
    }

    /// Check if block A descends from block B.
    fn is_descendant(&self, block_a: &Hash, block_b: &Hash) -> bool {
        if block_a == block_b {
            return true;
        }

        // Walk up the tree from block_a
        let mut current = block_a;
        let max_depth = 1000; // Prevent infinite loops

        for _ in 0..max_depth {
            match self.block_tree.get(current) {
                Some(parent) => {
                    if parent == block_b {
                        return true;
                    }
                    current = parent;
                }
                None => return false,
            }
        }

        false
    }

    /// Check if a reorg would conflict with finality.
    ///
    /// Returns true if the reorg target conflicts with finalized block.
    pub fn would_reorg_conflict(&self, reorg_target: &Hash) -> bool {
        match &self.last_finalized {
            None => false,
            Some(finalized) => {
                // Conflict if reorg target is not an ancestor of finalized
                !self.is_descendant(finalized, reorg_target)
            }
        }
    }

    /// Get last finalized block.
    pub fn last_finalized(&self) -> Option<Hash> {
        self.last_finalized
    }

    /// Get last finalized height.
    pub fn last_finalized_height(&self) -> u64 {
        self.last_finalized_height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(id: u8) -> Hash {
        [id; 32]
    }

    #[test]
    fn test_no_finality_allows_all() {
        let shield = ReversionShield::new();

        assert!(shield.is_valid_block(&block(1), 100));
        assert!(shield.is_valid_block(&block(2), 200));
    }

    #[test]
    fn test_finalized_block_is_valid() {
        let mut shield = ReversionShield::new();
        shield.set_finalized(block(5), 100);

        assert!(shield.is_valid_block(&block(5), 100));
    }

    #[test]
    fn test_descendant_is_valid() {
        let mut shield = ReversionShield::new();

        // Build chain: 1 <- 2 <- 3 <- 4 <- 5
        shield.record_block(block(2), block(1));
        shield.record_block(block(3), block(2));
        shield.record_block(block(4), block(3));
        shield.record_block(block(5), block(4));

        // Finalize block 3
        shield.set_finalized(block(3), 50);

        // Block 5 descends from 3
        assert!(shield.is_valid_block(&block(5), 70));
    }

    #[test]
    fn test_non_descendant_is_invalid() {
        let mut shield = ReversionShield::new();

        // Build main chain: 1 <- 2 <- 3
        shield.record_block(block(2), block(1));
        shield.record_block(block(3), block(2));

        // Build fork: 1 <- 10 <- 11
        shield.record_block(block(10), block(1));
        shield.record_block(block(11), block(10));

        // Finalize block 3
        shield.set_finalized(block(3), 50);

        // Block 11 does NOT descend from 3
        assert!(!shield.is_valid_block(&block(11), 60));
    }
}
