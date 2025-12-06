//! Adapters Layer (Driven Adapters)
//!
//! Reference: Architecture.md - Hexagonal Architecture
//!
//! Contains implementations of driven ports that connect to
//! external systems (Transaction Indexing, Event Bus).
//!
//! ## Adapters
//!
//! - `TxIndexingAdapter` - Queries qc-03 for transaction data via shared-bus
//! - `BloomFilterBusAdapter` - Subscribes to events and handles API queries
//! - `ApiGatewayHandler` - Exposes filter operations to qc-16 API Gateway

pub mod bus_adapter;
pub mod tx_indexing;

pub use bus_adapter::{ApiGatewayHandler, BloomFilterBusAdapter};
pub use tx_indexing::TxIndexingAdapter;
