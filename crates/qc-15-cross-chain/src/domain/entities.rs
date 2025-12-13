//! # Domain Entities
//!
//! Core entities for Cross-Chain Communication.
//!
//! Reference: SPEC-15 Section 2.1 (Lines 67-174)

use super::errors::{Address, CrossChainError, Hash, Secret};
use super::value_objects::{ChainAddress, ChainId, HTLCState, SwapState};
use serde::{Deserialize, Serialize};

/// Hash Time-Locked Contract (HTLC).
/// Reference: SPEC-15 Lines 67-87
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HTLC {
    /// Unique identifier.
    pub id: Hash,
    /// Hashlock: SHA-256 of the secret.
    pub hash_lock: Hash,
    /// Timelock: Unix timestamp after which refund is allowed.
    pub time_lock: u64,
    /// Amount locked.
    pub amount: u64,
    /// Sender address.
    pub sender: ChainAddress,
    /// Recipient address.
    pub recipient: ChainAddress,
    /// Current state.
    pub state: HTLCState,
    /// Secret (only set after claim).
    pub secret: Option<Secret>,
    /// Creation timestamp.
    pub created_at: u64,
}

/// Parameters for creating an HTLC.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HTLCParams {
    /// Unique identifier.
    pub id: Hash,
    /// Cryptographic hash of secret.
    pub hash_lock: Hash,
    /// Expiration timestamp.
    pub time_lock: u64,
    /// Amount locked.
    pub amount: u64,
    /// Sender address.
    pub sender: ChainAddress,
    /// Recipient address.
    pub recipient: ChainAddress,
    /// Creation timestamp.
    pub created_at: u64,
}

impl HTLC {
    /// Create a new HTLC.
    pub fn new(params: HTLCParams) -> Self {
        Self {
            id: params.id,
            hash_lock: params.hash_lock,
            time_lock: params.time_lock,
            amount: params.amount,
            sender: params.sender,
            recipient: params.recipient,
            state: HTLCState::Pending,
            secret: None,
            created_at: params.created_at,
        }
    }

    /// Check if HTLC is expired.
    pub fn is_expired(&self, current_time: u64) -> bool {
        current_time > self.time_lock
    }

    /// Check if claiming is allowed.
    pub fn can_claim(&self, current_time: u64) -> bool {
        self.state == HTLCState::Locked && !self.is_expired(current_time)
    }

    /// Check if refund is allowed.
    pub fn can_refund(&self, current_time: u64) -> bool {
        (self.state == HTLCState::Locked || self.state == HTLCState::Expired)
            && self.is_expired(current_time)
    }

    /// Transition to new state.
    pub fn transition_to(
        &mut self,
        new_state: HTLCState,
        current_time: u64,
    ) -> Result<(), CrossChainError> {
        if !self
            .state
            .can_transition_to(new_state, current_time, self.time_lock)
        {
            return Err(CrossChainError::InvalidHTLCTransition {
                from: format!("{:?}", self.state),
                to: format!("{:?}", new_state),
            });
        }
        self.state = new_state;
        Ok(())
    }

    /// Claim with secret.
    pub fn claim(&mut self, secret: Secret, current_time: u64) -> Result<(), CrossChainError> {
        if self.is_expired(current_time) {
            return Err(CrossChainError::HTLCExpired);
        }
        self.secret = Some(secret);
        self.state = HTLCState::Claimed;
        Ok(())
    }

    /// Refund to sender.
    pub fn refund(&mut self, current_time: u64) -> Result<(), CrossChainError> {
        if !self.is_expired(current_time) {
            return Err(CrossChainError::HTLCNotExpired);
        }
        self.state = HTLCState::Refunded;
        Ok(())
    }
}

/// Atomic swap between two chains.
/// Reference: SPEC-15 Lines 105-117
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AtomicSwap {
    /// Unique swap identifier.
    pub id: Hash,
    /// Source chain.
    pub source_chain: ChainId,
    /// Target chain.
    pub target_chain: ChainId,
    /// Initiator (has source assets, wants target assets).
    pub initiator: Address,
    /// Counterparty (has target assets, wants source assets).
    pub counterparty: Address,
    /// Source HTLC ID.
    pub source_htlc_id: Option<Hash>,
    /// Target HTLC ID.
    pub target_htlc_id: Option<Hash>,
    /// Current state.
    pub state: SwapState,
    /// Hash lock (shared by both HTLCs).
    pub hash_lock: Hash,
    /// Source amount.
    pub source_amount: u64,
    /// Target amount.
    pub target_amount: u64,
    /// Created at.
    pub created_at: u64,
}

