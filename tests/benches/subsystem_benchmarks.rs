//! # Quantum-Chain Subsystem Benchmarks
//!
//! Performance validation for SPEC claims:
//!
//! | Subsystem | SPEC Claim | Target |
//! |-----------|------------|--------|
//! | qc-02 Block Storage | O(1) height lookup | < 1ms |
//! | qc-03 Transaction Indexing | O(1) proof generation | < 1ms |
//! | qc-04 State Management | State root < 10s normal | < 10s |
//! | qc-06 Mempool | Two-phase commit | < 1ms per tx |
//! | qc-08 Consensus | Block validation | < 100ms |
//! | qc-10 Signature Verification | ECDSA verify | < 1ms |

// Allow excessive nesting in benchmark code
#![allow(clippy::excessive_nesting)]

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::Rng;
use sha3::{Digest, Keccak256};
use std::time::Duration;

// ============================================================================
// QC-10: Signature Verification Benchmarks
// SPEC-10 Claim: ECDSA verification should be fast enough for network edge
// ============================================================================

fn bench_ecdsa_signature_verification(c: &mut Criterion) {
    use k256::ecdsa::{
        signature::Signer, signature::Verifier, Signature, SigningKey, VerifyingKey,
    };

    let mut group = c.benchmark_group("qc-10-signature-verification");
    group.measurement_time(Duration::from_secs(10));

    // Generate test key pair
    let signing_key = SigningKey::random(&mut rand::thread_rng());
    let verifying_key = VerifyingKey::from(&signing_key);

    // Generate message and signature
    let message = b"test transaction data for benchmark";
    let signature: Signature = signing_key.sign(message);

    group.bench_function("ecdsa_verify_single", |b| {
        b.iter(|| black_box(verifying_key.verify(message, &signature).is_ok()))
    });

    // Batch verification benchmark
    let batch_sizes = [10, 50, 100, 500];
    for size in batch_sizes {
        let messages: Vec<_> = (0..size)
            .map(|i| format!("message_{}", i).into_bytes())
            .collect();
        let signatures: Vec<Signature> = messages
            .iter()
            .map(|m| signing_key.sign(m.as_slice()))
            .collect();

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::new("ecdsa_verify_batch", size),
            &(messages, signatures),
            |b, (msgs, sigs)| {
                b.iter(|| {
                    let mut valid_count = 0u32;
                    for (msg, sig) in msgs.iter().zip(sigs.iter()) {
                        if verifying_key.verify(msg.as_slice(), sig).is_ok() {
                            valid_count += 1;
                        }
                    }
                    black_box(valid_count)
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// QC-03: Transaction Indexing Benchmarks
// SPEC-03 Claim: O(1) proof generation, O(log n) verification
// ============================================================================

fn bench_merkle_tree_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-03-merkle-tree");
    group.measurement_time(Duration::from_secs(10));

    // Generate random transaction hashes
    fn generate_tx_hashes(count: usize) -> Vec<[u8; 32]> {
        let mut rng = rand::thread_rng();
        (0..count)
            .map(|_| {
                let mut hash = [0u8; 32];
                rng.fill(&mut hash);
                hash
            })
            .collect()
    }

    // Simple Merkle tree implementation for benchmarking
    fn build_merkle_tree(leaves: &[[u8; 32]]) -> Vec<[u8; 32]> {
        if leaves.is_empty() {
            return vec![[0u8; 32]];
        }

        let mut padded = leaves.to_vec();
        let next_pow2 = padded.len().next_power_of_two();
        padded.resize(next_pow2, [0u8; 32]);

        let mut tree = vec![[0u8; 32]; 2 * next_pow2];
        tree[next_pow2..].copy_from_slice(&padded);

        for i in (1..next_pow2).rev() {
            let mut hasher = Keccak256::new();
            hasher.update(tree[2 * i]);
            hasher.update(tree[2 * i + 1]);
            tree[i].copy_from_slice(&hasher.finalize());
        }

        tree
    }

    fn generate_proof(tree: &[[u8; 32]], index: usize) -> Vec<([u8; 32], bool)> {
        let leaf_count = tree.len() / 2;
        let mut proof = Vec::new();
        let mut idx = leaf_count + index;

        while idx > 1 {
            let sibling = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
            let is_left = idx % 2 == 1;
            proof.push((tree[sibling], is_left));
            idx /= 2;
        }

        proof
    }

    fn verify_proof(leaf: &[u8; 32], proof: &[([u8; 32], bool)], root: &[u8; 32]) -> bool {
        let mut current = *leaf;
        for (sibling, is_left) in proof {
            let mut hasher = Keccak256::new();
            if *is_left {
                hasher.update(sibling);
                hasher.update(current);
            } else {
                hasher.update(current);
                hasher.update(sibling);
            }
            current.copy_from_slice(&hasher.finalize());
        }
        current == *root
    }

    // Benchmark tree building
    let tx_counts = [100, 500, 1000, 5000, 10000];
    for count in tx_counts {
        let hashes = generate_tx_hashes(count);

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("merkle_tree_build", count),
            &hashes,
            |b, h| b.iter(|| black_box(build_merkle_tree(h))),
        );
    }

    // Benchmark proof generation (should be O(log n))
    for count in tx_counts {
        let hashes = generate_tx_hashes(count);
        let tree = build_merkle_tree(&hashes);

        group.bench_with_input(
            BenchmarkId::new("merkle_proof_generate", count),
            &tree,
            |b, t| {
                let mut rng = rand::thread_rng();
                b.iter(|| {
                    let idx = rng.gen_range(0..count);
                    black_box(generate_proof(t, idx))
                })
            },
        );
    }

    // Benchmark proof verification (should be O(log n))
    for count in tx_counts {
        let hashes = generate_tx_hashes(count);
        let tree = build_merkle_tree(&hashes);
        let root = tree[1];
        let proof = generate_proof(&tree, 0);
        let leaf = hashes[0];

        group.bench_with_input(
            BenchmarkId::new("merkle_proof_verify", count),
            &(leaf, proof, root),
            |b, (l, p, r)| b.iter(|| black_box(verify_proof(l, p, r))),
        );
    }

    group.finish();
}

// ============================================================================
// QC-04: State Management Benchmarks
// SPEC-04 Claim: State root computation < 10 seconds under normal load
// ============================================================================

fn bench_patricia_trie_operations(c: &mut Criterion) {
    use std::collections::HashMap;

    let mut group = c.benchmark_group("qc-04-state-trie");
    group.measurement_time(Duration::from_secs(15));

    // Simple Patricia trie for benchmarking
    fn compute_state_root(accounts: &HashMap<[u8; 20], ([u8; 32], u64)>) -> [u8; 32] {
        // Simplified: just hash all accounts together
        let mut hasher = Keccak256::new();
        let mut sorted: Vec<_> = accounts.iter().collect();
        sorted.sort_by_key(|(addr, _)| *addr);

        for (addr, (balance, nonce)) in sorted {
            hasher.update(addr);
            hasher.update(balance);
            hasher.update(nonce.to_le_bytes());
        }

        let mut result = [0u8; 32];
        result.copy_from_slice(&hasher.finalize());
        result
    }

    fn generate_accounts(count: usize) -> HashMap<[u8; 20], ([u8; 32], u64)> {
        let mut rng = rand::thread_rng();
        (0..count)
            .map(|_| {
                let mut addr = [0u8; 20];
                let mut balance = [0u8; 32];
                rng.fill(&mut addr);
                rng.fill(&mut balance);
                let nonce = rng.gen::<u64>();
                (addr, (balance, nonce))
            })
            .collect()
    }

    let account_counts = [1000, 10000, 50000, 100000];
    for count in account_counts {
        let accounts = generate_accounts(count);

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("state_root_compute", count),
            &accounts,
            |b, a| b.iter(|| black_box(compute_state_root(a))),
        );
    }

    // Benchmark incremental updates (what happens in real blocks)
    group.bench_function("state_incremental_update_1000_changes", |b| {
        let mut accounts = generate_accounts(100000);
        let mut rng = rand::thread_rng();

        b.iter(|| {
            // Simulate 1000 account changes (typical block)
            for _ in 0..1000 {
                let mut addr = [0u8; 20];
                let mut balance = [0u8; 32];
                rng.fill(&mut addr);
                rng.fill(&mut balance);
                accounts.insert(addr, (balance, rng.gen()));
            }
            black_box(compute_state_root(&accounts))
        })
    });

    group.finish();
}

// ============================================================================
// QC-06: Mempool Benchmarks
// SPEC-06 Claim: Two-phase commit, O(1) tx lookup, priority ordering
// ============================================================================

fn bench_mempool_operations(c: &mut Criterion) {
    use std::collections::{BTreeSet, HashMap};

    let mut group = c.benchmark_group("qc-06-mempool");
    group.measurement_time(Duration::from_secs(10));

    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq)]
    struct PricedTx {
        gas_price: u64,
        hash: [u8; 32],
    }

    struct SimplifiedMempool {
        by_hash: HashMap<[u8; 32], (u64, u64)>, // hash -> (gas_price, nonce)
        by_price: BTreeSet<PricedTx>,
    }

    impl SimplifiedMempool {
        fn new() -> Self {
            Self {
                by_hash: HashMap::new(),
                by_price: BTreeSet::new(),
            }
        }

        fn add(&mut self, hash: [u8; 32], gas_price: u64, nonce: u64) {
            self.by_hash.insert(hash, (gas_price, nonce));
            self.by_price.insert(PricedTx { gas_price, hash });
        }

        fn get(&self, hash: &[u8; 32]) -> Option<&(u64, u64)> {
            self.by_hash.get(hash)
        }

        fn get_top_n(&self, n: usize) -> Vec<[u8; 32]> {
            self.by_price.iter().rev().take(n).map(|t| t.hash).collect()
        }

        #[allow(dead_code)]
        fn remove(&mut self, hash: &[u8; 32]) -> bool {
            if let Some((gas_price, _)) = self.by_hash.remove(hash) {
                self.by_price.remove(&PricedTx {
                    gas_price,
                    hash: *hash,
                });
                true
            } else {
                false
            }
        }
    }

    // Benchmark transaction addition
    let tx_counts = [100, 1000, 5000, 10000];
    for count in tx_counts {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("mempool_add_txs", count),
            &count,
            |b, &c| {
                b.iter(|| {
                    let mut pool = SimplifiedMempool::new();
                    let mut rng = rand::thread_rng();
                    for i in 0..c {
                        let mut hash = [0u8; 32];
                        rng.fill(&mut hash);
                        pool.add(hash, rng.gen_range(1..1000), i as u64);
                    }
                    black_box(pool)
                })
            },
        );
    }

    // Benchmark lookup (should be O(1))
    group.bench_function("mempool_lookup_single", |b| {
        let mut pool = SimplifiedMempool::new();
        let mut rng = rand::thread_rng();
        let mut hashes = Vec::new();

        for i in 0..10000 {
            let mut hash = [0u8; 32];
            rng.fill(&mut hash);
            hashes.push(hash);
            pool.add(hash, rng.gen_range(1..1000), i);
        }

        b.iter(|| {
            let idx = rng.gen_range(0..hashes.len());
            black_box(pool.get(&hashes[idx]))
        })
    });

    // Benchmark get top N (block building)
    for n in [100, 500, 1000, 3000] {
        let mut pool = SimplifiedMempool::new();
        let mut rng = rand::thread_rng();

        for i in 0..10000 {
            let mut hash = [0u8; 32];
            rng.fill(&mut hash);
            pool.add(hash, rng.gen_range(1..10000), i);
        }

        group.bench_with_input(
            BenchmarkId::new("mempool_get_top_n", n),
            &(pool, n),
            |b, (p, n)| b.iter(|| black_box(p.get_top_n(*n))),
        );
    }

    group.finish();
}

