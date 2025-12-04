//! Ports layer (Hexagonal Architecture)
//!
//! Reference: SPEC-08-CONSENSUS.md Section 3

mod inbound;
mod outbound;

pub use inbound::*;
pub use outbound::*;
