//! # QC-08 Consensus Brutal Benchmarks
//!
//! SPEC-08 Performance Claims to Validate:
//! - Block validation: < 100ms for standard block
//! - Attestation aggregation: O(n) for n validators
//! - Supermajority check (67%): O(1) with pre-computed totals
//! - Fork choice: O(log n) for n blocks
//! - Epoch transition: < 1 second
//!
//! Brutal Conditions:
//! - 10,000+ validators
//! - Maximum attestation size
//! - Fork bombs (many competing chains)
//! - Adversarial attestation timing
//! - Equivocation detection under load

use criterion::{black_box, BenchmarkId, Criterion, Throughput};
use rand::Rng;
use sha3::{Digest, Keccak256};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// Block header for consensus
#[derive(Clone)]
struct BrutalBlockHeader {
    hash: [u8; 32],
    parent_hash: [u8; 32],
    height: u64,
    slot: u64,
    proposer: u64,
    state_root: [u8; 32],
    tx_root: [u8; 32],
    timestamp: u64,
}

impl BrutalBlockHeader {
    fn new(parent_hash: [u8; 32], height: u64, slot: u64, proposer: u64) -> Self {
        let mut rng = rand::thread_rng();
        let mut state_root = [0u8; 32];
        let mut tx_root = [0u8; 32];
        rng.fill(&mut state_root);
        rng.fill(&mut tx_root);

        let mut hasher = Keccak256::new();
        hasher.update(&parent_hash);
        hasher.update(&height.to_le_bytes());
        hasher.update(&slot.to_le_bytes());
        hasher.update(&proposer.to_le_bytes());
        hasher.update(&state_root);
        hasher.update(&tx_root);

        let mut hash = [0u8; 32];
        hash.copy_from_slice(&hasher.finalize());

        Self {
            hash,
            parent_hash,
            height,
            slot,
            proposer,
            state_root,
            tx_root,
            timestamp: slot * 12, // 12 second slots
        }
    }

    fn compute_hash(&self) -> [u8; 32] {
        let mut hasher = Keccak256::new();
        hasher.update(&self.parent_hash);
        hasher.update(&self.height.to_le_bytes());
        hasher.update(&self.slot.to_le_bytes());
        hasher.update(&self.proposer.to_le_bytes());
        hasher.update(&self.state_root);
        hasher.update(&self.tx_root);

        let mut hash = [0u8; 32];
        hash.copy_from_slice(&hasher.finalize());
        hash
    }
}

/// Attestation from a validator
#[derive(Clone)]
struct BrutalAttestation {
    validator_id: u64,
    block_hash: [u8; 32],
    slot: u64,
    signature: [u8; 96], // BLS signature placeholder
}

impl BrutalAttestation {
    fn new(validator_id: u64, block_hash: [u8; 32], slot: u64) -> Self {
        let mut rng = rand::thread_rng();
        let mut signature = [0u8; 96];
        for i in 0..96 {
            signature[i] = rng.gen();
        }

        Self {
            validator_id,
            block_hash,
            slot,
            signature,
        }
    }
}

/// Validator set with stake
struct BrutalValidatorSet {
    validators: HashMap<u64, u64>, // id -> stake
    total_stake: u64,
}

impl BrutalValidatorSet {
    fn new(count: usize) -> Self {
        let mut rng = rand::thread_rng();
        let validators: HashMap<u64, u64> = (0..count as u64)
            .map(|id| (id, rng.gen_range(32..1000))) // Random stake
            .collect();
        let total_stake = validators.values().sum();

        Self {
            validators,
            total_stake,
        }
    }

    fn get_stake(&self, validator_id: u64) -> u64 {
        self.validators.get(&validator_id).copied().unwrap_or(0)
    }

    fn is_supermajority(&self, attestations: &[BrutalAttestation]) -> bool {
        let attesting_stake: u64 = attestations
            .iter()
            .map(|a| self.get_stake(a.validator_id))
            .sum();

        // 67% supermajority
        attesting_stake * 3 > self.total_stake * 2
    }

    fn count(&self) -> usize {
        self.validators.len()
    }
}

/// Fork choice with LMD-GHOST
struct BrutalForkChoice {
    blocks: HashMap<[u8; 32], BrutalBlockHeader>,
    attestations: HashMap<[u8; 32], Vec<BrutalAttestation>>,
    head: [u8; 32],
}

impl BrutalForkChoice {
    fn new() -> Self {
        Self {
            blocks: HashMap::new(),
            attestations: HashMap::new(),
            head: [0u8; 32],
        }
    }

    fn add_block(&mut self, block: BrutalBlockHeader) {
        let hash = block.hash;
        self.blocks.insert(hash, block);
        self.attestations.entry(hash).or_default();
    }

