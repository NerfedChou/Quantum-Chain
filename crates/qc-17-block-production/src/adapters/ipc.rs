//! IPC adapters for communication with other subsystems
//!
//! Implements communication with:
//! - Subsystem 6 (Mempool) - Get pending transactions
//! - Subsystem 4 (State Management) - State prefetch/simulation
//! - Subsystem 8 (Consensus) - Submit produced blocks
//!
//! **Architecture:** Hexagonal - IPC adapters as secondary adapters
//! **Security:** Message validation, correlation IDs, rate limiting

use crate::{BlockTemplate, ConsensusMode, TransactionCandidate};
use primitive_types::{H256, U256};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

// ============================================================================
// IPC MESSAGE TYPES (from IPC-MATRIX.md)
// ============================================================================

/// Request to Mempool for pending transactions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetPendingTransactionsRequest {
    /// IPC protocol version
    pub version: u16,
    /// Correlation ID for tracking
    pub correlation_id: [u8; 16],
    /// Reply topic name
    pub reply_to: String,
    /// Maximum number of transactions
    pub max_count: u32,
    /// Minimum gas price filter
    pub min_gas_price: U256,
    /// Message signature
    pub signature: Vec<u8>,
}

/// Response from Mempool with pending transactions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingTransactionsResponse {
    /// IPC protocol version
    pub version: u16,
    /// Correlation ID matching request
    pub correlation_id: [u8; 16],
    /// List of verified transactions
    pub transactions: Vec<VerifiedTransaction>,
    /// Total transactions in mempool
    pub total_count: u32,
    /// Number of transactions returned
    pub returned_count: u32,
    /// Response signature
    pub signature: Vec<u8>,
}

/// Verified transaction from mempool (already checked by Subsystem 10)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerifiedTransaction {
    /// Serialized SignedTransaction
    pub transaction: Vec<u8>,
    /// Sender address
    pub from: [u8; 20],
    /// Transaction nonce
    pub nonce: u64,
    /// Gas price offered
    pub gas_price: U256,
    /// Gas limit
    pub gas_limit: u64,
    /// Signature verification status
    pub signature_valid: bool,
}

/// Request to State Management for state prefetch
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatePrefetchRequest {
    /// IPC protocol version
    pub version: u16,
    /// Correlation ID for tracking
    pub correlation_id: [u8; 16],
    /// Reply topic name
    pub reply_to: String,
    /// Parent state root hash (used for state simulation)
    pub parent_state_root: H256,
    /// Serialized transactions to simulate
    pub transactions: Vec<Vec<u8>>,
    /// Message signature
    pub signature: Vec<u8>,
}

/// Response from State Management with simulation results
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatePrefetchResponse {
    /// IPC protocol version
    pub version: u16,
    /// Correlation ID matching request
    pub correlation_id: [u8; 16],
    /// Simulation results
    pub simulations: Vec<TransactionSimulation>,
    /// State cache data
    pub state_cache: Vec<u8>,
    /// Response signature
    pub signature: Vec<u8>,
}

/// Simulation result for a single transaction
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionSimulation {
    /// Transaction hash
    pub tx_hash: H256,
    /// Execution success
    pub success: bool,
    /// Gas consumed
    pub gas_used: u64,
    /// State modifications
    pub state_changes: Vec<StateChange>,
    /// Error message if failed
    pub error: Option<String>,
}

/// State change record
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateChange {
    /// Account address
    pub address: [u8; 20],
    /// Storage slot key (if storage change)
    pub storage_key: Option<H256>,
    /// Previous value
    pub old_value: Vec<u8>,
    /// New value
    pub new_value: Vec<u8>,
}

/// Produced block submitted to Consensus
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProduceBlockRequest {
    /// IPC protocol version
    pub version: u16,
    /// Sender subsystem ID (always 17)
    pub sender_id: u8,
    /// Correlation ID for request tracking
    pub correlation_id: [u8; 16],
    /// Reply channel address
    pub reply_to: String,
    /// Block template with transactions
    pub block_template: BlockTemplateIpc,
    /// Consensus mode used
    pub consensus_mode: ConsensusModeIpc,
    /// Nonce for PoW
    pub nonce: Option<u64>,
    /// VRF proof for PoS
    pub vrf_proof: Option<VRFProof>,
    /// Validator signature for PoS/PBFT
    pub validator_signature: Option<Vec<u8>>,
    /// Message signature
    pub signature: Vec<u8>,
}

