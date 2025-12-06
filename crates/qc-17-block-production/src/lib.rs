//! # Quantum Chain - Block Production Engine (Subsystem 17)
//!
//! **Version:** 2.4  
//! **Bounded Context:** Block Production & Mining  
//! **Architecture Compliance:** DDD + Hexagonal + EDA + TDD
//!
//! ## Purpose
//!
//! The Block Production Engine is responsible for creating new blocks through:
//! - Intelligent transaction selection using Priority-Based Greedy Knapsack (O(n log n))
//! - State prefetch and simulation to avoid failed transactions
//! - Consensus-appropriate sealing (PoW mining, PoS proposing, PBFT leader proposal)
//! - MEV detection and fair ordering enforcement
//!
//! ## Key Design Principles
//!
//! 1. **Optimal Transaction Selection**: Solves the bounded knapsack problem
//! 2. **Zero-Trust Validation**: Consensus re-validates all transactions
//! 3. **Multi-Consensus Support**: PoW, PoS, PBFT in one subsystem
//! 4. **Nonce Ordering**: Maintains sequential nonces per sender
//! 5. **State Simulation**: Only includes transactions that will succeed
//!
//! ## Architecture Layers
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │  Adapters (Outer)                                   │
//! │  - IPC: Mempool, State, Consensus communication     │
//! │  - PoW: Parallel nonce search                       │
//! │  - PoS: VRF proposer selection                      │
//! │  - PBFT: Leader-based proposal                      │
//! └─────────────────────────────────────────────────────┘
//!                         │
//! ┌─────────────────────────────────────────────────────┐
//! │  Ports (Middle)                                     │
//! │  - Inbound: BlockProducerService                    │
//! │  - Outbound: MempoolReader, StateReader, etc.       │
//! └─────────────────────────────────────────────────────┘
//!                         │
//! ┌─────────────────────────────────────────────────────┐
//! │  Domain (Inner - Pure Logic)                        │
//! │  - TransactionSelector                              │
//! │  - StatePrefetchCache                               │
//! │  - BlockTemplateBuilder                             │
//! │  - Invariants: gas limit, nonce ordering, etc.      │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! ## Critical Invariants
//!
//! 1. **Gas Limit**: sum(tx.gas_used) ≤ block_gas_limit
//! 2. **Nonce Ordering**: Sequential nonces per sender
//! 3. **State Validity**: All transactions simulate successfully
//! 4. **No Duplicates**: Unique transaction hashes
//! 5. **Timestamp Monotonicity**: parent_time ≤ block_time ≤ now + 15s
//! 6. **Fee Profitability**: Transactions sorted by gas price (greedy)
//!
//! ## Usage Example
//!
//! ```rust,ignore
//! // Create block producer service (not yet implemented)
//! // This is a placeholder example
//! ```
//!
//! ## Module Structure
//!
//! - [`domain`]: Pure domain logic (transaction selection, nonce ordering)
//! - [`ports`]: Hexagonal architecture interfaces (inbound/outbound)
//! - [`adapters`]: External integrations (IPC, PoW, PoS, PBFT)
//! - [`events`]: Event schemas for EDA
//! - [`handler`]: Event handlers and orchestration
//!
//! ## References
//!
//! - **Specification**: `SPECS/SPEC-17-BLOCK-PRODUCTION.md`
//! - **Architecture**: `Documentation/System.md` (Subsystem 17)
//! - **IPC Protocol**: `Documentation/IPC-MATRIX.md` (Subsystem 17)

#![warn(missing_docs)]
#![warn(clippy::all)]

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
    BlockHeader, BlockTemplate, ConsensusMode, MiningJob, PoSProposer, PoWMiner, ProposerDuty,
    SimulationResult, StatePrefetchCache, TransactionBundle, TransactionCandidate,
    TransactionSelector, VRFProof,
};

pub use ports::{
    BlockProducerService, ConsensusSubmitter, EventPublisher, MempoolReader, ProductionConfig,
    ProductionStatus, SignatureProvider, StateReader,
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
