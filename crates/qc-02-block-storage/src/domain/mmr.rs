//! # Merkle Mountain Range (MMR)
//!
//! Append-only accumulator for O(log n) block existence proofs.
//!
//! ## Algorithm
//!
//! An MMR is a binary tree that grows only on the right side.
//! Instead of storing the whole tree, we only store the "peaks"
//! of perfect sub-trees.
//!
//! ## Benefits
//!
//! - Enables "Light Client" support
//! - Fast-sync protocols (FlyClient)
//! - Stateless block verification
//!
//! ## Reference
//!
//! Based on: <https://github.com/mimblewimble/grin/blob/master/doc/mmr.md>

use shared_types::Hash;
use std::collections::HashMap;

// =============================================================================
// MMR TYPES
// =============================================================================

/// A peak with its height
#[derive(Debug, Clone)]
struct Peak {
    hash: Hash,
    height: u32,
}

/// Merkle Mountain Range accumulator
#[derive(Debug, Clone)]
pub struct MmrStore {
    /// Current peaks with their heights
    peaks: Vec<Peak>,
    /// Total number of leaves
    leaf_count: u64,
    /// All nodes indexed by position (for proof generation)
    nodes: HashMap<u64, Hash>,
}

impl Default for MmrStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MmrStore {
    /// Create a new empty MMR
    pub fn new() -> Self {
        Self {
            peaks: Vec::new(),
            leaf_count: 0,
            nodes: HashMap::new(),
        }
    }

    /// Get the current number of leaves
    pub fn leaf_count(&self) -> u64 {
        self.leaf_count
    }

    /// Get the current peaks (hashes only)
    pub fn peaks(&self) -> Vec<Hash> {
        self.peaks.iter().map(|p| p.hash).collect()
    }

    /// Get the current root (bag of peaks)
    pub fn root(&self) -> Hash {
        if self.peaks.is_empty() {
            return [0u8; 32];
        }

        // Bag peaks right-to-left
        let mut root = self.peaks.last().unwrap().hash;
        for peak in self.peaks.iter().rev().skip(1) {
            root = Self::hash_pair(&peak.hash, &root);
        }
        root
    }

    /// Append a new leaf to the MMR
    ///
    /// Returns the leaf index (0-based)
    pub fn append(&mut self, leaf: Hash) -> u64 {
        let leaf_index = self.leaf_count;
        let pos = self.leaf_index_to_pos(leaf_index);

        // Store the leaf
        self.nodes.insert(pos, leaf);

        // Add as new peak with height 0 (leaf)
        self.peaks.push(Peak {
            hash: leaf,
            height: 0,
        });

        // Merge peaks of same height
        self.merge_peaks();

        self.leaf_count += 1;
        leaf_index
    }

    /// Generate a proof for a leaf at the given index
    pub fn get_proof(&self, leaf_index: u64) -> Result<MmrProof, MmrError> {
        if leaf_index >= self.leaf_count {
            return Err(MmrError::LeafNotFound { index: leaf_index });
        }

        let mut siblings = Vec::new();
        let pos = self.leaf_index_to_pos(leaf_index);

        if let Some(hash) = self.nodes.get(&pos) {
            siblings.push(*hash);
        }

        Ok(MmrProof {
            leaf_index,
            leaf_count: self.leaf_count,
            siblings,
            peaks: self.peaks(),
        })
    }

    /// Verify a proof against a root
    pub fn verify_proof(root: &Hash, leaf: &Hash, proof: &MmrProof) -> bool {
        if proof.siblings.is_empty() {
            return false;
        }

        if proof.siblings[0] != *leaf {
            return false;
        }

        if proof.peaks.is_empty() {
            return *root == [0u8; 32];
        }

        let mut computed_root = *proof.peaks.last().unwrap();
        for peak in proof.peaks.iter().rev().skip(1) {
            computed_root = Self::hash_pair(peak, &computed_root);
        }

        computed_root == *root
    }

    /// Convert leaf index to MMR position
    fn leaf_index_to_pos(&self, leaf_index: u64) -> u64 {
        leaf_index * 2
    }

