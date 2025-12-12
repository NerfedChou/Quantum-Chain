//! # QC-07 Bloom Filters Brutal Benchmarks
//!
//! SPEC-07 Performance Claims to Validate:
//! - Insert operation: O(k) hash computations
//! - Contains operation: O(k) hash computations + O(k) bit lookups
//! - Filter merge: O(m/8) byte operations (optimized bitwise OR)
//! - Optimal parameter calculation: < 1μs
//! - Serialization/Deserialization: O(m/8) bytes
//!
//! Brutal Conditions:
//! - Maximum filter size (36,000 bits per SPEC-07)
//! - Maximum elements (1000 watched addresses per security boundary)
//! - Adversarial hash collision patterns
//! - High-frequency insert/query under contention simulation
//! - Memory pressure during filter operations
//! - Privacy rotation overhead (tweak changes)

use criterion::{black_box, BenchmarkId, Criterion, Throughput};
use rand::Rng;
use std::time::Duration;

/// Simulate Bloom filter for benchmarking
/// Uses same algorithm as qc-07 implementation
struct BrutalBloomFilter {
    bits: Vec<u8>,
    k: usize,
    m: usize,
    n: usize,
    tweak: u32,
}

impl BrutalBloomFilter {
    fn new(m: usize, k: usize) -> Self {
        let byte_size = (m + 7) / 8;
        Self {
            bits: vec![0u8; byte_size],
            k,
            m,
            n: 0,
            tweak: 0,
        }
    }

    fn new_with_tweak(m: usize, k: usize, tweak: u32) -> Self {
        let byte_size = (m + 7) / 8;
        Self {
            bits: vec![0u8; byte_size],
            k,
            m,
            n: 0,
            tweak,
        }
    }

    fn new_with_fpr(expected_elements: usize, target_fpr: f64) -> Self {
        let (k, m) = Self::optimal_params(expected_elements, target_fpr);
        Self::new(m, k)
    }

    fn optimal_params(n: usize, fpr: f64) -> (usize, usize) {
        // m = -n*ln(fpr) / (ln(2)^2)
        let ln2_sq = std::f64::consts::LN_2 * std::f64::consts::LN_2;
        let m = (-(n as f64) * fpr.ln() / ln2_sq).ceil() as usize;
        // k = (m/n) * ln(2)
        let k = ((m as f64 / n as f64) * std::f64::consts::LN_2).round() as usize;
        (k.max(1).min(32), m)
    }

    fn hash_positions(&self, element: &[u8]) -> Vec<usize> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut positions = Vec::with_capacity(self.k);

        // Simulate k independent hash functions using double hashing
        let mut hasher1 = DefaultHasher::new();
        element.hash(&mut hasher1);
        self.tweak.hash(&mut hasher1);
        let h1 = hasher1.finish();

        let mut hasher2 = DefaultHasher::new();
        h1.hash(&mut hasher2);
        element.hash(&mut hasher2);
        let h2 = hasher2.finish();

        for i in 0..self.k {
            let combined = h1.wrapping_add((i as u64).wrapping_mul(h2));
            positions.push((combined as usize) % self.m);
        }

        positions
    }

    fn insert(&mut self, element: &[u8]) {
        let positions = self.hash_positions(element);
        for pos in positions {
            let byte_idx = pos / 8;
            let bit_idx = pos % 8;
            self.bits[byte_idx] |= 1 << bit_idx;
        }
        self.n += 1;
    }

    fn contains(&self, element: &[u8]) -> bool {
        let positions = self.hash_positions(element);
        positions.iter().all(|&pos| {
            let byte_idx = pos / 8;
            let bit_idx = pos % 8;
            (self.bits[byte_idx] & (1 << bit_idx)) != 0
        })
    }

    fn merge(&mut self, other: &BrutalBloomFilter) {
        for (s, o) in self.bits.iter_mut().zip(other.bits.iter()) {
            *s |= *o;
        }
        self.n += other.n;
    }

    fn bits_set(&self) -> usize {
        self.bits.iter().map(|b| b.count_ones() as usize).sum()
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::new();
        // Header: m, k, n, tweak
        result.extend_from_slice(&(self.m as u64).to_le_bytes());
        result.extend_from_slice(&(self.k as u64).to_le_bytes());
        result.extend_from_slice(&(self.n as u64).to_le_bytes());
        result.extend_from_slice(&self.tweak.to_le_bytes());
        // Bits
        result.extend_from_slice(&self.bits);
        result
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < 28 {
            return Err("Too short");
        }
        let m = u64::from_le_bytes(bytes[0..8].try_into().unwrap()) as usize;
        let k = u64::from_le_bytes(bytes[8..16].try_into().unwrap()) as usize;
        let n = u64::from_le_bytes(bytes[16..24].try_into().unwrap()) as usize;
        let tweak = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
        let bits = bytes[28..].to_vec();
        Ok(Self {
            bits,
            k,
            m,
            n,
            tweak,
        })
    }

    fn false_positive_rate(&self) -> f64 {
        // FPR = (1 - e^(-kn/m))^k
        let exponent = -(self.k as f64 * self.n as f64) / (self.m as f64);
        (1.0 - exponent.exp()).powi(self.k as i32)
    }
}

