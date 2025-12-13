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

    /// DEPRECATED (V2.4): Drain pending mined blocks from the queue
    /// This is no longer used - bridge now subscribes to BlockProduced events.
    /// Kept for API compatibility, will be removed in V2.5.
    #[deprecated(since = "2.4.0", note = "Bridge now subscribes to BlockProduced events")]
    async fn drain_pending_blocks(&self) -> Vec<MinedBlockInfo>;

    /// Update block gas limit
    async fn update_gas_limit(&self, new_limit: u64) -> Result<()>;

    /// Update minimum gas price
    async fn update_min_gas_price(&self, new_price: U256) -> Result<()>;
}

/// Historical block info for difficulty adjustment when resuming
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoricalBlockInfo {
    /// Block height
    pub height: u64,
    /// Block timestamp (Unix epoch seconds)
    pub timestamp: u64,
    /// Difficulty at this block
    pub difficulty: U256,
    /// Block hash
    pub hash: H256,
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

    /// Starting block height (for resuming from existing chain)
    pub starting_height: u64,

    /// Last known difficulty (for resuming PoW mining)
    /// If not provided, will use initial difficulty
    pub last_difficulty: Option<U256>,

    /// Recent block history for difficulty adjustment (newest first)
    /// Used to properly calculate difficulty when resuming
    pub recent_blocks: Vec<HistoricalBlockInfo>,
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
            starting_height: 0,
            last_difficulty: None,
            recent_blocks: Vec::new(),
        }
    }
}

/// Information about a single mined block
#[derive(Clone, Debug)]
pub struct MinedBlockInfo {
    /// Block height
    pub height: u64,
    /// Block timestamp
    pub timestamp: u64,
    /// Difficulty used for this block
    pub difficulty: U256,
    /// Nonce found (PoW only)
    pub nonce: u64,
    /// Parent block hash
    pub parent_hash: [u8; 32],
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

    /// Current PoW difficulty target (higher = easier)
    pub current_difficulty: Option<U256>,

    /// Last mined nonce (PoW only)
    pub last_nonce: Option<u64>,

    /// DEPRECATED (V2.4): Queue of mined blocks waiting for bridge to process
    /// This is no longer used - blocks are now published via BlockProduced events.
    /// Kept for API compatibility, will be removed in V2.5.
    #[deprecated(since = "2.4.0", note = "Blocks now published via BlockProduced events")]
    pub pending_blocks: Vec<MinedBlockInfo>,
}