// ============================================================================
// QC-08: Consensus Benchmarks
// SPEC-08 Claim: Block validation, attestation aggregation
// ============================================================================

fn bench_consensus_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-08-consensus");
    group.measurement_time(Duration::from_secs(10));

    // Simulate block hash computation
    fn compute_block_hash(
        parent: &[u8; 32],
        tx_root: &[u8; 32],
        state_root: &[u8; 32],
        height: u64,
    ) -> [u8; 32] {
        let mut hasher = Keccak256::new();
        hasher.update(parent);
        hasher.update(tx_root);
        hasher.update(state_root);
        hasher.update(height.to_le_bytes());

        let mut result = [0u8; 32];
        result.copy_from_slice(&hasher.finalize());
        result
    }

    group.bench_function("block_hash_compute", |b| {
        let parent = [1u8; 32];
        let tx_root = [2u8; 32];
        let state_root = [3u8; 32];

        b.iter(|| black_box(compute_block_hash(&parent, &tx_root, &state_root, 12345)))
    });

    // Simulate attestation counting (PoS)
    fn count_attestations(attestations: &[[u8; 96]], validator_count: usize) -> f64 {
        // Simplified: just count unique attestations
        let unique = attestations.len();
        (unique as f64) / (validator_count as f64) * 100.0
    }

    let validator_counts = [100, 500, 1000, 10000];
    for count in validator_counts {
        let attestations: Vec<[u8; 96]> = (0..(count * 2 / 3))
            .map(|i| {
                let mut att = [0u8; 96];
                att[0..8].copy_from_slice(&(i as u64).to_le_bytes());
                att
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("attestation_count", count),
            &(attestations, count),
            |b, (att, vc)| b.iter(|| black_box(count_attestations(att, *vc))),
        );
    }

    group.finish();
}

