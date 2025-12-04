//! # Consensus Metrics
//!
//! Prometheus metrics for monitoring consensus performance.
//!
//! ## Usage
//!
//! Enable with the `metrics` feature:
//! ```toml
//! qc-08-consensus = { path = "...", features = ["metrics"] }
//! ```
//!
//! ## Metrics Exported
//!
//! - `consensus_blocks_validated_total` - Counter of successfully validated blocks
//! - `consensus_blocks_rejected_total` - Counter of rejected blocks (by reason)
//! - `consensus_validation_latency_seconds` - Histogram of validation times
//! - `consensus_attestations_verified_total` - Counter of attestations verified

#[cfg(feature = "metrics")]
use lazy_static::lazy_static;

#[cfg(feature = "metrics")]
use prometheus::{
    register_counter_vec, register_histogram, register_int_counter, CounterVec, Histogram,
    IntCounter,
};

#[cfg(feature = "metrics")]
lazy_static! {
    /// Total blocks successfully validated
    pub static ref BLOCKS_VALIDATED: IntCounter = register_int_counter!(
        "consensus_blocks_validated_total",
        "Total number of blocks successfully validated"
    )
    .expect("Failed to create BLOCKS_VALIDATED metric");

    /// Total blocks rejected, labeled by rejection reason
    pub static ref BLOCKS_REJECTED: CounterVec = register_counter_vec!(
        "consensus_blocks_rejected_total",
        "Total number of blocks rejected",
        &["reason"]
    )
    .expect("Failed to create BLOCKS_REJECTED metric");

    /// Histogram of block validation latency
    pub static ref VALIDATION_LATENCY: Histogram = register_histogram!(
        "consensus_validation_latency_seconds",
        "Time taken to validate a block in seconds",
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]
    )
    .expect("Failed to create VALIDATION_LATENCY metric");

    /// Total attestations verified
    pub static ref ATTESTATIONS_VERIFIED: IntCounter = register_int_counter!(
        "consensus_attestations_verified_total",
        "Total number of attestations verified"
    )
    .expect("Failed to create ATTESTATIONS_VERIFIED metric");
}

/// Record a successful block validation
#[cfg(feature = "metrics")]
pub fn record_block_validated() {
    BLOCKS_VALIDATED.inc();
}

/// Record a rejected block with reason
#[cfg(feature = "metrics")]
pub fn record_block_rejected(reason: &str) {
    BLOCKS_REJECTED.with_label_values(&[reason]).inc();
}

/// Record validation latency
#[cfg(feature = "metrics")]
pub fn record_validation_latency(seconds: f64) {
    VALIDATION_LATENCY.observe(seconds);
}

/// Record attestation verified
#[cfg(feature = "metrics")]
pub fn record_attestation_verified() {
    ATTESTATIONS_VERIFIED.inc();
}

// No-op implementations when metrics feature is disabled
#[cfg(not(feature = "metrics"))]
pub fn record_block_validated() {}

#[cfg(not(feature = "metrics"))]
pub fn record_block_rejected(_reason: &str) {}

#[cfg(not(feature = "metrics"))]
pub fn record_validation_latency(_seconds: f64) {}

#[cfg(not(feature = "metrics"))]
pub fn record_attestation_verified() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_noop_when_disabled() {
        // These should compile and run without panic even without metrics feature
        record_block_validated();
        record_block_rejected("test");
        record_validation_latency(1.0);
        record_attestation_verified();
    }
}
