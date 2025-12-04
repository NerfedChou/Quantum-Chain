//! # Finality Metrics
//!
//! Prometheus metrics for monitoring finality performance and health.
//!
//! ## Usage
//!
//! Enable with the `metrics` feature:
//! ```toml
//! qc-09-finality = { path = "...", features = ["metrics"] }
//! ```
//!
//! ## Metrics Exported
//!
//! - `finality_checkpoints_justified_total` - Counter of justified checkpoints
//! - `finality_checkpoints_finalized_total` - Counter of finalized checkpoints
//! - `finality_attestations_processed_total` - Counter of attestations processed
//! - `finality_attestations_rejected_total` - Counter of rejected attestations (by reason)
//! - `finality_slashable_offenses_total` - Counter of slashable offenses detected
//! - `finality_epochs_without_finality` - Gauge of consecutive epochs without finality
//! - `finality_circuit_breaker_state` - Gauge of circuit breaker state (0=Running, 1=Sync, 2=Halted)
//! - `finality_inactivity_leak_active` - Gauge indicating if inactivity leak is active

#[cfg(feature = "metrics")]
use lazy_static::lazy_static;

#[cfg(feature = "metrics")]
use prometheus::{
    register_counter, register_counter_vec, register_gauge, register_int_counter, Counter,
    CounterVec, Gauge, IntCounter,
};

#[cfg(feature = "metrics")]
lazy_static! {
    /// Total checkpoints justified
    pub static ref CHECKPOINTS_JUSTIFIED: IntCounter = register_int_counter!(
        "finality_checkpoints_justified_total",
        "Total number of checkpoints justified"
    )
    .expect("Failed to create CHECKPOINTS_JUSTIFIED metric");

    /// Total checkpoints finalized
    pub static ref CHECKPOINTS_FINALIZED: IntCounter = register_int_counter!(
        "finality_checkpoints_finalized_total",
        "Total number of checkpoints finalized"
    )
    .expect("Failed to create CHECKPOINTS_FINALIZED metric");

    /// Total attestations processed
    pub static ref ATTESTATIONS_PROCESSED: IntCounter = register_int_counter!(
        "finality_attestations_processed_total",
        "Total number of attestations processed"
    )
    .expect("Failed to create ATTESTATIONS_PROCESSED metric");

    /// Total attestations rejected, labeled by reason
    pub static ref ATTESTATIONS_REJECTED: CounterVec = register_counter_vec!(
        "finality_attestations_rejected_total",
        "Total number of attestations rejected",
        &["reason"]
    )
    .expect("Failed to create ATTESTATIONS_REJECTED metric");

    /// Total slashable offenses detected, labeled by type
    pub static ref SLASHABLE_OFFENSES: CounterVec = register_counter_vec!(
        "finality_slashable_offenses_total",
        "Total number of slashable offenses detected",
        &["type"]
    )
    .expect("Failed to create SLASHABLE_OFFENSES metric");

    /// Consecutive epochs without finality
    pub static ref EPOCHS_WITHOUT_FINALITY: Gauge = register_gauge!(
        "finality_epochs_without_finality",
        "Number of consecutive epochs without finality"
    )
    .expect("Failed to create EPOCHS_WITHOUT_FINALITY metric");

    /// Circuit breaker state (0=Running, 1=Sync, 2=Halted)
    pub static ref CIRCUIT_BREAKER_STATE: Gauge = register_gauge!(
        "finality_circuit_breaker_state",
        "Current circuit breaker state (0=Running, 1=Sync, 2=Halted)"
    )
    .expect("Failed to create CIRCUIT_BREAKER_STATE metric");

    /// Inactivity leak active flag
    pub static ref INACTIVITY_LEAK_ACTIVE: Gauge = register_gauge!(
        "finality_inactivity_leak_active",
        "Whether inactivity leak is currently active (0=no, 1=yes)"
    )
    .expect("Failed to create INACTIVITY_LEAK_ACTIVE metric");

    /// Participation percentage of last finalized checkpoint
    pub static ref PARTICIPATION_PERCENT: Gauge = register_gauge!(
        "finality_participation_percent",
        "Participation percentage of last finalized checkpoint"
    )
    .expect("Failed to create PARTICIPATION_PERCENT metric");
}

// =============================================================================
// METRIC RECORDING FUNCTIONS
// =============================================================================

/// Record a checkpoint justified
#[cfg(feature = "metrics")]
pub fn record_checkpoint_justified() {
    CHECKPOINTS_JUSTIFIED.inc();
}

/// Record a checkpoint finalized with participation
#[cfg(feature = "metrics")]
pub fn record_checkpoint_finalized(participation_percent: f64) {
    CHECKPOINTS_FINALIZED.inc();
    PARTICIPATION_PERCENT.set(participation_percent);
}

/// Record attestations processed
#[cfg(feature = "metrics")]
pub fn record_attestations_processed(count: u64) {
    ATTESTATIONS_PROCESSED.inc_by(count);
}

/// Record attestation rejected with reason
#[cfg(feature = "metrics")]
pub fn record_attestation_rejected(reason: &str) {
    ATTESTATIONS_REJECTED.with_label_values(&[reason]).inc();
}

/// Record slashable offense detected
#[cfg(feature = "metrics")]
pub fn record_slashable_offense(offense_type: &str) {
    SLASHABLE_OFFENSES.with_label_values(&[offense_type]).inc();
}

/// Update epochs without finality gauge
#[cfg(feature = "metrics")]
pub fn set_epochs_without_finality(epochs: u64) {
    EPOCHS_WITHOUT_FINALITY.set(epochs as f64);
}

/// Update circuit breaker state gauge
#[cfg(feature = "metrics")]
pub fn set_circuit_breaker_state(state: u8) {
    CIRCUIT_BREAKER_STATE.set(state as f64);
}

/// Update inactivity leak active flag
#[cfg(feature = "metrics")]
pub fn set_inactivity_leak_active(active: bool) {
    INACTIVITY_LEAK_ACTIVE.set(if active { 1.0 } else { 0.0 });
}

// =============================================================================
// NO-OP IMPLEMENTATIONS (when metrics feature disabled)
// =============================================================================

#[cfg(not(feature = "metrics"))]
pub fn record_checkpoint_justified() {}

#[cfg(not(feature = "metrics"))]
pub fn record_checkpoint_finalized(_participation_percent: f64) {}

#[cfg(not(feature = "metrics"))]
pub fn record_attestations_processed(_count: u64) {}

#[cfg(not(feature = "metrics"))]
pub fn record_attestation_rejected(_reason: &str) {}

#[cfg(not(feature = "metrics"))]
pub fn record_slashable_offense(_offense_type: &str) {}

#[cfg(not(feature = "metrics"))]
pub fn set_epochs_without_finality(_epochs: u64) {}

#[cfg(not(feature = "metrics"))]
pub fn set_circuit_breaker_state(_state: u8) {}

#[cfg(not(feature = "metrics"))]
pub fn set_inactivity_leak_active(_active: bool) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_noop_when_disabled() {
        // These should compile and run without panic even without metrics feature
        record_checkpoint_justified();
        record_checkpoint_finalized(75.0);
        record_attestations_processed(100);
        record_attestation_rejected("invalid_signature");
        record_slashable_offense("double_vote");
        set_epochs_without_finality(5);
        set_circuit_breaker_state(0);
        set_inactivity_leak_active(true);
    }
}
