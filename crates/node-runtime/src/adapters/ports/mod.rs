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

pub mod consensus;
pub mod finality;
pub mod block_propagation;

pub use consensus::*;
pub use finality::*;
pub use block_propagation::*;
