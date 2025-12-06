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
#[derive(Debug, Clone, Default)]
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
    /// API Gateway configuration.
    pub api_gateway: ApiGatewayConfig,
    /// Mining/Block Production configuration.
    pub mining: MiningConfig,
}

impl NodeConfig {
    /// Validate configuration for production readiness.
    ///
    /// # Returns
    ///
    /// Returns `Err` if:
    /// - HMAC secret is the default zero value
    pub fn validate_for_production(&self) -> Result<(), ConfigError> {
        if self.security.hmac_secret == [0u8; 32] {
            return Err(ConfigError::InsecureHmacSecret);
        }
        Ok(())
    }
}

/// Configuration errors.
#[derive(Debug)]
pub enum ConfigError {
    /// HMAC secret is not set (zero value).
    InsecureHmacSecret,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::InsecureHmacSecret => {
                write!(
                    f,
                    "SECURITY VIOLATION: HMAC secret is default zero value. \
                     Set QC_HMAC_SECRET environment variable or provide in config."
                )
            }
        }
    }
}

impl std::error::Error for ConfigError {}

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

/// API Gateway configuration.
#[derive(Debug, Clone)]
pub struct ApiGatewayConfig {
    /// Enable the API Gateway.
    pub enabled: bool,
    /// HTTP/JSON-RPC port.
    pub http_port: u16,
    /// WebSocket port.
    pub ws_port: u16,
    /// Admin API port (localhost only by default).
    pub admin_port: u16,
    /// Optional API key for protected endpoints.
    pub api_key: Option<String>,
    /// Rate limit (requests per second per IP).
    pub rate_limit_per_second: u32,
    /// Maximum batch size.
    pub max_batch_size: usize,
    /// Chain ID for eth_chainId responses.
    pub chain_id: u64,
}

impl Default for ApiGatewayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            http_port: 8545,
            ws_port: 8546,
            admin_port: 8080,
            api_key: None,
            rate_limit_per_second: 100,
            max_batch_size: 100,
            chain_id: 1,
        }
    }
}

/// Mining/Block Production configuration.
#[derive(Debug, Clone)]
pub struct MiningConfig {
    /// Enable mining (block production).
    pub enabled: bool,
    /// Number of worker threads for mining.
    pub worker_threads: usize,
    /// Target block time in milliseconds.
    pub target_block_time_ms: u64,
    /// Initial difficulty (number of leading zero bits).
    pub initial_difficulty: u32,
    /// Number of blocks between difficulty adjustments.
    pub difficulty_adjustment_interval: u64,
    /// Maximum difficulty adjustment factor (e.g., 4.0 = can change by 4x).
    pub max_adjustment_factor: f64,
    /// Mempool refresh interval in milliseconds.
    pub pool_refresh_interval_ms: u64,
}

impl Default for MiningConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            worker_threads: num_cpus::get().max(1),
            target_block_time_ms: 12000,         // 12 seconds
            initial_difficulty: 20,              // 20 leading zero bits
            difficulty_adjustment_interval: 100, // Every 100 blocks
            max_adjustment_factor: 4.0,
            pool_refresh_interval_ms: 1000, // 1 second
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
    fn test_validate_rejects_default_hmac() {
        let config = NodeConfig::default();
        assert!(config.validate_for_production().is_err());
    }

    #[test]
    fn test_validate_accepts_nonzero_hmac() {
        let mut config = NodeConfig::default();
        config.security.hmac_secret = [1u8; 32];
        assert!(config.validate_for_production().is_ok());
    }
}
