//! Domain Layer - Pure business logic
//!
//! Reference: Architecture.md - Hexagonal Architecture
//!
//! This layer contains:
//! - Core Bloom filter implementation
//! - Hash functions
//! - Parameter calculations
//! - Configuration
//!
//! RULES:
//! - No I/O operations
//! - No async code
//! - Pure functions where possible

pub mod block_filter;
pub mod bloom_filter;
pub mod config;
pub mod hash_functions;
pub mod parameters;

pub use block_filter::BlockFilter;
pub use bloom_filter::BloomFilter;
pub use config::{BloomConfig, BloomConfigBuilder};
pub use parameters::{calculate_optimal_parameters, BloomFilterParams};
