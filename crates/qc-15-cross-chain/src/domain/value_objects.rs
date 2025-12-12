//! # Domain Value Objects
//!
//! Immutable value types for Cross-Chain Communication.
//!
//! Reference: SPEC-15 Section 2.1 (Lines 56-126)

use super::errors::Address;
use serde::{Deserialize, Serialize};

/// Supported blockchain identifiers.
/// Reference: SPEC-15 Lines 57-65
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChainId {
    /// QuantumChain (our chain).
    QuantumChain,
    /// Ethereum mainnet.
    Ethereum,
    /// Bitcoin mainnet.
    Bitcoin,
    /// Polygon (Matic).
    Polygon,
    /// Arbitrum L2.
    Arbitrum,
}

impl ChainId {
    /// Get required confirmations for finality.
    /// Reference: SPEC-15 Lines 650-654
    pub fn required_confirmations(&self) -> u64 {
        match self {
            ChainId::QuantumChain => 6,
            ChainId::Ethereum => 12, // PoS, 2 epochs
            ChainId::Bitcoin => 6,   // PoW, ~1 hour
            ChainId::Polygon => 128, // Fast finality
            ChainId::Arbitrum => 1,  // L2, verified by L1
        }
    }

    /// Get estimated block time in seconds.
    pub fn block_time_secs(&self) -> u64 {
        match self {
            ChainId::QuantumChain => 10,
            ChainId::Ethereum => 12,
            ChainId::Bitcoin => 600,
            ChainId::Polygon => 2,
            ChainId::Arbitrum => 1,
        }
    }
}

/// HTLC state machine.
/// Reference: SPEC-15 Lines 96-103
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HTLCState {
    /// Created but not yet locked on chain.
    #[default]
    Pending,
    /// Funds locked, awaiting claim or expiry.
    Locked,
    /// Secret revealed, funds transferred to recipient.
    Claimed,
    /// Past timelock, awaiting refund.
    Expired,
    /// Funds returned to sender.
    Refunded,
}

impl HTLCState {
    /// Check if transition is valid.
    pub fn can_transition_to(&self, next: HTLCState, current_time: u64, timelock: u64) -> bool {
        match (self, next) {
            (Self::Pending, Self::Locked) => true,
            (Self::Locked, Self::Claimed) => current_time <= timelock,
            (Self::Locked, Self::Expired) => current_time > timelock,
            (Self::Expired, Self::Refunded) => true,
            _ => false,
        }
    }

    /// Check if terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Claimed | Self::Refunded)
    }
}

/// Atomic swap state machine.
/// Reference: SPEC-15 Lines 119-126
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwapState {
    /// Swap created, not yet started.
    #[default]
    Initiated,
    /// Source HTLC locked.
    SourceLocked,
    /// Both source and target HTLCs locked.
    TargetLocked,
    /// Both HTLCs claimed, swap completed.
    Completed,
    /// Both HTLCs refunded.
    Refunded,
}

impl SwapState {
    /// Check if transition is valid.
    pub fn can_transition_to(&self, next: SwapState) -> bool {
        match (self, next) {
            (Self::Initiated, Self::SourceLocked) => true,
            (Self::Initiated, Self::Refunded) => true, // Abort before locking
            (Self::SourceLocked, Self::TargetLocked) => true,
            (Self::SourceLocked, Self::Refunded) => true, // Timeout
            (Self::TargetLocked, Self::Completed) => true,
            (Self::TargetLocked, Self::Refunded) => true, // Timeout
            _ => false,
        }
    }

    /// Check if terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Refunded)
    }
}

/// Chain-specific address.
/// Reference: SPEC-15 Lines 89-94
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainAddress {
    /// Chain this address belongs to.
    pub chain: ChainId,
    /// Address bytes.
    pub address: Address,
}

impl ChainAddress {
    /// Create a new chain address.
    pub fn new(chain: ChainId, address: Address) -> Self {
        Self { chain, address }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_id_confirmations() {
        assert_eq!(ChainId::Bitcoin.required_confirmations(), 6);
        assert_eq!(ChainId::Ethereum.required_confirmations(), 12);
        assert_eq!(ChainId::Arbitrum.required_confirmations(), 1);
    }

    #[test]
    fn test_htlc_state_pending_to_locked() {
        assert!(HTLCState::Pending.can_transition_to(HTLCState::Locked, 0, 0));
    }

    #[test]
    fn test_htlc_state_locked_to_claimed_before_expiry() {
        assert!(HTLCState::Locked.can_transition_to(HTLCState::Claimed, 100, 200));
    }

    #[test]
    fn test_htlc_state_locked_to_claimed_after_expiry_fails() {
        assert!(!HTLCState::Locked.can_transition_to(HTLCState::Claimed, 300, 200));
    }

    #[test]
    fn test_htlc_state_locked_to_expired() {
        assert!(HTLCState::Locked.can_transition_to(HTLCState::Expired, 300, 200));
    }

    #[test]
    fn test_htlc_state_terminal() {
        assert!(HTLCState::Claimed.is_terminal());
        assert!(HTLCState::Refunded.is_terminal());
        assert!(!HTLCState::Locked.is_terminal());
    }

    #[test]
    fn test_swap_state_transitions() {
        assert!(SwapState::Initiated.can_transition_to(SwapState::SourceLocked));
        assert!(SwapState::SourceLocked.can_transition_to(SwapState::TargetLocked));
        assert!(SwapState::TargetLocked.can_transition_to(SwapState::Completed));
    }

    #[test]
    fn test_swap_state_terminal() {
        assert!(SwapState::Completed.is_terminal());
        assert!(SwapState::Refunded.is_terminal());
    }

    #[test]
    fn test_chain_address() {
        let addr = ChainAddress::new(ChainId::Ethereum, [0xABu8; 20]);
        assert_eq!(addr.chain, ChainId::Ethereum);
    }
}
