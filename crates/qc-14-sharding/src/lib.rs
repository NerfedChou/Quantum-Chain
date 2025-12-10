//! # QC-14 Sharding
//!
//! Horizontal scaling via state sharding.
//!
//! **Subsystem ID:** 14  
//! **Specification:** SPEC-14-SHARDING.md  
//! **Architecture:** Hexagonal (DDD + Ports/Adapters)  
//! **Status:** Production-Ready
//!
//! ## Purpose
//!
//! Split blockchain state across multiple shards for horizontal scaling:
//! - Consistent hashing (rendezvous) for address-to-shard assignment
//! - Two-Phase Commit (2PC) for cross-shard atomicity
//! - Beacon chain coordination for validator management
//!
//! ## Security Features (System.md Lines 695-700)
//!
//! | Defense | Description |
//! |---------|-------------|
//! | Validator shuffling | Random rotation every epoch |
//! | Cross-links | Beacon validates shard headers |
//! | Fraud proofs | Immediate rollback on fraud |
//! | Minimum shard size | 128 validators per shard |
//!
//! ## Module Structure
//!
//! ```text
//! qc-14-sharding/
//! ├── domain/          # Core types: ShardConfig, CrossShardTransaction
//! ├── algorithms/      # Shard assignment, 2PC, global state root
//! └── ports/           # API traits + dependency traits
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod algorithms;
pub mod domain;
pub mod ports;

// Re-exports
pub use algorithms::{
    assign_shard, rendezvous_assign, is_cross_shard, get_involved_shards,
    compute_global_state_root, verify_shard_inclusion,
    TwoPhaseCoordinator, decide_outcome,
};
pub use domain::{
    ShardId, Hash, Address, ShardError,
    ShardConfig, CrossShardTransaction, ShardStateRoot, GlobalStateRoot,
    CrossShardState, AbortReason, ShardAssignment, ValidatorInfo,
    LockData, LockProof, Signature,
    MIN_SHARD_COUNT, MAX_SHARD_COUNT, MIN_VALIDATORS_PER_SHARD, SIGNATURE_THRESHOLD,
    invariant_deterministic_assignment, invariant_cross_shard_atomic,
    invariant_global_consistency, invariant_min_validators, invariant_signature_threshold,
};
pub use ports::{
    ShardingApi, RoutingResult,
    ShardConsensus, PartitionedState, BeaconChainProvider,
    MockBeaconChain,
};

/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn test_version() {
        assert!(!super::VERSION.is_empty());
    }
}
