//! # Outbound Ports
//!
//! Traits for external dependencies (full nodes, peer discovery, etc.).
//!
//! Reference: SPEC-13 Section 3.2 (Lines 219-270)

use super::inbound::Address;
use crate::domain::{BlockHeader, Hash, LightClientError, MerkleProof};
use async_trait::async_trait;

/// Full node connection - outbound port.
///
/// Reference: SPEC-13 Lines 223-245
#[async_trait]
pub trait FullNodeConnection: Send + Sync {
    /// Get block headers starting from a height.
    async fn get_headers(
        &self,
        from_height: u64,
        count: usize,
    ) -> Result<Vec<BlockHeader>, LightClientError>;

    /// Get a Merkle proof for a transaction.
    async fn get_merkle_proof(
        &self,
        tx_hash: Hash,
        block_hash: Hash,
    ) -> Result<MerkleProof, LightClientError>;

    /// Get the current chain tip from this node.
    async fn get_chain_tip(&self) -> Result<(Hash, u64), LightClientError>;

    /// Check node health/connectivity.
    async fn is_healthy(&self) -> bool;

    /// Get node identifier (for logging/debugging).
    fn node_id(&self) -> &str;
}

/// Peer discovery - outbound port.
///
/// Reference: SPEC-13 Lines 248-252
#[async_trait]
pub trait PeerDiscovery: Send + Sync {
    /// Get a list of full nodes from diverse sources.
    ///
    /// Reference: System.md Line 648 - "Random peer selection from diverse sources"
    async fn get_full_nodes(
        &self,
        count: usize,
    ) -> Result<Vec<Box<dyn FullNodeConnection>>, LightClientError>;

    /// Rotate to new peers (for privacy).
    ///
    /// Reference: SPEC-13 Line 629
    async fn rotate_peers(&mut self) -> Result<(), LightClientError>;
}

/// Merkle proof provider - outbound port.
///
/// Reference: SPEC-13 Lines 255-262
#[async_trait]
pub trait MerkleProofProvider: Send + Sync {
    /// Get Merkle proof from multiple nodes with consensus.
    ///
    /// Reference: System.md Line 644 - "Query 3+ independent nodes"
    async fn get_verified_proof(
        &self,
        tx_hash: Hash,
        block_hash: Hash,
        min_nodes: usize,
    ) -> Result<MerkleProof, LightClientError>;
}

/// Bloom filter provider - outbound port.
///
/// Reference: SPEC-13 Lines 265-269
#[async_trait]
pub trait BloomFilterProvider: Send + Sync {
    /// Submit a Bloom filter to full nodes for filtered sync.
    ///
    /// Reference: System.md Line 629
    async fn submit_filter(
        &self,
        addresses: &[Address],
        false_positive_rate: f64,
    ) -> Result<(), LightClientError>;

    /// Get filtered transactions matching the submitted filter.
    async fn get_filtered_txs(
        &self,
        from_height: u64,
        to_height: u64,
    ) -> Result<Vec<Hash>, LightClientError>;
}

// =============================================================================
// Mock Implementations for Testing
// =============================================================================

/// Mock full node for testing.
#[derive(Clone)]
pub struct MockFullNode {
    /// Node identifier.
    pub id: String,
    /// Simulated headers.
    pub headers: Vec<BlockHeader>,
    /// Simulated chain tip.
    pub tip_height: u64,
    /// Should return errors?
    pub should_fail: bool,
}

impl Default for MockFullNode {
    fn default() -> Self {
        Self {
            id: "mock-node-1".to_string(),
            headers: Vec::new(),
            tip_height: 0,
            should_fail: false,
        }
    }
}

#[async_trait]
impl FullNodeConnection for MockFullNode {
    async fn get_headers(
        &self,
        from_height: u64,
        count: usize,
    ) -> Result<Vec<BlockHeader>, LightClientError> {
        if self.should_fail {
            return Err(LightClientError::NetworkError("Mock failure".to_string()));
        }

        let start = from_height as usize;
        let end = (start + count).min(self.headers.len());

        if start >= self.headers.len() {
            return Ok(vec![]);
        }

        Ok(self.headers[start..end].to_vec())
    }

    async fn get_merkle_proof(
        &self,
        tx_hash: Hash,
        block_hash: Hash,
    ) -> Result<MerkleProof, LightClientError> {
        if self.should_fail {
            return Err(LightClientError::NetworkError("Mock failure".to_string()));
        }

        // Return a mock proof
        Ok(MerkleProof {
            tx_hash,
            path: vec![],
            merkle_root: [0u8; 32],
            block_hash,
            block_height: 0,
        })
    }

    async fn get_chain_tip(&self) -> Result<(Hash, u64), LightClientError> {
        if self.should_fail {
            return Err(LightClientError::NetworkError("Mock failure".to_string()));
        }

        Ok(([0u8; 32], self.tip_height))
    }

    async fn is_healthy(&self) -> bool {
        !self.should_fail
    }

    fn node_id(&self) -> &str {
        &self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_full_node_healthy() {
        let node = MockFullNode::default();
        assert!(node.is_healthy().await);
    }

    #[tokio::test]
    async fn test_mock_full_node_failed() {
        let node = MockFullNode {
            should_fail: true,
            ..Default::default()
        };
        assert!(!node.is_healthy().await);
    }

    #[tokio::test]
    async fn test_mock_full_node_get_headers() {
        let mut node = MockFullNode::default();
        node.headers = vec![BlockHeader::genesis([0u8; 32], 1000, [1u8; 32])];

        let headers = node.get_headers(0, 10).await.unwrap();
        assert_eq!(headers.len(), 1);
    }
}
