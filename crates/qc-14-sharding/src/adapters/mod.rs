//! # Adapters Layer (Hexagonal Architecture)
//!
//! Implements outbound port traits for sharding functionality.
//!
//! Reference: SPEC-14-SHARDING.md Section 7

mod shard_consensus;
mod partitioned_state;

pub use shard_consensus::EventBusShardConsensus;
pub use partitioned_state::InMemoryPartitionedState;
