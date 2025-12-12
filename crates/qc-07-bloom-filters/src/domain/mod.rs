//! Domain Layer - Pure business logic
//!
//! Reference: Architecture.md - Hexagonal Architecture
//!
//! This layer contains:
//! - Core Bloom filter implementation
//! - Hash functions
//! - Parameter calculations
//! - Configuration
//! - Gap limit enforcer (anti-dusting)
//! - GCS filters (BIP 158)
//! - Counting Bloom filter (incremental updates)
//! - Cuckoo filter (deletion-capable)
//!
//! RULES:
//! - No I/O operations
//! - No async code
//! - Pure functions where possible

pub mod block_filter;
pub mod bloom_filter;
pub mod config;
pub mod counting_bloom;
pub mod cuckoo;
pub mod gap_limit;
pub mod gcs_filter;
pub mod hash_functions;
pub mod parameters;

pub use block_filter::BlockFilter;
pub use bloom_filter::BloomFilter;
pub use config::{BloomConfig, BloomConfigBuilder};
pub use counting_bloom::CountingBloomFilter;
pub use cuckoo::{Bucket, CuckooFilter, Fingerprint, ENTRIES_PER_BUCKET};
pub use gap_limit::{ClientMatchHistory, GapLimitEnforcer, ThrottleReason};
pub use gcs_filter::{GcsFilter, GCS_FPR, GOLOMB_P};
pub use parameters::{calculate_optimal_parameters, AdaptiveBloomParams, BloomFilterParams};
