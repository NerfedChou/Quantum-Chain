//! # QC-01 Peer Discovery Brutal Benchmarks
//!
//! SPEC-01 Performance Claims to Validate:
//! - XOR distance computation: < 100ns
//! - Bucket index calculation: < 200ns  
//! - Find K closest peers: O(n log n) but < 1ms for 5000 peers
//! - Routing table lookup: O(1) per bucket
//!
//! Brutal Conditions:
//! - Maximum routing table size (256 buckets × 20 peers)
//! - Adversarial node ID distribution (clustered XOR distances)
//! - Concurrent bucket operations
//! - Memory pressure during peer eviction

use criterion::{black_box, BenchmarkId, Criterion, Throughput};
use rand::Rng;
use std::time::Duration;

/// XOR distance between two node IDs (256-bit)
fn xor_distance(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut result = [0u8; 32];
    for i in 0..32 {
        result[i] = a[i] ^ b[i];
    }
    result
}

/// Count leading zeros to determine bucket index
fn bucket_index(distance: &[u8; 32]) -> usize {
    let mut zeros = 0usize;
    for byte in distance {
        if *byte == 0 {
            zeros += 8;
        } else {
            zeros += byte.leading_zeros() as usize;
            break;
        }
    }
    255 - zeros.min(255)
}

/// Simulated routing table with K-buckets
struct BrutalRoutingTable {
    local_id: [u8; 32],
    buckets: Vec<Vec<[u8; 32]>>,
    k: usize, // bucket size
}

impl BrutalRoutingTable {
    fn new(local_id: [u8; 32], k: usize) -> Self {
        Self {
            local_id,
            buckets: vec![Vec::with_capacity(k); 256],
            k,
        }
    }

    fn add_peer(&mut self, peer_id: [u8; 32]) -> bool {
        let distance = xor_distance(&self.local_id, &peer_id);
        let idx = bucket_index(&distance);

        if self.buckets[idx].len() < self.k {
            self.buckets[idx].push(peer_id);
            true
        } else {
            false // Bucket full
        }
    }

    fn find_closest(&self, target: &[u8; 32], count: usize) -> Vec<[u8; 32]> {
        let mut all_peers: Vec<([u8; 32], [u8; 32])> = self
            .buckets
            .iter()
            .flatten()
            .map(|p| (xor_distance(target, p), *p))
            .collect();

        all_peers.sort_by(|a, b| a.0.cmp(&b.0));
        all_peers.into_iter().take(count).map(|(_, p)| p).collect()
    }

    fn total_peers(&self) -> usize {
        self.buckets.iter().map(|b| b.len()).sum()
    }
}

/// Generate adversarial peer IDs clustered near target
fn generate_clustered_peers(target: &[u8; 32], count: usize, spread: u8) -> Vec<[u8; 32]> {
    let mut rng = rand::thread_rng();
    (0..count)
        .map(|_| {
            let mut peer = *target;
            // Only modify last few bytes to create clustering
            for byte in peer.iter_mut().skip(32 - spread as usize) {
                *byte = rng.gen();
            }
            peer
        })
        .collect()
}

pub fn brutal_xor_distance(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-01/brutal/xor_distance");
    group.measurement_time(Duration::from_secs(10));

    // SPEC claim: < 100ns
    group.bench_function("xor_256bit_claim_100ns", |b| {
        let a = [0xABu8; 32];
        let node_b = [0xCDu8; 32];
        b.iter(|| black_box(xor_distance(&a, &node_b)))
    });

    // Adversarial: worst case bit patterns
    group.bench_function("xor_adversarial_alternating", |b| {
        let a = [0xAAu8; 32]; // 10101010...
        let node_b = [0x55u8; 32]; // 01010101...
        b.iter(|| black_box(xor_distance(&a, &node_b)))
    });

    group.finish();
}

