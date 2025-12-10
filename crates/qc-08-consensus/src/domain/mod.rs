//! Domain layer for Consensus subsystem
//!
//! Reference: SPEC-08-CONSENSUS.md Section 2
//!
//! Additional modules for advanced consensus:
//! - slashing: Double-vote detection
//! - checkpoints: Weak subjectivity
//! - fork_choice: LMD-GHOST
//! - bls_aggregation: Pipelined BLS verification
//! - pbs: Proposer-Builder Separation (MEV protection)
//! - vdf: Verifiable Delay Function (grinding protection)

mod block;
mod bls_aggregation;
mod chain;
mod checkpoints;
mod error;
mod fork_choice;
mod pbs;
mod proof;
mod slashing;
mod validator;
mod vdf;

pub use block::*;
pub use bls_aggregation::*;
pub use chain::*;
pub use checkpoints::*;
pub use error::*;
pub use fork_choice::*;
pub use pbs::*;
pub use proof::*;
pub use slashing::*;
pub use validator::*;
pub use vdf::*;

