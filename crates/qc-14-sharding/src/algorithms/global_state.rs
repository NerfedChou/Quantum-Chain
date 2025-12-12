//! # Global State Root Computation
//!
//! Compute global state root from shard roots.
//!
//! Reference: SPEC-14 Lines 165-172

use crate::domain::{GlobalStateRoot, Hash, ShardStateRoot};
use sha3::{Digest, Keccak256};

/// Compute global state root from shard roots.
///
/// Reference: SPEC-14 Line 455
///
/// Uses binary Merkle tree over sorted shard roots.
pub fn compute_global_state_root(
    shard_roots: &[ShardStateRoot],
    block_height: u64,
    epoch: u64,
) -> GlobalStateRoot {
    if shard_roots.is_empty() {
        return GlobalStateRoot::new([0u8; 32], vec![], block_height, epoch);
    }

    // Sort shard roots by shard_id for determinism
    let mut sorted = shard_roots.to_vec();
    sorted.sort_by_key(|r| r.shard_id);

    // Compute Merkle root
    let root = compute_merkle_root(&sorted.iter().map(|r| r.state_root).collect::<Vec<_>>());

    GlobalStateRoot::new(root, sorted, block_height, epoch)
}

/// Compute Merkle root from list of hashes.
fn compute_merkle_root(hashes: &[Hash]) -> Hash {
    if hashes.is_empty() {
        return [0u8; 32];
    }

    if hashes.len() == 1 {
        return hashes[0];
    }

    let mut level: Vec<Hash> = hashes.to_vec();

    while level.len() > 1 {
        let mut next_level = Vec::with_capacity((level.len() + 1) / 2);

        for chunk in level.chunks(2) {
            let left = &chunk[0];
            let right = chunk.get(1).unwrap_or(left);
            next_level.push(hash_concat(left, right));
        }

        level = next_level;
    }

    level[0]
}

/// Hash concatenation.
fn hash_concat(left: &Hash, right: &Hash) -> Hash {
    let mut hasher = Keccak256::new();
    hasher.update(left);
    hasher.update(right);
    let result = hasher.finalize();
    let mut output = [0u8; 32];
    output.copy_from_slice(&result);
    output
}

/// Verify a shard root is included in global state.
pub fn verify_shard_inclusion(shard_root: &Hash, proof: &[Hash], global_root: &Hash) -> bool {
    let mut current = *shard_root;

    for sibling in proof {
        current = if current < *sibling {
            hash_concat(&current, sibling)
        } else {
            hash_concat(sibling, &current)
        };
    }

    current == *global_root
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_shard_root(shard_id: u16, value: u8) -> ShardStateRoot {
        let mut hash = [0u8; 32];
        hash[0] = value;
        ShardStateRoot::new(shard_id, hash, 100, 10)
    }

    #[test]
    fn test_global_state_root_empty() {
        let global = compute_global_state_root(&[], 100, 10);
        assert_eq!(global.root, [0u8; 32]);
        assert!(global.shard_roots.is_empty());
    }

    #[test]
    fn test_global_state_root_single() {
        let roots = vec![make_shard_root(0, 1)];
        let global = compute_global_state_root(&roots, 100, 10);
        assert_eq!(global.root, roots[0].state_root);
    }

    #[test]
    fn test_global_state_root_deterministic() {
        let roots = vec![
            make_shard_root(0, 1),
            make_shard_root(1, 2),
            make_shard_root(2, 3),
            make_shard_root(3, 4),
        ];

        let global1 = compute_global_state_root(&roots, 100, 10);
        let global2 = compute_global_state_root(&roots, 100, 10);

        assert_eq!(global1.root, global2.root);
    }

    #[test]
    fn test_global_state_root_order_independent() {
        let roots1 = vec![make_shard_root(0, 1), make_shard_root(1, 2)];
        let roots2 = vec![make_shard_root(1, 2), make_shard_root(0, 1)];

        let global1 = compute_global_state_root(&roots1, 100, 10);
        let global2 = compute_global_state_root(&roots2, 100, 10);

        assert_eq!(global1.root, global2.root);
    }

    #[test]
    fn test_global_state_root_different_values() {
        let roots1 = vec![make_shard_root(0, 1)];
        let roots2 = vec![make_shard_root(0, 2)];

        let global1 = compute_global_state_root(&roots1, 100, 10);
        let global2 = compute_global_state_root(&roots2, 100, 10);

        assert_ne!(global1.root, global2.root);
    }

    #[test]
    fn test_global_state_root_sorted() {
        let roots = vec![
            make_shard_root(3, 4),
            make_shard_root(1, 2),
            make_shard_root(0, 1),
            make_shard_root(2, 3),
        ];

        let global = compute_global_state_root(&roots, 100, 10);

        // Should be sorted by shard_id
        assert_eq!(global.shard_roots[0].shard_id, 0);
        assert_eq!(global.shard_roots[1].shard_id, 1);
        assert_eq!(global.shard_roots[2].shard_id, 2);
        assert_eq!(global.shard_roots[3].shard_id, 3);
    }
}
