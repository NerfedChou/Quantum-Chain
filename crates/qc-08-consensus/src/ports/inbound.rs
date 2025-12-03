//! Driving ports (Inbound API)
//!
//! Reference: SPEC-08-CONSENSUS.md Section 3.1

use crate::domain::{Block, ChainHead, ConsensusError, ValidatedBlock};
use async_trait::async_trait;
use shared_types::Hash;

/// Primary Consensus API
///
/// Reference: SPEC-08 Section 3.1
#[async_trait]
pub trait ConsensusApi: Send + Sync {
    /// Validate a block received from the network
    ///
    /// # Security
    /// - Zero-Trust: Re-verifies ALL signatures independently
    /// - Checks 2/3 attestation threshold for PoS
    /// - Checks 2f+1 votes for PBFT
    async fn validate_block(
        &self,
        block: Block,
        source_peer: Option<[u8; 32]>,
    ) -> Result<ValidatedBlock, ConsensusError>;

    /// Build a new block (for validators)
    ///
    /// Gets transactions from mempool and creates a block proposal
    async fn build_block(&self) -> Result<Block, ConsensusError>;

    /// Get current chain head
    async fn get_chain_head(&self) -> ChainHead;

    /// Check if a block hash has been validated
    async fn is_validated(&self, block_hash: Hash) -> bool;

    /// Get the current epoch
    async fn current_epoch(&self) -> u64;
}
