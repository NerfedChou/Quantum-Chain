//! # QC-17 Block Production Engine
//!
//! **Subsystem ID:** 17  
//! **Specification:** SPEC-17-BLOCK-PRODUCTION.md v2.4  
//! **Architecture:** Architecture.md v2.4, IPC-MATRIX.md v2.4  
//! **Security Level:** INTERNAL (outputs untrusted until validated)  
//! **Status:** Production-Ready (Phase 3)
//!
//! ## Purpose
//!
//! The Block Production Engine is the **mining/proposing** subsystem, responsible for:
//! - Intelligent transaction selection using Priority-Based Greedy Knapsack (O(n log n))
//! - State prefetch and simulation to avoid failed transactions
//! - Consensus-appropriate sealing (PoW mining, PoS proposing, PBFT leader proposal)
//! - MEV detection and fair ordering enforcement
//!
//! ## Domain Invariants
//!
//! | ID | Invariant | Enforcement Location |
//! |----|-----------|---------------------|
//! | INV-1 | Gas Limit | `domain/invariants.rs:50-65` - `validate_gas_used()` |
//! | INV-2 | Nonce Ordering | `domain/services.rs:172-176` - `validate_nonce_ordering()` |
//! | INV-3 | State Validity | `domain/services.rs:232-265` - `simulate_transaction()` |
//! | INV-4 | No Duplicates | `domain/invariants.rs:150-175` - `validate_no_duplicates()` |
//! | INV-5 | Timestamp Monotonicity | `service.rs:254-256` - enforced in mining loop |
//! | INV-6 | Minimum Block Interval | `service.rs:430-440` - enforced after mining |
//!
//! ## Security (SPEC-17 Section 1.4)
//!
//! ### Trust Model
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    TRUST BOUNDARY                              │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  THIS SUBSYSTEM PRODUCES BLOCKS BUT DOES NOT VALIDATE THEM      │
//! │                                                                 │
//! │  INPUTS (Trusted):                                              │
//! │  ├─ Pending transactions from Mempool (6) - pre-verified       │
//! │  ├─ State from State Management (4) - authoritative            │
//! │  └─ Finality events from Finality (9) - triggers next block    │
//! │                                                                 │
//! │  OUTPUTS (Untrusted until validated):                           │
//! │  ├─ Block templates → Consensus (8) - MUST be validated        │
//! │  └─ Mining metrics → Telemetry - for observability             │
//! │                                                                 │
//! │  SECURITY PRINCIPLE:                                            │
//! │  - Consensus (8) re-validates ALL transactions                  │
//! │  - This subsystem can be Byzantine without breaking chain       │
//! │  - Worst case: Censorship (detectable) or empty blocks         │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## IPC Authorization
//!
//! ### Inbound Events (Subscribed)
//!
//! | Event | Allowed Senders | Purpose |
//! |-------|-----------------|---------|
//! | `BlockFinalizedEvent` | Finality (9) | Trigger next block production |
//! | `SlotAssignedEvent` | Consensus (8) | PoS proposer duty notification |
//! | `NewPendingTransactionEvent` | Mempool (6) | Transaction availability hint |
//!
//! ### Outbound Events (Published)
//!
//! | Event | Target | Purpose |
//! |-------|--------|---------|
//! | `BlockProducedEvent` | Consensus (8), Block Storage (2) | Block ready for validation |
//! | `MiningMetrics` | Telemetry (18) | Observability data |
//!
//! ## Difficulty Adjustment
//!
//! The subsystem implements **Dark Gravity Wave (DGW)** per-block difficulty adjustment:
//!
//! ```text
//! Target: hash(header) <= difficulty_target
//!
//! IMPORTANT: Higher target = EASIER mining, Lower target = HARDER mining
//!
//! Default Configuration:
//! - Initial difficulty: 2^235 (~2-5 seconds on single CPU)
//! - Target block time: 10 seconds
//! - DGW window: 24 blocks
//! - Max adjustment: 4x per block
//! - Min difficulty: 2^200 (hardest allowed)
//! - Max difficulty: 2^248 (easiest allowed)
//! ```
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                 BLOCK PRODUCTION ENGINE (qc-17)                      │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │  ┌─────────────────────────────────────────────────────────────┐   │
//! │  │              ConcreteBlockProducer (Service)                 │   │
//! │  │  - Orchestrates PoW/PoS/PBFT modes                          │   │
//! │  │  - Manages mining threads                                    │   │
//! │  │  - Enforces minimum block interval                          │   │
//! │  └────────────────────────┬────────────────────────────────────┘   │
//! │                           │                                         │
//! │  ┌────────────────────────┴────────────────────────┐               │
//! │  │               Domain Layer                       │               │
//! │  │  - TransactionSelector (Greedy Knapsack)        │               │
//! │  │  - DifficultyAdjuster (DGW Algorithm)           │               │
//! │  │  - PoWMiner (Parallel Nonce Search)             │               │
//! │  │  - PoSProposer (VRF Selection)                  │               │
//! │  │  - Invariant Validators                          │               │
//! │  └─────────────────────────────────────────────────┘               │
//! │                           │                                         │
//! │  ┌─────────────────────────────────────────────────┐               │
//! │  │               Outbound Ports                     │               │
//! │  │  - MempoolReader → qc-06                        │               │
//! │  │  - StateReader → qc-04                          │               │
//! │  │  - ConsensusSubmitter → qc-08                   │               │
//! │  │  - EventPublisher → Event Bus                   │               │
//! │  └─────────────────────────────────────────────────┘               │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage Example
//!
//! ```ignore
//! use qc_17_block_production::{ConcreteBlockProducer, BlockProductionConfig, ConsensusMode};
//! use qc_17_block_production::{BlockProducerService, ProductionConfig};
//! use std::sync::Arc;
//!
//! // Create producer with default config
//! let event_bus = Arc::new(shared_bus::InMemoryEventBus::new());
//! let config = BlockProductionConfig::default();
//! let producer = ConcreteBlockProducer::new(event_bus, config);
//!
//! // Start PoW mining
//! producer.start_production(
//!     ConsensusMode::ProofOfWork,
//!     ProductionConfig::default(),
//! ).await?;
//!
//! // Check status
//! let status = producer.get_status().await;
//! println!("Blocks produced: {}", status.blocks_produced);
//!
//! // Stop production
//! producer.stop_production().await?;
//! ```
//!
//! ## SPEC Reference
//!
//! See [SPEC-17-BLOCK-PRODUCTION.md](../../SPECS/SPEC-17-BLOCK-PRODUCTION.md) for full specification.

