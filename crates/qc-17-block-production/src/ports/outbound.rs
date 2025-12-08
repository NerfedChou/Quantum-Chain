//! Outbound ports (driven side - SPI)

use crate::domain::{BlockTemplate, SimulationResult, TransactionCandidate};
use crate::error::Result;
use async_trait::async_trait;
use primitive_types::{H256, U256};

/// Port: Fetch pending transactions from Mempool
#[async_trait]
pub trait MempoolReader: Send + Sync {
    /// Get pending transactions
    async fn get_pending_transactions(
        &self,
        max_count: u32,
        min_gas_price: U256,
    ) -> Result<Vec<TransactionCandidate>>;
}

/// Port: State prefetch and simulation
#[async_trait]
pub trait StateReader: Send + Sync {
    /// Simulate transaction batch
    async fn simulate_transactions(
        &self,
        state_root: H256,
        transactions: Vec<Vec<u8>>,
    ) -> Result<Vec<SimulationResult>>;
}

/// Port: Submit produced block to Consensus
#[async_trait]
pub trait ConsensusSubmitter: Send + Sync {
    /// Submit block template for validation
    async fn submit_block(
        &self,
        template: BlockTemplate,
        consensus_proof: ConsensusProof,
    ) -> Result<SubmissionReceipt>;
}

/// Consensus proof for block submission
#[derive(Clone, Debug)]
pub struct ConsensusProof {
    /// PoW nonce (if applicable)
    pub pow_nonce: Option<u64>,

    /// PoS VRF proof (if applicable)
    pub pos_vrf_proof: Option<Vec<u8>>,

    /// PoS validator signature (if applicable)
    pub pos_signature: Option<Vec<u8>>,

    /// PBFT leader signature (if applicable)
    pub pbft_signature: Option<Vec<u8>>,
}

/// Block submission receipt
#[derive(Clone, Debug)]
pub struct SubmissionReceipt {
    /// Block hash
    pub block_hash: H256,

    /// Submission timestamp
    pub submitted_at: u64,

    /// Acceptance status
    pub accepted: bool,
}

/// Port: Sign blocks with validator key
#[async_trait]
pub trait SignatureProvider: Send + Sync {
    /// Sign block header
    async fn sign_block_header(&self, header_bytes: &[u8]) -> Result<Vec<u8>>;
}

/// Port: Publish events to Event Bus
#[async_trait]
pub trait EventPublisher: Send + Sync {
    /// Publish generic event
    async fn publish_event(&self, topic: &str, payload: Vec<u8>) -> Result<()>;
}

/// Port: Read chain state from Block Storage (qc-02)
///
/// V2.4: Used on startup to query chain tip and recent blocks for
/// proper difficulty adjustment continuity across restarts.
#[async_trait]
pub trait BlockStorageReader: Send + Sync {
    /// Get current chain info including recent blocks for DGW difficulty
    ///
    /// # Arguments
    /// * `recent_blocks_count` - Number of recent blocks to include (typically 24 for DGW)
    ///
    /// # Returns
    /// Chain tip information and recent block history for difficulty seeding
    async fn get_chain_info(&self, recent_blocks_count: u32) -> Result<ChainInfo>;
}

/// Chain state information from Block Storage
///
/// Contains everything Block Production needs to resume mining
/// with correct difficulty after a restart.
#[derive(Clone, Debug, Default)]
pub struct ChainInfo {
    /// Latest block height (0 if chain is empty)
    pub chain_tip_height: u64,

    /// Latest block hash ([0; 32] if empty)
    pub chain_tip_hash: H256,

    /// Latest block timestamp (0 if empty)
    pub chain_tip_timestamp: u64,

    /// Recent blocks for DGW difficulty calculation
    /// Ordered from newest to oldest
    /// Uses HistoricalBlockInfo from inbound ports
    pub recent_blocks: Vec<super::inbound::HistoricalBlockInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::inbound::HistoricalBlockInfo;

    #[test]
    fn test_chain_info_default() {
        let info = ChainInfo::default();
        assert_eq!(info.chain_tip_height, 0);
        assert!(info.recent_blocks.is_empty());
    }

    #[test]
    fn test_chain_info_with_blocks() {
        let mut info = ChainInfo::default();
        info.recent_blocks.push(HistoricalBlockInfo {
            height: 100,
            timestamp: 1700000000,
            difficulty: U256::from(2).pow(U256::from(235)),
            hash: H256::zero(),
        });
        
        assert_eq!(info.recent_blocks.len(), 1);
        assert_eq!(info.recent_blocks[0].height, 100);
    }
}

