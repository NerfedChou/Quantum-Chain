//! Domain layer for Consensus subsystem
//!
//! Reference: SPEC-08-CONSENSUS.md Section 2
//!
//! Additional modules for advanced consensus:
//! - slashing: Double-vote detection
//! - checkpoints: Weak subjectivity
//! - fork_choice: LMD-GHOST

mod block;
mod chain;
mod checkpoints;
mod error;
mod fork_choice;
mod proof;
mod slashing;
mod validator;

pub use block::*;
pub use chain::*;
pub use checkpoints::*;
pub use error::*;
pub use fork_choice::*;
pub use proof::*;
pub use slashing::*;
pub use validator::*;

