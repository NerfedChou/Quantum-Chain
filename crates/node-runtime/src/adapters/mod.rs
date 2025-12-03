//! # Adapter Implementations
//!
//! This module provides concrete adapter implementations that:
//! 1. Implement the **outbound ports** (SPI traits) of each subsystem
//! 2. Connect subsystems to the event bus for choreography
//! 3. Handle IPC security via shared-types MessageVerifier
//!
//! ## Hexagonal Architecture (from Architecture.md)
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                     OUTER LAYER (Adapters)                          │
//! │  ┌───────────────────────────────────────────────────────────────┐  │
//! │  │  EventBusAdapter, BlockStorageAdapter, MempoolAdapter, etc.   │  │
//! │  └───────────────────────────────────────────────────────────────┘  │
//! │                              ↑ implements ↑                         │
//! │  ┌───────────────────────────────────────────────────────────────┐  │
//! │  │                    MIDDLE LAYER (Ports)                        │  │
//! │  │  trait BlockStorageGateway, trait MempoolGateway, etc.        │  │
//! │  └───────────────────────────────────────────────────────────────┘  │
//! │                              ↑ uses ↑                               │
//! │  ┌───────────────────────────────────────────────────────────────┐  │
//! │  │                    INNER LAYER (Domain)                        │  │
//! │  │  Pure business logic - no I/O, no async, no external deps     │  │
//! │  └───────────────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```

pub mod block_storage;
pub mod consensus;
pub mod event_bus;
pub mod finality;
pub mod mempool;
pub mod ports;
pub mod signature;
pub mod state;
pub mod storage;
pub mod transaction_indexing;

pub use block_storage::*;
pub use consensus::*;
pub use event_bus::*;
pub use finality::*;
pub use mempool::*;
pub use ports::*;
pub use signature::*;
pub use state::*;
pub use storage::*;
pub use transaction_indexing::*;
