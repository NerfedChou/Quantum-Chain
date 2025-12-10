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
    generate_random_secret, create_hash_lock, verify_secret,
    verify_claim, verify_refund,
    create_atomic_swap, validate_swap_timelocks, calculate_timelocks,
    is_swap_complete, is_swap_refunded,
};
pub use domain::{
    Hash, Address, Secret, CrossChainError,
    ChainId, HTLCState, SwapState, ChainAddress,
    HTLC, AtomicSwap, CrossChainProof, CrossChainConfig,
    MIN_TIMELOCK_MARGIN_SECS,
    invariant_timelock_ordering, invariant_hashlock_match,
    invariant_secret_matches, invariant_authorized_claimer,
    invariant_sufficient_confirmations,
};
pub use ports::{
    CrossChainApi,
    ExternalChainClient, HTLCContract, FinalityChecker,
    BlockHeader, MockChainClient,
};

/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn test_version() {
        assert!(!super::VERSION.is_empty());
    }
}
