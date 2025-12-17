//! # MMR Module
//!
//! Merkle Mountain Range for O(log n) block existence proofs.

pub mod security;
mod store;

#[cfg(test)]
mod tests;

// Re-export public types
pub use store::{MmrError, MmrProof, MmrStore};
