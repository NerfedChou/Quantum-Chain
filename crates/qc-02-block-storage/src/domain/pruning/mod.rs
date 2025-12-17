//! # Pruning Module
//!
//! Smart pruning with anchor blocks per SPEC-02 Section 5.2.

pub mod security;
mod service;

#[cfg(test)]
mod tests;

// Re-export public types
pub use service::{PruneResult, PruningConfig, PruningService, StoredBlockHeader};
