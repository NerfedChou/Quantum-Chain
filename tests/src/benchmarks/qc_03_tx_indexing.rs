//! # QC-03 Transaction Indexing Brutal Benchmarks
//!
//! SPEC-03 Performance Claims to Validate:
//! - Merkle tree construction: O(n) for n transactions
//! - Proof generation: O(log n)
//! - Proof verification: O(log n)
//! - Power-of-two padding: constant overhead
//! - LRU cache hit: < 100ns
//!
//! Brutal Conditions:
//! - 10,000+ transactions per block
//! - Maximum proof depth (log2(65536) = 16 levels)
//! - Cache eviction under pressure
//! - Adversarial transaction ordering
//! - Concurrent proof requests

use criterion::{black_box, BenchmarkId, Criterion, Throughput};
use rand::Rng;
use sha3::{Digest, Keccak256};
use std::collections::HashMap;
use std::time::Duration;

/// Merkle tree with power-of-two padding (per INVARIANT-1)
struct BrutalMerkleTree {
    leaves: Vec<[u8; 32]>,
    tree: Vec<[u8; 32]>,
    leaf_count: usize,
}

impl BrutalMerkleTree {
    fn new(tx_hashes: &[[u8; 32]]) -> Self {
        let leaf_count = tx_hashes.len();
        if leaf_count == 0 {
            return Self {
                leaves: vec![],
                tree: vec![[0u8; 32]],
                leaf_count: 0,
            };
        }

        // INVARIANT-1: Power-of-two padding
        let padded_size = leaf_count.next_power_of_two();
        let mut leaves = tx_hashes.to_vec();
        leaves.resize(padded_size, [0u8; 32]); // Pad with zeros

        // Build tree
        let tree_size = 2 * padded_size;
        let mut tree = vec![[0u8; 32]; tree_size];

        // Copy leaves to second half
        tree[padded_size..].copy_from_slice(&leaves);

        // Build internal nodes bottom-up
        for i in (1..padded_size).rev() {
            let mut hasher = Keccak256::new();
            hasher.update(tree[2 * i]);
            hasher.update(tree[2 * i + 1]);
            tree[i].copy_from_slice(&hasher.finalize());
        }

        Self {
            leaves,
            tree,
            leaf_count,
        }
    }

    fn root(&self) -> [u8; 32] {
        if self.tree.len() > 1 {
            self.tree[1]
        } else {
            [0u8; 32]
        }
    }

    fn generate_proof(&self, index: usize) -> Option<Vec<([u8; 32], bool)>> {
        if index >= self.leaf_count {
            return None;
        }

        let padded_size = self.leaves.len();
        let mut proof = Vec::with_capacity(16); // Max depth for 65536 txs
        let mut idx = padded_size + index;

        while idx > 1 {
            let sibling = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
            let is_left = idx % 2 == 1;
            proof.push((self.tree[sibling], is_left));
            idx /= 2;
        }

        Some(proof)
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
}

/// LRU cache for Merkle trees
struct BrutalMerkleCache {
    cache: HashMap<[u8; 32], BrutalMerkleTree>,
    order: Vec<[u8; 32]>,
    capacity: usize,
}

impl BrutalMerkleCache {
    fn new(capacity: usize) -> Self {
        Self {
            cache: HashMap::with_capacity(capacity),
            order: Vec::with_capacity(capacity),
            capacity,
        }
    }

    fn get(&mut self, block_hash: &[u8; 32]) -> Option<&BrutalMerkleTree> {
        if self.cache.contains_key(block_hash) {
            // Move to front (LRU update)
            if let Some(pos) = self.order.iter().position(|h| h == block_hash) {
                self.order.remove(pos);
                self.order.push(*block_hash);
            }
            self.cache.get(block_hash)
        } else {
            None
        }
    }

    fn insert(&mut self, block_hash: [u8; 32], tree: BrutalMerkleTree) {
        if self.cache.len() >= self.capacity {
            // Evict oldest
            if let Some(oldest) = self.order.first().copied() {
                self.cache.remove(&oldest);
                self.order.remove(0);
            }
        }

        self.cache.insert(block_hash, tree);
        self.order.push(block_hash);
    }

    fn len(&self) -> usize {
        self.cache.len()
    }
}

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

pub fn brutal_tree_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-03/brutal/tree_build");
    group.measurement_time(Duration::from_secs(15));

    // SPEC claim: O(n) construction
    let tx_counts = [100, 500, 1000, 5000, 10000, 50000];

    for count in tx_counts {
        let hashes = generate_tx_hashes(count);

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("construct_tree", count),
            &hashes,
            |b, h| b.iter(|| black_box(BrutalMerkleTree::new(h))),
        );
    }

    // Brutal: worst-case padding (2^n + 1 txs)
    let adversarial_counts = [129, 1025, 4097, 16385]; // Forces doubling

    for count in adversarial_counts {
        let hashes = generate_tx_hashes(count);

        group.bench_with_input(
            BenchmarkId::new("construct_adversarial_padding", count),
            &hashes,
            |b, h| b.iter(|| black_box(BrutalMerkleTree::new(h))),
        );
    }

    group.finish();
}

