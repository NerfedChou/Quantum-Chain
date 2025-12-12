//! # QC-04 State Management Brutal Benchmarks
//!
//! SPEC-04 Performance Claims to Validate:
//! - State root computation: < 10 seconds under normal load
//! - Account lookup: O(log n) via Patricia trie
//! - Proof generation: O(log n) path length
//! - Storage slot access: O(1) within account
//! - Incremental update: proportional to changes only
//!
//! Brutal Conditions:
//! - 1M+ accounts in state trie
//! - 10,000+ storage slots per contract
//! - Concurrent state access
//! - Adversarial state bloat attacks
//! - Deep contract nesting

use criterion::{black_box, BenchmarkId, Criterion, Throughput};
use rand::Rng;
use sha3::{Digest, Keccak256};
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

/// Simulated account state (per SPEC-04)
#[derive(Clone)]
struct BrutalAccount {
    balance: [u8; 32],
    nonce: u64,
    code_hash: [u8; 32],
    storage_root: [u8; 32],
    storage: BTreeMap<[u8; 32], [u8; 32]>, // slot -> value
}

impl BrutalAccount {
    fn new() -> Self {
        Self {
            balance: [0u8; 32],
            nonce: 0,
            code_hash: [0u8; 32],
            storage_root: [0u8; 32],
            storage: BTreeMap::new(),
        }
    }

    fn with_storage(slot_count: usize) -> Self {
        let mut rng = rand::thread_rng();
        let mut account = Self::new();

        for _ in 0..slot_count {
            let mut slot = [0u8; 32];
            let mut value = [0u8; 32];
            rng.fill(&mut slot);
            rng.fill(&mut value);
            account.storage.insert(slot, value);
        }

        account.update_storage_root();
        account
    }

    fn update_storage_root(&mut self) {
        let mut hasher = Keccak256::new();
        for (slot, value) in &self.storage {
            hasher.update(slot);
            hasher.update(value);
        }
        self.storage_root.copy_from_slice(&hasher.finalize());
    }

    fn hash(&self) -> [u8; 32] {
        let mut hasher = Keccak256::new();
        hasher.update(&self.balance);
        hasher.update(&self.nonce.to_le_bytes());
        hasher.update(&self.code_hash);
        hasher.update(&self.storage_root);

        let mut result = [0u8; 32];
        result.copy_from_slice(&hasher.finalize());
        result
    }
}

/// Simulated state trie
struct BrutalStateTrie {
    accounts: HashMap<[u8; 20], BrutalAccount>,
    cached_root: Option<[u8; 32]>,
}

impl BrutalStateTrie {
    fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            cached_root: None,
        }
    }

    fn insert_account(&mut self, address: [u8; 20], account: BrutalAccount) {
        self.accounts.insert(address, account);
        self.cached_root = None; // Invalidate cache
    }

    fn get_account(&self, address: &[u8; 20]) -> Option<&BrutalAccount> {
        self.accounts.get(address)
    }

    fn update_storage(&mut self, address: &[u8; 20], slot: [u8; 32], value: [u8; 32]) -> bool {
        if let Some(account) = self.accounts.get_mut(address) {
            account.storage.insert(slot, value);
            account.update_storage_root();
            self.cached_root = None;
            true
        } else {
            false
        }
    }

    fn compute_root(&mut self) -> [u8; 32] {
        if let Some(root) = self.cached_root {
            return root;
        }

        // Compute state root (simplified - real impl uses Patricia trie)
        let mut hasher = Keccak256::new();

        // Sort addresses for determinism
        let mut sorted: Vec<_> = self.accounts.iter().collect();
        sorted.sort_by_key(|(addr, _)| *addr);

        for (address, account) in sorted {
            hasher.update(address);
            hasher.update(&account.hash());
        }

        let mut root = [0u8; 32];
        root.copy_from_slice(&hasher.finalize());
        self.cached_root = Some(root);
        root
    }

    fn generate_proof(&self, address: &[u8; 20]) -> Option<Vec<[u8; 32]>> {
        // Simplified proof - in real impl, this is Patricia path
        if !self.accounts.contains_key(address) {
            return None;
        }

        // Proof includes sibling hashes along path (simulated)
        let depth = 8; // Typical depth
        let proof: Vec<[u8; 32]> = (0..depth)
            .map(|i| {
                let mut h = [0u8; 32];
                h[0] = i as u8;
                h
            })
            .collect();

        Some(proof)
    }

    fn account_count(&self) -> usize {
        self.accounts.len()
    }
}

