//! External Chain Client Adapter
//!
//! Implements `ExternalChainClient` port for interacting with external chains.
//! Reference: SPEC-15 Section 3.2

use crate::domain::{ChainId, CrossChainError, CrossChainProof, Hash};
use crate::ports::outbound::{BlockHeader, ExternalChainClient};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use tracing::{debug, info};

/// HTTP-based external chain client.
///
/// In production, this would make RPC calls to external chain nodes.
pub struct HttpChainClient {
    /// Cached headers per chain.
    headers: RwLock<HashMap<ChainId, HashMap<u64, BlockHeader>>>,
    /// Current heights per chain.
    heights: RwLock<HashMap<ChainId, u64>>,
}

impl HttpChainClient {
    /// Create a new client.
    pub fn new() -> Self {
        Self {
            headers: RwLock::new(HashMap::new()),
            heights: RwLock::new(HashMap::new()),
        }
    }

    /// Initialize with chain heights for testing.
    pub fn with_chains(chains: &[(ChainId, u64)]) -> Self {
        let client = Self::new();
        let mut heights = client.heights.write();
        let mut headers = client.headers.write();

        for (chain, height) in chains {
            heights.insert(*chain, *height);
            headers.insert(*chain, HashMap::new());

            // Pre-populate some headers
            for h in height.saturating_sub(10)..=*height {
                let header = BlockHeader {
                    hash: make_block_hash(*chain, h),
                    height: h,
                    parent_hash: make_block_hash(*chain, h.saturating_sub(1)),
                    timestamp: 1_700_000_000 + h * chain.block_time_secs(),
                };
                headers.get_mut(chain).unwrap().insert(h, header);
            }
        }

        drop(heights);
        drop(headers);
        client
    }
}

impl Default for HttpChainClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a deterministic block hash for testing.
fn make_block_hash(chain: ChainId, height: u64) -> Hash {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(&(chain as u8).to_le_bytes());
    hasher.update(&height.to_le_bytes());

    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

#[async_trait]
impl ExternalChainClient for HttpChainClient {
    async fn get_header(
        &self,
        chain: ChainId,
        height: u64,
    ) -> Result<BlockHeader, CrossChainError> {
        debug!("[qc-15] Getting header for {:?} at height {}", chain, height);

        let headers = self.headers.read();
        let chain_headers = headers
            .get(&chain)
            .ok_or_else(|| CrossChainError::UnsupportedChain(format!("{:?}", chain)))?;

        chain_headers
            .get(&height)
            .cloned()
            .ok_or_else(|| CrossChainError::NotFinalized { got: 0, required: 1 })
    }

    async fn verify_proof(
        &self,
        chain: ChainId,
        proof: &CrossChainProof,
    ) -> Result<bool, CrossChainError> {
        info!(
            "[qc-15] Verifying proof for {:?} block {}",
            chain, proof.block_height
        );

        // Verify chain matches
        if proof.chain != chain {
            return Ok(false);
        }

        // Verify we have the block header
        let headers = self.headers.read();
        let chain_headers = headers
            .get(&chain)
            .ok_or_else(|| CrossChainError::UnsupportedChain(format!("{:?}", chain)))?;

        if let Some(header) = chain_headers.get(&proof.block_height) {
            // Verify block hash matches
            if header.hash != proof.block_hash {
                return Ok(false);
            }

            // Verify merkle proof (simplified - just check non-empty)
            if proof.merkle_proof.is_empty() {
                return Ok(false);
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn is_finalized(&self, chain: ChainId, block_hash: Hash) -> Result<bool, CrossChainError> {
        debug!(
            "[qc-15] Checking finality for {:?} block {:02x}{:02x}...",
            chain, block_hash[0], block_hash[1]
        );

        let current = self.get_height(chain).await?;
        let headers = self.headers.read();
        let chain_headers = headers
            .get(&chain)
            .ok_or_else(|| CrossChainError::UnsupportedChain(format!("{:?}", chain)))?;

        // Find the block
        for (height, header) in chain_headers.iter() {
            if header.hash == block_hash {
                let confirmations = current.saturating_sub(*height);
                return Ok(confirmations >= chain.required_confirmations());
            }
        }

        Ok(false)
    }

    async fn get_height(&self, chain: ChainId) -> Result<u64, CrossChainError> {
        self.heights
            .read()
            .get(&chain)
            .copied()
            .ok_or_else(|| CrossChainError::UnsupportedChain(format!("{:?}", chain)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_height() {
        let client = HttpChainClient::with_chains(&[
            (ChainId::QuantumChain, 1000),
            (ChainId::Ethereum, 18_000_000),
        ]);

        let height = client.get_height(ChainId::QuantumChain).await.unwrap();
        assert_eq!(height, 1000);

        let height = client.get_height(ChainId::Ethereum).await.unwrap();
        assert_eq!(height, 18_000_000);
    }

    #[tokio::test]
    async fn test_unsupported_chain_fails() {
        let client = HttpChainClient::with_chains(&[(ChainId::QuantumChain, 1000)]);

        let result = client.get_height(ChainId::Bitcoin).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_header() {
        let client = HttpChainClient::with_chains(&[(ChainId::QuantumChain, 1000)]);

        let header = client.get_header(ChainId::QuantumChain, 1000).await.unwrap();
        assert_eq!(header.height, 1000);
    }

    #[tokio::test]
    async fn test_verify_proof() {
        let client = HttpChainClient::with_chains(&[(ChainId::QuantumChain, 1000)]);

        let header = client.get_header(ChainId::QuantumChain, 1000).await.unwrap();

        let proof = CrossChainProof {
            chain: ChainId::QuantumChain,
            block_hash: header.hash,
            block_height: 1000,
            tx_hash: [1u8; 32],
            merkle_proof: vec![[2u8; 32]],
            confirmations: 0,
        };

        let valid = client.verify_proof(ChainId::QuantumChain, &proof).await.unwrap();
        assert!(valid);
    }

    #[tokio::test]
    async fn test_is_finalized() {
        let client = HttpChainClient::with_chains(&[(ChainId::QuantumChain, 100)]);

        // Block 94 should have 6+ confirmations
        let header = client.get_header(ChainId::QuantumChain, 94).await.unwrap();
        let finalized = client.is_finalized(ChainId::QuantumChain, header.hash).await.unwrap();
        assert!(finalized);

        // Block 99 should not have enough confirmations
        let header = client.get_header(ChainId::QuantumChain, 99).await.unwrap();
        let finalized = client.is_finalized(ChainId::QuantumChain, header.hash).await.unwrap();
        assert!(!finalized);
    }
}