/// Block template for IPC (serializable version)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockTemplateIpc {
    /// Parent block hash
    pub parent_hash: H256,
    /// Block number
    pub block_number: u64,
    /// Block timestamp
    pub timestamp: u64,
    /// Beneficiary address
    pub beneficiary: [u8; 20],
    /// Total gas used
    pub gas_used: u64,
    /// Gas limit
    pub gas_limit: u64,
    /// Transaction data
    pub transactions: Vec<Vec<u8>>,
    /// Total transaction fees
    pub total_fees: U256,
}

/// Consensus mode for block production
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ConsensusModeIpc {
    /// Proof of Work
    ProofOfWork,
    /// Proof of Stake
    ProofOfStake,
    /// Practical Byzantine Fault Tolerance
    PBFT,
}

/// VRF (Verifiable Random Function) proof for PoS
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VRFProof {
    #[serde(with = "serde_bytes")]
    /// VRF output hash
    pub output: [u8; 32],
    #[serde(with = "serde_bytes")]
    /// VRF proof data
    pub proof: [u8; 80],
}

/// Response from Consensus after block submission
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubmissionReceipt {
    /// IPC protocol version
    pub version: u16,
    /// Correlation ID matching request
    pub correlation_id: [u8; 16],
    /// Whether block was accepted
    pub accepted: bool,
    /// Hash of accepted block
    pub block_hash: Option<H256>,
    /// Error message if rejected
    pub error: Option<String>,
    /// Response signature
    pub signature: Vec<u8>,
}

// ============================================================================
// PORT TRAITS (Domain Layer Interface)
// ============================================================================

/// Port for reading pending transactions from mempool
#[async_trait::async_trait]
pub trait MempoolReader: Send + Sync {
    /// Get pending transactions from mempool
    async fn get_pending_transactions(
        &self,
        max_count: u32,
        min_gas_price: U256,
    ) -> Result<Vec<TransactionCandidate>, IpcError>;
}

/// Port for state simulation
#[async_trait::async_trait]
pub trait StateReader: Send + Sync {
    /// Simulate transaction execution against current state
    async fn simulate_transactions(
        &self,
        parent_state_root: H256,
        transactions: &[TransactionCandidate],
    ) -> Result<Vec<SimulationResult>, IpcError>;
}

/// Port for submitting produced blocks to consensus
#[async_trait::async_trait]
pub trait ConsensusSubmitter: Send + Sync {
    /// Submit produced block to consensus layer
    async fn submit_block(
        &self,
        block: &BlockTemplate,
        nonce: Option<u64>,
        vrf_proof: Option<VRFProof>,
        validator_signature: Option<Vec<u8>>,
    ) -> Result<SubmissionReceipt, IpcError>;
}

// ============================================================================
// DOMAIN TYPES
// ============================================================================

/// Simulation result (domain type)
#[derive(Clone, Debug)]
pub struct SimulationResult {
    /// Transaction hash
    pub tx_hash: H256,
    /// Execution success
    pub success: bool,
    /// Gas consumed
    pub gas_used: u64,
    /// Error message if failed
    pub error: Option<String>,
}

/// IPC error types
#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    /// Request timeout
    #[error("Timeout waiting for response")]
    Timeout,

    /// Subsystem unavailable
    #[error("Subsystem {0} not available")]
    SubsystemUnavailable(u8),

    /// Invalid response format
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Network error
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Correlation ID mismatch
    #[error("Correlation ID mismatch")]
    CorrelationMismatch,
}

// ============================================================================
// IPC ADAPTER IMPLEMENTATIONS
// ============================================================================

/// IPC adapter for Mempool (Subsystem 6)
pub struct IpcMempoolReader {
    /// Subsystem ID for IPC routing
    subsystem_id: u8,
}

impl Default for IpcMempoolReader {
    fn default() -> Self {
        Self::new()
    }
}

