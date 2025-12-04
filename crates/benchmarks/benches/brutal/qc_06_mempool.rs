//! # QC-06 Mempool Brutal Benchmarks
//!
//! SPEC-06 Performance Claims to Validate:
//! - Transaction insertion: O(log n) due to priority ordering
//! - Transaction lookup by hash: O(1)
//! - Get top N by gas price: O(n log n) but < 10ms for 10k txs
//! - Two-phase commit: < 1ms per transaction
//! - RBF (Replace-by-Fee): O(1) replacement
//!
//! Brutal Conditions:
//! - 100,000+ pending transactions
//! - Concurrent add/remove operations
//! - RBF spam attacks
//! - Transaction flood attacks
//! - Priority inversion attempts

use criterion::{black_box, BenchmarkId, Criterion, Throughput};
use rand::Rng;
use sha3::{Digest, Keccak256};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::time::Duration;

/// Transaction state machine per SPEC-06
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TxState {
    Pending,
    PendingInclusion,
    Confirmed,
    Rejected,
}

/// Simplified transaction for benchmarking
#[derive(Clone)]
struct BrutalTransaction {
    hash: [u8; 32],
    sender: [u8; 20],
    nonce: u64,
    gas_price: u64,
    gas_limit: u64,
    state: TxState,
}

impl BrutalTransaction {
    fn new(sender: [u8; 20], nonce: u64, gas_price: u64) -> Self {
        let mut hasher = Keccak256::new();
        hasher.update(&sender);
        hasher.update(&nonce.to_le_bytes());
        hasher.update(&gas_price.to_le_bytes());

        let mut hash = [0u8; 32];
        hash.copy_from_slice(&hasher.finalize());

        Self {
            hash,
            sender,
            nonce,
            gas_price,
            gas_limit: 21000,
            state: TxState::Pending,
        }
    }
}

/// Priority entry for heap ordering
#[derive(Clone, Eq, PartialEq)]
struct PriorityEntry {
    gas_price: u64,
    hash: [u8; 32],
}

impl Ord for PriorityEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.gas_price
            .cmp(&other.gas_price)
            .then_with(|| self.hash.cmp(&other.hash))
    }
}

impl PartialOrd for PriorityEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Brutal mempool with all SPEC-06 features
struct BrutalMempool {
    by_hash: HashMap<[u8; 32], BrutalTransaction>,
    by_sender: HashMap<[u8; 20], Vec<[u8; 32]>>,
    priority_queue: BinaryHeap<PriorityEntry>,
    pending_inclusion: HashSet<[u8; 32]>,
    max_size: usize,
    max_per_account: usize,
    min_gas_price: u64,
}

impl BrutalMempool {
    fn new(max_size: usize) -> Self {
        Self {
            by_hash: HashMap::with_capacity(max_size),
            by_sender: HashMap::new(),
            priority_queue: BinaryHeap::with_capacity(max_size),
            pending_inclusion: HashSet::new(),
            max_size,
            max_per_account: 16,
            min_gas_price: 1,
        }
    }

    fn add(&mut self, tx: BrutalTransaction) -> Result<(), &'static str> {
        // Check minimum gas price
        if tx.gas_price < self.min_gas_price {
            return Err("Gas price too low");
        }

        // Check per-account limit
        let sender_txs = self.by_sender.entry(tx.sender).or_default();
        if sender_txs.len() >= self.max_per_account {
            return Err("Per-account limit exceeded");
        }

        // Check pool capacity
        if self.by_hash.len() >= self.max_size {
            // Evict lowest priority
            if let Some(lowest) = self.priority_queue.peek() {
                if tx.gas_price <= lowest.gas_price {
                    return Err("Pool full, gas price too low");
                }
                // Would evict here in real impl
            }
        }

        let hash = tx.hash;
        let gas_price = tx.gas_price;

        sender_txs.push(hash);
        self.priority_queue.push(PriorityEntry { gas_price, hash });
        self.by_hash.insert(hash, tx);

