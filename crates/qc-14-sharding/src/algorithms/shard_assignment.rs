//! # Shard Assignment Algorithm
//!
//! Deterministic shard assignment using consistent hashing.
//!
//! Reference: System.md Lines 676-680, SPEC-14 Lines 120-142

use crate::domain::{Address, Hash, ShardId};
use sha3::{Digest, Keccak256};

/// Simple modulo-based shard assignment.
///
/// Reference: System.md Line 680 - "Hash(account) % num_shards"
///
/// Fast but causes many reassignments when shard count changes.
pub fn assign_shard(address: &Address, shard_count: u16) -> ShardId {
    if shard_count == 0 {
        return 0;
    }

    let hash = keccak256(address);
    let value = u16::from_be_bytes([hash[0], hash[1]]);
    value % shard_count
}

/// Rendezvous hashing for minimal reassignment.
///
/// Reference: SPEC-14 Lines 131-141
///
/// When adding shard N, only 1/N addresses move to the new shard.
/// Also known as "highest random weight" hashing.
///
/// # Optimization
///
/// Pre-allocates a single buffer for all hash computations to avoid
/// repeated allocations in the hot path.
pub fn rendezvous_assign(address: &Address, shards: &[ShardId]) -> ShardId {
    if shards.is_empty() {
        return 0;
    }

    if shards.len() == 1 {
        return shards[0];
    }

    // OPTIMIZATION: Pre-allocate buffer outside loop
    let mut input = [0u8; 22]; // 20 bytes address + 2 bytes shard ID
    input[..20].copy_from_slice(address);

    let mut best_shard = shards[0];
    let mut best_hash = [0u8; 32];

    for shard in shards {
        // Copy shard ID into buffer
        input[20..22].copy_from_slice(&shard.to_be_bytes());
        let combined = keccak256(&input);

        if combined > best_hash {
            best_hash = combined;
            best_shard = *shard;
        }
    }

    best_shard
}

/// Detect if a transaction is cross-shard.
///
/// Reference: SPEC-14 Line 427
pub fn is_cross_shard(sender: &Address, recipients: &[Address], shard_count: u16) -> bool {
    let sender_shard = assign_shard(sender, shard_count);

    recipients
        .iter()
        .any(|r| assign_shard(r, shard_count) != sender_shard)
}

/// Get all shards involved in a transaction.
pub fn get_involved_shards(
    sender: &Address,
    recipients: &[Address],
    shard_count: u16,
) -> Vec<ShardId> {
    let mut shards = vec![assign_shard(sender, shard_count)];

    for recipient in recipients {
        let shard = assign_shard(recipient, shard_count);
        if !shards.contains(&shard) {
            shards.push(shard);
        }
    }

    shards
}

/// Helper: keccak256 hash.
fn keccak256(data: &[u8]) -> Hash {
    let mut hasher = Keccak256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut output = [0u8; 32];
    output.copy_from_slice(&result);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_address(n: u8) -> Address {
        let mut addr = [0u8; 20];
        addr[0] = n;
        addr
    }

    #[test]
    fn test_assign_shard_deterministic() {
        let addr = make_address(42);
        let shard1 = assign_shard(&addr, 16);
        let shard2 = assign_shard(&addr, 16);
        assert_eq!(shard1, shard2);
    }

    #[test]
    fn test_assign_shard_within_range() {
        for i in 0..100 {
            let addr = make_address(i);
            let shard = assign_shard(&addr, 16);
            assert!(shard < 16);
        }
    }

    #[test]
    fn test_assign_shard_zero_count() {
        let addr = make_address(1);
        assert_eq!(assign_shard(&addr, 0), 0);
    }

    #[test]
    fn test_rendezvous_assign_deterministic() {
        let addr = make_address(42);
        let shards = vec![0, 1, 2, 3];
        let shard1 = rendezvous_assign(&addr, &shards);
        let shard2 = rendezvous_assign(&addr, &shards);
        assert_eq!(shard1, shard2);
    }

    #[test]
    fn test_rendezvous_assign_within_shards() {
        let addr = make_address(42);
        let shards = vec![0, 1, 2, 3];
        let shard = rendezvous_assign(&addr, &shards);
        assert!(shards.contains(&shard));
    }

    #[test]
    fn test_rendezvous_minimal_reassignment() {
        // When adding a new shard, approximately 1/n addresses should move
        let shards_4 = vec![0, 1, 2, 3];
        let shards_5 = vec![0, 1, 2, 3, 4];

        let mut moved = 0;
        for i in 0..100 {
            let addr = make_address(i);
            let old_shard = rendezvous_assign(&addr, &shards_4);
            let new_shard = rendezvous_assign(&addr, &shards_5);
            if old_shard != new_shard {
                moved += 1;
            }
        }

        // Expect roughly 20% to move (1/5), allow 10-40%
        assert!(moved >= 10 && moved <= 40, "Moved {} addresses", moved);
    }

    #[test]
    fn test_is_cross_shard_same() {
        let sender = make_address(1);
        let recipient = make_address(1); // Same address = same shard
        assert!(!is_cross_shard(&sender, &[recipient], 16));
    }

    #[test]
    fn test_is_cross_shard_different() {
        // Find two addresses in different shards
        let sender = make_address(0);
        let mut recipient = make_address(1);
        while assign_shard(&sender, 4) == assign_shard(&recipient, 4) {
            recipient[0] += 1;
        }
        assert!(is_cross_shard(&sender, &[recipient], 4));
    }

    #[test]
    fn test_get_involved_shards() {
        let sender = make_address(0);
        let recipients = vec![make_address(1), make_address(2)];
        let shards = get_involved_shards(&sender, &recipients, 64);
        // At least sender's shard
        assert!(!shards.is_empty());
    }
}
