//! # Cross-Chain Adapter
//!
//! Connects qc-15 Cross-Chain to the event bus for IPC.
//!
//! ## IPC Interactions (IPC-MATRIX.md)
//!
//! | From | To | Message |
//! |------|-----|---------|
//! | 15 → 11 | Smart Contracts | HTLC deployment |
//! | 15 → 8 | Consensus | Cross-chain proofs |

#[cfg(feature = "qc-15")]
use qc_15_cross_chain::{
    calculate_timelocks, create_atomic_swap, verify_secret, AtomicSwap, ChainId, CrossChainConfig, Hash, Secret, HTLC,
};

#[cfg(feature = "qc-15")]
use std::collections::HashMap;

/// Cross-chain adapter for event bus integration.
#[cfg(feature = "qc-15")]
pub struct CrossChainAdapter {
    /// Configuration.
    config: CrossChainConfig,
    /// Subsystem ID.
    subsystem_id: u8,
    /// Active swaps.
    swaps: HashMap<Hash, AtomicSwap>,
    /// Active HTLCs.
    htlcs: HashMap<Hash, HTLC>,
}

#[cfg(feature = "qc-15")]
impl CrossChainAdapter {
    /// Create a new cross-chain adapter.
    pub fn new(config: CrossChainConfig) -> Self {
        Self {
            config,
            subsystem_id: 15,
            swaps: HashMap::new(),
            htlcs: HashMap::new(),
        }
    }

    /// Create with default config.
    pub fn with_defaults() -> Self {
        Self::new(CrossChainConfig::default())
    }

    /// Get subsystem ID.
    pub fn subsystem_id(&self) -> u8 {
        self.subsystem_id
    }

    /// Initiate a new atomic swap.
    pub fn initiate_swap(
        &mut self,
        source_chain: ChainId,
        target_chain: ChainId,
        initiator: [u8; 20],
        counterparty: [u8; 20],
        source_amount: u64,
        target_amount: u64,
    ) -> (AtomicSwap, Secret) {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let (swap, secret) = create_atomic_swap(
            source_chain,
            target_chain,
            initiator,
            counterparty,
            source_amount,
            target_amount,
            current_time,
        );

        self.swaps.insert(swap.id, swap.clone());
        (swap, secret)
    }

    /// Calculate recommended timelocks for a swap.
    pub fn get_timelocks(&self, source: ChainId, target: ChainId) -> (u64, u64) {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        calculate_timelocks(source, target, current_time)
    }

    /// Check if chain is supported.
    pub fn is_chain_supported(&self, chain: ChainId) -> bool {
        self.config.supported_chains.contains(&chain)
    }

    /// Get swap count.
    pub fn swap_count(&self) -> usize {
        self.swaps.len()
    }
}

#[cfg(feature = "qc-15")]
impl Default for CrossChainAdapter {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(all(test, feature = "qc-15"))]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let adapter = CrossChainAdapter::with_defaults();
        assert_eq!(adapter.subsystem_id(), 15);
    }

    #[test]
    fn test_adapter_initiate_swap() {
        let mut adapter = CrossChainAdapter::with_defaults();
        let (swap, secret) = adapter.initiate_swap(
            ChainId::QuantumChain,
            ChainId::Ethereum,
            [1u8; 20],
            [2u8; 20],
            1000,
            2000,
        );

        assert_eq!(adapter.swap_count(), 1);
        assert_eq!(swap.source_amount, 1000);

        // Verify secret matches hashlock
        assert!(verify_secret(&secret, &swap.hash_lock));
    }

    #[test]
    fn test_adapter_chain_support() {
        let adapter = CrossChainAdapter::with_defaults();
        assert!(adapter.is_chain_supported(ChainId::Ethereum));
        assert!(adapter.is_chain_supported(ChainId::Bitcoin));
    }

    #[test]
    fn test_adapter_get_timelocks() {
        let adapter = CrossChainAdapter::with_defaults();
        let (source, target) = adapter.get_timelocks(ChainId::QuantumChain, ChainId::Ethereum);
        assert!(source > target);
    }
}