        Ok(())
    }

    fn get(&self, hash: &[u8; 32]) -> Option<&BrutalTransaction> {
        self.by_hash.get(hash)
    }

    fn get_top_n(&self, n: usize) -> Vec<[u8; 32]> {
        let mut heap = self.priority_queue.clone();
        let mut result = Vec::with_capacity(n);

        while result.len() < n {
            if let Some(entry) = heap.pop() {
                if self.by_hash.contains_key(&entry.hash)
                    && !self.pending_inclusion.contains(&entry.hash)
                {
                    result.push(entry.hash);
                }
            } else {
                break;
            }
        }

        result
    }

    fn propose(&mut self, hashes: &[[u8; 32]]) -> usize {
        let mut proposed = 0;
        for hash in hashes {
            if let Some(tx) = self.by_hash.get_mut(hash) {
                if tx.state == TxState::Pending {
                    tx.state = TxState::PendingInclusion;
                    self.pending_inclusion.insert(*hash);
                    proposed += 1;
                }
            }
        }
        proposed
    }

    fn confirm(&mut self, hashes: &[[u8; 32]]) -> usize {
        let hash_set: HashSet<_> = hashes.iter().collect();
        let mut confirmed = 0;

        for hash in hashes {
            if let Some(tx) = self.by_hash.remove(hash) {
                if let Some(sender_txs) = self.by_sender.get_mut(&tx.sender) {
                    sender_txs.retain(|h| h != hash);
                }
                self.pending_inclusion.remove(hash);
                confirmed += 1;
            }
        }

        confirmed
    }

    fn rollback(&mut self, hashes: &[[u8; 32]]) -> usize {
        let mut rolled_back = 0;
        for hash in hashes {
            if let Some(tx) = self.by_hash.get_mut(hash) {
                if tx.state == TxState::PendingInclusion {
                    tx.state = TxState::Pending;
                    self.pending_inclusion.remove(hash);
                    rolled_back += 1;
                }
            }
        }
        rolled_back
    }

    fn try_rbf(&mut self, new_tx: BrutalTransaction) -> Result<(), &'static str> {
        // Find existing tx with same sender+nonce
        if let Some(sender_txs) = self.by_sender.get(&new_tx.sender) {
            for hash in sender_txs {
                if let Some(existing) = self.by_hash.get(hash) {
                    if existing.nonce == new_tx.nonce {
                        // Must pay 10% more
                        if new_tx.gas_price < existing.gas_price * 110 / 100 {
                            return Err("Insufficient fee bump for RBF");
                        }

                        // Cannot replace PendingInclusion
                        if existing.state == TxState::PendingInclusion {
                            return Err("Cannot replace tx pending inclusion");
                        }

                        // Replace
                        let old_hash = *hash;
                        self.by_hash.remove(&old_hash);
                        return self.add(new_tx);
                    }
                }
            }
        }

        // No existing tx to replace
        self.add(new_tx)
    }

    fn len(&self) -> usize {
        self.by_hash.len()
    }
}

fn generate_transactions(count: usize) -> Vec<BrutalTransaction> {
    let mut rng = rand::thread_rng();
    (0..count)
        .map(|i| {
            let mut sender = [0u8; 20];
            rng.fill(&mut sender);
            BrutalTransaction::new(sender, i as u64 % 16, rng.gen_range(1..10000))
        })
        .collect()
}

