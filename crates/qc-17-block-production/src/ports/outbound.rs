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