pub fn brutal_proof_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-03/brutal/proof_gen");
    group.measurement_time(Duration::from_secs(10));

    // SPEC claim: O(log n) proof generation
    let tx_counts = [100, 1000, 10000, 65536];

    for count in tx_counts {
        let hashes = generate_tx_hashes(count);
        let tree = BrutalMerkleTree::new(&hashes);
        let mut rng = rand::thread_rng();

        // Proof depth should be log2(count)
        let expected_depth = (count as f64).log2().ceil() as usize;

        group.bench_with_input(
            BenchmarkId::new(format!("proof_depth_{}", expected_depth), count),
            &tree,
            |b, t| {
                b.iter(|| {
                    let idx = rng.gen_range(0..count);
                    black_box(t.generate_proof(idx))
                })
            },
        );
    }

    // Brutal: generate all proofs for a block
    let block_tx_count = 3000; // Realistic block
    let hashes = generate_tx_hashes(block_tx_count);
    let tree = BrutalMerkleTree::new(&hashes);

    group.throughput(Throughput::Elements(block_tx_count as u64));
    group.bench_function("generate_all_3000_proofs", |b| {
        b.iter(|| {
            let proofs: Vec<_> = (0..block_tx_count)
                .filter_map(|i| tree.generate_proof(i))
                .collect();
            black_box(proofs.len())
        })
    });

    group.finish();
}

pub fn brutal_proof_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-03/brutal/proof_verify");
    group.measurement_time(Duration::from_secs(10));

    // SPEC claim: O(log n) verification
    let tx_counts = [100, 1000, 10000, 65536];

    for count in tx_counts {
        let hashes = generate_tx_hashes(count);
        let tree = BrutalMerkleTree::new(&hashes);
        let root = tree.root();
        let proof = tree.generate_proof(0).unwrap();
        let leaf = hashes[0];

        let depth = proof.len();

        group.bench_with_input(
            BenchmarkId::new(format!("verify_depth_{}", depth), count),
            &(leaf, proof, root),
            |b, (l, p, r)| b.iter(|| black_box(BrutalMerkleTree::verify_proof(l, p, r))),
        );
    }

    // Brutal: verify batch of proofs
    let hashes = generate_tx_hashes(1000);
    let tree = BrutalMerkleTree::new(&hashes);
    let root = tree.root();
    let proofs: Vec<_> = (0..1000)
        .map(|i| (hashes[i], tree.generate_proof(i).unwrap()))
        .collect();

    group.throughput(Throughput::Elements(1000));
    group.bench_function("verify_batch_1000", |b| {
        b.iter(|| {
            let valid_count: usize = proofs
                .iter()
                .filter(|(leaf, proof)| BrutalMerkleTree::verify_proof(leaf, proof, &root))
                .count();
            black_box(valid_count)
        })
    });

    group.finish();
}

pub fn brutal_cache_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-03/brutal/cache");
    group.measurement_time(Duration::from_secs(10));

    // SPEC claim: cache hit < 100ns
    let cache_sizes = [64, 256, 1024];

    for size in cache_sizes {
        let mut cache = BrutalMerkleCache::new(size);
        let mut rng = rand::thread_rng();

        // Pre-fill cache
        let mut hashes = Vec::new();
        for _ in 0..size {
            let block_hash: [u8; 32] = rng.gen();
            hashes.push(block_hash);
            let tx_hashes = generate_tx_hashes(100);
            let tree = BrutalMerkleTree::new(&tx_hashes);
            cache.insert(block_hash, tree);
        }

        group.bench_with_input(
            BenchmarkId::new("cache_hit_claim_100ns", size),
            &(cache, hashes),
            |b, (c, h)| {
                let mut cache = BrutalMerkleCache::new(size);
                // Re-populate (can't mutate in benchmark)
                for hash in h {
                    let tx_hashes = generate_tx_hashes(100);
                    let tree = BrutalMerkleTree::new(&tx_hashes);
                    cache.insert(*hash, tree);
                }

                b.iter(|| {
                    let idx = rng.gen_range(0..h.len());
                    black_box(cache.get(&h[idx]).is_some())
                })
            },
        );
    }

    // Brutal: cache under eviction pressure
    group.bench_function("cache_eviction_pressure", |b| {
        let mut rng = rand::thread_rng();

        b.iter(|| {
            let mut cache = BrutalMerkleCache::new(100);

            // Insert 1000 items into cache of 100
            for _ in 0..1000 {
                let block_hash: [u8; 32] = rng.gen();
                let tx_hashes = generate_tx_hashes(50);
                let tree = BrutalMerkleTree::new(&tx_hashes);
                cache.insert(block_hash, tree);
            }

            black_box(cache.len())
        })
    });

    group.finish();
}

pub fn brutal_determinism(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-03/brutal/determinism");
    group.measurement_time(Duration::from_secs(5));

    // INVARIANT-3: Canonical serialization produces same hash
    group.bench_function("deterministic_root_1000_txs", |b| {
        // Fixed seed for reproducibility
        let hashes: Vec<[u8; 32]> = (0..1000u64)
            .map(|i| {
                let mut h = [0u8; 32];
                h[..8].copy_from_slice(&i.to_le_bytes());
                h
            })
            .collect();

        let expected_root = BrutalMerkleTree::new(&hashes).root();

        b.iter(|| {
            let tree = BrutalMerkleTree::new(&hashes);
            assert_eq!(tree.root(), expected_root, "INVARIANT-3 violated!");
            black_box(tree.root())
        })
    });

    group.finish();
}

pub fn register_benchmarks(c: &mut Criterion) {
    brutal_tree_construction(c);
    brutal_proof_generation(c);
    brutal_proof_verification(c);
    brutal_cache_operations(c);
    brutal_determinism(c);
}
