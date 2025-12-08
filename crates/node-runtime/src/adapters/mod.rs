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
//!
//! ## Plug-and-Play (v2.4)
//!
//! Adapters are conditionally compiled based on which subsystems are enabled.

// Core adapters (always available)
pub mod event_bus;
pub mod storage;

pub use event_bus::*;
pub use storage::*;

// Subsystem-specific adapters (conditional)
#[cfg(feature = "qc-01")]
pub mod peer_discovery;
#[cfg(feature = "qc-01")]
pub use peer_discovery::*;

#[cfg(feature = "qc-02")]
pub mod block_storage;
#[cfg(feature = "qc-02")]
pub use block_storage::*;

#[cfg(feature = "qc-03")]
pub mod transaction_indexing;
#[cfg(feature = "qc-03")]
pub use transaction_indexing::*;

#[cfg(feature = "qc-04")]
pub mod state;
#[cfg(feature = "qc-04")]
pub use state::*;

#[cfg(feature = "qc-06")]
pub mod mempool;
#[cfg(feature = "qc-06")]
pub use mempool::*;

#[cfg(feature = "qc-08")]
pub mod consensus;
#[cfg(feature = "qc-08")]
pub use consensus::*;

#[cfg(feature = "qc-09")]
pub mod finality;
#[cfg(feature = "qc-09")]
pub use finality::*;

#[cfg(feature = "qc-10")]
pub mod signature;
#[cfg(feature = "qc-10")]
pub use signature::*;

#[cfg(feature = "qc-16")]
pub mod api_gateway;
#[cfg(feature = "qc-16")]
pub use api_gateway::*;

#[cfg(feature = "qc-16")]
pub mod ipc_receiver;
#[cfg(feature = "qc-16")]
pub use ipc_receiver::EventBusIpcReceiver;

// Port adapters (conditional based on what they connect)
pub mod ports;
