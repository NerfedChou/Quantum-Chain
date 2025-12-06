//! Merkle tree computation tasks

use crate::{ComputeEngine, ComputeError};
use std::sync::Arc;

/// Compute merkle root from transaction hashes
pub struct MerkleRootTask {
    pub leaf_hashes: Vec<[u8; 32]>,
}

impl MerkleRootTask {
    /// Execute merkle root computation
    pub async fn execute(self, engine: &Arc<dyn ComputeEngine>) -> Result<[u8; 32], ComputeError> {
        if self.leaf_hashes.is_empty() {
            // Return sentinel hash for empty tree
            return Ok([0u8; 32]);
        }

        if self.leaf_hashes.len() == 1 {
            return Ok(self.leaf_hashes[0]);
        }

        // Pad to power of 2
        let mut leaves = self.leaf_hashes;
        let next_pow2 = leaves.len().next_power_of_two();
        while leaves.len() < next_pow2 {
            leaves.push(leaves[leaves.len() - 1]);
        }

        // Build tree level by level
        let mut current_level = leaves;

        while current_level.len() > 1 {
            // Prepare pairs for hashing
            let pairs: Vec<Vec<u8>> = current_level
                .chunks(2)
                .map(|pair| {
                    let mut combined = Vec::with_capacity(64);
                    combined.extend_from_slice(&pair[0]);
                    combined.extend_from_slice(&pair[1]);
                    combined
                })
                .collect();

            // Batch hash all pairs
            let parent_hashes = engine.batch_sha256(&pairs).await?;
            current_level = parent_hashes;
        }

        Ok(current_level[0])
    }
}

/// Generate merkle proof for a specific leaf
pub struct MerkleProofTask {
    pub leaf_hashes: Vec<[u8; 32]>,
    pub leaf_index: usize,
}

/// Merkle proof result
#[derive(Debug, Clone)]
pub struct MerkleProof {
    pub leaf_hash: [u8; 32],
    pub siblings: Vec<([u8; 32], bool)>, // (hash, is_left)
    pub root: [u8; 32],
}

impl MerkleProofTask {
    /// Generate a merkle proof
    pub async fn execute(
        self,
        engine: &Arc<dyn ComputeEngine>,
    ) -> Result<MerkleProof, ComputeError> {
        if self.leaf_index >= self.leaf_hashes.len() {
            return Err(ComputeError::InvalidInput(
                "Leaf index out of bounds".to_string(),
            ));
        }

        let mut leaves = self.leaf_hashes.clone();
        let next_pow2 = leaves.len().next_power_of_two();
        while leaves.len() < next_pow2 {
            leaves.push(leaves[leaves.len() - 1]);
        }

        let leaf_hash = leaves[self.leaf_index];
        let mut siblings = Vec::new();
        let mut current_level = leaves;
        let mut index = self.leaf_index;

        while current_level.len() > 1 {
            // Record sibling
            let sibling_index = if index % 2 == 0 { index + 1 } else { index - 1 };
            let is_left = index % 2 == 1;
            siblings.push((current_level[sibling_index], is_left));

            // Build next level
            let pairs: Vec<Vec<u8>> = current_level
                .chunks(2)
                .map(|pair| {
                    let mut combined = Vec::with_capacity(64);
                    combined.extend_from_slice(&pair[0]);
                    combined.extend_from_slice(&pair[1]);
                    combined
                })
                .collect();

            current_level = engine.batch_sha256(&pairs).await?;
            index /= 2;
        }

        Ok(MerkleProof {
            leaf_hash,
            siblings,
            root: current_level[0],
        })
    }
}
