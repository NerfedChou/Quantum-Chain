//! # Domain Entities
//!
//! Core domain entities for the Transaction Indexing subsystem.
//!
//! ## SPEC-03 Reference
//!
//! - Section 2.2: MerkleTree, MerkleProof, ProofNode, TransactionLocation
//! - Section 2.3: TransactionIndex
//! - Section 2.5: Domain Invariants

use lru::LruCache;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use shared_types::Hash;
use std::collections::HashMap;
use std::num::NonZeroUsize;

use super::errors::IndexingError;
use super::value_objects::{IndexConfig, SENTINEL_HASH};

/// A binary Merkle tree built from transaction hashes.
///
/// ALGORITHM: Binary hash tree where each non-leaf node is the hash
/// of its two children concatenated: H(left || right).
///
/// ## SPEC-03 Invariants
///
/// - **INVARIANT-1** (Power of Two): Leaves are padded to nearest power of two.
///   Empty slots are filled with a sentinel hash (all zeros).
/// - **INVARIANT-3** (Deterministic Hashing): Same transactions produce same root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerkleTree {
    /// All nodes in the tree, stored level by level (root at index 0).
    /// Tree is stored in array form: [root, level1..., level2..., leaves...]
    nodes: Vec<Hash>,
    /// Number of actual transactions (before padding).
    transaction_count: usize,
    /// Number of leaves after padding to power of two.
    padded_leaf_count: usize,
    /// The computed root hash.
    root: Hash,
}

impl MerkleTree {
    /// Build a Merkle tree from transaction hashes.
    ///
    /// ## INVARIANT-1: Power of Two Padding
    ///
    /// Pads leaves to nearest power of two using SENTINEL_HASH.
    ///
    /// ## INVARIANT-3: Deterministic Hashing
    ///
    /// Same input hashes always produce same root.
    ///
    /// ## Algorithm
    ///
    /// 1. Pad leaves to power of two with SENTINEL_HASH
    /// 2. Build tree bottom-up: each parent = H(left_child || right_child)
    /// 3. Root is at level 0
    pub fn build(transaction_hashes: Vec<Hash>) -> Self {
        let transaction_count = transaction_hashes.len();

        // Handle empty case
        if transaction_count == 0 {
            return Self {
                nodes: vec![SENTINEL_HASH],
                transaction_count: 0,
                padded_leaf_count: 0,
                root: SENTINEL_HASH,
            };
        }

        // INVARIANT-1: Pad to power of two (minimum 2 for proper tree structure)
        let padded_leaf_count = if transaction_count == 1 {
            2 // Special case: 1 tx needs padding to 2 leaves
        } else {
            transaction_count.next_power_of_two()
        };
        let mut leaves = transaction_hashes;
        leaves.resize(padded_leaf_count, SENTINEL_HASH);

        // Build tree bottom-up
        // Total nodes = 2 * padded_leaf_count - 1 (for a complete binary tree)
        let total_nodes = 2 * padded_leaf_count - 1;
        let mut nodes = vec![SENTINEL_HASH; total_nodes];

        // Place leaves at the end of the array
        let leaf_start = padded_leaf_count - 1;
        for (i, hash) in leaves.iter().enumerate() {
            nodes[leaf_start + i] = *hash;
        }

        // Build internal nodes from bottom to top
        // Parent at index i has children at 2i+1 and 2i+2
        for i in (0..leaf_start).rev() {
            let left_child = 2 * i + 1;
            let right_child = 2 * i + 2;
            nodes[i] = Self::hash_pair(&nodes[left_child], &nodes[right_child]);
        }

        let root = nodes[0];

        Self {
            nodes,
            transaction_count,
            padded_leaf_count,
            root,
        }
    }

    /// Get the root hash of this tree.
    pub fn root(&self) -> Hash {
        self.root
    }

    /// Get the number of actual transactions (before padding).
    pub fn transaction_count(&self) -> usize {
        self.transaction_count
    }