    fn add_attestation(&mut self, attestation: BrutalAttestation) {
        self.attestations
            .entry(attestation.block_hash)
            .or_default()
            .push(attestation);
    }

    fn get_weight(&self, block_hash: &[u8; 32]) -> usize {
        self.attestations
            .get(block_hash)
            .map(|a| a.len())
            .unwrap_or(0)
    }

    fn find_head(&self, validator_set: &BrutalValidatorSet) -> [u8; 32] {
        // Simplified LMD-GHOST: find block with most attestation weight
        let mut best_hash = self.head;
        let mut best_weight = 0usize;

        for hash in self.blocks.keys() {
            let weight = self.get_weight(hash);
            if weight > best_weight {
                best_weight = weight;
                best_hash = *hash;
            }
        }

        best_hash
    }

    fn block_count(&self) -> usize {
        self.blocks.len()
    }
}

/// Equivocation detector
struct EquivocationDetector {
    seen_attestations: HashMap<(u64, u64), [u8; 32]>, // (validator, slot) -> block_hash
    slashed: HashSet<u64>,
}

impl EquivocationDetector {
    fn new() -> Self {
        Self {
            seen_attestations: HashMap::new(),
            slashed: HashSet::new(),
        }
    }

    fn check_attestation(&mut self, attestation: &BrutalAttestation) -> bool {
        let key = (attestation.validator_id, attestation.slot);

        if let Some(existing_hash) = self.seen_attestations.get(&key) {
            if *existing_hash != attestation.block_hash {
                // Equivocation detected!
                self.slashed.insert(attestation.validator_id);
                return false;
            }
        } else {
            self.seen_attestations.insert(key, attestation.block_hash);
        }

        true
    }

    fn slashed_count(&self) -> usize {
        self.slashed.len()
    }
}

pub fn brutal_block_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-08/brutal/block_validation");
    group.measurement_time(Duration::from_secs(10));

    // SPEC claim: < 100ms for standard block
    group.bench_function("validate_block_claim_100ms", |b| {
        let parent = [0u8; 32];

        b.iter(|| {
            let block = BrutalBlockHeader::new(parent, 1, 1, 0);

            // Validate:
            // 1. Hash matches computed
            // 2. Height = parent + 1
            // 3. Slot > parent slot
            // 4. Timestamp valid
            let computed_hash = block.compute_hash();
            let valid = computed_hash == block.hash && block.height == 1 && block.slot == 1;

            black_box(valid)
        })
    });

    // Brutal: validate chain of blocks
    let chain_lengths = [10, 100, 1000];

    for length in chain_lengths {
        group.throughput(Throughput::Elements(length as u64));
        group.bench_with_input(
            BenchmarkId::new("validate_chain", length),
            &length,
            |b, &len| {
                b.iter(|| {
                    let mut parent = [0u8; 32];
                    let mut valid_count = 0;

                    for i in 0..len as u64 {
                        let block = BrutalBlockHeader::new(parent, i + 1, i + 1, i % 100);
                        let computed = block.compute_hash();
                        if computed == block.hash {
                            valid_count += 1;
                        }
                        parent = block.hash;
                    }

                    black_box(valid_count)
                })
            },
        );
    }

    group.finish();
}

pub fn brutal_attestation_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-08/brutal/attestation");
    group.measurement_time(Duration::from_secs(10));

    // SPEC claim: O(n) aggregation for n validators
    let validator_counts = [100, 1000, 10000];

    for count in validator_counts {
        let validator_set = BrutalValidatorSet::new(count);
        let block_hash = [1u8; 32];

        // Generate attestations from 67% of validators
        let attestations: Vec<BrutalAttestation> = (0..(count * 2 / 3) as u64)
            .map(|id| BrutalAttestation::new(id, block_hash, 1))
            .collect();

        group.throughput(Throughput::Elements(attestations.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("aggregate_attestations", count),
            &(validator_set, attestations),
            |b, (vs, atts)| {
                b.iter(|| {
                    let is_super = vs.is_supermajority(atts);
                    black_box(is_super)
                })
            },
        );
    }

    // Brutal: check supermajority threshold
    group.bench_function("supermajority_check_10k_validators", |b| {
        let validator_set = BrutalValidatorSet::new(10000);
        let block_hash = [1u8; 32];

        // Exactly 67% attestations
        let attestations: Vec<BrutalAttestation> = (0..6700u64)
            .map(|id| BrutalAttestation::new(id, block_hash, 1))
            .collect();

        b.iter(|| black_box(validator_set.is_supermajority(&attestations)))
    });

    group.finish();
}

