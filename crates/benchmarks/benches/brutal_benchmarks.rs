//! # Quantum-Chain Brutal Benchmarks
//!
//! Modular performance validation for all core subsystems.
//! Each subsystem has dedicated brutal benchmarks based on SPEC claims.
//!
//! ## Usage
//!
//! Run all brutal benchmarks:
//! ```bash
//! cargo bench --package qc-benchmarks --bench brutal_benchmarks
//! ```
//!
//! Run specific subsystem:
//! ```bash
//! cargo bench --package qc-benchmarks --bench brutal_benchmarks -- qc-01
//! cargo bench --package qc-benchmarks --bench brutal_benchmarks -- qc-04/brutal/state_root
//! ```
//!
//! ## Subsystem Coverage
//!
//! | Subsystem | SPEC | Key Claims |
//! |-----------|------|------------|
//! | qc-01 | Peer Discovery | XOR < 100ns, find closest < 1ms |
//! | qc-02 | Block Storage | O(1) lookup, atomic write < 10ms |
//! | qc-03 | Transaction Indexing | O(log n) proof, tree build O(n) |
//! | qc-04 | State Management | State root < 10s, O(log n) lookup |
//! | qc-06 | Mempool | O(1) lookup, 2PC < 1ms/tx |
//! | qc-08 | Consensus | Block validation < 100ms, fork choice O(log n) |
//! | qc-10 | Signature Verification | ECDSA < 1ms, batch 2x faster |

mod brutal;

use criterion::{criterion_group, criterion_main, Criterion};

fn bench_qc_01_peer_discovery(c: &mut Criterion) {
    brutal::qc_01_peer_discovery::register_benchmarks(c);
}

fn bench_qc_02_block_storage(c: &mut Criterion) {
    brutal::qc_02_block_storage::register_benchmarks(c);
}

fn bench_qc_03_tx_indexing(c: &mut Criterion) {
    brutal::qc_03_tx_indexing::register_benchmarks(c);
}

fn bench_qc_04_state_mgmt(c: &mut Criterion) {
    brutal::qc_04_state_mgmt::register_benchmarks(c);
}

fn bench_qc_06_mempool(c: &mut Criterion) {
    brutal::qc_06_mempool::register_benchmarks(c);
}

fn bench_qc_08_consensus(c: &mut Criterion) {
    brutal::qc_08_consensus::register_benchmarks(c);
}

fn bench_qc_10_signature(c: &mut Criterion) {
    brutal::qc_10_signature::register_benchmarks(c);
}

criterion_group!(
    name = brutal_benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(std::time::Duration::from_secs(10));
    targets =
        bench_qc_01_peer_discovery,
        bench_qc_02_block_storage,
        bench_qc_03_tx_indexing,
        bench_qc_04_state_mgmt,
        bench_qc_06_mempool,
        bench_qc_08_consensus,
        bench_qc_10_signature,
);

criterion_main!(brutal_benches);
