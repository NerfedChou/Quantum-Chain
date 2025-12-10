//! # Light Client Configuration
//!
//! Configuration for the Light Client service.
//!
//! Reference: SPEC-13 Section 7 (Lines 99-124)

use serde::{Deserialize, Serialize};
use crate::domain::{MIN_FULL_NODES, DEFAULT_CONFIRMATIONS};

/// Light client configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LightClientConfig {
    /// Minimum number of full nodes to query.
    /// Reference: System.md Line 644
    pub min_full_nodes: usize,

    /// Required confirmations for transaction verification.
    pub required_confirmations: u64,

    /// Maximum headers to sync in one batch.
    pub header_batch_size: usize,

    /// Proof cache size (number of proofs to cache).
    pub proof_cache_size: usize,

    /// Sync timeout in seconds.
    pub sync_timeout_secs: u64,

    /// Enable Bloom filter privacy mode.
    /// Reference: SPEC-13 Lines 620-629
    pub privacy_mode: bool,

    /// Number of random addresses to add to Bloom filter.
    pub bloom_noise_count: usize,

    /// Connection rotation interval in seconds.
    /// Reference: SPEC-13 Line 629
    pub peer_rotation_secs: u64,
}

impl Default for LightClientConfig {
    fn default() -> Self {
        Self {
            min_full_nodes: MIN_FULL_NODES,
            required_confirmations: DEFAULT_CONFIRMATIONS,
            header_batch_size: 2000,
            proof_cache_size: 1000,
            sync_timeout_secs: 30,
            privacy_mode: true,
            bloom_noise_count: 50,
            peer_rotation_secs: 600,
        }
    }
}

impl LightClientConfig {
    /// Create a config for testing (smaller values).
    pub fn for_testing() -> Self {
        Self {
            min_full_nodes: 1,
            required_confirmations: 1,
            header_batch_size: 100,
            proof_cache_size: 100,
            sync_timeout_secs: 5,
            privacy_mode: false,
            bloom_noise_count: 0,
            peer_rotation_secs: 60,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LightClientConfig::default();
        assert_eq!(config.min_full_nodes, 3);
        assert_eq!(config.required_confirmations, 6);
        assert!(config.privacy_mode);
    }

    #[test]
    fn test_testing_config() {
        let config = LightClientConfig::for_testing();
        assert_eq!(config.min_full_nodes, 1);
        assert!(!config.privacy_mode);
    }
}