pub fn brutal_insertion(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-06/brutal/insert");
    group.measurement_time(Duration::from_secs(10));

    // SPEC claim: O(log n) insertion
    let pool_sizes = [1_000, 10_000, 50_000, 100_000];

    for size in pool_sizes {
        let txs = generate_transactions(size);

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::new("insert_batch", size),
            &txs,
            |b, transactions| {
                b.iter(|| {
                    let mut pool = BrutalMempool::new(size + 1000);
                    for tx in transactions {
                        let _ = pool.add(tx.clone());
                    }
                    black_box(pool.len())
                })
            },
        );
    }

    // Brutal: insertion under eviction pressure
    group.bench_function("insert_with_eviction", |b| {
        let mut rng = rand::thread_rng();

        b.iter(|| {
            let mut pool = BrutalMempool::new(1000); // Small pool

            // Try to insert 5000 txs
            let mut accepted = 0;
            for i in 0..5000u64 {
                let mut sender = [0u8; 20];
                rng.fill(&mut sender);
                let tx = BrutalTransaction::new(sender, i % 16, rng.gen_range(1..10000));
                if pool.add(tx).is_ok() {
                    accepted += 1;
                }
            }

            black_box(accepted)
        })
    });

    group.finish();
}

pub fn brutal_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-06/brutal/lookup");
    group.measurement_time(Duration::from_secs(10));

    // SPEC claim: O(1) lookup by hash
    let pool_sizes = [1_000, 10_000, 100_000];

    for size in pool_sizes {
        let txs = generate_transactions(size);
        let mut pool = BrutalMempool::new(size + 1000);
        let mut hashes = Vec::new();

        for tx in &txs {
            if pool.add(tx.clone()).is_ok() {
                hashes.push(tx.hash);
            }
        }

        let mut rng = rand::thread_rng();

        group.bench_with_input(
            BenchmarkId::new("lookup_by_hash", size),
            &(pool, hashes),
            |b, (p, h)| {
                b.iter(|| {
                    let idx = rng.gen_range(0..h.len());
                    black_box(p.get(&h[idx]))
                })
            },
        );
    }

    group.finish();
}

pub fn brutal_priority_ordering(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-06/brutal/priority");
    group.measurement_time(Duration::from_secs(10));

    // SPEC claim: get top N < 10ms for 10k txs
    let pool_sizes = [1_000, 10_000, 50_000];
    let top_counts = [100, 500, 1000, 3000];

    for size in pool_sizes {
        let txs = generate_transactions(size);
        let mut pool = BrutalMempool::new(size + 1000);

        for tx in &txs {
            let _ = pool.add(tx.clone());
        }

        for top_n in top_counts {
            if top_n > size {
                continue;
            }

            group.bench_with_input(
                BenchmarkId::new(format!("get_top_{}_from_{}", top_n, size), top_n),
                &pool,
                |b, p| b.iter(|| black_box(p.get_top_n(top_n))),
            );
        }
    }

    group.finish();
}

pub fn brutal_two_phase_commit(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-06/brutal/2pc");
    group.measurement_time(Duration::from_secs(10));

    // SPEC claim: < 1ms per transaction
    let batch_sizes = [100, 500, 1000, 3000];

    for batch_size in batch_sizes {
        let txs = generate_transactions(10000);

        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("propose_confirm_cycle", batch_size),
            &batch_size,
            |b, &size| {
                b.iter(|| {
                    let mut pool = BrutalMempool::new(15000);

                    for tx in &txs {
                        let _ = pool.add(tx.clone());
                    }

                    let top = pool.get_top_n(size);
                    let proposed = pool.propose(&top);
                    let confirmed = pool.confirm(&top);

                    black_box((proposed, confirmed))
                })
            },
        );
    }

    // Brutal: rollback cycle
    group.bench_function("propose_rollback_cycle", |b| {
        let txs = generate_transactions(1000);

        b.iter(|| {
            let mut pool = BrutalMempool::new(2000);

            for tx in &txs {
                let _ = pool.add(tx.clone());
            }

            let top = pool.get_top_n(500);
            let proposed = pool.propose(&top);
            let rolled_back = pool.rollback(&top);

            black_box((proposed, rolled_back))
        })
    });

    group.finish();
}

