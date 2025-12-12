//! # Algorithms Module
//!
//! Core algorithms for sharding subsystem.
//!
//! Reference: System.md Lines 676-683

pub mod global_state;
pub mod shard_assignment;
pub mod two_phase_commit;

pub use global_state::{compute_global_state_root, verify_shard_inclusion};
pub use shard_assignment::{assign_shard, get_involved_shards, is_cross_shard, rendezvous_assign};
pub use two_phase_commit::{decide_outcome, TwoPhaseCoordinator};