fn generate_addresses(count: usize) -> Vec<[u8; 20]> {
    let mut rng = rand::thread_rng();
    (0..count)
        .map(|_| {
            let mut addr = [0u8; 20];
            rng.fill(&mut addr);
            addr
        })
        .collect()
}

pub fn brutal_state_root(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-04/brutal/state_root");
    group.measurement_time(Duration::from_secs(30));

    // SPEC claim: < 10 seconds for normal load
    let account_counts = [1_000, 10_000, 100_000, 500_000];

    for count in account_counts {
        let addresses = generate_addresses(count);
        let mut trie = BrutalStateTrie::new();

        for addr in &addresses {
            trie.insert_account(*addr, BrutalAccount::new());
        }

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("compute_root_claim_10s", count),
            &trie,
            |b, t| {
                let mut trie = BrutalStateTrie::new();
                for addr in &addresses {
                    trie.insert_account(*addr, BrutalAccount::new());
                }
                b.iter(|| {
                    trie.cached_root = None; // Force recomputation
                    black_box(trie.compute_root())
                })
            },
        );
    }

    group.finish();
}

pub fn brutal_account_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-04/brutal/lookup");
    group.measurement_time(Duration::from_secs(10));

    // SPEC claim: O(log n) lookup
    let account_counts = [1_000, 10_000, 100_000, 1_000_000];

    for count in account_counts {
        let addresses = generate_addresses(count);
        let mut trie = BrutalStateTrie::new();

        for addr in &addresses {
            trie.insert_account(*addr, BrutalAccount::new());
        }

        let mut rng = rand::thread_rng();

        group.bench_with_input(
            BenchmarkId::new("lookup_account", count),
            &(trie, addresses),
            |b, (t, addrs)| {
                b.iter(|| {
                    let idx = rng.gen_range(0..addrs.len());
                    black_box(t.get_account(&addrs[idx]))
                })
            },
        );
    }

    group.finish();
}

pub fn brutal_storage_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-04/brutal/storage");
    group.measurement_time(Duration::from_secs(10));

    // SPEC claim: O(1) storage slot access within account
    let slot_counts = [100, 1_000, 10_000, 50_000];

    for count in slot_counts {
        let account = BrutalAccount::with_storage(count);
        let slots: Vec<[u8; 32]> = account.storage.keys().cloned().collect();
        let mut rng = rand::thread_rng();

        group.bench_with_input(
            BenchmarkId::new("storage_read", count),
            &(account, slots),
            |b, (acc, s)| {
                b.iter(|| {
                    let idx = rng.gen_range(0..s.len());
                    black_box(acc.storage.get(&s[idx]))
                })
            },
        );
    }

    // Brutal: storage update with root recomputation
    group.bench_function("storage_update_with_root", |b| {
        let mut rng = rand::thread_rng();
        let addr: [u8; 20] = rng.gen();

        b.iter(|| {
            let mut trie = BrutalStateTrie::new();
            let account = BrutalAccount::with_storage(100);
            trie.insert_account(addr, account);

            // Update 10 slots
            for _ in 0..10 {
                let slot: [u8; 32] = rng.gen();
                let value: [u8; 32] = rng.gen();
                trie.update_storage(&addr, slot, value);
            }

            black_box(trie.compute_root())
        })
    });

    group.finish();
}