// Crate-level lints
#![warn(missing_docs)]
#![warn(clippy::all)]
#![allow(clippy::excessive_nesting)] // Acceptable for mining loops
#![deny(unsafe_code)]

/// IPC adapters for external communication
pub mod adapters;
/// Domain models and business logic
pub mod domain;
/// Event type definitions
pub mod events;
/// Event handlers
pub mod handler;
pub mod ports;
pub mod security;
pub mod service;
pub mod utils;

mod config;
mod error;
mod metrics;

pub use config::{
    BlockProductionConfig, HashAlgorithm, PBFTConfig, PerformanceConfig, PoSConfig, PoWConfig,
};
pub use error::{BlockProductionError, Result};
pub use metrics::Metrics;

// Re-export commonly used types
pub use domain::{
    BlockHeader, BlockTemplate, ConsensusMode, DifficultyConfig, MiningJob, PoSProposer, PoWMiner,
    ProposerDuty, SimulationResult, StatePrefetchCache, TransactionBundle, TransactionCandidate,
    TransactionSelector, VRFProof,
};

pub use ports::{
    BlockProducerService, ConsensusSubmitter, EventPublisher, HistoricalBlockInfo, MempoolReader,
    MinedBlockInfo, ProductionConfig, ProductionStatus, SignatureProvider, StateReader,
};

pub use events::{
    BlockFinalizedEvent, BlockProducedEvent, MiningMetrics, NewPendingTransactionEvent,
    SlotAssignedEvent,
};

pub use security::SecurityValidator;

pub use service::ConcreteBlockProducer;

/// Subsystem identifier for IPC communication
pub const SUBSYSTEM_ID: u8 = 17;

/// Default block gas limit (30 million gas)
pub const DEFAULT_GAS_LIMIT: u64 = 30_000_000;

/// Default minimum gas price (1 gwei)
pub const DEFAULT_MIN_GAS_PRICE: u64 = 1_000_000_000;

/// Maximum block timestamp skew (15 seconds into future)
pub const MAX_TIMESTAMP_SKEW: u64 = 15;

/// Maximum transactions to consider per block production round
pub const MAX_TRANSACTION_CANDIDATES: u32 = 10_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subsystem_id() {
        assert_eq!(SUBSYSTEM_ID, 17);
    }

    #[test]
    fn test_constants() {
        assert_eq!(DEFAULT_GAS_LIMIT, 30_000_000);
        assert_eq!(DEFAULT_MIN_GAS_PRICE, 1_000_000_000);
        assert_eq!(MAX_TIMESTAMP_SKEW, 15);
        assert_eq!(MAX_TRANSACTION_CANDIDATES, 10_000);
    }
}
