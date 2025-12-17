//! # Metrics Module
//!
//! Compaction and storage metrics for monitoring.

mod collector;
pub mod security;

#[cfg(test)]
mod tests;

// Re-export public types
pub use collector::{CompactionMetrics, StorageMetricsCollector};