    /// Get the number of leaves after padding.
    pub fn leaf_count(&self) -> usize {
        self.padded_leaf_count
    }

    /// Generate a proof for the transaction at the given index.
    ///
    /// ## INVARIANT-2: Proof Validity
    ///
    /// The generated proof MUST verify against this tree's root.
    pub fn generate_proof(
        &self,
        tx_index: usize,
        block_height: u64,
        block_hash: Hash,
    ) -> Result<MerkleProof, IndexingError> {
        // Validate index bounds
        if tx_index >= self.transaction_count {
            return Err(IndexingError::InvalidIndex {
                index: tx_index,
                max: self.transaction_count,
            });
        }

        // Empty tree case
        if self.padded_leaf_count == 0 {
            return Err(IndexingError::EmptyBlock { block_hash });
        }

        // Get leaf index in the nodes array
        let leaf_start = self.padded_leaf_count - 1;
        let mut current_idx = leaf_start + tx_index;

        // Get the leaf hash
        let leaf_hash =
            self.nodes
                .get(current_idx)
                .copied()
                .ok_or(IndexingError::InvalidIndex {
                    index: tx_index,
                    max: self.transaction_count,
                })?;

        // Build path from leaf to root
        let mut path = Vec::new();

        while current_idx > 0 {
            // Determine sibling index
            let sibling_idx = if current_idx % 2 == 0 {
                // Current is right child, sibling is left
                current_idx - 1
            } else {
                // Current is left child, sibling is right
                current_idx + 1
            };

            // Get sibling hash
            let sibling_hash =
                self.nodes
                    .get(sibling_idx)
                    .copied()
                    .ok_or(IndexingError::InvalidIndex {
                        index: sibling_idx,
                        max: self.nodes.len(),
                    })?;

            // Determine sibling position
            let position = if current_idx % 2 == 0 {
                SiblingPosition::Left
            } else {
                SiblingPosition::Right
            };

            path.push(ProofNode {
                hash: sibling_hash,
                position,
            });

            // Move to parent
            current_idx = (current_idx - 1) / 2;
        }

        Ok(MerkleProof {
            leaf_hash,
            tx_index,
            block_height,
            block_hash,
            root: self.root,
            path,
        })
    }

    /// Verify a proof against this tree's root.
    pub fn verify_proof(&self, proof: &MerkleProof) -> bool {
        Self::verify_proof_static(&proof.leaf_hash, &proof.path, &self.root)
    }

    /// Static verification without tree instance.
    ///
    /// ## INVARIANT-2: Proof Validity
    ///
    /// If proof is valid, this returns true.
    /// Recomputes the root from leaf + path and compares.
    pub fn verify_proof_static(leaf_hash: &Hash, path: &[ProofNode], expected_root: &Hash) -> bool {
        let mut current_hash = *leaf_hash;

        for node in path {
            current_hash = match node.position {
                SiblingPosition::Left => Self::hash_pair(&node.hash, &current_hash),
                SiblingPosition::Right => Self::hash_pair(&current_hash, &node.hash),
            };
        }

        current_hash == *expected_root
    }

    /// Hash two concatenated hashes using SHA3-256.
    ///
    /// This is the core operation for building the tree:
    /// parent = H(left || right)
    fn hash_pair(left: &Hash, right: &Hash) -> Hash {
        let mut hasher = Sha3_256::new();
        hasher.update(left);
        hasher.update(right);
        hasher.finalize().into()
    }
}

/// A cryptographic proof of transaction inclusion in a Merkle tree.
///
/// This proof allows verification that a specific transaction is included
/// in a block without having access to all other transactions.
///
/// ## SPEC-03 Section 2.2
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerkleProof {
    /// Hash of the transaction being proven.
    pub leaf_hash: Hash,
    /// Index of the transaction in the original list.
    pub tx_index: usize,
    /// Block height where this transaction exists.
    pub block_height: u64,
    /// Block hash for additional verification.
    pub block_hash: Hash,
    /// The Merkle root this proof verifies against.
    pub root: Hash,
    /// Path of sibling hashes from leaf to root.
    pub path: Vec<ProofNode>,
}

