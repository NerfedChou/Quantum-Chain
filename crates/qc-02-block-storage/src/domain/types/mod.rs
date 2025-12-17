//! # Types Module
//!
//! Configuration and immutable value types.

mod config;
pub mod security;

#[cfg(test)]
mod tests;

// Re-export public types
pub use config::{CompactionStrategy, KeyPrefix, StorageConfig, TransactionLocation};