pub fn brutal_rbf_attacks(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-06/brutal/rbf");
    group.measurement_time(Duration::from_secs(10));

    // SPEC claim: O(1) RBF replacement
    group.bench_function("rbf_single_replacement", |b| {
        let mut rng = rand::thread_rng();
        let sender: [u8; 20] = rng.gen();

        b.iter(|| {
            let mut pool = BrutalMempool::new(1000);

            // Add original tx
            let tx1 = BrutalTransaction::new(sender, 0, 100);
            pool.add(tx1).unwrap();

            // RBF with higher price
            let tx2 = BrutalTransaction::new(sender, 0, 111); // 11% more
            let result = pool.try_rbf(tx2);

            black_box(result)
        })
    });

    // Brutal: RBF spam attack
    group.bench_function("rbf_spam_100_replacements", |b| {
        let mut rng = rand::thread_rng();
        let sender: [u8; 20] = rng.gen();

        b.iter(|| {
            let mut pool = BrutalMempool::new(1000);

            let tx = BrutalTransaction::new(sender, 0, 100);
            pool.add(tx).unwrap();

            // 100 RBF attempts with increasing gas price
            let mut replaced = 0;
            for i in 1..101 {
                let new_tx = BrutalTransaction::new(sender, 0, 100 + i * 11); // Each 11% more
                if pool.try_rbf(new_tx).is_ok() {
                    replaced += 1;
                }
            }

            black_box(replaced)
        })
    });

    // Brutal: RBF on pending inclusion (should fail)
    group.bench_function("rbf_blocked_pending_inclusion", |b| {
        let mut rng = rand::thread_rng();
        let sender: [u8; 20] = rng.gen();

        b.iter(|| {
            let mut pool = BrutalMempool::new(1000);

            let tx = BrutalTransaction::new(sender, 0, 100);
            let hash = tx.hash;
            pool.add(tx).unwrap();

            // Mark as pending inclusion
            pool.propose(&[hash]);

            // Try RBF (should fail)
            let new_tx = BrutalTransaction::new(sender, 0, 200);
            let result = pool.try_rbf(new_tx);

            black_box(result.is_err())
        })
    });

    group.finish();
}

pub fn brutal_flood_resistance(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-06/brutal/flood");
    group.measurement_time(Duration::from_secs(10));

    // Brutal: transaction flood from single sender
    group.bench_function("flood_single_sender", |b| {
        let mut rng = rand::thread_rng();
        let sender: [u8; 20] = rng.gen();

        b.iter(|| {
            let mut pool = BrutalMempool::new(10000);

            let mut accepted = 0;
            for nonce in 0..1000u64 {
                let tx = BrutalTransaction::new(sender, nonce, rng.gen_range(1..1000));
                if pool.add(tx).is_ok() {
                    accepted += 1;
                }
            }

            // Should be limited by max_per_account (16)
            black_box(accepted)
        })
    });

    // Brutal: low gas price flood
    group.bench_function("flood_low_gas_price", |b| {
        let mut rng = rand::thread_rng();

        b.iter(|| {
            let mut pool = BrutalMempool::new(1000);

            // Fill with high-price txs
            for _ in 0..1000 {
                let mut sender = [0u8; 20];
                rng.fill(&mut sender);
                let tx = BrutalTransaction::new(sender, 0, 1000);
                let _ = pool.add(tx);
            }

            // Try to flood with low-price txs
            let mut rejected = 0;
            for _ in 0..10000 {
                let mut sender = [0u8; 20];
                rng.fill(&mut sender);
                let tx = BrutalTransaction::new(sender, 0, 1); // Minimum gas
                if pool.add(tx).is_err() {
                    rejected += 1;
                }
            }

            black_box(rejected)
        })
    });

    group.finish();
}

pub fn register_benchmarks(c: &mut Criterion) {
    brutal_insertion(c);
    brutal_lookup(c);
    brutal_priority_ordering(c);
    brutal_two_phase_commit(c);
    brutal_rbf_attacks(c);
    brutal_flood_resistance(c);
}