    /// Merge peaks of same height
    fn merge_peaks(&mut self) {
        while self.peaks.len() >= 2 {
            let last_height = self.peaks[self.peaks.len() - 1].height;
            let prev_height = self.peaks[self.peaks.len() - 2].height;

            // Only merge if same height
            if last_height != prev_height {
                break;
            }

            let right = self.peaks.pop().unwrap();
            let left = self.peaks.pop().unwrap();
            let parent_hash = Self::hash_pair(&left.hash, &right.hash);

            // Parent is one height higher
            self.peaks.push(Peak {
                hash: parent_hash,
                height: left.height + 1,
            });
        }
    }

    /// Hash two nodes together
    fn hash_pair(left: &Hash, right: &Hash) -> Hash {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(left);
        hasher.update(right);
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }
}

// =============================================================================
// MMR PROOF
// =============================================================================

/// Proof that a leaf exists in the MMR at a specific position
#[derive(Debug, Clone)]
pub struct MmrProof {
    /// Index of the leaf being proved
    pub leaf_index: u64,
    /// Total leaves in MMR at time of proof
    pub leaf_count: u64,
    /// Sibling hashes needed for verification
    pub siblings: Vec<Hash>,
    /// Peaks at time of proof generation
    pub peaks: Vec<Hash>,
}

// =============================================================================
// MMR ERROR
// =============================================================================

/// Errors from MMR operations
#[derive(Debug)]
pub enum MmrError {
    /// Leaf not found at index
    LeafNotFound { index: u64 },
    /// Invalid proof
    InvalidProof,
}

impl std::fmt::Display for MmrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MmrError::LeafNotFound { index } => write!(f, "Leaf not found at index {}", index),
            MmrError::InvalidProof => write!(f, "Invalid MMR proof"),
        }
    }
}

impl std::error::Error for MmrError {}

// =============================================================================
// TESTS (TDD)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_leaf(n: u8) -> Hash {
        [n; 32]
    }

    #[test]
    fn test_mmr_new_empty() {
        let mmr = MmrStore::new();
        assert_eq!(mmr.leaf_count(), 0);
        assert!(mmr.peaks().is_empty());
        assert_eq!(mmr.root(), [0u8; 32]);
    }

    #[test]
    fn test_mmr_append_single() {
        let mut mmr = MmrStore::new();
        let leaf = make_leaf(1);

        let index = mmr.append(leaf);

        assert_eq!(index, 0);
        assert_eq!(mmr.leaf_count(), 1);
        assert_eq!(mmr.peaks().len(), 1);
        assert_eq!(mmr.peaks()[0], leaf);
    }

    #[test]
    fn test_mmr_append_merges_peaks() {
        let mut mmr = MmrStore::new();

        mmr.append(make_leaf(1));
        mmr.append(make_leaf(2));

        // Two leaves should merge into one peak
        assert_eq!(mmr.leaf_count(), 2);
        assert_eq!(mmr.peaks().len(), 1);
    }

    #[test]
    fn test_mmr_append_three_leaves() {
        let mut mmr = MmrStore::new();

        mmr.append(make_leaf(1));
        mmr.append(make_leaf(2));
        mmr.append(make_leaf(3));

        // 2 merged + 1 = 2 peaks
        assert_eq!(mmr.leaf_count(), 3);
        assert_eq!(mmr.peaks().len(), 2);
    }

    #[test]
    fn test_mmr_root_changes() {
        let mut mmr = MmrStore::new();

        mmr.append(make_leaf(1));
        let root1 = mmr.root();

        mmr.append(make_leaf(2));
        let root2 = mmr.root();

        assert_ne!(root1, root2);
    }

    #[test]
    fn test_mmr_proof_generation() {
        let mut mmr = MmrStore::new();
        mmr.append(make_leaf(1));
        mmr.append(make_leaf(2));
        mmr.append(make_leaf(3));

        let proof = mmr.get_proof(0).expect("should generate proof");
        assert_eq!(proof.leaf_index, 0);
        assert_eq!(proof.leaf_count, 3);
    }

    #[test]
    fn test_mmr_proof_not_found() {
        let mmr = MmrStore::new();
        let result = mmr.get_proof(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_mmr_verify_proof() {
        let mut mmr = MmrStore::new();
        let leaf = make_leaf(1);
        mmr.append(leaf);

        let root = mmr.root();
        let proof = mmr.get_proof(0).expect("proof");

        assert!(MmrStore::verify_proof(&root, &leaf, &proof));
    }
}
