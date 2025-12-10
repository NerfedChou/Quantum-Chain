//! # Algorithms Module
//!
//! Core algorithms for sharding subsystem.
//!
//! Reference: System.md Lines 676-683

pub mod shard_assignment;
pub mod global_state;
pub mod two_phase_commit;

pub use shard_assignment::{assign_shard, rendezvous_assign, is_cross_shard, get_involved_shards};
pub use global_state::{compute_global_state_root, verify_shard_inclusion};
pub use two_phase_commit::{TwoPhaseCoordinator, decide_outcome};
