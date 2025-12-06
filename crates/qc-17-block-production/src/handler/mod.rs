//! Event handlers and orchestration
//!
//! Handlers for choreography events:
//! - BlockFinalized: Triggered when a block is finalized by qc-09
//! - SlotAssigned: Triggered when this validator is assigned a slot (PoS)

pub mod block_finalized;
pub mod slot_assigned;