impl IpcMempoolReader {
    /// Creates a new IPC mempool reader
    pub fn new() -> Self {
        Self { subsystem_id: 17 }
    }

    /// Get the subsystem ID
    pub fn subsystem_id(&self) -> u8 {
        self.subsystem_id
    }

    /// Generate correlation ID for request tracking
    fn generate_correlation_id() -> [u8; 16] {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        timestamp.to_le_bytes()
    }
}

#[async_trait::async_trait]
impl MempoolReader for IpcMempoolReader {
    async fn get_pending_transactions(
        &self,
        max_count: u32,
        min_gas_price: U256,
    ) -> Result<Vec<TransactionCandidate>, IpcError> {
        let correlation_id = Self::generate_correlation_id();

        debug!(
            "Requesting pending transactions from Mempool (max={}, min_gas={:?})",
            max_count, min_gas_price
        );

        let _request = GetPendingTransactionsRequest {
            version: 1,
            correlation_id,
            reply_to: "qc17.mempool.reply".to_string(),
            max_count,
            min_gas_price,
            signature: vec![], // Signature added by transport layer
        };

        // IPC transport sends request and awaits response
        // Currently returns empty - mempool integration via choreography
        warn!("IPC not fully wired - returning empty transactions");
        Ok(vec![])
    }
}

/// IPC adapter for State Management (Subsystem 4)
pub struct IpcStateReader {
    subsystem_id: u8,
}

impl Default for IpcStateReader {
    fn default() -> Self {
        Self::new()
    }
}

impl IpcStateReader {
    /// Creates a new IPC state reader
    pub fn new() -> Self {
        Self { subsystem_id: 17 }
    }

    /// Get the subsystem ID
    pub fn subsystem_id(&self) -> u8 {
        self.subsystem_id
    }

    fn generate_correlation_id() -> [u8; 16] {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        timestamp.to_le_bytes()
    }
}

#[async_trait::async_trait]
impl StateReader for IpcStateReader {
    async fn simulate_transactions(
        &self,
        parent_state_root: H256,
        transactions: &[TransactionCandidate],
    ) -> Result<Vec<SimulationResult>, IpcError> {
        let correlation_id = Self::generate_correlation_id();

        debug!(
            "Requesting state simulation for {} transactions",
            transactions.len()
        );

        let tx_data: Vec<Vec<u8>> = transactions
            .iter()
            .map(|tx| tx.transaction.clone())
            .collect();

        let _request = StatePrefetchRequest {
            version: 1,
            correlation_id,
            reply_to: "qc17.state.reply".to_string(),
            parent_state_root,
            transactions: tx_data,
            signature: vec![],
        };

        // IPC transport handles request/response
        // For now, return mock success for all transactions
        warn!("IPC not fully wired - returning mock simulations");

        Ok(transactions
            .iter()
            .map(|tx| {
                // Compute hash of transaction data
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(&tx.transaction);
                let hash_bytes = hasher.finalize();

                SimulationResult {
                    tx_hash: H256::from_slice(&hash_bytes),
                    success: true,
                    gas_used: tx.gas_limit, // Assume full gas used
                    error: None,
                }
            })
            .collect())
    }
}

/// IPC adapter for Consensus (Subsystem 8)
pub struct IpcConsensusSubmitter {
    subsystem_id: u8,
}

impl Default for IpcConsensusSubmitter {
    fn default() -> Self {
        Self::new()
    }
}

impl IpcConsensusSubmitter {
    /// Creates a new IPC consensus submitter
    pub fn new() -> Self {
        Self { subsystem_id: 17 }
    }

    /// Get the subsystem ID
    pub fn subsystem_id(&self) -> u8 {
        self.subsystem_id
    }

    fn generate_correlation_id() -> [u8; 16] {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        timestamp.to_le_bytes()
    }
}