// ============================================================================
// QC-02: Block Storage Benchmarks
// SPEC-02 Claim: O(1) height lookup, atomic writes
// ============================================================================

fn bench_block_storage_operations(c: &mut Criterion) {
    use std::collections::HashMap;

    let mut group = c.benchmark_group("qc-02-block-storage");
    group.measurement_time(Duration::from_secs(10));

    // Simulate in-memory block storage
    struct SimpleBlockStore {
        by_hash: HashMap<[u8; 32], Vec<u8>>,
        by_height: HashMap<u64, [u8; 32]>,
    }

    impl SimpleBlockStore {
        fn new() -> Self {
            Self {
                by_hash: HashMap::new(),
                by_height: HashMap::new(),
            }
        }

        fn write(&mut self, hash: [u8; 32], height: u64, data: Vec<u8>) {
            self.by_hash.insert(hash, data);
            self.by_height.insert(height, hash);
        }

        #[allow(dead_code)]
        fn get_by_hash(&self, hash: &[u8; 32]) -> Option<&Vec<u8>> {
            self.by_hash.get(hash)
        }

        fn get_by_height(&self, height: u64) -> Option<&Vec<u8>> {
            self.by_height
                .get(&height)
                .and_then(|hash| self.by_hash.get(hash))
        }
    }

    // Populate store
    fn populate_store(count: usize) -> SimpleBlockStore {
        let mut store = SimpleBlockStore::new();
        let mut rng = rand::thread_rng();

        for height in 0..count as u64 {
            let mut hash = [0u8; 32];
            rng.fill(&mut hash);
            let data = vec![0u8; 1000]; // 1KB block
            store.write(hash, height, data);
        }

        store
    }

    // Benchmark O(1) height lookup
    let block_counts = [1000, 10000, 100000];
    for count in block_counts {
        let store = populate_store(count);
        let mut rng = rand::thread_rng();

        group.bench_with_input(
            BenchmarkId::new("lookup_by_height", count),
            &store,
            |b, s| {
                b.iter(|| {
                    let height = rng.gen_range(0..count as u64);
                    black_box(s.get_by_height(height))
                })
            },
        );
    }

    // Benchmark write throughput
    group.bench_function("block_write_single", |b| {
        let mut store = SimpleBlockStore::new();
        let mut rng = rand::thread_rng();
        let mut height = 0u64;

        b.iter(|| {
            let mut hash = [0u8; 32];
            rng.fill(&mut hash);
            store.write(hash, height, vec![0u8; 1000]);
            height += 1;
            black_box(())
        })
    });

    group.finish();
}

