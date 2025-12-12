//! # Light Client Service
//!
//! Application service orchestrating SPV verification.
//!
//! Reference: Architecture.md Section 2.1

use async_trait::async_trait;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Arc;

use crate::algorithms::{check_consensus, verify_merkle_proof};
use crate::config::LightClientConfig;
use crate::domain::{
    BlockHeader, ChainTip, Hash, HeaderChain, LightClientError, MerkleProof, ProofNode,
    ProvenTransaction, SyncResult,
};
use crate::ports::{Address, FullNodeConnection, LightClientApi};

/// Light Client Service - orchestrates SPV verification.
pub struct LightClientService<N: FullNodeConnection> {
    /// Configuration.
    config: LightClientConfig,
    /// Header chain.
    header_chain: HeaderChain,
    /// Connected full nodes.
    nodes: Vec<Arc<N>>,
    /// Proof cache.
    proof_cache: LruCache<Hash, MerkleProof>,
    /// Is synced?
    synced: bool,
    /// Network chain height (from nodes).
    network_height: u64,
}

impl<N: FullNodeConnection> LightClientService<N> {
    /// Create a new light client service.
    pub fn new(config: LightClientConfig, genesis: BlockHeader) -> Self {
        let cache_size =
            NonZeroUsize::new(config.proof_cache_size).unwrap_or(NonZeroUsize::new(100).unwrap());
        Self {
            config,
            header_chain: HeaderChain::new(genesis),
            nodes: Vec::new(),
            proof_cache: LruCache::new(cache_size),
            synced: false,
            network_height: 0,
        }
    }

    /// Add a full node connection.
    pub fn add_node(&mut self, node: Arc<N>) {
        self.nodes.push(node);
    }

    /// Set nodes.
    pub fn set_nodes(&mut self, nodes: Vec<Arc<N>>) {
        self.nodes = nodes;
    }

    /// Get number of connected nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Internal: ensure we have enough nodes.
    fn check_node_count(&self) -> Result<(), LightClientError> {
        if self.nodes.len() < self.config.min_full_nodes {
            return Err(LightClientError::InsufficientNodes {
                got: self.nodes.len(),
                required: self.config.min_full_nodes,
            });
        }
        Ok(())
    }

    /// Internal: get headers from nodes with consensus.
    async fn fetch_headers_with_consensus(
        &self,
        from_height: u64,
        count: usize,
    ) -> Result<Vec<BlockHeader>, LightClientError> {
        self.check_node_count()?;

        // Query all nodes
        let mut responses = Vec::new();
        for node in &self.nodes {
            match node.get_headers(from_height, count).await {
                Ok(headers) => responses.push(headers),
                Err(e) => {
                    tracing::warn!("Node {} failed: {}", node.node_id(), e);
                }
            }
        }

        // Check consensus
        check_consensus(&responses, self.config.min_full_nodes)
    }

    /// Internal: get Merkle proof with multi-node verification.
    async fn fetch_proof_with_consensus(
        &self,
        tx_hash: Hash,
        block_hash: Hash,
    ) -> Result<MerkleProof, LightClientError> {
        self.check_node_count()?;

        let mut responses = Vec::new();
        for node in &self.nodes {
            match node.get_merkle_proof(tx_hash, block_hash).await {
                Ok(proof) => responses.push(proof),
                Err(e) => {
                    tracing::warn!("Node {} failed to get proof: {}", node.node_id(), e);
                }
            }
        }

        check_consensus(&responses, self.config.min_full_nodes)
    }

    /// Internal: get network height from nodes.
    async fn fetch_network_height(&mut self) -> Result<u64, LightClientError> {
        self.check_node_count()?;

        let mut heights = Vec::new();
        for node in &self.nodes {
            match node.get_chain_tip().await {
                Ok((_, height)) => heights.push(height),
                Err(e) => {
                    tracing::warn!("Node {} failed to get tip: {}", node.node_id(), e);
                }
            }
        }

        let height = check_consensus(&heights, self.config.min_full_nodes)?;
        self.network_height = height;
        Ok(height)
    }
}