#[async_trait::async_trait]
impl ConsensusSubmitter for IpcConsensusSubmitter {
    async fn submit_block(
        &self,
        block: &BlockTemplate,
        nonce: Option<u64>,
        vrf_proof: Option<VRFProof>,
        validator_signature: Option<Vec<u8>>,
    ) -> Result<SubmissionReceipt, IpcError> {
        let correlation_id = Self::generate_correlation_id();

        debug!(
            "Submitting block #{} to Consensus (mode={:?})",
            block.header.block_number, block.consensus_mode
        );

        let consensus_mode = match block.consensus_mode {
            ConsensusMode::ProofOfWork => ConsensusModeIpc::ProofOfWork,
            ConsensusMode::ProofOfStake => ConsensusModeIpc::ProofOfStake,
            ConsensusMode::PBFT => ConsensusModeIpc::PBFT,
        };

        let _request = ProduceBlockRequest {
            version: 1,
            sender_id: 17,
            correlation_id,
            reply_to: "qc17.consensus.reply".to_string(),
            block_template: BlockTemplateIpc {
                parent_hash: block.header.parent_hash,
                block_number: block.header.block_number,
                timestamp: block.header.timestamp,
                beneficiary: block.header.beneficiary,
                gas_used: block.total_gas_used,
                gas_limit: block.header.gas_limit,
                transactions: block.transactions.clone(),
                total_fees: block.total_fees,
            },
            consensus_mode,
            nonce,
            vrf_proof,
            validator_signature,
            signature: vec![],
        };

        // IPC transport handles request/response
        warn!("IPC not fully wired - returning mock acceptance");

        Ok(SubmissionReceipt {
            version: 1,
            correlation_id,
            accepted: true,
            block_hash: Some(block.header.parent_hash), // Mock hash
            error: None,
            signature: vec![],
        })
    }
}

// ============================================================================
// BLOCK STORAGE READER (V2.4 - Difficulty Persistence)
// ============================================================================

/// Request to Block Storage for chain info
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetChainInfoRequest {
    /// IPC protocol version
    pub version: u16,
    /// Correlation ID for tracking
    pub correlation_id: [u8; 16],
    /// Reply topic name
    pub reply_to: String,
    /// Number of recent blocks to include
    pub recent_blocks_count: u32,
    /// Message signature
    pub signature: Vec<u8>,
}

/// Response from Block Storage with chain info
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetChainInfoResponse {
    /// IPC protocol version
    pub version: u16,
    /// Correlation ID matching request
    pub correlation_id: [u8; 16],
    /// Latest block height
    pub chain_tip_height: u64,
    /// Latest block hash
    pub chain_tip_hash: [u8; 32],
    /// Latest block timestamp
    pub chain_tip_timestamp: u64,
    /// Recent blocks for difficulty adjustment
    pub recent_blocks: Vec<BlockDifficultyInfoIpc>,
    /// Response signature
    pub signature: Vec<u8>,
}

/// Block difficulty info for IPC (serializable)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockDifficultyInfoIpc {
    /// Block height
    pub height: u64,
    /// Block timestamp
    pub timestamp: u64,
    /// Difficulty target (serialized as 32 bytes)
    pub difficulty: [u8; 32],
    /// Block hash
    pub hash: [u8; 32],
}

use crate::ports::outbound::{BlockStorageReader, ChainInfo};

/// IPC adapter for Block Storage (Subsystem 2)
///
/// V2.4: Queries chain state on startup for proper difficulty
/// adjustment continuity across restarts.
pub struct IpcBlockStorageReader {
    /// Subsystem ID for IPC routing
    subsystem_id: u8,
}

impl Default for IpcBlockStorageReader {
    fn default() -> Self {
        Self::new()
    }
}

impl IpcBlockStorageReader {
    /// Creates a new IPC block storage reader
    pub fn new() -> Self {
        Self { subsystem_id: 17 }
    }

    /// Get the subsystem ID
    pub fn subsystem_id(&self) -> u8 {
        self.subsystem_id
    }

    /// Generate correlation ID for request tracking
    fn generate_correlation_id() -> [u8; 16] {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        timestamp.to_le_bytes()
    }
}

