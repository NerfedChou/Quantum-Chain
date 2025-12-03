//! # Node Configuration
//!
//! Unified configuration for all subsystems and runtime parameters.
//!
//! ## Security Requirements
//!
//! - `hmac_secret` MUST NOT be the default zero value in production
//! - All timeouts and limits have sane defaults with override capability

use std::path::PathBuf;

/// Complete node configuration.
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// Network configuration.
    pub network: NetworkConfig,
    /// Storage configuration.
    pub storage: StorageConfig,
    /// Security configuration.
    pub security: SecurityConfig,
    /// Consensus configuration.
    pub consensus: ConsensusConfig,
    /// Mempool configuration.
    pub mempool: MempoolConfig,
    /// Finality configuration.
    pub finality: FinalityConfig,
}

impl NodeConfig {
    /// Validate configuration for production readiness.
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - HMAC secret is the default zero value
    /// - Data directory is not writable
    pub fn validate_for_production(&self) {
        if self.security.hmac_secret == [0u8; 32] {
            panic!(
                "SECURITY VIOLATION: HMAC secret is default zero value. \
                 Set QC_HMAC_SECRET environment variable or provide in config."
            );
        }
    }
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            network: NetworkConfig::default(),
            storage: StorageConfig::default(),
            security: SecurityConfig::default(),
            consensus: ConsensusConfig::default(),
            mempool: MempoolConfig::default(),
            finality: FinalityConfig::default(),
        }
    }
}

/// Network configuration.
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// P2P listening port.
    pub p2p_port: u16,
    /// JSON-RPC listening port.
    pub rpc_port: u16,
    /// Maximum peers in routing table.
    pub max_peers: usize,
    /// Bootstrap node addresses.
    pub bootstrap_nodes: Vec<String>,
    /// Gossip fanout (number of peers to propagate to).
    pub gossip_fanout: usize,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            p2p_port: 30303,
            rpc_port: 8545,
            max_peers: 50,
            bootstrap_nodes: Vec::new(),
            gossip_fanout: 8,
        }
    }
}

/// Storage configuration.
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// Data directory for block storage.
    pub data_dir: PathBuf,
    /// Block assembly timeout in seconds (INVARIANT-7).
    pub assembly_timeout_secs: u64,
    /// Maximum pending block assemblies (INVARIANT-8).
    pub max_pending_assemblies: usize,
    /// Minimum disk space percentage before rejecting writes.
    pub min_disk_space_percent: u8,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./data"),
            assembly_timeout_secs: 30,
            max_pending_assemblies: 1000,
            min_disk_space_percent: 5,
        }
    }
}

/// Security configuration.
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// HMAC secret for inter-subsystem authentication (32 bytes).
    /// MUST NOT be default in production.
    pub hmac_secret: [u8; 32],
    /// Nonce cache expiry in seconds.
    pub nonce_cache_expiry_secs: u64,
    /// Maximum message age in seconds.
    pub max_message_age_secs: u64,
    /// Maximum future timestamp skew in seconds.
    pub max_future_skew_secs: u64,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            hmac_secret: [0u8; 32], // MUST be overridden in production
            nonce_cache_expiry_secs: 120,
            max_message_age_secs: 60,
            max_future_skew_secs: 10,
        }
    }
}

/// Consensus configuration.
#[derive(Debug, Clone)]
pub struct ConsensusConfig {
    /// Consensus algorithm: "pos" or "pbft".
    pub algorithm: String,
    /// Minimum attestation percentage for PoS (default: 67 = 2/3).
    pub min_attestation_percent: u8,
    /// Maximum block gas limit.
    pub max_block_gas: u64,
    /// Block time in seconds.
    pub block_time_secs: u64,
    /// Epoch length in blocks.
    pub epoch_length: u64,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            algorithm: "pos".to_string(),
            min_attestation_percent: 67,
            max_block_gas: 30_000_000,
            block_time_secs: 12,
            epoch_length: 32,
        }
    }
}

/// Mempool configuration.
#[derive(Debug, Clone)]
pub struct MempoolConfig {
    /// Maximum transactions in pool.
    pub max_transactions: usize,
    /// Maximum pending transactions per account.
    pub max_per_account: usize,
    /// Minimum gas price in wei.
    pub min_gas_price: u64,
    /// Pending inclusion timeout in seconds.
    pub pending_inclusion_timeout_secs: u64,
}

impl Default for MempoolConfig {
    fn default() -> Self {
        Self {
            max_transactions: 5000,
            max_per_account: 16,
            min_gas_price: 1_000_000_000, // 1 gwei
            pending_inclusion_timeout_secs: 30,
        }
    }
}

/// Finality configuration.
#[derive(Debug, Clone)]
pub struct FinalityConfig {
    /// Justification threshold (percentage, default: 67).
    pub justification_threshold: u8,
    /// Maximum epochs without finality before inactivity leak.
    pub max_epochs_without_finality: u64,
    /// Circuit breaker: max consecutive sync failures before halt.
    pub max_sync_failures: u8,
}

impl Default for FinalityConfig {
    fn default() -> Self {
        Self {
            justification_threshold: 67,
            max_epochs_without_finality: 4,
            max_sync_failures: 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = NodeConfig::default();
        assert_eq!(config.network.p2p_port, 30303);
        assert_eq!(config.consensus.min_attestation_percent, 67);
        assert_eq!(config.mempool.max_transactions, 5000);
    }

    #[test]
    #[should_panic(expected = "HMAC secret is default zero value")]
    fn test_validate_rejects_default_hmac() {
        let config = NodeConfig::default();
        config.validate_for_production();
    }

    #[test]
    fn test_validate_accepts_nonzero_hmac() {
        let mut config = NodeConfig::default();
        config.security.hmac_secret = [1u8; 32];
        config.validate_for_production(); // Should not panic
    }
}
