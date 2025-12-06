//! # Port Adapters for Subsystem Integration
//!
//! This module provides concrete implementations of the outbound port traits
//! required by each subsystem. These adapters bridge subsystems together
//! through the container.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                    SubsystemContainer                               │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌────────────┐ │
//! │  │ Consensus   │  │ Finality    │  │ BlockProp   │  │ Others...  │ │
//! │  │ Service     │  │ Service     │  │ Service     │  │            │ │
//! │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └────────────┘ │
//! │         │                │                │                        │
//! │         ↓                ↓                ↓                        │
//! │  ┌─────────────────────────────────────────────────────────────┐   │
//! │  │              Port Adapters (this module)                     │   │
//! │  │  - ConsensusEventBusAdapter                                  │   │
//! │  │  - ConsensusMempoolAdapter                                   │   │
//! │  │  - ConsensusValidatorSetAdapter                              │   │
//! │  │  - FinalityBlockStorageAdapter                               │   │
//! │  │  - BlockPropNetworkAdapter                                   │   │
//! │  └─────────────────────────────────────────────────────────────┘   │
//! │         │                │                │                        │
//! │         ↓                ↓                ↓                        │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                │
//! │  │ EventBus    │  │ BlockStorage│  │ Mempool     │                │
//! │  └─────────────┘  └─────────────┘  └─────────────┘                │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Plug-and-Play (v2.4)
//!
//! Port adapters are conditionally compiled based on which subsystems are enabled.

#[cfg(feature = "qc-05")]
pub mod block_propagation;
#[cfg(feature = "qc-05")]
pub use block_propagation::*;

#[cfg(feature = "qc-08")]
pub mod consensus;
#[cfg(feature = "qc-08")]
pub use consensus::*;

#[cfg(all(feature = "qc-09", feature = "qc-02"))]
pub mod finality;
#[cfg(all(feature = "qc-09", feature = "qc-02"))]
pub use finality::*;
