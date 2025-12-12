//! # Light Client Adapter
//!
//! Connects qc-13 Light Client Sync to the event bus for IPC.
//!
//! ## IPC Interactions (IPC-MATRIX.md)
//!
//! | From | To | Message |
//! |------|-----|---------|
//! | 13 → 1 | Peer Discovery | Request full nodes |
//! | 13 → 3 | Transaction Indexing | Request Merkle proofs |
//! | 13 → 7 | Bloom Filters | Build/update filters |

#[cfg(feature = "qc-13")]
use qc_13_light_client_sync::{
    BlockHeader, ChainTip, Hash, LightClientApi, LightClientConfig, LightClientError,
    LightClientService, MockFullNode, ProvenTransaction, SyncResult,
};

#[cfg(feature = "qc-13")]
use std::sync::Arc;

#[cfg(feature = "qc-13")]
use tokio::sync::RwLock;

/// Light Client event bus adapter.
///
/// Wraps the LightClientService for event-driven integration.
#[cfg(feature = "qc-13")]
pub struct LightClientAdapter {
    /// Inner service (uses MockFullNode for now, real nodes via peer discovery later)
    service: Arc<RwLock<LightClientService<MockFullNode>>>,
    /// Subsystem ID
    subsystem_id: u8,
}

#[cfg(feature = "qc-13")]
impl LightClientAdapter {
    /// Create a new light client adapter.
    pub fn new(config: LightClientConfig, genesis: BlockHeader) -> Self {
        let service = LightClientService::new(config, genesis);
        Self {
            service: Arc::new(RwLock::new(service)),
            subsystem_id: 13,
        }
    }

    /// Create with default config.
    pub fn with_defaults() -> Self {
        let genesis = BlockHeader::genesis([0u8; 32], 1000, [0u8; 32]);
        Self::new(LightClientConfig::default(), genesis)
    }

    /// Get subsystem ID.
    pub fn subsystem_id(&self) -> u8 {
        self.subsystem_id
    }

    /// Add a mock full node (for testing).
    pub async fn add_mock_node(&self, node: MockFullNode) {
        let mut service = self.service.write().await;
        service.add_node(Arc::new(node));
    }

    /// Sync headers from network.
    pub async fn sync_headers(&self) -> Result<SyncResult, LightClientError> {
        let mut service = self.service.write().await;
        service.sync_headers().await
    }

    /// Verify a transaction.
    pub async fn verify_transaction(
        &self,
        tx_hash: Hash,
        block_hash: Hash,
    ) -> Result<bool, LightClientError> {
        let service = self.service.read().await;
        service.verify_transaction(tx_hash, block_hash).await
    }

    /// Get chain tip.
    pub async fn get_chain_tip(&self) -> ChainTip {
        let service = self.service.read().await;
        service.get_chain_tip()
    }

    /// Check if synced.
    pub async fn is_synced(&self) -> bool {
        let service = self.service.read().await;
        service.is_synced()
    }

    /// Get sync progress.
    pub async fn sync_progress(&self) -> f64 {
        let service = self.service.read().await;
        service.sync_progress()
    }
}

#[cfg(feature = "qc-13")]
impl Default for LightClientAdapter {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(all(test, feature = "qc-13"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adapter_creation() {
        let adapter = LightClientAdapter::with_defaults();
        assert_eq!(adapter.subsystem_id(), 13);
    }

    #[tokio::test]
    async fn test_adapter_chain_tip() {
        let adapter = LightClientAdapter::with_defaults();
        let tip = adapter.get_chain_tip().await;
        assert_eq!(tip.height, 0);
    }

    #[tokio::test]
    async fn test_adapter_not_synced_initially() {
        let adapter = LightClientAdapter::with_defaults();
        assert!(!adapter.is_synced().await);
    }

    #[tokio::test]
    async fn test_adapter_sync_progress_zero() {
        let adapter = LightClientAdapter::with_defaults();
        assert_eq!(adapter.sync_progress().await, 0.0);
    }
}
