//! Full Node Connection Adapter
//!
//! Implements `FullNodeConnection` port for connecting to full nodes.
//! Reference: SPEC-13 Section 3.2

use crate::domain::{BlockHeader, Hash, LightClientError, MerkleProof, ProofNode};
use crate::ports::outbound::FullNodeConnection;
use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, warn};

/// HTTP-based full node connection.
///
/// Connects to full nodes via JSON-RPC over HTTP.
pub struct HttpFullNodeConnection {
    /// Node URL (e.g., "http://localhost:8545").
    url: String,
    /// Node identifier.
    node_id: String,
    /// Health status.
    healthy: Arc<AtomicBool>,
    /// Request timeout in milliseconds.
    timeout_ms: u64,
}

impl HttpFullNodeConnection {
    /// Create a new connection to a full node.
    pub fn new(url: String, node_id: String) -> Self {
        Self {
            url,
            node_id,
            healthy: Arc::new(AtomicBool::new(true)),
            timeout_ms: 5000,
        }
    }

    /// Create with custom timeout.
    pub fn with_timeout(url: String, node_id: String, timeout_ms: u64) -> Self {
        Self {
            url,
            node_id,
            healthy: Arc::new(AtomicBool::new(true)),
            timeout_ms,
        }
    }

    /// Mark node as unhealthy.
    pub fn mark_unhealthy(&self) {
        self.healthy.store(false, Ordering::SeqCst);
        warn!("[qc-13] Marked node {} as unhealthy", self.node_id);
    }

    /// Mark node as healthy.
    pub fn mark_healthy(&self) {
        self.healthy.store(true, Ordering::SeqCst);
    }
}

#[async_trait]
impl FullNodeConnection for HttpFullNodeConnection {
    async fn get_headers(
        &self,
        from_height: u64,
        count: usize,
    ) -> Result<Vec<BlockHeader>, LightClientError> {
        debug!(
            "[qc-13] Getting {} headers from height {} via {}",
            count, from_height, self.node_id
        );

        if !self.healthy.load(Ordering::SeqCst) {
            return Err(LightClientError::NodeUnhealthy(self.node_id.clone()));
        }

        // TODO: Implement actual HTTP/JSON-RPC call
        // For now, return synthetic headers for testing

        let headers: Vec<BlockHeader> = (0..count)
            .map(|i| {
                let height = from_height + i as u64;
                let mut parent_hash = [0u8; 32];
                if height > 0 {
                    parent_hash[0] = ((height - 1) % 256) as u8;
                }

                BlockHeader {
                    height,
                    hash: {
                        let mut h = [0u8; 32];
                        h[0] = (height % 256) as u8;
                        h
                    },
                    parent_hash,
                    timestamp: 1700000000 + height * 12,
                    merkle_root: [0u8; 32],
                    state_root: [0u8; 32],
                    difficulty: 1000 + height,
                }
            })
            .collect();

        Ok(headers)
    }

    async fn get_merkle_proof(
        &self,
        tx_hash: Hash,
        block_hash: Hash,
    ) -> Result<MerkleProof, LightClientError> {
        debug!(
            "[qc-13] Getting Merkle proof for tx {:02x}{:02x}... in block {:02x}{:02x}...",
            tx_hash[0], tx_hash[1], block_hash[0], block_hash[1]
        );

        if !self.healthy.load(Ordering::SeqCst) {
            return Err(LightClientError::NodeUnhealthy(self.node_id.clone()));
        }

        // TODO: Implement actual HTTP/JSON-RPC call
        // Return a minimal valid proof for testing

        Ok(MerkleProof {
            tx_hash,
            block_hash,
            proof_nodes: vec![
                ProofNode {
                    hash: [1u8; 32],
                    position: crate::domain::Position::Left,
                },
                ProofNode {
                    hash: [2u8; 32],
                    position: crate::domain::Position::Right,
                },
            ],
            tx_index: 0,
        })
    }

    async fn get_chain_tip(&self) -> Result<(Hash, u64), LightClientError> {
        debug!("[qc-13] Getting chain tip from {}", self.node_id);

        if !self.healthy.load(Ordering::SeqCst) {
            return Err(LightClientError::NodeUnhealthy(self.node_id.clone()));
        }

        // TODO: Implement actual HTTP/JSON-RPC call
        Ok(([255u8; 32], 1000))
    }

    async fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::SeqCst)
    }

    fn node_id(&self) -> &str {
        &self.node_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_headers() {
        let conn = HttpFullNodeConnection::new(
            "http://localhost:8545".to_string(),
            "test-node".to_string(),
        );

        let headers = conn.get_headers(0, 5).await.unwrap();
        assert_eq!(headers.len(), 5);
        assert_eq!(headers[0].height, 0);
        assert_eq!(headers[4].height, 4);
    }

    #[tokio::test]
    async fn test_unhealthy_node_fails() {
        let conn = HttpFullNodeConnection::new(
            "http://localhost:8545".to_string(),
            "test-node".to_string(),
        );

        conn.mark_unhealthy();
        let result = conn.get_headers(0, 5).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_merkle_proof() {
        let conn = HttpFullNodeConnection::new(
            "http://localhost:8545".to_string(),
            "test-node".to_string(),
        );

        let proof = conn.get_merkle_proof([1u8; 32], [2u8; 32]).await.unwrap();
        assert_eq!(proof.tx_hash, [1u8; 32]);
        assert!(!proof.proof_nodes.is_empty());
    }
}