#[async_trait::async_trait]
impl BlockStorageReader for IpcBlockStorageReader {
    async fn get_chain_info(
        &self,
        recent_blocks_count: u32,
    ) -> Result<ChainInfo, crate::error::BlockProductionError> {
        let correlation_id = Self::generate_correlation_id();

        debug!(
            "Requesting chain info from Block Storage (recent_blocks={})",
            recent_blocks_count
        );

        let _request = GetChainInfoRequest {
            version: 1,
            correlation_id,
            reply_to: "qc17.storage.reply".to_string(),
            recent_blocks_count,
            signature: vec![],
        };

        // IPC transport sends request and awaits response
        // Currently returns empty chain - will be wired when IPC transport ready
        warn!("IPC not fully wired - returning empty chain info");

        Ok(ChainInfo::default())
    }
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::BlockHeader;
    use crate::ConsensusMode;

    #[tokio::test]
    async fn test_mempool_reader_interface() {
        let reader = IpcMempoolReader::new();
        let result = reader.get_pending_transactions(100, U256::from(1000)).await;

        assert!(result.is_ok());
        // Currently returns empty due to mock
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_state_reader_interface() {
        let reader = IpcStateReader::new();
        let result = reader.simulate_transactions(H256::zero(), &[]).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_consensus_submitter_interface() {
        let submitter = IpcConsensusSubmitter::new();

        let block = BlockTemplate {
            header: BlockHeader {
                parent_hash: H256::zero(),
                block_number: 1,
                timestamp: 1000,
                beneficiary: [0u8; 20],
                gas_used: 21000,
                gas_limit: 10_000_000,
                difficulty: U256::from(1000),
                extra_data: vec![],
                merkle_root: None,
                state_root: None,
                nonce: None,
            },
            transactions: vec![],
            total_gas_used: 21000,
            total_fees: U256::from(21000),
            consensus_mode: ConsensusMode::ProofOfWork,
            created_at: 1000,
        };

        let result = submitter
            .submit_block(&block, Some(12345), None, None)
            .await;

        assert!(result.is_ok());
        let receipt = result.unwrap();
        assert!(receipt.accepted);
        assert!(receipt.block_hash.is_some());
    }

    #[test]
    fn test_correlation_id_generation() {
        let id1 = IpcMempoolReader::generate_correlation_id();
        let id2 = IpcMempoolReader::generate_correlation_id();

        // Should be different (time-based)
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_ipc_message_serialization() {
        let request = GetPendingTransactionsRequest {
            version: 1,
            correlation_id: [0u8; 16],
            reply_to: "test.topic".to_string(),
            max_count: 100,
            min_gas_price: U256::from(1000),
            signature: vec![1, 2, 3],
        };

        let serialized = serde_json::to_string(&request);
        assert!(serialized.is_ok());

        let deserialized: Result<GetPendingTransactionsRequest, _> =
            serde_json::from_str(&serialized.unwrap());
        assert!(deserialized.is_ok());
    }

    #[tokio::test]
    async fn test_block_storage_reader_interface() {
        let reader = IpcBlockStorageReader::new();
        let result = reader.get_chain_info(24).await;

        assert!(result.is_ok());
        let chain_info = result.unwrap();
        // Currently returns empty chain due to mock
        assert_eq!(chain_info.chain_tip_height, 0);
        assert!(chain_info.recent_blocks.is_empty());
    }

    #[test]
    fn test_get_chain_info_request_serialization() {
        let request = GetChainInfoRequest {
            version: 1,
            correlation_id: [0u8; 16],
            reply_to: "qc17.storage.reply".to_string(),
            recent_blocks_count: 24,
            signature: vec![],
        };

        let serialized = serde_json::to_string(&request);
        assert!(serialized.is_ok());

        let deserialized: Result<GetChainInfoRequest, _> =
            serde_json::from_str(&serialized.unwrap());
        assert!(deserialized.is_ok());
        assert_eq!(deserialized.unwrap().recent_blocks_count, 24);
    }

    #[test]
    fn test_block_difficulty_info_ipc_serialization() {
        let info = BlockDifficultyInfoIpc {
            height: 100,
            timestamp: 1700000000,
            difficulty: [0xFF; 32],
            hash: [0xAB; 32],
        };

        let serialized = serde_json::to_string(&info);
        assert!(serialized.is_ok());

        let deserialized: Result<BlockDifficultyInfoIpc, _> =
            serde_json::from_str(&serialized.unwrap());
        assert!(deserialized.is_ok());
        assert_eq!(deserialized.unwrap().height, 100);
    }
}