/// A single node in the Merkle proof path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofNode {
    /// The sibling hash at this level.
    pub hash: Hash,
    /// Position of sibling (left or right).
    pub position: SiblingPosition,
}

/// Position of a sibling in the Merkle tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SiblingPosition {
    Left,
    Right,
}

/// Location of a transaction in the blockchain.
///
/// ## SPEC-03 Section 2.2
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionLocation {
    /// Block height containing this transaction.
    pub block_height: u64,
    /// Block hash containing this transaction.
    pub block_hash: Hash,
    /// Index of transaction within the block.
    pub tx_index: usize,
    /// Merkle root of the block (cached for proof generation).
    pub merkle_root: Hash,
}

/// Index for efficient transaction lookups and proof generation.
///
/// This structure maintains mappings from transaction hashes to their
/// locations in the blockchain, enabling O(1) proof generation.
///
/// ## SPEC-03 Section 2.3
///
/// ## INVARIANT-5: Bounded Tree Cache
///
/// trees.len() <= max_cached_trees (enforced via LRU eviction)
pub struct TransactionIndex {
    /// Transaction hash → location mapping.
    locations: HashMap<Hash, TransactionLocation>,
    /// Block hash → Merkle tree mapping (LRU cache for proof generation).
    trees: LruCache<Hash, MerkleTree>,
    /// Configuration.
    config: IndexConfig,
    /// Statistics.
    stats: IndexingStats,
}

impl TransactionIndex {
    /// Create a new transaction index with the given configuration.
    pub fn new(config: IndexConfig) -> Self {
        // SAFETY: 1000 is non-zero, compile-time constant
        const DEFAULT_CACHE_SIZE: NonZeroUsize = match NonZeroUsize::new(1000) {
            Some(n) => n,
            None => unreachable!(),
        };
        
        let cache_size = NonZeroUsize::new(config.max_cached_trees).unwrap_or(DEFAULT_CACHE_SIZE);

        Self {
            locations: HashMap::new(),
            trees: LruCache::new(cache_size),
            config,
            stats: IndexingStats::default(),
        }
    }

    /// Index a transaction location.
    pub fn put_location(&mut self, tx_hash: Hash, location: TransactionLocation) {
        self.locations.insert(tx_hash, location);
        self.stats.total_indexed += 1;
    }

    /// Get a transaction location by hash.
    pub fn get_location(&self, tx_hash: &Hash) -> Option<&TransactionLocation> {
        self.locations.get(tx_hash)
    }

    /// Check if a transaction is indexed.
    pub fn is_indexed(&self, tx_hash: &Hash) -> bool {
        self.locations.contains_key(tx_hash)
    }

    /// Cache a Merkle tree for a block.
    ///
    /// ## INVARIANT-5: Bounded Cache
    ///
    /// LRU eviction is automatic when cache is full.
    pub fn cache_tree(&mut self, block_hash: Hash, tree: MerkleTree) {
        self.trees.put(block_hash, tree);
        self.stats.cached_trees = self.trees.len();
    }

    /// Get a cached Merkle tree.
    pub fn get_tree(&mut self, block_hash: &Hash) -> Option<&MerkleTree> {
        self.trees.get(block_hash)
    }

    /// Check if a tree is cached.
    pub fn has_tree(&self, block_hash: &Hash) -> bool {
        self.trees.contains(block_hash)
    }

    /// Get the configuration.
    pub fn config(&self) -> &IndexConfig {
        &self.config
    }

    /// Get indexing statistics.
    pub fn stats(&self) -> IndexingStats {
        IndexingStats {
            total_indexed: self.stats.total_indexed,
            cached_trees: self.trees.len(),
            max_cached_trees: self.config.max_cached_trees,
            proofs_generated: self.stats.proofs_generated,
            proofs_verified: self.stats.proofs_verified,
        }
    }

