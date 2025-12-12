//! # QC-15 Cross-Chain Communication
//!
//! Trustless cross-chain asset transfers using HTLC.
//!
//! **Subsystem ID:** 15  
//! **Specification:** SPEC-15-CROSS-CHAIN.md  
//! **Architecture:** Hexagonal (DDD + Ports/Adapters)  
//! **Status:** Production-Ready
//!
//! ## Purpose
//!
//! Enable atomic swaps between QuantumChain and external blockchains:
//! - Hash Time-Locked Contracts (HTLC) for trustless swaps
//! - SHA-256 hashlocks for cryptographic security
//! - Timelock ordering for atomicity guarantees
//!
//! ## Security Features (System.md Lines 751-756)
//!
//! | Defense | Description |
//! |---------|-------------|
//! | Timelock margins | Source > Target + 6 hours |
//! | SHA-256 only | No weak hash functions |
//! | Finality checks | Chain-specific confirmations |
//! | Secret atomicity | Reveal on one chain = claimable on both |
//!
//! ## Module Structure
//!
//! ```text
//! qc-15-cross-chain/
//! ├── domain/          # HTLC, AtomicSwap, ChainId, errors
//! ├── algorithms/      # Secret generation, swap logic
//! └── ports/           # CrossChainApi, ExternalChainClient
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod algorithms;
pub mod domain;
pub mod ports;

// Re-exports
pub use algorithms::{
    calculate_timelocks, create_atomic_swap, create_hash_lock, generate_random_secret,
    is_swap_complete, is_swap_refunded, validate_swap_timelocks, verify_claim, verify_refund,
    verify_secret,
};
pub use domain::{
    invariant_authorized_claimer, invariant_hashlock_match, invariant_secret_matches,
    invariant_sufficient_confirmations, invariant_timelock_ordering, Address, AtomicSwap,
    ChainAddress, ChainId, CrossChainConfig, CrossChainError, CrossChainProof, HTLCState, Hash,
    Secret, SwapState, HTLC, MIN_TIMELOCK_MARGIN_SECS,
};
pub use ports::{
    BlockHeader, CrossChainApi, ExternalChainClient, FinalityChecker, HTLCContract, MockChainClient,
};

/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    #[allow(clippy::const_is_empty)]
    fn test_version() {
        assert!(!super::VERSION.is_empty());
    }
}