#[async_trait]
impl<N: FullNodeConnection + 'static> LightClientApi for LightClientService<N> {
    async fn sync_headers(&mut self) -> Result<SyncResult, LightClientError> {
        let start = std::time::Instant::now();

        // Get network height
        let network_height = self.fetch_network_height().await?;
        let local_height = self.header_chain.height();

        if local_height >= network_height {
            self.synced = true;
            return Ok(SyncResult::success(
                0,
                self.get_chain_tip(),
                start.elapsed().as_millis() as u64,
            ));
        }

        // Sync headers in batches
        let mut synced_count = 0u64;
        let mut current_height = local_height + 1;

        while current_height <= network_height {
            let batch_size = self
                .config
                .header_batch_size
                .min((network_height - current_height + 1) as usize);

            match self
                .fetch_headers_with_consensus(current_height, batch_size)
                .await
            {
                Ok(headers) => {
                    for header in headers {
                        self.header_chain.append(header)?;
                        synced_count += 1;
                        current_height += 1;
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to sync headers at {}: {}", current_height, e);
                    return Ok(SyncResult::failed(
                        self.get_chain_tip(),
                        start.elapsed().as_millis() as u64,
                    ));
                }
            }
        }

        // Verify checkpoints
        self.header_chain.verify_checkpoints()?;

        self.synced = true;
        Ok(SyncResult::success(
            synced_count,
            self.get_chain_tip(),
            start.elapsed().as_millis() as u64,
        ))
    }

    async fn get_proven_transaction(
        &self,
        tx_hash: Hash,
    ) -> Result<ProvenTransaction, LightClientError> {
        // Check cache
        if let Some(cached_proof) = self.proof_cache.peek(&tx_hash) {
            let confirmations = self
                .header_chain
                .height()
                .saturating_sub(cached_proof.block_height);
            let mut ptx = ProvenTransaction::new(
                tx_hash,
                cached_proof.block_hash,
                cached_proof.block_height,
                cached_proof.clone(),
            );
            ptx.mark_verified(confirmations);
            return Ok(ptx);
        }

        // Need to fetch from network - but we need block hash
        // This would require querying nodes for tx location first
        Err(LightClientError::TransactionNotFound(tx_hash))
    }

    async fn verify_transaction(
        &self,
        tx_hash: Hash,
        block_hash: Hash,
    ) -> Result<bool, LightClientError> {
        // Get header
        let header = self
            .header_chain
            .get_header(&block_hash)
            .ok_or(LightClientError::HeaderNotFound(block_hash))?;

        // Get proof with multi-node consensus
        let proof = self.fetch_proof_with_consensus(tx_hash, block_hash).await?;

        // Verify Merkle proof
        let proof_nodes: Vec<ProofNode> = proof.path.clone();
        let verified = verify_merkle_proof(&tx_hash, &proof_nodes, &header.merkle_root);

        if !verified {
            return Err(LightClientError::InvalidProof);
        }

        // Check confirmations
        let confirmations = self.header_chain.height().saturating_sub(header.height);
        if confirmations < self.config.required_confirmations {
            return Err(LightClientError::InsufficientConfirmations {
                got: confirmations,
                required: self.config.required_confirmations,
            });
        }

        Ok(true)
    }

    async fn get_filtered_transactions(
        &self,
        _addresses: &[Address],
        _from_height: u64,
        _to_height: u64,
    ) -> Result<Vec<ProvenTransaction>, LightClientError> {
        // Would use Bloom filter provider
        // For now, return empty
        Ok(vec![])
    }

    fn get_chain_tip(&self) -> ChainTip {
        self.header_chain.get_tip()
    }

    fn is_synced(&self) -> bool {
        self.synced
    }

    fn sync_progress(&self) -> f64 {
        if self.network_height == 0 {
            return 0.0;
        }
        (self.header_chain.height() as f64) / (self.network_height as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::MockFullNode;

    fn create_test_service() -> LightClientService<MockFullNode> {
        let config = LightClientConfig::for_testing();
        let genesis = BlockHeader::genesis([0u8; 32], 1000, [1u8; 32]);
        LightClientService::new(config, genesis)
    }

    #[test]
    fn test_service_new() {
        let service = create_test_service();
        assert_eq!(service.node_count(), 0);
        assert!(!service.is_synced());
    }

    #[test]
    fn test_service_add_node() {
        let mut service = create_test_service();
        let node = Arc::new(MockFullNode::default());
        service.add_node(node);
        assert_eq!(service.node_count(), 1);
    }

    #[test]
    fn test_service_chain_tip() {
        let service = create_test_service();
        let tip = service.get_chain_tip();
        assert_eq!(tip.height, 0);
    }

    #[test]
    fn test_service_sync_progress_initial() {
        let service = create_test_service();
        assert_eq!(service.sync_progress(), 0.0);
    }

    #[tokio::test]
    async fn test_service_insufficient_nodes() {
        let mut service = create_test_service();
        // No nodes added
        let result = service.sync_headers().await;
        assert!(matches!(
            result,
            Err(LightClientError::InsufficientNodes { .. })
        ));
    }
}
