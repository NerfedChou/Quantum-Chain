//! # Domain Invariants
//!
//! Business rules that must always hold true for sharding.
//!
//! Reference: SPEC-14 Section 2.3 (Lines 144-173)

use super::errors::{Address, Hash, ShardError, ShardId};

/// Minimum shard count.
pub const MIN_SHARD_COUNT: u16 = 1;

/// Maximum shard count.
pub const MAX_SHARD_COUNT: u16 = 1024;

/// Minimum validators per shard.
/// Reference: System.md Line 700
pub const MIN_VALIDATORS_PER_SHARD: usize = 128;

/// Signature threshold for 2/3 majority.
pub const SIGNATURE_THRESHOLD: f64 = 2.0 / 3.0;

/// Invariant: Shard assignment is deterministic.
/// Reference: SPEC-14 Lines 147-153
///
/// Same address + same shard count = same shard ID.
pub fn invariant_deterministic_assignment<F>(
    assign_fn: F,
    address: &Address,
    shard_count: u16,
) -> bool
where
    F: Fn(&Address, u16) -> ShardId,
{
    let first = assign_fn(address, shard_count);
    let second = assign_fn(address, shard_count);
    first == second
}

/// Invariant: Cross-shard transactions are atomic.
/// Reference: SPEC-14 Lines 155-163
///
/// Either all shards commit or all abort - never partial.
pub fn invariant_cross_shard_atomic(
    committed_shards: &[ShardId],
    aborted_shards: &[ShardId],
    total_shards: &[ShardId],
) -> Result<(), ShardError> {
    // Check for overlap
    for shard in committed_shards {
        if aborted_shards.contains(shard) {
            return Err(ShardError::StateInconsistency(
                "Shard both committed and aborted".to_string(),
            ));
        }
    }

    // Must be all committed or all aborted
    let all_committed = committed_shards.len() == total_shards.len();
    let all_aborted = aborted_shards.len() == total_shards.len();
    let in_progress = committed_shards.is_empty() && aborted_shards.is_empty();

    if !all_committed && !all_aborted && !in_progress {
        return Err(ShardError::StateInconsistency(
            "Partial commit/abort not allowed".to_string(),
        ));
    }

    Ok(())
}

/// Invariant: Global state root is consistent across all shards.
/// Reference: SPEC-14 Lines 165-172
pub fn invariant_global_consistency(
    shard_roots: &[(ShardId, Hash)],
    expected_shards: &[ShardId],
) -> Result<(), ShardError> {
    // All expected shards must have a root
    for shard in expected_shards {
        if !shard_roots.iter().any(|(id, _)| id == shard) {
            return Err(ShardError::StateInconsistency(format!(
                "Missing state root for shard {}",
                shard
            )));
        }
    }

    // No duplicate shard roots
    let mut seen = std::collections::HashSet::new();
    for (shard, _) in shard_roots {
        if !seen.insert(shard) {
            return Err(ShardError::StateInconsistency(format!(
                "Duplicate state root for shard {}",
                shard
            )));
        }
    }

    Ok(())
}

/// Invariant: Validator count meets minimum threshold.
/// Reference: System.md Line 700
pub fn invariant_min_validators(
    validator_count: usize,
    min_required: usize,
) -> Result<(), ShardError> {
    if validator_count < min_required {
        return Err(ShardError::InsufficientSignatures {
            got: validator_count,
            required: min_required,
        });
    }
    Ok(())
}

/// Invariant: Signature threshold met.
/// Reference: SPEC-14 Lines 160-167
pub fn invariant_signature_threshold(
    valid_signatures: usize,
    total_validators: usize,
) -> Result<(), ShardError> {
    let threshold = (total_validators as f64 * SIGNATURE_THRESHOLD).ceil() as usize;
    if valid_signatures < threshold {
        return Err(ShardError::InsufficientSignatures {
            got: valid_signatures,
            required: threshold,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_assign(address: &Address, shard_count: u16) -> ShardId {
        (address[0] as u16 + address[1] as u16) % shard_count
    }

    #[test]
    fn test_invariant_deterministic_assignment() {
        let addr = [5u8; 20];
        assert!(invariant_deterministic_assignment(simple_assign, &addr, 16));
    }

    #[test]
    fn test_invariant_cross_shard_atomic_all_committed() {
        let total = vec![0, 1, 2];
        let committed = vec![0, 1, 2];
        let aborted = vec![];
        assert!(invariant_cross_shard_atomic(&committed, &aborted, &total).is_ok());
    }

    #[test]
    fn test_invariant_cross_shard_atomic_all_aborted() {
        let total = vec![0, 1, 2];
        let committed = vec![];
        let aborted = vec![0, 1, 2];
        assert!(invariant_cross_shard_atomic(&committed, &aborted, &total).is_ok());
    }

    #[test]
    fn test_invariant_cross_shard_atomic_partial_fails() {
        let total = vec![0, 1, 2];
        let committed = vec![0, 1];
        let aborted = vec![];
        assert!(invariant_cross_shard_atomic(&committed, &aborted, &total).is_err());
    }

    #[test]
    fn test_invariant_cross_shard_overlap_fails() {
        let total = vec![0, 1];
        let committed = vec![0];
        let aborted = vec![0]; // Same shard in both!
        assert!(invariant_cross_shard_atomic(&committed, &aborted, &total).is_err());
    }

    #[test]
    fn test_invariant_global_consistency_pass() {
        let roots = vec![(0u16, [1u8; 32]), (1, [2u8; 32])];
        let expected = vec![0, 1];
        assert!(invariant_global_consistency(&roots, &expected).is_ok());
    }

    #[test]
    fn test_invariant_global_consistency_missing() {
        let roots = vec![(0u16, [1u8; 32])];
        let expected = vec![0, 1]; // Shard 1 missing
        assert!(invariant_global_consistency(&roots, &expected).is_err());
    }

    #[test]
    fn test_invariant_min_validators_pass() {
        assert!(invariant_min_validators(128, 128).is_ok());
    }

    #[test]
    fn test_invariant_min_validators_fail() {
        assert!(invariant_min_validators(100, 128).is_err());
    }

    #[test]
    fn test_invariant_signature_threshold_pass() {
        // 2/3 of 9 = 6
        assert!(invariant_signature_threshold(6, 9).is_ok());
    }

    #[test]
    fn test_invariant_signature_threshold_fail() {
        // 2/3 of 9 = 6, but only 5
        assert!(invariant_signature_threshold(5, 9).is_err());
    }
}
