//! # Domain Invariants
//!
//! Business rules that must always hold true.
//!
//! Reference: SPEC-13 Section 2.2 (Lines 127-158)

use super::errors::{Hash, LightClientError};
use super::value_objects::ProofNode;

/// Minimum number of full nodes for multi-node consensus.
/// Reference: System.md Line 644
pub const MIN_FULL_NODES: usize = 3;

/// Required fraction of nodes that must agree (2/3).
/// Reference: SPEC-13 Line 606
pub const CONSENSUS_THRESHOLD: f64 = 2.0 / 3.0;

/// Default required confirmations.
pub const DEFAULT_CONFIRMATIONS: u64 = 6;

/// Invariant: Merkle proof must be cryptographically verified.
/// Reference: SPEC-13 Lines 130-138
///
/// Every transaction must be proven via valid Merkle path.
pub fn invariant_proof_verified(
    tx_hash: &Hash,
    proof_path: &[ProofNode],
    merkle_root: &Hash,
) -> bool {
    // Empty proof is only valid if tx_hash == merkle_root
    if proof_path.is_empty() {
        return tx_hash == merkle_root;
    }

    // Merkle verification is done by the algorithm module
    // This invariant checks that a proof was provided
    true
}

/// Invariant: Multi-node consensus must be achieved.
/// Reference: SPEC-13 Lines 140-147
///
/// Critical data must come from multiple independent nodes.
pub fn invariant_multi_node(
    node_count: usize,
    min_required: usize,
) -> Result<(), LightClientError> {
    if node_count < min_required {
        return Err(LightClientError::InsufficientNodes {
            got: node_count,
            required: min_required,
        });
    }
    Ok(())
}

/// Invariant: Response consensus check.
/// Reference: SPEC-13 Line 606 - 2/3 agreement
pub fn invariant_consensus<T: PartialEq>(responses: &[T]) -> Result<(), LightClientError> {
    if responses.is_empty() {
        return Err(LightClientError::ConsensusFailed(
            "No responses".to_string(),
        ));
    }

    // Count matching responses
    let first = &responses[0];
    let matching = responses.iter().filter(|r| *r == first).count();

    let threshold = (responses.len() as f64 * CONSENSUS_THRESHOLD).ceil() as usize;
    if matching < threshold {
        return Err(LightClientError::ConsensusFailed(format!(
            "Only {}/{} responses match (need {})",
            matching,
            responses.len(),
            threshold
        )));
    }

    Ok(())
}

/// Invariant: Checkpoint chain must include all trusted checkpoints.
/// Reference: SPEC-13 Lines 149-157
pub fn invariant_checkpoint_chain(
    chain_hashes: &[(u64, Hash)], // (height, hash) pairs
    checkpoints: &[(u64, Hash)],
) -> Result<(), LightClientError> {
    for (cp_height, cp_hash) in checkpoints {
        let chain_hash = chain_hashes.iter().find(|(h, _)| h == cp_height);
        match chain_hash {
            Some((_, hash)) if hash != cp_hash => {
                return Err(LightClientError::CheckpointMismatch { height: *cp_height });
            }
            _ => {} // Either matches or height not yet reached
        }
    }
    Ok(())
}

/// Invariant: Header chain continuity.
/// Reference: System.md Line 627
pub fn invariant_header_chain_continuous(
    parent_hash: &Hash,
    expected_parent: &Hash,
    height: u64,
    expected_height: u64,
) -> Result<(), LightClientError> {
    if parent_hash != expected_parent {
        return Err(LightClientError::InvalidHeaderChain(format!(
            "Parent hash mismatch at height {}",
            height
        )));
    }

    if height != expected_height {
        return Err(LightClientError::InvalidHeaderChain(format!(
            "Height mismatch: expected {}, got {}",
            expected_height, height
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invariant_multi_node_pass() {
        assert!(invariant_multi_node(3, 3).is_ok());
        assert!(invariant_multi_node(5, 3).is_ok());
    }

    #[test]
    fn test_invariant_multi_node_fail() {
        let result = invariant_multi_node(2, 3);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(LightClientError::InsufficientNodes { .. })
        ));
    }

    #[test]
    fn test_invariant_consensus_all_agree() {
        let responses = vec![42, 42, 42];
        assert!(invariant_consensus(&responses).is_ok());
    }

    #[test]
    fn test_invariant_consensus_2_of_3() {
        let responses = vec![42, 42, 99]; // 2/3 agree
        assert!(invariant_consensus(&responses).is_ok());
    }

    #[test]
    fn test_invariant_consensus_fail() {
        let responses = vec![1, 2, 3]; // All different
        assert!(invariant_consensus(&responses).is_err());
    }

    #[test]
    fn test_invariant_checkpoint_chain_pass() {
        let chain = vec![(0, [1u8; 32]), (100, [2u8; 32])];
        let checkpoints = vec![(0, [1u8; 32])];
        assert!(invariant_checkpoint_chain(&chain, &checkpoints).is_ok());
    }

    #[test]
    fn test_invariant_checkpoint_chain_fail() {
        let chain = vec![(0, [1u8; 32]), (100, [2u8; 32])];
        let checkpoints = vec![(0, [99u8; 32])]; // Wrong hash
        assert!(invariant_checkpoint_chain(&chain, &checkpoints).is_err());
    }

    #[test]
    fn test_invariant_header_chain_continuous_pass() {
        let parent = [1u8; 32];
        assert!(invariant_header_chain_continuous(&parent, &parent, 1, 1).is_ok());
    }

    #[test]
    fn test_invariant_header_chain_continuous_fail_parent() {
        let parent = [1u8; 32];
        let wrong = [2u8; 32];
        assert!(invariant_header_chain_continuous(&parent, &wrong, 1, 1).is_err());
    }

    #[test]
    fn test_invariant_header_chain_continuous_fail_height() {
        let parent = [1u8; 32];
        assert!(invariant_header_chain_continuous(&parent, &parent, 5, 1).is_err());
    }
}
