//! Configuration types for block production

use crate::domain::ConsensusMode;
use primitive_types::U256;
use serde::Deserialize;
use std::path::PathBuf;

/// Runtime configuration for block production
#[derive(Clone, Debug, Deserialize)]
pub struct BlockProductionConfig {
    /// Consensus mode
    pub mode: ConsensusMode,

    /// Block gas limit
    pub gas_limit: u64,

    /// Minimum gas price
    pub min_gas_price: U256,

    /// Enable MEV protection (fair ordering)
    #[allow(dead_code)]
    pub fair_ordering: bool,

    /// Minimum transactions per block (0 = allow empty blocks)
    pub min_transactions: u32,

    /// PoW specific settings
    pub pow: Option<PoWConfig>,

    /// PoS specific settings
    pub pos: Option<PoSConfig>,

    /// PBFT specific settings
    pub pbft: Option<PBFTConfig>,

    /// Performance tuning
    pub performance: PerformanceConfig,
}

impl Default for BlockProductionConfig {
    fn default() -> Self {
        Self {
            mode: ConsensusMode::ProofOfStake,
            gas_limit: crate::DEFAULT_GAS_LIMIT,
            min_gas_price: U256::from(crate::DEFAULT_MIN_GAS_PRICE),
            fair_ordering: true,
            min_transactions: 1,
            pow: None,
            pos: None,
            pbft: None,
            performance: PerformanceConfig::default(),
        }
    }
}

/// PoW configuration
#[derive(Clone, Debug, Deserialize)]
pub struct PoWConfig {
    /// Number of mining threads (default: num_cpus)
    pub threads: u8,

    /// Hash algorithm
    pub algorithm: HashAlgorithm,

    /// Target block time in seconds (default: 10)
    pub target_block_time: Option<u64>,

    /// Use Dark Gravity Wave for per-block difficulty adjustment (default: true)
    pub use_dgw: Option<bool>,

    /// Number of blocks to look back for DGW (default: 24)
    pub dgw_window: Option<usize>,

    /// Mining batch size for GPU/CPU compute engines (default: 10_000_000)
    /// This is the number of nonces to try in each mining iteration.
    /// Higher values may improve GPU efficiency but increase iteration time.
    /// Lower values provide better responsiveness but may reduce throughput.
    pub batch_size: Option<u64>,
}

impl Default for PoWConfig {
    fn default() -> Self {
        Self {
            threads: num_cpus::get() as u8,
            algorithm: HashAlgorithm::Keccak256,
            target_block_time: Some(10),  // 10 seconds per block
            use_dgw: Some(true),          // Enable Dark Gravity Wave
            dgw_window: Some(24),         // Look at last 24 blocks
            batch_size: Some(10_000_000), // Default mining batch size
        }
    }
}

/// Hash algorithm for PoW
#[derive(Copy, Clone, Debug, Deserialize, PartialEq, Eq)]
pub enum HashAlgorithm {
    /// SHA-256d (Bitcoin-style): sha256(sha256(header))
    #[serde(rename = "sha256d")]
    Sha256d,

    /// Keccak-256 (Ethereum-style): keccak256(header)
    #[serde(rename = "keccak256")]
    Keccak256,
}

/// PoS configuration
#[derive(Clone, Debug, Deserialize)]
pub struct PoSConfig {
    /// Path to validator private key
    pub validator_key_path: PathBuf,

    /// Slot duration in seconds (default: 12)
    pub slot_duration: u64,
}

impl Default for PoSConfig {
    fn default() -> Self {
        Self {
            validator_key_path: PathBuf::from("/keys/validator.key"),
            slot_duration: 12,
        }
    }
}

/// PBFT configuration
#[derive(Clone, Debug, Deserialize)]
pub struct PBFTConfig {
    /// Validator ID in the validator set
    pub validator_id: u32,

    /// Total number of validators
    pub total_validators: u32,

    /// View change timeout in seconds
    pub view_change_timeout: u64,
}

impl Default for PBFTConfig {
    fn default() -> Self {
        Self {
            validator_id: 0,
            total_validators: 4,
            view_change_timeout: 30,
        }
    }
}

/// Performance tuning configuration
#[derive(Clone, Debug, Deserialize)]
pub struct PerformanceConfig {
    /// Max transactions to consider (default: 10000)
    pub max_transaction_candidates: u32,

    /// State prefetch cache size in MB (default: 256)
    pub prefetch_cache_size_mb: u64,

    /// Enable parallel simulation (experimental)
    pub parallel_simulation: bool,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            max_transaction_candidates: crate::MAX_TRANSACTION_CANDIDATES,
            prefetch_cache_size_mb: 256,
            parallel_simulation: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = BlockProductionConfig::default();
        assert_eq!(config.mode, ConsensusMode::ProofOfStake);
        assert_eq!(config.gas_limit, crate::DEFAULT_GAS_LIMIT);
        assert!(config.fair_ordering);
    }

    #[test]
    fn test_hash_algorithm() {
        assert_eq!(HashAlgorithm::Sha256d, HashAlgorithm::Sha256d);
        assert_ne!(HashAlgorithm::Sha256d, HashAlgorithm::Keccak256);
    }

    #[test]
    fn test_performance_defaults() {
        let perf = PerformanceConfig::default();
        assert_eq!(
            perf.max_transaction_candidates,
            crate::MAX_TRANSACTION_CANDIDATES
        );
        assert_eq!(perf.prefetch_cache_size_mb, 256);
        assert!(!perf.parallel_simulation);
    }
}