// ============================================================================
// QC-01: Peer Discovery Benchmarks
// SPEC-01 Claim: Kademlia XOR distance, bucket operations
// ============================================================================

fn bench_peer_discovery_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-01-peer-discovery");
    group.measurement_time(Duration::from_secs(10));

    // XOR distance calculation
    fn xor_distance(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
        let mut result = [0u8; 32];
        for i in 0..32 {
            result[i] = a[i] ^ b[i];
        }
        result
    }

    fn leading_zeros(d: &[u8; 32]) -> u32 {
        let mut zeros = 0u32;
        for byte in d {
            if *byte == 0 {
                zeros += 8;
            } else {
                zeros += byte.leading_zeros();
                break;
            }
        }
        zeros
    }

    group.bench_function("xor_distance_compute", |b| {
        let a = [1u8; 32];
        let node_b = [2u8; 32];

        b.iter(|| black_box(xor_distance(&a, &node_b)))
    });

    group.bench_function("bucket_index_compute", |b| {
        let local = [1u8; 32];
        let remote = [2u8; 32];

        b.iter(|| {
            let dist = xor_distance(&local, &remote);
            black_box(256 - leading_zeros(&dist) as usize)
        })
    });

    // Benchmark finding closest peers
    fn find_closest(local: &[u8; 32], peers: &[[u8; 32]], k: usize) -> Vec<[u8; 32]> {
        let mut with_distance: Vec<_> =
            peers.iter().map(|p| (xor_distance(local, p), *p)).collect();
        with_distance.sort_by(|a, b| a.0.cmp(&b.0));
        with_distance.into_iter().take(k).map(|(_, p)| p).collect()
    }

    let peer_counts = [100, 500, 1000, 5000];
    for count in peer_counts {
        let mut rng = rand::thread_rng();
        let local = [1u8; 32];
        let peers: Vec<[u8; 32]> = (0..count)
            .map(|_| {
                let mut p = [0u8; 32];
                rng.fill(&mut p);
                p
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("find_closest_20", count),
            &peers,
            |b, p| b.iter(|| black_box(find_closest(&local, p, 20))),
        );
    }

    group.finish();
}