    /// Increment proof generation counter.
    pub fn record_proof_generated(&mut self) {
        self.stats.proofs_generated += 1;
    }

    /// Increment proof verification counter.
    pub fn record_proof_verified(&mut self) {
        self.stats.proofs_verified += 1;
    }
}

/// Statistics about the indexing subsystem.
#[derive(Debug, Clone, Default)]
pub struct IndexingStats {
    /// Total transactions indexed.
    pub total_indexed: u64,
    /// Number of Merkle trees cached.
    pub cached_trees: usize,
    /// Maximum cached trees allowed.
    pub max_cached_trees: usize,
    /// Number of proofs generated.
    pub proofs_generated: u64,
    /// Number of proofs verified.
    pub proofs_verified: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash_from_byte(b: u8) -> Hash {
        let mut h = [0u8; 32];
        h[0] = b;
        h
    }

    // ========== Test Group 1: Merkle Tree Construction ==========

    #[test]
    fn test_merkle_tree_empty_transactions() {
        let tree = MerkleTree::build(vec![]);
        assert_eq!(tree.root(), SENTINEL_HASH);
        assert_eq!(tree.transaction_count(), 0);
        assert_eq!(tree.leaf_count(), 0);
    }

    #[test]
    fn test_merkle_tree_single_transaction() {
        let tx1 = hash_from_byte(0x01);
        let tree = MerkleTree::build(vec![tx1]);

        // INVARIANT-1: 1 tx pads to 2 leaves
        assert_eq!(tree.transaction_count(), 1);
        assert_eq!(tree.leaf_count(), 2);

        // Root = H(tx1 || SENTINEL_HASH) per Merkle tree algorithm
        let expected_root = MerkleTree::hash_pair(&tx1, &SENTINEL_HASH);
        assert_eq!(tree.root(), expected_root);
    }

    #[test]
    fn test_merkle_tree_two_transactions() {
        let tx1 = hash_from_byte(0x01);
        let tx2 = hash_from_byte(0x02);
        let tree = MerkleTree::build(vec![tx1, tx2]);

        // No padding needed for 2
        assert_eq!(tree.transaction_count(), 2);
        assert_eq!(tree.leaf_count(), 2);

        // Root = H(tx1 || tx2)
        let expected_root = MerkleTree::hash_pair(&tx1, &tx2);
        assert_eq!(tree.root(), expected_root);
    }

    #[test]
    fn test_merkle_tree_three_transactions() {
        let tx1 = hash_from_byte(0x01);
        let tx2 = hash_from_byte(0x02);
        let tx3 = hash_from_byte(0x03);
        let tree = MerkleTree::build(vec![tx1, tx2, tx3]);

        // INVARIANT-1: 3 txs pad to 4 leaves
        assert_eq!(tree.transaction_count(), 3);
        assert_eq!(tree.leaf_count(), 4);
    }

    #[test]
    fn test_merkle_tree_power_of_two_no_padding() {
        let txs: Vec<Hash> = (0..4).map(|i| hash_from_byte(i as u8)).collect();
        let tree = MerkleTree::build(txs);

        // No padding for 4
        assert_eq!(tree.transaction_count(), 4);
        assert_eq!(tree.leaf_count(), 4);
    }

    #[test]
    fn test_merkle_tree_deterministic() {
        // INVARIANT-3: Same input = same output
        let txs1: Vec<Hash> = (0..5).map(|i| hash_from_byte(i as u8)).collect();
        let txs2: Vec<Hash> = (0..5).map(|i| hash_from_byte(i as u8)).collect();

        let tree1 = MerkleTree::build(txs1);
        let tree2 = MerkleTree::build(txs2);

        assert_eq!(tree1.root(), tree2.root());
    }

    // ========== Test Group 2: Proof Generation ==========

