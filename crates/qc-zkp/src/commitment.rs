//! # Merkle Commitment
//!
//! Merkle tree commitments for polynomial coefficients.

use crate::field::FieldElement;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Hash output (simulated, would use BLAKE3 in production).
pub type HashOutput = [u8; 32];

/// Merkle tree commitment for polynomial evaluations.
#[derive(Clone, Debug)]
pub struct MerkleCommitment {
    root: HashOutput,
    leaves: Vec<HashOutput>,
    height: usize,
}

impl MerkleCommitment {
    /// Commit to a vector of field elements.
    pub fn commit(values: &[FieldElement]) -> Self {
        if values.is_empty() {
            return Self {
                root: [0u8; 32],
                leaves: vec![],
                height: 0,
            };
        }

        // Hash leaves
        let leaves: Vec<HashOutput> = values.iter().map(|v| hash_field_element(v)).collect();

        // Build tree
        let height = (leaves.len() as f64).log2().ceil() as usize;
        let root = Self::build_tree(&leaves);

        Self {
            root,
            leaves,
            height,
        }
    }

    /// Get commitment root.
    pub fn root(&self) -> &HashOutput {
        &self.root
    }

    /// Get tree height.
    pub fn height(&self) -> usize {
        self.height
    }

    /// Generate opening proof for leaf at index.
    pub fn open(&self, index: usize) -> Option<MerkleProof> {
        if index >= self.leaves.len() {
            return None;
        }

        let mut siblings = Vec::new();
        let mut current_idx = index;
        let mut layer = self.leaves.clone();

        while layer.len() > 1 {
            // Pad to even length
            if layer.len() % 2 == 1 {
                layer.push([0u8; 32]);
            }

            let sibling_idx = if current_idx % 2 == 0 {
                current_idx + 1
            } else {
                current_idx - 1
            };

            if sibling_idx < layer.len() {
                siblings.push(layer[sibling_idx]);
            }

            // Compute next layer
            let mut next_layer = Vec::new();
            for chunk in layer.chunks(2) {
                next_layer.push(hash_pair(&chunk[0], &chunk[1]));
            }
            layer = next_layer;
            current_idx /= 2;
        }

        Some(MerkleProof {
            leaf: self.leaves[index],
            index,
            siblings,
        })
    }

    fn build_tree(leaves: &[HashOutput]) -> HashOutput {
        if leaves.is_empty() {
            return [0u8; 32];
        }
        if leaves.len() == 1 {
            return leaves[0];
        }

        let mut layer = leaves.to_vec();

        while layer.len() > 1 {
            if layer.len() % 2 == 1 {
                layer.push([0u8; 32]);
            }

            let mut next_layer = Vec::new();
            for chunk in layer.chunks(2) {
                next_layer.push(hash_pair(&chunk[0], &chunk[1]));
            }
            layer = next_layer;
        }

        layer[0]
    }
}

/// Merkle proof for a single leaf.
#[derive(Clone, Debug)]
pub struct MerkleProof {
    /// Leaf hash
    pub leaf: HashOutput,
    /// Leaf index
    pub index: usize,
    /// Sibling hashes on path to root
    pub siblings: Vec<HashOutput>,
}

impl MerkleProof {
    /// Verify proof against commitment root.
    pub fn verify(&self, root: &HashOutput) -> bool {
        let mut current = self.leaf;
        let mut idx = self.index;

        for sibling in &self.siblings {
            current = if idx % 2 == 0 {
                hash_pair(&current, sibling)
            } else {
                hash_pair(sibling, &current)
            };
            idx /= 2;
        }

        &current == root
    }
}

/// Hash a field element.
fn hash_field_element(elem: &FieldElement) -> HashOutput {
    let mut hasher = DefaultHasher::new();
    elem.value().hash(&mut hasher);
    let hash = hasher.finish();
    let mut output = [0u8; 32];
    output[0..8].copy_from_slice(&hash.to_le_bytes());
    output
}

/// Hash two nodes together.
fn hash_pair(left: &HashOutput, right: &HashOutput) -> HashOutput {
    let mut hasher = DefaultHasher::new();
    left.hash(&mut hasher);
    right.hash(&mut hasher);
    let hash = hasher.finish();
    let mut output = [0u8; 32];
    output[0..8].copy_from_slice(&hash.to_le_bytes());
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commit_and_open() {
        let values = vec![
            FieldElement::new(1),
            FieldElement::new(2),
            FieldElement::new(3),
            FieldElement::new(4),
        ];

        let commitment = MerkleCommitment::commit(&values);
        let proof = commitment.open(0).unwrap();

        assert!(proof.verify(commitment.root()));
    }

    #[test]
    fn test_invalid_proof() {
        let values = vec![FieldElement::new(1), FieldElement::new(2)];
        let commitment = MerkleCommitment::commit(&values);

        let mut proof = commitment.open(0).unwrap();
        proof.leaf = [0xFF; 32]; // Tamper with leaf

        assert!(!proof.verify(commitment.root()));
    }

    #[test]
    fn test_empty_commitment() {
        let commitment = MerkleCommitment::commit(&[]);
        assert_eq!(commitment.root(), &[0u8; 32]);
    }
}