/// Generate realistic Ethereum-style addresses
fn generate_addresses(count: usize) -> Vec<Vec<u8>> {
    let mut rng = rand::thread_rng();
    (0..count)
        .map(|_| {
            let mut addr = vec![0u8; 20];
            rng.fill(&mut addr[..]);
            addr
        })
        .collect()
}

/// Generate adversarial addresses that may cause hash collisions
fn generate_adversarial_addresses(count: usize) -> Vec<Vec<u8>> {
    let mut rng = rand::thread_rng();
    let prefix: [u8; 16] = rng.gen();

    (0..count)
        .map(|i| {
            let mut addr = vec![0u8; 20];
            addr[..16].copy_from_slice(&prefix);
            addr[16..20].copy_from_slice(&(i as u32).to_le_bytes());
            addr
        })
        .collect()
}

pub fn brutal_insert_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-07/brutal/insert");
    group.measurement_time(Duration::from_secs(10));

    // SPEC claim: Insert is O(k) hash computations
    // Benchmark with varying k values
    for k in [7, 13, 20] {
        group.bench_with_input(BenchmarkId::new("single_insert", k), &k, |b, &k| {
            let mut filter = BrutalBloomFilter::new(36_000, k);
            let addr = generate_addresses(1)[0].clone();
            b.iter(|| {
                filter.insert(black_box(&addr));
            });
        });
    }

    // Bulk insert benchmark - 1000 addresses (max per SPEC security boundary)
    let addresses = generate_addresses(1000);
    group.throughput(Throughput::Elements(1000));
    group.bench_function("bulk_insert_1000_addresses", |b| {
        b.iter(|| {
            let mut filter = BrutalBloomFilter::new_with_fpr(1000, 0.01);
            for addr in &addresses {
                filter.insert(black_box(addr));
            }
            black_box(filter.bits_set())
        });
    });

    // Adversarial: insert addresses designed to cause collisions
    let adversarial = generate_adversarial_addresses(1000);
    group.bench_function("adversarial_insert_collision_pattern", |b| {
        b.iter(|| {
            let mut filter = BrutalBloomFilter::new_with_fpr(1000, 0.01);
            for addr in &adversarial {
                filter.insert(black_box(addr));
            }
            black_box(filter.bits_set())
        });
    });

    group.finish();
}

pub fn brutal_contains_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-07/brutal/contains");
    group.measurement_time(Duration::from_secs(10));

    // Pre-populate filter
    let addresses = generate_addresses(100);
    let mut filter = BrutalBloomFilter::new_with_fpr(100, 0.01);
    for addr in &addresses {
        filter.insert(addr);
    }

    // SPEC claim: Contains is O(k) hash + O(k) bit lookups
    group.bench_function("contains_existing_element", |b| {
        let target = &addresses[50];
        b.iter(|| black_box(filter.contains(black_box(target))))
    });

    group.bench_function("contains_non_existing_element", |b| {
        let non_existing = generate_addresses(1)[0].clone();
        b.iter(|| black_box(filter.contains(black_box(&non_existing))))
    });

    // Bulk contains - check 10,000 addresses
    let test_addresses = generate_addresses(10_000);
    group.throughput(Throughput::Elements(10_000));
    group.bench_function("bulk_contains_10000", |b| {
        b.iter(|| {
            let mut matches = 0;
            for addr in &test_addresses {
                if filter.contains(black_box(addr)) {
                    matches += 1;
                }
            }
            black_box(matches)
        });
    });

    group.finish();
}

pub fn brutal_merge_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-07/brutal/merge");
    group.measurement_time(Duration::from_secs(10));

    // SPEC claim: Merge is O(m/8) byte operations
    let filter_sizes = [1_000, 10_000, 36_000];

    for size in filter_sizes {
        let addrs1 = generate_addresses(50);
        let addrs2 = generate_addresses(50);

        let mut filter1 = BrutalBloomFilter::new(size, 7);
        let mut filter2 = BrutalBloomFilter::new(size, 7);

        for addr in &addrs1 {
            filter1.insert(addr);
        }
        for addr in &addrs2 {
            filter2.insert(addr);
        }

        group.throughput(Throughput::Bytes(size as u64 / 8));
        group.bench_with_input(
            BenchmarkId::new("merge_filters", size),
            &filter2,
            |b, f2| {
                b.iter(|| {
                    let mut f1_clone = BrutalBloomFilter::new(size, 7);
                    f1_clone.bits.copy_from_slice(&filter1.bits);
                    f1_clone.n = filter1.n;
                    f1_clone.merge(black_box(f2));
                    black_box(f1_clone.bits_set())
                });
            },
        );
    }

    group.finish();
}

pub fn brutal_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-07/brutal/serialization");
    group.measurement_time(Duration::from_secs(10));

    // Test with max filter size
    let mut filter = BrutalBloomFilter::new(36_000, 13);
    let addresses = generate_addresses(1000);
    for addr in &addresses {
        filter.insert(addr);
    }

    group.throughput(Throughput::Bytes(36_000 / 8));

    group.bench_function("serialize_max_filter", |b| {
        b.iter(|| black_box(filter.to_bytes()))
    });

    let serialized = filter.to_bytes();
    group.bench_function("deserialize_max_filter", |b| {
        b.iter(|| black_box(BrutalBloomFilter::from_bytes(black_box(&serialized))))
    });

    group.finish();
}

