//! # Quantum-Chain Benchmarks
//!
//! Performance benchmarks per subsystem.
//! All benchmarks are "brutal" stress tests validating SPEC claims.

pub mod qc_01_peer_discovery;
pub mod qc_02_block_storage;
pub mod qc_03_tx_indexing;
pub mod qc_04_state_mgmt;
pub mod qc_06_mempool;
pub mod qc_07_bloom_filters;
pub mod qc_08_consensus;
pub mod qc_10_signature;

/// Re-export all benchmarks under the "brutal" namespace for the bench harness.
pub mod brutal {
    pub use super::qc_01_peer_discovery;
    pub use super::qc_02_block_storage;
    pub use super::qc_03_tx_indexing;
    pub use super::qc_04_state_mgmt;
    pub use super::qc_06_mempool;
    pub use super::qc_07_bloom_filters;
    pub use super::qc_08_consensus;
    pub use super::qc_10_signature;
}
