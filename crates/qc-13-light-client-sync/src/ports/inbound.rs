//! # Inbound Ports
//!
//! API trait defining what the Light Client can do.
//!
//! Reference: SPEC-13 Section 3.1 (Lines 166-217)

use async_trait::async_trait;
use crate::domain::{
    Hash, ChainTip, SyncResult, ProvenTransaction, LightClientError,
};

/// Address type alias
pub type Address = [u8; 20];

/// Light Client API - inbound port.
///
/// Reference: SPEC-13 Lines 166-217
#[async_trait]
pub trait LightClientApi: Send + Sync {
    /// Sync block headers from the network.
    ///
    /// Reference: System.md Line 627
    async fn sync_headers(&mut self) -> Result<SyncResult, LightClientError>;

    /// Get a proven transaction with Merkle proof.
    ///
    /// Reference: System.md Line 628
    async fn get_proven_transaction(
        &self,
        tx_hash: Hash,
    ) -> Result<ProvenTransaction, LightClientError>;

    /// Verify a transaction exists in a specific block.
    ///
    /// Reference: SPEC-13 Lines 130-138
    async fn verify_transaction(
        &self,
        tx_hash: Hash,
        block_hash: Hash,
    ) -> Result<bool, LightClientError>;

    /// Get filtered transactions for watched addresses.
    ///
    /// Reference: System.md Line 629
    async fn get_filtered_transactions(
        &self,
        addresses: &[Address],
        from_height: u64,
        to_height: u64,
    ) -> Result<Vec<ProvenTransaction>, LightClientError>;

    /// Get current chain tip.
    fn get_chain_tip(&self) -> ChainTip;

    /// Check if client is synced.
    fn is_synced(&self) -> bool;

    /// Get sync progress (0.0 to 1.0).
    fn sync_progress(&self) -> f64;
}
