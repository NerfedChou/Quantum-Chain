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
    calculate_timelocks, create_atomic_swap, AtomicSwap, AtomicSwapParams, ChainId,
    CrossChainConfig, Hash, Secret, HTLC,
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
    pub fn initiate_swap(&mut self, params: AtomicSwapParams) -> (AtomicSwap, Secret) {
        let (swap, secret) = create_atomic_swap(params);

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

    // =========================================================================
    // HTLC Lifecycle Methods
    // =========================================================================

    /// Register a new HTLC after deploying on-chain.
    pub fn register_htlc(&mut self, htlc: HTLC) {
        self.htlcs.insert(htlc.id, htlc);
    }

    /// Get an HTLC by ID.
    pub fn get_htlc(&self, id: &Hash) -> Option<&HTLC> {
        self.htlcs.get(id)
    }

    /// Get mutable reference to an HTLC by ID.
    pub fn get_htlc_mut(&mut self, id: &Hash) -> Option<&mut HTLC> {
        self.htlcs.get_mut(id)
    }

    /// Count of active HTLCs.
    pub fn htlc_count(&self) -> usize {
        self.htlcs.len()
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
    use qc_15_cross_chain::{
        create_hash_lock, generate_random_secret, verify_secret, ChainAddress, HTLCParams,
        HTLCState,
    };

    #[test]
    fn test_adapter_creation() {
        let adapter = CrossChainAdapter::with_defaults();
        assert_eq!(adapter.subsystem_id(), 15);
    }

    #[test]
    fn test_adapter_initiate_swap() {
        let mut adapter = CrossChainAdapter::with_defaults();
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let (swap, secret) = adapter.initiate_swap(AtomicSwapParams {
            source_chain: ChainId::QuantumChain,
            target_chain: ChainId::Ethereum,
            initiator: [1u8; 20],
            counterparty: [2u8; 20],
            source_amount: 1000,
            target_amount: 2000,
            current_time,
        });

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

    // =========================================================================
    // TDD TESTS: Phase 2 - HTLC Lifecycle
    // =========================================================================

    fn create_test_htlc() -> HTLC {
        let secret = generate_random_secret();
        let hash_lock = create_hash_lock(&secret);
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        HTLC::new(HTLCParams {
            id: [0xAA; 32],
            hash_lock,
            time_lock: current_time + 86400, // 24h from now
            amount: 1000,
            sender: ChainAddress::new(ChainId::QuantumChain, [1u8; 20]),
            recipient: ChainAddress::new(ChainId::Ethereum, [2u8; 20]),
            created_at: current_time,
        })
    }

    #[test]
    fn test_register_and_get_htlc() {
        let mut adapter = CrossChainAdapter::with_defaults();
        let htlc = create_test_htlc();
        let htlc_id = htlc.id;

        adapter.register_htlc(htlc);

        assert_eq!(adapter.htlc_count(), 1);
        let retrieved = adapter.get_htlc(&htlc_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().amount, 1000);
    }

    #[test]
    fn test_htlc_not_found() {
        let adapter = CrossChainAdapter::with_defaults();
        let unknown_id = [0xFF; 32];

        assert!(adapter.get_htlc(&unknown_id).is_none());
    }

    #[test]
    fn test_htlc_claim_via_adapter() {
        let mut adapter = CrossChainAdapter::with_defaults();
        let secret = generate_random_secret();
        let hash_lock = create_hash_lock(&secret);
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let htlc = HTLC::new(HTLCParams {
            id: [0xBB; 32],
            hash_lock,
            time_lock: current_time + 86400, // expires in 24h
            amount: 2000,
            sender: ChainAddress::new(ChainId::QuantumChain, [1u8; 20]),
            recipient: ChainAddress::new(ChainId::Ethereum, [2u8; 20]),
            created_at: current_time,
        });
        let htlc_id = htlc.id;

        adapter.register_htlc(htlc);

        if let Some(htlc) = adapter.get_htlc_mut(&htlc_id) {
            htlc.state = HTLCState::Locked;
        }

        // Claim with valid secret
        if let Some(htlc) = adapter.get_htlc_mut(&htlc_id) {
            let result = htlc.claim(secret, current_time);
            assert!(result.is_ok());
        }

        // Verify state changed to Claimed
        let htlc = adapter.get_htlc(&htlc_id).unwrap();
        assert_eq!(htlc.state, HTLCState::Claimed);
    }

    #[test]
    fn test_htlc_refund_via_adapter() {
        let mut adapter = CrossChainAdapter::with_defaults();
        let secret = generate_random_secret();
        let hash_lock = create_hash_lock(&secret);
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let htlc = HTLC::new(HTLCParams {
            id: [0xCC; 32],
            hash_lock,
            time_lock: current_time - 1, // Already expired
            amount: 3000,
            sender: ChainAddress::new(ChainId::QuantumChain, [1u8; 20]),
            recipient: ChainAddress::new(ChainId::Ethereum, [2u8; 20]),
            created_at: current_time - 86400, // Created yesterday
        });
        let htlc_id = htlc.id;

        adapter.register_htlc(htlc);

        if let Some(htlc) = adapter.get_htlc_mut(&htlc_id) {
            htlc.state = HTLCState::Locked;
        }

        // Refund after expiry
        if let Some(htlc) = adapter.get_htlc_mut(&htlc_id) {
            let result = htlc.refund(current_time);
            assert!(result.is_ok());
        }

        // Verify state changed to Refunded
        let htlc = adapter.get_htlc(&htlc_id).unwrap();
        assert_eq!(htlc.state, HTLCState::Refunded);
    }
}