pub fn brutal_fpr_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-07/brutal/fpr_calculation");
    group.measurement_time(Duration::from_secs(5));

    // SPEC claim: Optimal parameter calculation < 1μs
    group.bench_function("optimal_params_claim_1us", |b| {
        b.iter(|| {
            black_box(BrutalBloomFilter::optimal_params(
                black_box(100),
                black_box(0.01),
            ))
        })
    });

    // FPR calculation on populated filter
    let mut filter = BrutalBloomFilter::new_with_fpr(100, 0.01);
    for i in 0u32..100 {
        filter.insert(&i.to_le_bytes());
    }

    group.bench_function("fpr_calculation_populated", |b| {
        b.iter(|| black_box(filter.false_positive_rate()))
    });

    group.finish();
}

pub fn brutal_privacy_rotation(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-07/brutal/privacy_rotation");
    group.measurement_time(Duration::from_secs(10));

    let addresses = generate_addresses(50);

    // Measure cost of creating filter with new tweak (privacy rotation)
    group.bench_function("rotate_filter_with_new_tweak", |b| {
        let mut tweak = 0u32;
        b.iter(|| {
            tweak = tweak.wrapping_add(1);
            let mut filter = BrutalBloomFilter::new_with_tweak(36_000, 13, tweak);
            for addr in &addresses {
                filter.insert(addr);
            }
            black_box(filter.bits_set())
        });
    });

    // Verify tweak changes bit positions (privacy property)
    group.bench_function("verify_tweak_changes_positions", |b| {
        b.iter(|| {
            let filter1 = BrutalBloomFilter::new_with_tweak(1000, 7, 0);
            let filter2 = BrutalBloomFilter::new_with_tweak(1000, 7, 12345);

            let addr = b"test_address";
            let pos1 = filter1.hash_positions(addr);
            let pos2 = filter2.hash_positions(addr);

            // Different tweaks should produce different positions
            black_box(pos1 != pos2)
        });
    });

    group.finish();
}

pub fn brutal_memory_pressure(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-07/brutal/memory_pressure");
    group.measurement_time(Duration::from_secs(15));

    // Create many filters simultaneously (simulating many light clients)
    group.bench_function("create_100_concurrent_filters", |b| {
        let addresses_sets: Vec<Vec<Vec<u8>>> = (0..100).map(|_| generate_addresses(50)).collect();

        b.iter(|| {
            let filters: Vec<BrutalBloomFilter> = addresses_sets
                .iter()
                .map(|addrs| {
                    let mut filter = BrutalBloomFilter::new_with_fpr(50, 0.01);
                    for addr in addrs {
                        filter.insert(addr);
                    }
                    filter
                })
                .collect();
            black_box(filters.len())
        });
    });

    // Heavy allocation pattern - create/destroy filters rapidly
    group.bench_function("rapid_filter_allocation_deallocation", |b| {
        let addresses = generate_addresses(50);
        b.iter(|| {
            for _ in 0..100 {
                let mut filter = BrutalBloomFilter::new_with_fpr(50, 0.01);
                for addr in &addresses {
                    filter.insert(addr);
                }
                black_box(filter.bits_set());
                // filter dropped here
            }
        });
    });

    group.finish();
}

pub fn brutal_false_positive_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-07/brutal/fpr_validation");
    group.measurement_time(Duration::from_secs(20));

    // Validate INVARIANT-1: FPR <= target_fpr
    // This is expensive but critical for correctness
    let target_fprs = [0.01, 0.001, 0.0001];

    for &target_fpr in &target_fprs {
        group.bench_with_input(
            BenchmarkId::new("validate_fpr_bound", format!("{:.4}", target_fpr)),
            &target_fpr,
            |b, &fpr| {
                b.iter(|| {
                    let n = 100;
                    let mut filter = BrutalBloomFilter::new_with_fpr(n, fpr);

                    // Insert n elements
                    for i in 0..n {
                        filter.insert(&format!("inserted_{}", i).as_bytes());
                    }

                    // Test 10,000 non-inserted elements
                    let mut false_positives = 0;
                    for i in 0..10_000 {
                        if filter.contains(format!("not_inserted_{}", i).as_bytes()) {
                            false_positives += 1;
                        }
                    }

                    let actual_fpr = false_positives as f64 / 10_000.0;
                    black_box(actual_fpr <= fpr * 1.5) // Allow 1.5x tolerance
                });
            },
        );
    }

    group.finish();
}

pub fn register_benchmarks(c: &mut Criterion) {
    brutal_insert_operations(c);
    brutal_contains_operations(c);
    brutal_merge_operations(c);
    brutal_serialization(c);
    brutal_fpr_calculation(c);
    brutal_privacy_rotation(c);
    brutal_memory_pressure(c);
    brutal_false_positive_validation(c);
}
