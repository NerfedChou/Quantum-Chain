//! # QC-07 Bloom Filters - Transaction Filtering Subsystem
//!
//! **Subsystem ID:** 7  
//! **Specification:** SPEC-07-BLOOM-FILTERS.md v2.3  
//! **Architecture:** Architecture.md v2.3, IPC-MATRIX.md v2.3  
//! **Status:** Production-Ready
//!
//! ## Purpose
//!
//! Provides Bloom filter-based probabilistic filtering for light clients,
//! enabling efficient SPV (Simplified Payment Verification) by matching
//! transactions against watched addresses without downloading full blocks.
//!
//! ## Domain Invariants
//!
//! | ID | Invariant | Enforcement Location |
//! |----|-----------|---------------------|
//! | INVARIANT-1 | FPR ≤ target_fpr | `domain/bloom_filter.rs:283-310` - statistical test |
//! | INVARIANT-2 | No False Negatives | `domain/bloom_filter.rs:115-118` - `contains()` guaranteed |
//!
//! ## Security (IPC-MATRIX.md)
//!
//! - **Centralized Security**: Uses `shared-types::AuthenticatedMessage` for IPC
//! - **Envelope-Only Identity**: Identity derived solely from `sender_id`
//! - **Rate Limiting**: Max 1 filter update per 10 blocks per client
//!
//! ### IPC Authorization Matrix
//!
//! | Message | Authorized Sender(s) | Enforcement |
//! |---------|---------------------|-------------|
//! | `BuildFilterRequest` | Light Clients (13) ONLY | `handler/ipc_handler.rs:60-69` |
//! | `UpdateFilterRequest` | Light Clients (13) ONLY | `handler/ipc_handler.rs:103-113` |
//! | `TransactionHashUpdate` | Transaction Indexing (3) ONLY | `handler/ipc_handler.rs:143-155` |
//!
//! ### Privacy Protections
//!
//! | Defense | Description | Enforcement |
//! |---------|-------------|-------------|
//! | FPR Bounds | Reject FPR <0.01 (too precise) or >0.1 (too noisy) | `handler/ipc_handler.rs:82-93` |
//! | Address Limit | Reject >1000 watched addresses | `handler/ipc_handler.rs:74-79` |
//! | Rate Limiting | Max 1 update per 10 blocks | `handler/ipc_handler.rs:119-134` |
//! | Privacy Noise | Add random fake elements | `service/bloom_filter_service.rs:47-71` |
//! | Filter Rotation | Tweak changes bit positions | `domain/bloom_filter.rs:86-94` |
//!
//! ## Outbound Dependencies
//!
//! | Subsystem | Trait | Purpose |
//! |-----------|-------|---------|
//! | 3 (Transaction Indexing) | `TransactionDataProvider` | Transaction hashes for filter population |
//!
//! ## Module Structure (Hexagonal Architecture)
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      OUTER LAYER                                │
//! │  adapters/ - Event bus, API gateway connections                 │
//! └─────────────────────────────────────────────────────────────────┘
//!                          ↑ implements ↑
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      MIDDLE LAYER                               │
//! │  ports/inbound.rs  - BloomFilterApi trait                       │
//! │  ports/outbound.rs - TransactionDataProvider trait              │
//! └─────────────────────────────────────────────────────────────────┘
//!                          ↑ uses ↑
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      INNER LAYER                                │
//! │  domain/bloom_filter.rs  - Core BloomFilter implementation      │
//! │  domain/block_filter.rs  - BlockFilter for per-block filtering  │
//! │  domain/config.rs        - BloomConfig with validation          │
//! │  domain/hash_functions.rs - MurmurHash3 with double hashing     │
//! │  domain/parameters.rs    - Optimal FPR parameter calculation    │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Algorithm (System.md)
//!
//! - **Hash Function**: MurmurHash3 with double hashing technique
//! - **FPR Formula**: FPR = (1 - e^(-kn/m))^k
//! - **Optimal m**: m = -n*ln(fpr) / (ln(2)^2)
//! - **Optimal k**: k = (m/n) * ln(2)
//!
//! ## Usage Example
//!
//! ```rust,ignore
//! use qc_07_bloom_filters::{BloomFilter, BloomConfigBuilder};
//!
//! // Create a filter with optimal parameters for 50 elements, 1% FPR
//! let mut filter = BloomFilter::new_with_fpr(50, 0.01);
//! filter.insert(b"0xABCD1234...");
//!
//! // Check membership (guaranteed no false negatives)
//! assert!(filter.contains(b"0xABCD1234..."));
//! ```

#![warn(missing_docs)]
#![allow(missing_docs)] // TODO: Add documentation for all public items

pub mod adapters;
pub mod domain;
pub mod error;
pub mod events;
pub mod handler;
pub mod metrics;
pub mod ports;
pub mod service;

// Re-exports for convenience
pub use domain::{BlockFilter, BloomConfig, BloomConfigBuilder, BloomFilter};
pub use error::{DataError, FilterError};
pub use handler::BloomFilterHandler;
pub use metrics::{Metrics, MetricsRecorder, MetricsSnapshot, NoOpMetrics};
pub use ports::{BloomFilterApi, MatchResult, MatchedField, TransactionDataProvider};
pub use service::BloomFilterService;

// Phase 4 adapter exports
pub use adapters::{ApiGatewayHandler, BloomFilterBusAdapter, TxIndexingAdapter};