pub fn brutal_proof_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-04/brutal/proof");
    group.measurement_time(Duration::from_secs(10));

    // SPEC claim: O(log n) proof generation
    let account_counts = [1_000, 10_000, 100_000];

    for count in account_counts {
        let addresses = generate_addresses(count);
        let mut trie = BrutalStateTrie::new();

        for addr in &addresses {
            trie.insert_account(*addr, BrutalAccount::new());
        }

        let mut rng = rand::thread_rng();

        group.bench_with_input(
            BenchmarkId::new("generate_proof", count),
            &(trie, addresses),
            |b, (t, addrs)| {
                b.iter(|| {
                    let idx = rng.gen_range(0..addrs.len());
                    black_box(t.generate_proof(&addrs[idx]))
                })
            },
        );
    }

    // Brutal: generate proofs for all accounts in block
    let block_account_count = 1000; // Accounts touched in block
    let addresses = generate_addresses(block_account_count);
    let mut trie = BrutalStateTrie::new();
    for addr in &addresses {
        trie.insert_account(*addr, BrutalAccount::new());
    }

    group.throughput(Throughput::Elements(block_account_count as u64));
    group.bench_function("generate_all_block_proofs", |b| {
        b.iter(|| {
            let proofs: Vec<_> = addresses
                .iter()
                .filter_map(|addr| trie.generate_proof(addr))
                .collect();
            black_box(proofs.len())
        })
    });

    group.finish();
}

pub fn brutal_incremental_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-04/brutal/incremental");
    group.measurement_time(Duration::from_secs(15));

    // SPEC claim: Incremental updates proportional to changes only
    let base_account_count = 100_000;
    let addresses = generate_addresses(base_account_count);

    // Pre-populate state
    let mut trie = BrutalStateTrie::new();
    for addr in &addresses {
        trie.insert_account(*addr, BrutalAccount::new());
    }
    trie.compute_root(); // Cache initial root

    let change_counts = [10, 100, 1000, 5000];
    let mut rng = rand::thread_rng();

    for changes in change_counts {
        group.throughput(Throughput::Elements(changes as u64));
        group.bench_with_input(
            BenchmarkId::new("incremental_update", changes),
            &changes,
            |b, &c| {
                b.iter(|| {
                    let mut trie = BrutalStateTrie::new();
                    for addr in &addresses {
                        trie.insert_account(*addr, BrutalAccount::new());
                    }
                    trie.compute_root();

                    // Apply changes
                    for _ in 0..c {
                        let idx = rng.gen_range(0..addresses.len());
                        let slot: [u8; 32] = rng.gen();
                        let value: [u8; 32] = rng.gen();
                        trie.update_storage(&addresses[idx], slot, value);
                    }

                    black_box(trie.compute_root())
                })
            },
        );
    }

    group.finish();
}

pub fn brutal_bloat_resistance(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-04/brutal/bloat");
    group.measurement_time(Duration::from_secs(10));

    // Brutal: adversarial state bloat attack
    // Attacker creates many accounts with maximum storage
    group.bench_function("resist_bloat_attack_1000_contracts", |b| {
        let mut rng = rand::thread_rng();

        b.iter(|| {
            let mut trie = BrutalStateTrie::new();

            // 1000 contracts, each with 1000 storage slots
            for _ in 0..1000 {
                let addr: [u8; 20] = rng.gen();
                let account = BrutalAccount::with_storage(1000);
                trie.insert_account(addr, account);
            }

            black_box(trie.compute_root())
        })
    });

    // Brutal: deep nesting simulation
    group.bench_function("deep_storage_nesting", |b| {
        let mut rng = rand::thread_rng();
        let addr: [u8; 20] = rng.gen();

        b.iter(|| {
            let mut trie = BrutalStateTrie::new();

            // Single contract with 50,000 storage slots (maximum depth)
            let account = BrutalAccount::with_storage(50_000);
            trie.insert_account(addr, account);

            black_box(trie.compute_root())
        })
    });

    group.finish();
}

pub fn register_benchmarks(c: &mut Criterion) {
    brutal_state_root(c);
    brutal_account_lookup(c);
    brutal_storage_operations(c);
    brutal_proof_generation(c);
    brutal_incremental_update(c);
    brutal_bloat_resistance(c);
}
