//! Inbound ports (driving side - API)

use crate::domain::{BlockTemplate, ConsensusMode};
use crate::error::Result;
use async_trait::async_trait;
use primitive_types::{H256, U256};

/// Primary port: Block production service
#[async_trait]
pub trait BlockProducerService: Send + Sync {
    /// Produce a new block
    async fn produce_block(
        &self,
        parent_hash: H256,
        beneficiary: [u8; 20],
    ) -> Result<BlockTemplate>;

    /// Start mining/proposing
    async fn start_production(&self, mode: ConsensusMode, config: ProductionConfig) -> Result<()>;

    /// Stop mining/proposing
    async fn stop_production(&self) -> Result<()>;

    /// Get current mining/proposing status
    async fn get_status(&self) -> ProductionStatus;

    /// Update block gas limit
    async fn update_gas_limit(&self, new_limit: u64) -> Result<()>;

    /// Update minimum gas price
    async fn update_min_gas_price(&self, new_price: U256) -> Result<()>;
}

/// Production configuration
#[derive(Clone, Debug)]
pub struct ProductionConfig {
    /// Consensus mode
    pub mode: ConsensusMode,

    /// Number of threads (PoW only)
    pub pow_threads: Option<u8>,

    /// Validator key path (PoS only)
    pub validator_key_path: Option<String>,

    /// Block gas limit
    pub gas_limit: u64,

    /// Minimum gas price
    pub min_gas_price: U256,

    /// Enable MEV protection
    pub fair_ordering: bool,
}

impl Default for ProductionConfig {
    fn default() -> Self {
        Self {
            mode: ConsensusMode::ProofOfStake,
            pow_threads: None,
            validator_key_path: None,
            gas_limit: crate::DEFAULT_GAS_LIMIT,
            min_gas_price: U256::from(crate::DEFAULT_MIN_GAS_PRICE),
            fair_ordering: true,
        }
    }
}

/// Production status
#[derive(Clone, Debug)]
pub struct ProductionStatus {
    /// Is currently producing blocks
    pub active: bool,

    /// Current consensus mode
    pub mode: Option<ConsensusMode>,

    /// Blocks produced this session
    pub blocks_produced: u64,

    /// Total fees collected
    pub total_fees: U256,

    /// Current hashrate (PoW only)
    pub hashrate: Option<f64>,

    /// Last block produced timestamp
    pub last_block_at: Option<u64>,
}
