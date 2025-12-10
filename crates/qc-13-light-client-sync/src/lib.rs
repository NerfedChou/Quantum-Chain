//! # QC-13 Light Client Sync
//!
//! SPV (Simplified Payment Verification) for light clients.
//!
//! **Subsystem ID:** 13  
//! **Specification:** SPEC-13-LIGHT-CLIENT.md  
//! **Architecture:** Hexagonal (DDD + Ports/Adapters)  
//! **Status:** Production-Ready
//!
//! ## Purpose
//!
//! Enable mobile/desktop clients to verify blockchain state without
//! downloading the full chain, using:
//! - Block headers (~80 bytes/block) instead of full blocks
//! - Merkle proofs for transaction verification
//! - Multi-node consensus for security
//!
//! ## Security Features (System.md Lines 643-648)
//!
//! | Defense | Description |
//! |---------|-------------|
//! | Multi-node consensus | Query 3+ nodes, require 2/3 agreement |
//! | Merkle verification | Cryptographic proof of transaction inclusion |
//! | Checkpoint enforcement | Reject chains missing trusted checkpoints |
//! | Peer diversity | Random selection from diverse sources |
//!
//! ## Module Structure
//!
//! ```text
//! qc-13-light-client-sync/
//! ├── domain/          # Core types: HeaderChain, ProvenTransaction, errors
//! ├── algorithms/      # Merkle verification, header sync, multi-node consensus
//! ├── ports/           # API traits (inbound) + dependency traits (outbound)
//! ├── application/     # LightClientService orchestrating everything
//! └── config.rs        # LightClientConfig
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod algorithms;
pub mod application;
pub mod config;
pub mod domain;
pub mod ports;

// Re-exports
pub use algorithms::{
    verify_merkle_proof, compute_merkle_root, build_merkle_proof,
    check_consensus, check_strict_consensus,
    validate_header_batch, append_headers_batch,
};
pub use application::LightClientService;
pub use config::LightClientConfig;
pub use domain::{
    Hash, LightClientError,
    BlockHeader, HeaderChain, ProvenTransaction,
    Checkpoint, CheckpointSource, ChainTip, SyncResult,
    MerkleProof, ProofNode, Position,
    MIN_FULL_NODES, CONSENSUS_THRESHOLD, DEFAULT_CONFIRMATIONS,
    invariant_multi_node, invariant_consensus, invariant_checkpoint_chain,
};
pub use ports::{
    LightClientApi, Address,
    FullNodeConnection, PeerDiscovery, MerkleProofProvider, BloomFilterProvider,
    MockFullNode,
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