/// Builder for creating AtomicSwap instances.
/// Avoids too many arguments in constructor.
#[derive(Clone, Debug)]
pub struct AtomicSwapBuilder {
    id: Hash,
    source_chain: ChainId,
    target_chain: ChainId,
    initiator: Address,
    counterparty: Address,
    hash_lock: Hash,
    source_amount: u64,
    target_amount: u64,
    created_at: u64,
}

impl AtomicSwapBuilder {
    /// Create a new builder with required fields.
    pub fn new(id: Hash, hash_lock: Hash, created_at: u64) -> Self {
        Self {
            id,
            source_chain: ChainId::QuantumChain,
            target_chain: ChainId::Ethereum,
            initiator: [0u8; 20],
            counterparty: [0u8; 20],
            hash_lock,
            source_amount: 0,
            target_amount: 0,
            created_at,
        }
    }

    /// Set source chain.
    pub fn source_chain(mut self, chain: ChainId) -> Self {
        self.source_chain = chain;
        self
    }

    /// Set target chain.
    pub fn target_chain(mut self, chain: ChainId) -> Self {
        self.target_chain = chain;
        self
    }

    /// Set initiator address.
    pub fn initiator(mut self, addr: Address) -> Self {
        self.initiator = addr;
        self
    }

    /// Set counterparty address.
    pub fn counterparty(mut self, addr: Address) -> Self {
        self.counterparty = addr;
        self
    }

    /// Set source amount.
    pub fn source_amount(mut self, amount: u64) -> Self {
        self.source_amount = amount;
        self
    }

    /// Set target amount.
    pub fn target_amount(mut self, amount: u64) -> Self {
        self.target_amount = amount;
        self
    }

    /// Build the AtomicSwap.
    pub fn build(self) -> AtomicSwap {
        AtomicSwap {
            id: self.id,
            source_chain: self.source_chain,
            target_chain: self.target_chain,
            initiator: self.initiator,
            counterparty: self.counterparty,
            source_htlc_id: None,
            target_htlc_id: None,
            state: SwapState::Initiated,
            hash_lock: self.hash_lock,
            source_amount: self.source_amount,
            target_amount: self.target_amount,
            created_at: self.created_at,
        }
    }
}

impl AtomicSwap {
    /// Create a new atomic swap using the builder pattern.
    /// For direct construction, use `AtomicSwapBuilder`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Hash,
        source_chain: ChainId,
        target_chain: ChainId,
        initiator: Address,
        counterparty: Address,
        hash_lock: Hash,
        source_amount: u64,
        target_amount: u64,
        created_at: u64,
    ) -> Self {
        Self {
            id,
            source_chain,
            target_chain,
            initiator,
            counterparty,
            source_htlc_id: None,
            target_htlc_id: None,
            state: SwapState::Initiated,
            hash_lock,
            source_amount,
            target_amount,
            created_at,
        }
    }

    /// Transition to new state.
    pub fn transition_to(&mut self, new_state: SwapState) -> Result<(), CrossChainError> {
        if !self.state.can_transition_to(new_state) {
            return Err(CrossChainError::InvalidSwapTransition {
                from: format!("{:?}", self.state),
                to: format!("{:?}", new_state),
            });
        }
        self.state = new_state;
        Ok(())
    }

    /// Set source HTLC.
    pub fn set_source_htlc(&mut self, htlc_id: Hash) -> Result<(), CrossChainError> {
        self.source_htlc_id = Some(htlc_id);
        self.transition_to(SwapState::SourceLocked)
    }

    /// Set target HTLC.
    pub fn set_target_htlc(&mut self, htlc_id: Hash) -> Result<(), CrossChainError> {
        self.target_htlc_id = Some(htlc_id);
        self.transition_to(SwapState::TargetLocked)
    }
}

/// Cross-chain proof for HTLC verification.
/// Reference: SPEC-15 Lines 140-147
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrossChainProof {
    /// Source chain.
    pub chain: ChainId,
    /// Block hash.
    pub block_hash: Hash,
    /// Block height.
    pub block_height: u64,
    /// Transaction hash.
    pub tx_hash: Hash,
    /// Merkle proof.
    pub merkle_proof: Vec<Hash>,
    /// Number of confirmations.
    pub confirmations: u64,
}