    #[test]
    fn test_proof_generation_first_transaction() {
        let txs: Vec<Hash> = (0..4).map(|i| hash_from_byte(i as u8)).collect();
        let tree = MerkleTree::build(txs.clone());

        let proof = tree.generate_proof(0, 100, hash_from_byte(0xFF)).unwrap();

        assert_eq!(proof.leaf_hash, txs[0]);
        assert_eq!(proof.tx_index, 0);
        assert_eq!(proof.block_height, 100);
        assert_eq!(proof.root, tree.root());
        // Path length = log2(4) = 2
        assert_eq!(proof.path.len(), 2);
    }

    #[test]
    fn test_proof_generation_last_transaction() {
        let txs: Vec<Hash> = (0..4).map(|i| hash_from_byte(i as u8)).collect();
        let tree = MerkleTree::build(txs.clone());

        let proof = tree.generate_proof(3, 100, hash_from_byte(0xFF)).unwrap();

        assert_eq!(proof.leaf_hash, txs[3]);
        assert_eq!(proof.tx_index, 3);
    }

    #[test]
    fn test_proof_generation_invalid_index() {
        let txs: Vec<Hash> = (0..4).map(|i| hash_from_byte(i as u8)).collect();
        let tree = MerkleTree::build(txs);

        let result = tree.generate_proof(10, 100, hash_from_byte(0xFF));

        assert!(matches!(
            result,
            Err(IndexingError::InvalidIndex { index: 10, max: 4 })
        ));
    }

    // ========== Test Group 3: Proof Verification ==========

    #[test]
    fn test_proof_verification_valid_proof() {
        // INVARIANT-2: Valid proofs verify
        let txs: Vec<Hash> = (0..8).map(|i| hash_from_byte(i as u8)).collect();
        let tree = MerkleTree::build(txs);

        for i in 0..8 {
            let proof = tree.generate_proof(i, 100, hash_from_byte(0xFF)).unwrap();
            assert!(
                tree.verify_proof(&proof),
                "Proof for tx {} should verify",
                i
            );
        }
    }

    #[test]
    fn test_proof_verification_static() {
        let txs: Vec<Hash> = (0..4).map(|i| hash_from_byte(i as u8)).collect();
        let tree = MerkleTree::build(txs);

        let proof = tree.generate_proof(2, 100, hash_from_byte(0xFF)).unwrap();

        // Static verification without tree instance
        assert!(MerkleTree::verify_proof_static(
            &proof.leaf_hash,
            &proof.path,
            &proof.root
        ));
    }

    #[test]
    fn test_proof_verification_tampered_leaf() {
        let txs: Vec<Hash> = (0..4).map(|i| hash_from_byte(i as u8)).collect();
        let tree = MerkleTree::build(txs);

        let mut proof = tree.generate_proof(1, 100, hash_from_byte(0xFF)).unwrap();

        // Tamper with leaf hash
        proof.leaf_hash[0] ^= 0xFF;

        assert!(
            !tree.verify_proof(&proof),
            "Tampered proof should not verify"
        );
    }

    #[test]
    fn test_proof_verification_tampered_path() {
        let txs: Vec<Hash> = (0..4).map(|i| hash_from_byte(i as u8)).collect();
        let tree = MerkleTree::build(txs);

        let mut proof = tree.generate_proof(1, 100, hash_from_byte(0xFF)).unwrap();

        // Tamper with path hash
        if let Some(node) = proof.path.first_mut() {
            node.hash[0] ^= 0xFF;
        }

        assert!(
            !tree.verify_proof(&proof),
            "Tampered proof should not verify"
        );
    }

    #[test]
    fn test_proof_verification_wrong_root() {
        let txs: Vec<Hash> = (0..4).map(|i| hash_from_byte(i as u8)).collect();
        let tree = MerkleTree::build(txs);

        let proof = tree.generate_proof(1, 100, hash_from_byte(0xFF)).unwrap();

        // Verify against wrong root
        let wrong_root = hash_from_byte(0xEE);
        assert!(!MerkleTree::verify_proof_static(
            &proof.leaf_hash,
            &proof.path,
            &wrong_root
        ));
    }