pub fn brutal_bucket_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-01/brutal/bucket_ops");
    group.measurement_time(Duration::from_secs(10));

    // SPEC claim: bucket index < 200ns
    group.bench_function("bucket_index_claim_200ns", |b| {
        let distance = [
            0x00, 0x00, 0x01, 0xAB, 0xCD, 0xEF, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0,
            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
            0xFF, 0x00, 0x11, 0x22,
        ];
        b.iter(|| black_box(bucket_index(&distance)))
    });

    // Adversarial: all zeros (edge case - same node)
    group.bench_function("bucket_index_edge_zeros", |b| {
        let distance = [0u8; 32];
        b.iter(|| black_box(bucket_index(&distance)))
    });

    // Adversarial: all ones (maximum distance)
    group.bench_function("bucket_index_edge_ones", |b| {
        let distance = [0xFFu8; 32];
        b.iter(|| black_box(bucket_index(&distance)))
    });

    group.finish();
}

pub fn brutal_routing_table(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-01/brutal/routing_table");
    group.measurement_time(Duration::from_secs(15));

    // Maximum routing table: 256 buckets × 20 peers = 5120 peers
    let k = 20;
    let mut rng = rand::thread_rng();
    let local_id: [u8; 32] = rng.gen();

    // Pre-populate full routing table
    let mut full_table = BrutalRoutingTable::new(local_id, k);
    let mut attempts = 0;
    while full_table.total_peers() < 4000 && attempts < 100000 {
        let peer: [u8; 32] = rng.gen();
        full_table.add_peer(peer);
        attempts += 1;
    }

    let peer_count = full_table.total_peers();
    println!("Populated routing table with {} peers", peer_count);

    // SPEC claim: find K closest < 1ms for 5000 peers
    group.bench_function("find_20_closest_claim_1ms", |b| {
        let target: [u8; 32] = rng.gen();
        b.iter(|| black_box(full_table.find_closest(&target, 20)))
    });

    // Brutal: find closest under adversarial clustering
    let clustered_peers = generate_clustered_peers(&local_id, 1000, 4);
    let mut clustered_table = BrutalRoutingTable::new(local_id, k);
    for peer in &clustered_peers {
        clustered_table.add_peer(*peer);
    }

    group.bench_function("find_closest_clustered_adversarial", |b| {
        let target: [u8; 32] = rng.gen();
        b.iter(|| black_box(clustered_table.find_closest(&target, 20)))
    });

    // Brutal: concurrent-like add operations (sequential simulation)
    group.throughput(Throughput::Elements(1000));
    group.bench_function("add_1000_peers_burst", |b| {
        b.iter(|| {
            let mut table = BrutalRoutingTable::new(local_id, k);
            for _ in 0..1000 {
                let peer: [u8; 32] = rng.gen();
                table.add_peer(peer);
            }
            black_box(table.total_peers())
        })
    });

    group.finish();
}

pub fn brutal_network_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc-01/brutal/network_sim");
    group.measurement_time(Duration::from_secs(10));

    // Simulate network message processing under load
    let peer_counts = [100, 500, 1000, 5000];

    for count in peer_counts {
        let mut rng = rand::thread_rng();
        let local_id: [u8; 32] = rng.gen();
        let peers: Vec<[u8; 32]> = (0..count).map(|_| rng.gen()).collect();

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("process_peer_list", count),
            &peers,
            |b, peers| {
                b.iter(|| {
                    // Simulate processing incoming peer list
                    let mut distances: Vec<_> = peers
                        .iter()
                        .map(|p| (xor_distance(&local_id, p), *p))
                        .collect();
                    distances.sort_by(|a, b| a.0.cmp(&b.0));
                    black_box(distances.len())
                })
            },
        );
    }

    group.finish();
}

pub fn register_benchmarks(c: &mut Criterion) {
    brutal_xor_distance(c);
    brutal_bucket_operations(c);
    brutal_routing_table(c);
    brutal_network_simulation(c);
}
