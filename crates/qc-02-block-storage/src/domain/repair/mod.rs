//! # Repair Module
//!
//! Self-healing index for disaster recovery.

mod report;
pub mod security;

#[cfg(test)]
mod tests;

// Re-export public types
pub use report::{
    RepairContext, RepairError, RepairFatalError, RepairReport, Repairable,
    TransactionLocationRepair,
};
