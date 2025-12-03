//! # Value Objects
//!
//! Immutable value objects for the Transaction Indexing subsystem.
//!
//! ## SPEC-03 Reference
//!
//! - Section 2.4: Value Objects (SENTINEL_HASH, IndexConfig, MerkleConfig)

use serde::{Deserialize, Serialize};
use shared_types::Hash;

/// Sentinel hash used for padding Merkle tree leaves (all zeros).
///
/// ## SPEC-03 Section 2.4
///
/// When padding leaves to a power of two, empty slots are filled
/// with this sentinel value.
pub const SENTINEL_HASH: Hash = [0u8; 32];

/// Configuration for the transaction index.
///
/// ## SPEC-03 Section 2.3
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    /// Maximum number of Merkle trees to cache (default: 1000).
    ///
    /// ## SECURITY (INVARIANT-5)
    ///
    /// Bounds memory usage. Old trees are evicted LRU.
    pub max_cached_trees: usize,
    /// Whether to persist index to storage (default: true).
    pub persist_index: bool,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            max_cached_trees: 1000,
            persist_index: true,
        }
    }
}

/// Configuration for Merkle tree computation.
///
/// ## SPEC-03 Section 2.4
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleConfig {
    /// Hash algorithm identifier (default: SHA3-256).
    pub hash_algorithm: HashAlgorithm,
}

impl Default for MerkleConfig {
    fn default() -> Self {
        Self {
            hash_algorithm: HashAlgorithm::Sha3_256,
        }
    }
}

/// Supported hash algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HashAlgorithm {
    Sha3_256,
    Blake3,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sentinel_hash_is_all_zeros() {
        assert_eq!(SENTINEL_HASH, [0u8; 32]);
    }

    #[test]
    fn test_index_config_default() {
        let config = IndexConfig::default();
        assert_eq!(config.max_cached_trees, 1000);
        assert!(config.persist_index);
    }

    #[test]
    fn test_merkle_config_default() {
        let config = MerkleConfig::default();
        assert_eq!(config.hash_algorithm, HashAlgorithm::Sha3_256);
    }
}
