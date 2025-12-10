//! # Merkle Proof Verification
//!
//! SPV Merkle proof verification algorithm.
//!
//! Reference: System.md Line 628, SPEC-13 Lines 130-138

use sha2::{Digest, Sha256};
use crate::domain::{Hash, LightClientError, ProofNode, Position};

/// Verify a Merkle proof for transaction inclusion.
///
/// Reference: System.md Line 628 - "Verify via Merkle proofs"
///
/// # Algorithm
///
/// 1. Start with the transaction hash as current hash
/// 2. For each node in the proof path:
///    - If sibling is on left: hash = SHA256(sibling || current)
///    - If sibling is on right: hash = SHA256(current || sibling)
/// 3. Final hash should equal merkle_root
///
/// # Time Complexity: O(log n)
/// # Space Complexity: O(1)
pub fn verify_merkle_proof(
    tx_hash: &Hash,
    proof_path: &[ProofNode],
    expected_root: &Hash,
) -> bool {
    // Edge case: empty proof is valid only if tx_hash == root
    if proof_path.is_empty() {
        return tx_hash == expected_root;
    }

    let mut current = *tx_hash;

    for node in proof_path {
        current = match node.position {
            Position::Left => hash_concat(&node.hash, &current),
            Position::Right => hash_concat(&current, &node.hash),
        };
    }

    current == *expected_root
}

/// Hash two nodes together.
fn hash_concat(left: &Hash, right: &Hash) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(left);
    hasher.update(right);
    let result = hasher.finalize();
    let mut output = [0u8; 32];
    output.copy_from_slice(&result);
    output
}

/// Build a Merkle tree from transaction hashes.
///
/// Returns the Merkle root hash.
pub fn compute_merkle_root(tx_hashes: &[Hash]) -> Hash {
    if tx_hashes.is_empty() {
        return [0u8; 32];
    }

    if tx_hashes.len() == 1 {
        return tx_hashes[0];
    }

    let mut level: Vec<Hash> = tx_hashes.to_vec();

    while level.len() > 1 {
        let mut next_level = Vec::with_capacity((level.len() + 1) / 2);

        for chunk in level.chunks(2) {
            let left = &chunk[0];
            let right = chunk.get(1).unwrap_or(left); // Duplicate last if odd
            next_level.push(hash_concat(left, right));
        }

        level = next_level;
    }

    level[0]
}

/// Build a Merkle proof for a specific transaction.
pub fn build_merkle_proof(tx_hashes: &[Hash], tx_index: usize) -> Result<Vec<ProofNode>, LightClientError> {
    if tx_index >= tx_hashes.len() {
        return Err(LightClientError::TransactionNotFound([0u8; 32]));
    }

    if tx_hashes.len() == 1 {
        return Ok(vec![]);
    }

    let mut proof = Vec::new();
    let mut level: Vec<Hash> = tx_hashes.to_vec();
    let mut index = tx_index;

    while level.len() > 1 {
        let sibling_index = if index % 2 == 0 { index + 1 } else { index - 1 };

        if sibling_index < level.len() {
            let position = if index % 2 == 0 {
                Position::Right // Sibling is on the right
            } else {
                Position::Left // Sibling is on the left
            };
            proof.push(ProofNode {
                hash: level[sibling_index],
                position,
            });
        } else if index + 1 == level.len() {
            // Last element with no pair - duplicate self
            proof.push(ProofNode {
                hash: level[index],
                position: Position::Right,
            });
        }

        // Move to next level
        let mut next_level = Vec::with_capacity((level.len() + 1) / 2);
        for chunk in level.chunks(2) {
            let left = &chunk[0];
            let right = chunk.get(1).unwrap_or(left);
            next_level.push(hash_concat(left, right));
        }
        level = next_level;
        index /= 2;
    }

    Ok(proof)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create deterministic hash
    fn make_hash(n: u8) -> Hash {
        let mut h = [0u8; 32];
        h[0] = n;
        h
    }

    #[test]
    fn test_verify_merkle_proof_single_tx() {
        let tx_hash = make_hash(1);
        // Single tx means proof is empty, root == tx_hash
        assert!(verify_merkle_proof(&tx_hash, &[], &tx_hash));
    }

    #[test]
    fn test_verify_merkle_proof_two_tx() {
        let tx1 = make_hash(1);
        let tx2 = make_hash(2);

        let root = hash_concat(&tx1, &tx2);

        // Proof for tx1: sibling is tx2 on the right
        let proof1 = vec![ProofNode::right(tx2)];
        assert!(verify_merkle_proof(&tx1, &proof1, &root));

        // Proof for tx2: sibling is tx1 on the left
        let proof2 = vec![ProofNode::left(tx1)];
        assert!(verify_merkle_proof(&tx2, &proof2, &root));
    }

    #[test]
    fn test_verify_merkle_proof_invalid() {
        let tx_hash = make_hash(1);
        let wrong_root = make_hash(99);

        // Valid tx_hash but wrong root should fail
        assert!(!verify_merkle_proof(&tx_hash, &[], &wrong_root));
    }

    #[test]
    fn test_verify_merkle_proof_tampered() {
        let tx1 = make_hash(1);
        let tx2 = make_hash(2);
        let root = hash_concat(&tx1, &tx2);

        // Tampered proof (wrong sibling hash)
        let tampered_proof = vec![ProofNode::right(make_hash(99))];
        assert!(!verify_merkle_proof(&tx1, &tampered_proof, &root));
    }

    #[test]
    fn test_compute_merkle_root_empty() {
        let root = compute_merkle_root(&[]);
        assert_eq!(root, [0u8; 32]);
    }

    #[test]
    fn test_compute_merkle_root_single() {
        let tx = make_hash(42);
        let root = compute_merkle_root(&[tx]);
        assert_eq!(root, tx);
    }

    #[test]
    fn test_compute_merkle_root_two() {
        let tx1 = make_hash(1);
        let tx2 = make_hash(2);
        let root = compute_merkle_root(&[tx1, tx2]);
        assert_eq!(root, hash_concat(&tx1, &tx2));
    }

    #[test]
    fn test_compute_merkle_root_four() {
        let txs: Vec<Hash> = (1..=4).map(make_hash).collect();
        let root = compute_merkle_root(&txs);

        // Expected: hash(hash(tx1,tx2), hash(tx3,tx4))
        let left = hash_concat(&txs[0], &txs[1]);
        let right = hash_concat(&txs[2], &txs[3]);
        assert_eq!(root, hash_concat(&left, &right));
    }

    #[test]
    fn test_build_and_verify_proof() {
        let txs: Vec<Hash> = (1..=8).map(make_hash).collect();
        let root = compute_merkle_root(&txs);

        // Build and verify proof for each transaction
        for (i, tx) in txs.iter().enumerate() {
            let proof = build_merkle_proof(&txs, i).unwrap();
            assert!(
                verify_merkle_proof(tx, &proof, &root),
                "Proof verification failed for tx {}",
                i
            );
        }
    }

    #[test]
    fn test_build_proof_invalid_index() {
        let txs: Vec<Hash> = (1..=4).map(make_hash).collect();
        let result = build_merkle_proof(&txs, 10);
        assert!(result.is_err());
    }
}