    // ========== Test Group 4: Power of Two Padding ==========

    #[test]
    fn test_padding_1_to_2() {
        let tree = MerkleTree::build(vec![hash_from_byte(0x01)]);
        assert_eq!(tree.leaf_count(), 2);
    }

    #[test]
    fn test_padding_3_to_4() {
        let txs: Vec<Hash> = (0..3).map(|i| hash_from_byte(i as u8)).collect();
        let tree = MerkleTree::build(txs);
        assert_eq!(tree.leaf_count(), 4);
    }

    #[test]
    fn test_padding_5_to_8() {
        let txs: Vec<Hash> = (0..5).map(|i| hash_from_byte(i as u8)).collect();
        let tree = MerkleTree::build(txs);
        assert_eq!(tree.leaf_count(), 8);
    }

    #[test]
    fn test_padding_17_to_32() {
        let txs: Vec<Hash> = (0..17).map(|i| hash_from_byte(i as u8)).collect();
        let tree = MerkleTree::build(txs);
        assert_eq!(tree.leaf_count(), 32);
    }

    // ========== Test Group 5: Transaction Index ==========

    #[test]
    fn test_transaction_index_put_get() {
        let mut index = TransactionIndex::new(IndexConfig::default());
        let tx_hash = hash_from_byte(0x01);
        let location = TransactionLocation {
            block_height: 100,
            block_hash: hash_from_byte(0xFF),
            tx_index: 5,
            merkle_root: hash_from_byte(0xAA),
        };

        index.put_location(tx_hash, location.clone());

        let retrieved = index.get_location(&tx_hash);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), &location);
    }

    #[test]
    fn test_transaction_index_not_found() {
        let index = TransactionIndex::new(IndexConfig::default());
        let tx_hash = hash_from_byte(0x01);

        assert!(index.get_location(&tx_hash).is_none());
        assert!(!index.is_indexed(&tx_hash));
    }

    // ========== Test Group 6: Cache Management (INVARIANT-5) ==========

    #[test]
    fn test_tree_cache_bounded() {
        // INVARIANT-5: Cache respects max size
        let config = IndexConfig {
            max_cached_trees: 3,
            persist_index: false,
        };
        let mut index = TransactionIndex::new(config);

        // Cache 5 trees
        for i in 0..5u8 {
            let block_hash = hash_from_byte(i);
            let tree = MerkleTree::build(vec![hash_from_byte(i)]);
            index.cache_tree(block_hash, tree);
        }

        // INVARIANT-5: max_cached_trees enforced via LRU eviction
        assert_eq!(index.stats().cached_trees, 3);
    }

    #[test]
    fn test_tree_cache_lru_eviction() {
        let config = IndexConfig {
            max_cached_trees: 3,
            persist_index: false,
        };
        let mut index = TransactionIndex::new(config);

        let block_a = hash_from_byte(0x0A);
        let block_b = hash_from_byte(0x0B);
        let block_c = hash_from_byte(0x0C);
        let block_d = hash_from_byte(0x0D);

        // Cache A, B, C (fills to max)
        index.cache_tree(block_a, MerkleTree::build(vec![hash_from_byte(1)]));
        index.cache_tree(block_b, MerkleTree::build(vec![hash_from_byte(2)]));
        index.cache_tree(block_c, MerkleTree::build(vec![hash_from_byte(3)]));

        // Cache D → evicts A (least recently used)
        index.cache_tree(block_d, MerkleTree::build(vec![hash_from_byte(4)]));

        // A evicted, B/C/D retained
        assert!(!index.has_tree(&block_a));
        assert!(index.has_tree(&block_b));
        assert!(index.has_tree(&block_c));
        assert!(index.has_tree(&block_d));
    }
}
