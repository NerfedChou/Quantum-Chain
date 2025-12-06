//! # Brutal Modular Benchmarks
//!
//! Each subsystem has dedicated brutal benchmarks that validate SPEC claims
//! under adversarial conditions. These are NOT polite benchmarks - they push
//! each subsystem to breaking point.
//!
//! ## Structure
//!
//! Each module tests claims from its dedicated SPEC:
//! - `qc_01_peer_discovery` - SPEC-01: Kademlia XOR, bucket ops, network latency
//! - `qc_02_block_storage` - SPEC-02: O(1) lookup, atomic writes, disk I/O
//! - `qc_03_tx_indexing` - SPEC-03: Merkle tree O(log n), proof generation
//! - `qc_04_state_mgmt` - SPEC-04: State root < 10s, proof generation
//! - `qc_06_mempool` - SPEC-06: Two-phase commit, priority ordering
//! - `qc_07_bloom_filters` - SPEC-07: Insert/contains O(k), FPR validation, privacy rotation
//! - `qc_08_consensus` - SPEC-08: Block validation < 100ms, attestation
//! - `qc_10_signature` - SPEC-10: ECDSA < 1ms, batch 2x speedup

pub mod qc_01_peer_discovery;
pub mod qc_02_block_storage;
pub mod qc_03_tx_indexing;
pub mod qc_04_state_mgmt;
pub mod qc_06_mempool;
pub mod qc_07_bloom_filters;
pub mod qc_08_consensus;
pub mod qc_10_signature;