// ============================================================================
// Shared Types Security Benchmarks
// IPC security operations: HMAC, nonce validation
// ============================================================================

fn bench_security_operations(c: &mut Criterion) {
    use sha3::Sha3_256;

    let mut group = c.benchmark_group("shared-types-security");
    group.measurement_time(Duration::from_secs(10));

    // Simulate HMAC computation
    fn compute_hmac(key: &[u8; 32], message: &[u8]) -> [u8; 32] {
        let mut hasher = Sha3_256::new();
        hasher.update(key);
        hasher.update(message);

        let mut result = [0u8; 32];
        result.copy_from_slice(&hasher.finalize());
        result
    }

    group.bench_function("hmac_compute_1kb", |b| {
        let key = [1u8; 32];
        let message = vec![0u8; 1024];

        b.iter(|| black_box(compute_hmac(&key, &message)))
    });

    group.bench_function("hmac_compute_10kb", |b| {
        let key = [1u8; 32];
        let message = vec![0u8; 10240];

        b.iter(|| black_box(compute_hmac(&key, &message)))
    });

    // Nonce cache simulation
    use std::collections::HashSet;

    fn check_nonce(cache: &mut HashSet<[u8; 16]>, nonce: [u8; 16]) -> bool {
        cache.insert(nonce)
    }

    group.bench_function("nonce_check", |b| {
        let mut cache = HashSet::with_capacity(10000);
        let mut rng = rand::thread_rng();

        b.iter(|| {
            let mut nonce = [0u8; 16];
            rng.fill(&mut nonce);
            black_box(check_nonce(&mut cache, nonce))
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_ecdsa_signature_verification,
    bench_merkle_tree_operations,
    bench_patricia_trie_operations,
    bench_mempool_operations,
    bench_consensus_operations,
    bench_block_storage_operations,
    bench_peer_discovery_operations,
    bench_security_operations,
);

criterion_main!(benches);
