//! # Outbound Ports
//!
//! Traits for external dependencies (chain clients, contracts).
//!
//! Reference: SPEC-15 Section 3.2 (Lines 253-291)

use crate::domain::{Address, ChainId, CrossChainError, CrossChainProof, Hash, Secret};
use async_trait::async_trait;

/// External chain client - outbound port.
///
/// Reference: SPEC-15 Lines 273-283
#[async_trait]
pub trait ExternalChainClient: Send + Sync {
    /// Get block header.
    async fn get_header(&self, chain: ChainId, height: u64)
        -> Result<BlockHeader, CrossChainError>;

    /// Verify a cross-chain proof.
    async fn verify_proof(
        &self,
        chain: ChainId,
        proof: &CrossChainProof,
    ) -> Result<bool, CrossChainError>;

    /// Check if block is finalized.
    async fn is_finalized(&self, chain: ChainId, block_hash: Hash)
        -> Result<bool, CrossChainError>;

    /// Get current block height.
    async fn get_height(&self, chain: ChainId) -> Result<u64, CrossChainError>;
}

/// Block header from external chain.
#[derive(Clone, Debug)]
pub struct BlockHeader {
    /// Block hash.
    pub hash: Hash,
    /// Block height.
    pub height: u64,
    /// Parent hash.
    pub parent_hash: Hash,
    /// Timestamp.
    pub timestamp: u64,
}

/// HTLC contract interface - outbound port.
///
/// Reference: SPEC-15 Lines 257-270
#[async_trait]
pub trait HTLCContract: Send + Sync {
    /// Deploy a new HTLC.
    async fn deploy(
        &self,
        chain: ChainId,
        hash_lock: Hash,
        time_lock: u64,
        amount: u64,
        sender: Address,
        recipient: Address,
    ) -> Result<Hash, CrossChainError>;

    /// Claim with secret.
    async fn claim(
        &self,
        chain: ChainId,
        htlc_id: Hash,
        secret: Secret,
    ) -> Result<(), CrossChainError>;

    /// Refund expired HTLC.
    async fn refund(&self, chain: ChainId, htlc_id: Hash) -> Result<(), CrossChainError>;

    /// Get HTLC proof.
    async fn get_proof(
        &self,
        chain: ChainId,
        htlc_id: Hash,
    ) -> Result<CrossChainProof, CrossChainError>;
}

/// Finality checker - outbound port.
///
/// Reference: SPEC-15 Lines 286-290
#[async_trait]
pub trait FinalityChecker: Send + Sync {
    /// Get required confirmations for chain.
    fn required_confirmations(&self, chain: ChainId) -> u64;

    /// Check if proof has sufficient confirmations.
    async fn is_proof_final(&self, proof: &CrossChainProof) -> Result<bool, CrossChainError>;
}

// =============================================================================
// Mock Implementations for Testing
// =============================================================================

/// Mock chain client for testing.
#[derive(Clone, Default)]
pub struct MockChainClient {
    /// Current height per chain.
    pub heights: std::collections::HashMap<ChainId, u64>,
    /// Should fail?
    pub should_fail: bool,
}

#[async_trait]
impl ExternalChainClient for MockChainClient {
    async fn get_header(
        &self,
        _chain: ChainId,
        height: u64,
    ) -> Result<BlockHeader, CrossChainError> {
        if self.should_fail {
            return Err(CrossChainError::NetworkError("Mock failure".to_string()));
        }

        Ok(BlockHeader {
            hash: [height as u8; 32],
            height,
            parent_hash: [(height - 1) as u8; 32],
            timestamp: 1000 + height * 10,
        })
    }

    async fn verify_proof(
        &self,
        _chain: ChainId,
        _proof: &CrossChainProof,
    ) -> Result<bool, CrossChainError> {
        Ok(!self.should_fail)
    }

    async fn is_finalized(
        &self,
        _chain: ChainId,
        _block_hash: Hash,
    ) -> Result<bool, CrossChainError> {
        Ok(!self.should_fail)
    }

    async fn get_height(&self, chain: ChainId) -> Result<u64, CrossChainError> {
        Ok(*self.heights.get(&chain).unwrap_or(&100))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_chain_client_get_header() {
        let client = MockChainClient::default();
        let header = client.get_header(ChainId::Ethereum, 100).await.unwrap();
        assert_eq!(header.height, 100);
    }

    #[tokio::test]
    async fn test_mock_chain_client_verify_proof() {
        let client = MockChainClient::default();
        let proof = CrossChainProof {
            chain: ChainId::Ethereum,
            block_hash: [1u8; 32],
            block_height: 100,
            tx_hash: [2u8; 32],
            merkle_proof: vec![],
            confirmations: 12,
        };
        assert!(client
            .verify_proof(ChainId::Ethereum, &proof)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_mock_chain_client_failure() {
        let client = MockChainClient {
            should_fail: true,
            ..Default::default()
        };
        assert!(client.get_header(ChainId::Ethereum, 100).await.is_err());
    }
}
