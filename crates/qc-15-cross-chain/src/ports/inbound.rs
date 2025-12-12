//! # Inbound Ports
//!
//! API trait defining what the Cross-Chain subsystem can do.
//!
//! Reference: SPEC-15 Section 3.1 (Lines 219-250)

use crate::domain::{
    Address, AtomicSwap, ChainId, CrossChainError, CrossChainProof, Hash, Secret, HTLC,
};
use async_trait::async_trait;

/// Cross-chain API - inbound port.
///
/// Reference: SPEC-15 Lines 219-250
#[async_trait]
pub trait CrossChainApi: Send + Sync {
    /// Initiate an atomic swap.
    async fn initiate_swap(
        &mut self,
        source_chain: ChainId,
        target_chain: ChainId,
        initiator: Address,
        counterparty: Address,
        source_amount: u64,
        target_amount: u64,
    ) -> Result<(AtomicSwap, Secret), CrossChainError>;

    /// Lock source HTLC.
    async fn lock_source(
        &mut self,
        swap_id: Hash,
        amount: u64,
        time_lock: u64,
    ) -> Result<HTLC, CrossChainError>;

    /// Lock target HTLC (after verifying source proof).
    async fn lock_target(
        &mut self,
        swap_id: Hash,
        amount: u64,
        time_lock: u64,
        source_proof: CrossChainProof,
    ) -> Result<HTLC, CrossChainError>;

    /// Claim an HTLC with secret.
    async fn claim(&self, htlc_id: Hash, secret: Secret) -> Result<(), CrossChainError>;

    /// Refund an expired HTLC.
    async fn refund(&self, htlc_id: Hash) -> Result<(), CrossChainError>;

    /// Get swap by ID.
    fn get_swap(&self, swap_id: &Hash) -> Option<&AtomicSwap>;

    /// Get HTLC by ID.
    fn get_htlc(&self, htlc_id: &Hash) -> Option<&HTLC>;

    /// Check if chain is supported.
    fn is_chain_supported(&self, chain: ChainId) -> bool;
}