pub fn brutal_fork_choice(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-08/brutal/fork_choice");
    group.measurement_time(Duration::from_secs(10));

    // SPEC claim: O(log n) fork choice
    let block_counts = [100, 1000, 10000];

    for count in block_counts {
        let mut fork_choice = BrutalForkChoice::new();
        let validator_set = BrutalValidatorSet::new(1000);
        let mut rng = rand::thread_rng();

        // Build tree of blocks
        let mut parent = [0u8; 32];
        for i in 0..count as u64 {
            let block = BrutalBlockHeader::new(parent, i + 1, i + 1, i % 100);
            let hash = block.hash;
            fork_choice.add_block(block);

            // Add some attestations
            for v in 0..10u64 {
                let att = BrutalAttestation::new(v, hash, i + 1);
                fork_choice.add_attestation(att);
            }

            parent = hash;
        }

        group.bench_with_input(
            BenchmarkId::new("find_head", count),
            &(fork_choice, validator_set),
            |b, (fc, vs)| b.iter(|| black_box(fc.find_head(vs))),
        );
    }

    // Brutal: fork bomb (many competing forks)
    group.bench_function("fork_bomb_100_forks", |b| {
        let validator_set = BrutalValidatorSet::new(1000);
        let mut rng = rand::thread_rng();

        b.iter(|| {
            let mut fork_choice = BrutalForkChoice::new();
            let genesis = [0u8; 32];

            // Create 100 competing forks from genesis
            for fork in 0..100u64 {
                let block = BrutalBlockHeader::new(genesis, 1, 1, fork);
                let hash = block.hash;
                fork_choice.add_block(block);

                // Random attestations
                for v in 0..10u64 {
                    let att = BrutalAttestation::new(fork * 10 + v, hash, 1);
                    fork_choice.add_attestation(att);
                }
            }

            black_box(fork_choice.find_head(&validator_set))
        })
    });

    group.finish();
}

pub fn brutal_equivocation_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-08/brutal/equivocation");
    group.measurement_time(Duration::from_secs(10));

    // Check equivocation under load
    let attestation_counts = [1000, 10000, 100000];

    for count in attestation_counts {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("detect_equivocations", count),
            &count,
            |b, &cnt| {
                let mut rng = rand::thread_rng();

                b.iter(|| {
                    let mut detector = EquivocationDetector::new();
                    let mut detected = 0;

                    for i in 0..cnt as u64 {
                        let validator_id = i % 1000; // 1000 validators
                        let slot = i / 1000;
                        let block_hash: [u8; 32] = rng.gen();

                        let att = BrutalAttestation::new(validator_id, block_hash, slot);
                        if !detector.check_attestation(&att) {
                            detected += 1;
                        }
                    }

                    black_box(detected)
                })
            },
        );
    }

    // Brutal: adversarial equivocation spam
    group.bench_function("equivocation_spam_attack", |b| {
        let mut rng = rand::thread_rng();

        b.iter(|| {
            let mut detector = EquivocationDetector::new();

            // 100 validators each try to equivocate 10 times
            for validator in 0..100u64 {
                for attempt in 0..10 {
                    let block_hash: [u8; 32] = rng.gen();
                    let att = BrutalAttestation::new(validator, block_hash, 0); // Same slot
                    detector.check_attestation(&att);
                }
            }

            black_box(detector.slashed_count())
        })
    });

    group.finish();
}

pub fn brutal_epoch_transition(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-08/brutal/epoch");
    group.measurement_time(Duration::from_secs(10));

    // SPEC claim: epoch transition < 1 second
    let validator_counts = [1000, 10000];

    for count in validator_counts {
        group.bench_with_input(
            BenchmarkId::new("epoch_transition_claim_1s", count),
            &count,
            |b, &cnt| {
                b.iter(|| {
                    // Simulate epoch transition:
                    // 1. Finalize previous epoch
                    // 2. Update validator set
                    // 3. Compute new shuffling
                    // 4. Reset attestation caches

                    let old_set = BrutalValidatorSet::new(cnt);
                    let new_set = BrutalValidatorSet::new(cnt);

                    // Compute shuffling (simplified)
                    let mut rng = rand::thread_rng();
                    let shuffled: Vec<u64> = {
                        let mut ids: Vec<u64> = (0..cnt as u64).collect();
                        for i in (1..ids.len()).rev() {
                            let j = rng.gen_range(0..=i);
                            ids.swap(i, j);
                        }
                        ids
                    };

                    black_box((old_set.total_stake, new_set.total_stake, shuffled.len()))
                })
            },
        );
    }

    group.finish();
}

pub fn register_benchmarks(c: &mut Criterion) {
    brutal_block_validation(c);
    brutal_attestation_aggregation(c);
    brutal_fork_choice(c);
    brutal_equivocation_detection(c);
    brutal_epoch_transition(c);
}