/// Cross-chain configuration.
/// Reference: SPEC-15 Lines 149-174
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrossChainConfig {
    /// Minimum timelock margin in seconds (6 hours).
    /// Reference: System.md Line 752
    pub min_timelock_margin_secs: u64,
    /// Default source HTLC timeout in seconds (24 hours).
    pub default_source_timeout_secs: u64,
    /// Default target HTLC timeout in seconds (18 hours).
    pub default_target_timeout_secs: u64,
    /// Supported chains.
    pub supported_chains: Vec<ChainId>,
}

impl Default for CrossChainConfig {
    fn default() -> Self {
        Self {
            min_timelock_margin_secs: 6 * 3600,     // 6 hours
            default_source_timeout_secs: 24 * 3600, // 24 hours
            default_target_timeout_secs: 18 * 3600, // 18 hours
            supported_chains: vec![
                ChainId::QuantumChain,
                ChainId::Ethereum,
                ChainId::Bitcoin,
                ChainId::Polygon,
                ChainId::Arbitrum,
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_htlc() -> HTLC {
        HTLC::new(HTLCParams {
            id: [1u8; 32],
            hash_lock: [2u8; 32],
            time_lock: 10000,
            amount: 1000,
            sender: ChainAddress::new(ChainId::QuantumChain, [10u8; 20]),
            recipient: ChainAddress::new(ChainId::Ethereum, [20u8; 20]),
            created_at: 1000,
        })
    }

    #[test]
    fn test_htlc_new() {
        let htlc = create_test_htlc();
        assert_eq!(htlc.state, HTLCState::Pending);
        assert!(htlc.secret.is_none());
    }

    #[test]
    fn test_htlc_is_expired() {
        let htlc = create_test_htlc();
        assert!(!htlc.is_expired(5000));
        assert!(htlc.is_expired(15000));
    }

    #[test]
    fn test_htlc_can_claim() {
        let mut htlc = create_test_htlc();
        htlc.state = HTLCState::Locked;
        assert!(htlc.can_claim(5000));
        assert!(!htlc.can_claim(15000)); // Expired
    }

    #[test]
    fn test_htlc_can_refund() {
        let mut htlc = create_test_htlc();
        htlc.state = HTLCState::Locked;
        assert!(!htlc.can_refund(5000)); // Not expired
        assert!(htlc.can_refund(15000)); // Expired
    }

    #[test]
    fn test_htlc_claim_success() {
        let mut htlc = create_test_htlc();
        htlc.state = HTLCState::Locked;
        assert!(htlc.claim([0xABu8; 32], 5000).is_ok());
        assert_eq!(htlc.state, HTLCState::Claimed);
        assert!(htlc.secret.is_some());
    }

    #[test]
    fn test_htlc_claim_expired_fails() {
        let mut htlc = create_test_htlc();
        htlc.state = HTLCState::Locked;
        assert!(htlc.claim([0xABu8; 32], 15000).is_err());
    }

    #[test]
    fn test_htlc_refund_success() {
        let mut htlc = create_test_htlc();
        htlc.state = HTLCState::Locked;
        assert!(htlc.refund(15000).is_ok());
        assert_eq!(htlc.state, HTLCState::Refunded);
    }

    #[test]
    fn test_htlc_refund_not_expired_fails() {
        let mut htlc = create_test_htlc();
        htlc.state = HTLCState::Locked;
        assert!(htlc.refund(5000).is_err());
    }

    #[test]
    fn test_atomic_swap_new() {
        let swap = AtomicSwap::new(
            [1u8; 32],
            ChainId::QuantumChain,
            ChainId::Ethereum,
            [10u8; 20],
            [20u8; 20],
            [3u8; 32],
            1000,
            2000,
            1000,
        );
        assert_eq!(swap.state, SwapState::Initiated);
    }

    #[test]
    fn test_atomic_swap_set_source_htlc() {
        let mut swap = AtomicSwap::new(
            [1u8; 32],
            ChainId::QuantumChain,
            ChainId::Ethereum,
            [10u8; 20],
            [20u8; 20],
            [3u8; 32],
            1000,
            2000,
            1000,
        );
        swap.set_source_htlc([4u8; 32]).unwrap();
        assert_eq!(swap.state, SwapState::SourceLocked);
    }

    #[test]
    fn test_config_default() {
        let config = CrossChainConfig::default();
        assert_eq!(config.min_timelock_margin_secs, 6 * 3600);
        assert_eq!(config.supported_chains.len(), 5);
    }
}
