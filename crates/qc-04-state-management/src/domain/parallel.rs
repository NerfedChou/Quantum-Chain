//! # Parallel Storage Root Computation
//!
//! Parallel computation of contract storage roots using rayon.
//!
//! ## Problem
//!
//! A block might update storage in 50 different smart contracts.
//! Sequential storage_root computation is slow.
//!
//! ## Solution: Map-Reduce State Commit
//!
//! 1. Group: Collect all AccountTransition objects
//! 2. Filter: Identify accounts with storage_changes
//! 3. Map (Parallel): Compute new storage_root for each contract
//! 4. Reduce (Sequential): Update main trie with new roots

use super::{Address, Hash, StorageKey, StorageValue};
use rayon::prelude::*;
use sha3::{Digest, Keccak256};

/// Storage update for a single account.
#[derive(Clone, Debug)]
pub struct StorageUpdate {
    pub address: Address,
    pub changes: Vec<(StorageKey, Option<StorageValue>)>,
}

/// Result of parallel storage root computation.
#[derive(Clone, Debug)]
pub struct StorageRootResult {
    pub address: Address,
    pub new_storage_root: Hash,
}

/// Parallel threshold - use sequential for small batches.
pub const PARALLEL_THRESHOLD: usize = 4;

/// Compute storage roots for multiple accounts in parallel.
///
/// ## Algorithm: Map-Reduce State Commit
///
/// Uses rayon par_iter to compute independent storage roots concurrently.
/// Falls back to sequential for small batches (< PARALLEL_THRESHOLD).
pub fn compute_storage_roots_parallel(
    updates: Vec<StorageUpdate>,
    get_current_root: impl Fn(&Address) -> Hash + Sync,
) -> Vec<StorageRootResult> {
    if updates.len() < PARALLEL_THRESHOLD {
        // Sequential for small batches
        updates
            .into_iter()
            .map(|update| compute_single_storage_root(update, &get_current_root))
            .collect()
    } else {
        // Parallel for large batches
        updates
            .into_par_iter()
            .map(|update| compute_single_storage_root(update, &get_current_root))
            .collect()
    }
}

/// Compute storage root for a single account.
fn compute_single_storage_root(
    update: StorageUpdate,
    get_current_root: &impl Fn(&Address) -> Hash,
) -> StorageRootResult {
    let current_root = get_current_root(&update.address);

    // Simplified: hash all changes together
    // Full implementation would update actual storage trie
    let mut hasher = Keccak256::new();
    hasher.update(current_root);

    for (key, value) in &update.changes {
        hasher.update(key);
        if let Some(v) = value {
            hasher.update(v);
        }
    }

    StorageRootResult {
        address: update.address,
        new_storage_root: hasher.finalize().into(),
    }
}

/// Batch state transition with parallel storage root computation.
#[derive(Clone, Debug)]
pub struct ParallelStateTransition {
    pub block_hash: Hash,
    pub block_height: u64,
    pub storage_updates: Vec<StorageUpdate>,
}

impl ParallelStateTransition {
    /// Execute parallel storage root computation.
    pub fn execute(
        &self,
        get_current_root: impl Fn(&Address) -> Hash + Sync,
    ) -> Vec<StorageRootResult> {
        compute_storage_roots_parallel(self.storage_updates.clone(), get_current_root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_root_getter(_addr: &Address) -> Hash {
        [0x00; 32]
    }

    #[test]
    fn test_parallel_threshold() {
        assert!(PARALLEL_THRESHOLD >= 2);
        assert!(PARALLEL_THRESHOLD <= 16);
    }

    #[test]
    fn test_single_storage_root() {
        let update = StorageUpdate {
            address: [0x01; 20],
            changes: vec![([0xAA; 32], Some([0xBB; 32]))],
        };

        let result = compute_single_storage_root(update, &dummy_root_getter);
        assert_eq!(result.address, [0x01; 20]);
        assert_ne!(result.new_storage_root, [0x00; 32]);
    }

    #[test]
    fn test_parallel_computation() {
        let updates: Vec<StorageUpdate> = (0..10u8)
            .map(|i| StorageUpdate {
                address: [i; 20],
                changes: vec![([i; 32], Some([i; 32]))],
            })
            .collect();

        let results = compute_storage_roots_parallel(updates, dummy_root_getter);
        assert_eq!(results.len(), 10);
    }

    #[test]
    fn test_sequential_for_small_batch() {
        let updates: Vec<StorageUpdate> = (0..2u8)
            .map(|i| StorageUpdate {
                address: [i; 20],
                changes: vec![([i; 32], Some([i; 32]))],
            })
            .collect();

        // Should use sequential path (< PARALLEL_THRESHOLD)
        let results = compute_storage_roots_parallel(updates, dummy_root_getter);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_deterministic_roots() {
        let update = StorageUpdate {
            address: [0x01; 20],
            changes: vec![([0xAA; 32], Some([0xBB; 32]))],
        };

        let result1 = compute_single_storage_root(update.clone(), &dummy_root_getter);
        let result2 = compute_single_storage_root(update, &dummy_root_getter);

        assert_eq!(result1.new_storage_root, result2.new_storage_root);
    }
}
