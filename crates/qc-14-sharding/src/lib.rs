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
    assign_shard, compute_global_state_root, decide_outcome, get_involved_shards, is_cross_shard,
    rendezvous_assign, verify_shard_inclusion, TwoPhaseCoordinator,
};
pub use domain::{
    invariant_cross_shard_atomic, invariant_deterministic_assignment, invariant_global_consistency,
    invariant_min_validators, invariant_signature_threshold, AbortReason, Address, CrossShardState,
    CrossShardTransaction, GlobalStateRoot, Hash, LockData, LockProof, ShardAssignment,
    ShardConfig, ShardError, ShardId, ShardStateRoot, Signature, ValidatorInfo, MAX_SHARD_COUNT,
    MIN_SHARD_COUNT, MIN_VALIDATORS_PER_SHARD, SIGNATURE_THRESHOLD,
};
pub use ports::{
    BeaconChainProvider, MockBeaconChain, PartitionedState, RoutingResult, ShardConsensus,
    ShardingApi,
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
