//! # Atomic Swap Logic
//!
//! State machine and operations for atomic swaps.
//!
//! Reference: System.md Line 738

use super::secret::{create_hash_lock, generate_random_secret};
use crate::domain::{
    Address, AtomicSwap, ChainId, CrossChainError, SwapState, MIN_TIMELOCK_MARGIN_SECS,
};

/// Parameters for creating an atomic swap.
#[derive(Clone, Debug)]
pub struct AtomicSwapParams {
    /// Source chain identifier.
    pub source_chain: ChainId,
    /// Target chain identifier.
    pub target_chain: ChainId,
    /// Initiator address.
    pub initiator: Address,
    /// Counterparty address.
    pub counterparty: Address,
    /// Amount on source chain.
    pub source_amount: u64,
    /// Amount on target chain.
    pub target_amount: u64,
    /// Current timestamp.
    pub current_time: u64,
}

/// Create a new atomic swap with generated secret.
///
/// Returns (swap, secret) - secret must be kept safe by initiator.
pub fn create_atomic_swap(params: AtomicSwapParams) -> (AtomicSwap, [u8; 32]) {
    let secret = generate_random_secret();
    let hash_lock = create_hash_lock(&secret);

    // Generate swap ID from hashlock
    let mut swap_id = [0u8; 32];
    swap_id.copy_from_slice(&hash_lock);
    swap_id[0] ^= 0xFF; // Differentiate from hashlock

    let swap = AtomicSwap::new(
        swap_id,
        params.source_chain,
        params.target_chain,
        params.initiator,
        params.counterparty,
        hash_lock,
        params.source_amount,
        params.target_amount,
        params.current_time,
    );

    (swap, secret)
}

/// Validate timelock ordering for swap.
///
/// Reference: System.md Line 752
pub fn validate_swap_timelocks(
    source_timelock: u64,
    target_timelock: u64,
) -> Result<(), CrossChainError> {
    crate::domain::invariant_timelock_ordering(
        source_timelock,
        target_timelock,
        MIN_TIMELOCK_MARGIN_SECS,
    )
}

/// Calculate recommended timelocks for swap.
pub fn calculate_timelocks(
    source_chain: ChainId,
    target_chain: ChainId,
    current_time: u64,
) -> (u64, u64) {
    // Source chain gets longer timeout
    let _source_finality = source_chain.required_confirmations() * source_chain.block_time_secs();
    let target_finality = target_chain.required_confirmations() * target_chain.block_time_secs();

    // Target timeout: enough time for finality + some buffer
    let target_timeout = current_time + target_finality + 6 * 3600; // +6 hours buffer

    // Source timeout: target timeout + margin
    let source_timeout = target_timeout + MIN_TIMELOCK_MARGIN_SECS;

    (source_timeout, target_timeout)
}

/// Get swap completion status.
pub fn is_swap_complete(swap: &AtomicSwap) -> bool {
    swap.state == SwapState::Completed
}

/// Get swap refund status.
pub fn is_swap_refunded(swap: &AtomicSwap) -> bool {
    swap.state == SwapState::Refunded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_atomic_swap() {
        let (swap, secret) = create_atomic_swap(AtomicSwapParams {
            source_chain: ChainId::QuantumChain,
            target_chain: ChainId::Ethereum,
            initiator: [1u8; 20],
            counterparty: [2u8; 20],
            source_amount: 1000,
            target_amount: 2000,
            current_time: 1000,
        });

        assert_eq!(swap.state, SwapState::Initiated);
        assert_eq!(swap.source_amount, 1000);
        assert_eq!(swap.target_amount, 2000);

        // Verify hash lock matches secret
        let computed_hash = create_hash_lock(&secret);
        assert_eq!(swap.hash_lock, computed_hash);
    }

    #[test]
    fn test_validate_swap_timelocks_valid() {
        // Source: 50000, Target: 20000
        // 50000 > 20000 + 21600 = 41600? Yes!
        assert!(validate_swap_timelocks(50000, 20000).is_ok());
    }

    #[test]
    fn test_validate_swap_timelocks_invalid() {
        // Source: 30000, Target: 20000
        // 30000 > 20000 + 21600 = 41600? No!
        assert!(validate_swap_timelocks(30000, 20000).is_err());
    }

    #[test]
    fn test_calculate_timelocks() {
        let current_time = 1000;
        let (source, target) =
            calculate_timelocks(ChainId::QuantumChain, ChainId::Ethereum, current_time);

        // Source should be >= target + margin
        assert!(source >= target + MIN_TIMELOCK_MARGIN_SECS);
    }

    #[test]
    fn test_is_swap_complete() {
        let (mut swap, _) = create_atomic_swap(AtomicSwapParams {
            source_chain: ChainId::QuantumChain,
            target_chain: ChainId::Ethereum,
            initiator: [1u8; 20],
            counterparty: [2u8; 20],
            source_amount: 1000,
            target_amount: 2000,
            current_time: 1000,
        });

        assert!(!is_swap_complete(&swap));

        swap.state = SwapState::Completed;
        assert!(is_swap_complete(&swap));
    }
}
