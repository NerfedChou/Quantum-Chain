//! # QC-07 Bloom Filters
//!
//! Transaction Filtering subsystem using Bloom filters for light client support.
//!
//! ## Architecture
//!
//! This crate follows Hexagonal Architecture (Ports & Adapters):
//!
//! - **Domain Layer** (`domain/`): Pure business logic, no I/O
//!   - `BloomFilter`: Core probabilistic data structure
//!   - `BlockFilter`: Block-level filter containing transaction addresses
//!   - `BloomConfig`: Configuration with validation
//!   - `BloomConfigBuilder`: Fluent builder for configuration
//!
//! - **Ports Layer** (`ports/`): Trait definitions
//!   - `BloomFilterApi`: Driving port (inbound API)
//!   - `TransactionDataProvider`: Driven port (dependency on qc-03)
//!
//! - **Service Layer** (`service/`): Orchestration
//!   - `BloomFilterService`: Implements `BloomFilterApi`
//!
//! - **Handler Layer** (`handler/`): IPC security
//!   - `BloomFilterHandler`: Validates messages per IPC-MATRIX.md
//!
//! - **Events Layer** (`events/`): IPC message types
//!
//! - **Adapters Layer** (`adapters/`): External connections
//!   - `TxIndexingAdapter`: Queries qc-03 via shared-bus
//!   - `BloomFilterBusAdapter`: Event bus subscriber for API queries
//!   - `ApiGatewayHandler`: Exposes operations to qc-16
//!
//! ## Security
//!
//! Per IPC-MATRIX.md Subsystem 7:
//! - Accept `BuildFilterRequest` from Subsystem 13 (Light Clients) ONLY
//! - Accept `UpdateFilterRequest` from Subsystem 13 ONLY
//! - Accept `TransactionHashUpdate` from Subsystem 3 (Transaction Indexing) ONLY
//! - Reject filters with >1000 watched addresses
//! - Reject FPR <0.01 or >0.1
//! - Reject >1 filter update per 10 blocks per client
//!
//! ## Invariants
//!
//! - **INVARIANT-1**: FPR = (1 - e^(-kn/m))^k <= target_fpr
//! - **INVARIANT-2**: No false negatives - if inserted, contains() MUST return true
//!
//! ## Usage Example
//!
//! ```ignore
//! use qc_07_bloom_filters::{BloomFilter, BloomConfigBuilder};
//!
//! // Create a filter using the builder
//! let config = BloomConfigBuilder::new()
//!     .target_fpr(0.05)
//!     .max_elements(100)
//!     .build()?;
//!
//! // Create filter and insert addresses
//! let mut filter = BloomFilter::new_with_fpr(50, 0.01);
//! filter.insert(b"0xABCD...");
//!
//! // Check membership
//! assert!(filter.contains(b"0xABCD..."));
//! ```
//!
//! ## Wiring to Runtime (Phase 4)
//!
//! ```ignore
//! use qc_07_bloom_filters::{
//!     BloomFilterBusAdapter, TxIndexingAdapter, BloomFilterService, BloomFilterHandler
//! };
//! use shared_bus::InMemoryEventBus;
//! use std::sync::Arc;
//!
//! // Create event bus and adapters
//! let bus = Arc::new(InMemoryEventBus::new());
//! let tx_adapter = TxIndexingAdapter::new(bus.clone());
//! let service = BloomFilterService::new(tx_adapter);
//! let handler = BloomFilterHandler::new();
//!
//! // Create and run the bus adapter
//! let adapter = Arc::new(BloomFilterBusAdapter::new(bus.clone(), service, handler));
//! tokio::spawn(adapter.run());
//! ```
//!
//! ## References
//!
//! - SPEC-07-BLOOM-FILTERS.md
//! - System.md, Subsystem 7
//! - IPC-MATRIX.md, Subsystem 7
//! - Architecture.md

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
