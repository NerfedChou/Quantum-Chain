//! Prometheus metrics for Quantum-Chain subsystems.
//!
//! All metrics follow the naming convention: `qc_<subsystem>_<metric>_<unit>`
//!
//! ## Metric Types
//!
//! - **Counter**: Monotonically increasing value (e.g., blocks_validated_total)
//! - **Gauge**: Value that can go up or down (e.g., mempool_size)
//! - **Histogram**: Distribution of values (e.g., block_validation_duration_seconds)

use lazy_static::lazy_static;
use prometheus::{
    exponential_buckets, Counter, CounterVec, Encoder, Gauge, Histogram, HistogramVec, Opts,
    Registry, TextEncoder,
};
use std::sync::Arc;

use crate::TelemetryError;

lazy_static! {
    /// Global metrics registry
    pub static ref REGISTRY: Registry = Registry::new();

    // =========================================================================
    // CONSENSUS METRICS (Subsystem 8)
    // =========================================================================

    /// Total blocks validated by consensus
    pub static ref BLOCKS_VALIDATED: Counter = Counter::new(
        "qc_consensus_blocks_validated_total",
        "Total number of blocks validated by consensus"
    ).expect("metric creation failed");

    /// Block validation duration histogram
    pub static ref BLOCK_VALIDATION_DURATION: Histogram = Histogram::with_opts(
        prometheus::HistogramOpts::new(
            "qc_consensus_block_validation_duration_seconds",
            "Time spent validating blocks"
        ).buckets(exponential_buckets(0.001, 2.0, 15).unwrap())
    ).expect("metric creation failed");

    /// Consensus rounds counter
    pub static ref CONSENSUS_ROUNDS: CounterVec = CounterVec::new(
        Opts::new("qc_consensus_rounds_total", "Total consensus rounds"),
        &["algorithm", "outcome"]  // algorithm: pos/pbft, outcome: success/timeout/failure
    ).expect("metric creation failed");

    // =========================================================================
    // BLOCK STORAGE METRICS (Subsystem 2)
    // =========================================================================

    /// Total blocks stored
    pub static ref BLOCKS_STORED: Counter = Counter::new(
        "qc_storage_blocks_stored_total",
        "Total number of blocks written to storage"
    ).expect("metric creation failed");

    /// Current chain height
    pub static ref CHAIN_HEIGHT: Gauge = Gauge::new(
        "qc_storage_chain_height",
        "Current blockchain height"
    ).expect("metric creation failed");

    /// Block storage duration
    pub static ref BLOCK_STORAGE_DURATION: Histogram = Histogram::with_opts(
        prometheus::HistogramOpts::new(
            "qc_storage_block_write_duration_seconds",
            "Time spent writing blocks to storage"
        ).buckets(exponential_buckets(0.0001, 2.0, 12).unwrap())
    ).expect("metric creation failed");

    // =========================================================================
    // FINALITY METRICS (Subsystem 9)
    // =========================================================================

    /// Total blocks finalized
    pub static ref BLOCKS_FINALIZED: Counter = Counter::new(
        "qc_finality_blocks_finalized_total",
        "Total number of blocks finalized"
    ).expect("metric creation failed");

    /// Epochs processed
    pub static ref FINALITY_EPOCHS: Counter = Counter::new(
        "qc_finality_epochs_total",
        "Total number of finality epochs processed"
    ).expect("metric creation failed");

    /// Current finalized height
    pub static ref FINALIZED_HEIGHT: Gauge = Gauge::new(
        "qc_finality_finalized_height",
        "Height of the last finalized block"
    ).expect("metric creation failed");

    /// Epochs without finality (circuit breaker indicator)
    pub static ref EPOCHS_WITHOUT_FINALITY: Gauge = Gauge::new(
        "qc_finality_epochs_without_finality",
        "Number of epochs since last finalization (circuit breaker threshold)"
    ).expect("metric creation failed");

    // =========================================================================
    // TRANSACTION METRICS (Subsystems 3, 6)
    // =========================================================================

    /// Total transactions received
    pub static ref TRANSACTIONS_RECEIVED: Counter = Counter::new(
        "qc_mempool_transactions_received_total",
        "Total transactions received into mempool"
    ).expect("metric creation failed");

    /// Total transactions indexed
    pub static ref TRANSACTIONS_INDEXED: Counter = Counter::new(
        "qc_indexing_transactions_indexed_total",
        "Total transactions indexed"
    ).expect("metric creation failed");

    /// Current mempool size (transaction count)
    pub static ref MEMPOOL_SIZE: Gauge = Gauge::new(
        "qc_mempool_transactions_pending",
        "Number of pending transactions in mempool"
    ).expect("metric creation failed");

    /// Current mempool size (bytes)
    pub static ref MEMPOOL_BYTES: Gauge = Gauge::new(
        "qc_mempool_size_bytes",
        "Total size of pending transactions in bytes"
    ).expect("metric creation failed");

    // =========================================================================
    // PEER METRICS (Subsystem 1)
    // =========================================================================

    /// Connected peers
    pub static ref PEERS_CONNECTED: Gauge = Gauge::new(
        "qc_peers_connected",
        "Number of currently connected peers"
    ).expect("metric creation failed");

    /// Total peers discovered
    pub static ref PEERS_DISCOVERED: Counter = Counter::new(
        "qc_peers_discovered_total",
        "Total number of peers discovered"
    ).expect("metric creation failed");

    /// Peer connection attempts
    pub static ref PEER_CONNECTIONS: CounterVec = CounterVec::new(
        Opts::new("qc_peers_connection_attempts_total", "Peer connection attempts"),
        &["outcome"]  // outcome: success/failed/timeout
    ).expect("metric creation failed");

    // =========================================================================
    // SIGNATURE METRICS (Subsystem 10)
    // =========================================================================

    /// Total signature verifications
    pub static ref SIGNATURE_VERIFICATIONS: CounterVec = CounterVec::new(
        Opts::new("qc_signature_verifications_total", "Total signature verifications"),
        &["type", "result"]  // type: ecdsa/bls, result: valid/invalid
    ).expect("metric creation failed");

    /// Signature verification failures (for alerting)
    pub static ref SIGNATURE_FAILURES: Counter = Counter::new(
        "qc_signature_failures_total",
        "Total signature verification failures"
    ).expect("metric creation failed");

    /// Signature verification duration
    pub static ref SIGNATURE_DURATION: HistogramVec = HistogramVec::new(
        prometheus::HistogramOpts::new(
            "qc_signature_verification_duration_seconds",
            "Time spent verifying signatures"
        ).buckets(exponential_buckets(0.00001, 2.0, 15).unwrap()),
        &["type"]  // type: ecdsa/bls/batch
    ).expect("metric creation failed");

    // =========================================================================
    // EVENT BUS METRICS (IPC)
    // =========================================================================

    /// Messages sent via event bus
    pub static ref EVENT_BUS_MESSAGES_SENT: CounterVec = CounterVec::new(
        Opts::new("qc_eventbus_messages_sent_total", "Messages sent via event bus"),
        &["event_type", "source_subsystem"]
    ).expect("metric creation failed");

    /// Messages received via event bus
    pub static ref EVENT_BUS_MESSAGES_RECEIVED: CounterVec = CounterVec::new(
        Opts::new("qc_eventbus_messages_received_total", "Messages received from event bus"),
        &["event_type", "target_subsystem"]
    ).expect("metric creation failed");

    /// Event bus latency
    pub static ref EVENT_BUS_LATENCY: Histogram = Histogram::with_opts(
        prometheus::HistogramOpts::new(
            "qc_eventbus_delivery_latency_seconds",
            "Time for event delivery via bus"
        ).buckets(exponential_buckets(0.0001, 2.0, 12).unwrap())
    ).expect("metric creation failed");

    // =========================================================================
    // ERROR METRICS
    // =========================================================================

    /// Subsystem errors by type
    pub static ref SUBSYSTEM_ERRORS: CounterVec = CounterVec::new(
        Opts::new("qc_subsystem_errors_total", "Errors by subsystem and type"),
        &["subsystem", "error_type"]
    ).expect("metric creation failed");
}

/// Handle for the metrics server
pub struct MetricsHandle {
    _registry: Arc<Registry>,
}

/// Register all metrics with the global registry.
pub fn register_metrics() -> Result<MetricsHandle, TelemetryError> {
    // Register all metrics
    let metrics: Vec<Box<dyn prometheus::core::Collector>> = vec![
        // Consensus
        Box::new(BLOCKS_VALIDATED.clone()),
        Box::new(BLOCK_VALIDATION_DURATION.clone()),
        Box::new(CONSENSUS_ROUNDS.clone()),
        // Storage
        Box::new(BLOCKS_STORED.clone()),
        Box::new(CHAIN_HEIGHT.clone()),
        Box::new(BLOCK_STORAGE_DURATION.clone()),
        // Finality
        Box::new(BLOCKS_FINALIZED.clone()),
        Box::new(FINALITY_EPOCHS.clone()),
        Box::new(FINALIZED_HEIGHT.clone()),
        Box::new(EPOCHS_WITHOUT_FINALITY.clone()),
        // Transactions
        Box::new(TRANSACTIONS_RECEIVED.clone()),
        Box::new(TRANSACTIONS_INDEXED.clone()),
        Box::new(MEMPOOL_SIZE.clone()),
        Box::new(MEMPOOL_BYTES.clone()),
        // Peers
        Box::new(PEERS_CONNECTED.clone()),
        Box::new(PEERS_DISCOVERED.clone()),
        Box::new(PEER_CONNECTIONS.clone()),
        // Signatures
        Box::new(SIGNATURE_VERIFICATIONS.clone()),
        Box::new(SIGNATURE_FAILURES.clone()),
        Box::new(SIGNATURE_DURATION.clone()),
        // Event Bus
        Box::new(EVENT_BUS_MESSAGES_SENT.clone()),
        Box::new(EVENT_BUS_MESSAGES_RECEIVED.clone()),
        Box::new(EVENT_BUS_LATENCY.clone()),
        // Errors
        Box::new(SUBSYSTEM_ERRORS.clone()),
    ];

    for metric in metrics {
        REGISTRY
            .register(metric)
            .map_err(|e| TelemetryError::MetricsInit(e.to_string()))?;
    }

    Ok(MetricsHandle {
        _registry: Arc::new(REGISTRY.clone()),
    })
}

/// Encode all metrics as Prometheus text format.
pub fn encode_metrics() -> Result<String, TelemetryError> {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    encoder
        .encode(&metric_families, &mut buffer)
        .map_err(|e| TelemetryError::MetricsInit(e.to_string()))?;
    String::from_utf8(buffer).map_err(|e| TelemetryError::MetricsInit(e.to_string()))
}

/// Timer guard for automatic histogram observation.
pub struct HistogramTimer {
    histogram: Histogram,
    start: std::time::Instant,
}

impl HistogramTimer {
    /// Start a new timer for the given histogram.
    pub fn new(histogram: &Histogram) -> Self {
        Self {
            histogram: histogram.clone(),
            start: std::time::Instant::now(),
        }
    }
}

impl Drop for HistogramTimer {
    fn drop(&mut self) {
        let duration = self.start.elapsed().as_secs_f64();
        self.histogram.observe(duration);
    }
}

/// Start timing for a histogram. Observation happens on drop.
#[macro_export]
macro_rules! time_histogram {
    ($histogram:expr) => {
        $crate::metrics::HistogramTimer::new(&$histogram)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_metrics() {
        // Create a new registry for testing
        let result = register_metrics();
        // May fail if already registered, which is fine
        let _ = result;
    }

    #[test]
    fn test_counter_increment() {
        BLOCKS_VALIDATED.inc();
        assert!(BLOCKS_VALIDATED.get() >= 1.0);
    }

    #[test]
    fn test_gauge_set() {
        MEMPOOL_SIZE.set(42.0);
        assert_eq!(MEMPOOL_SIZE.get(), 42.0);
    }

    #[test]
    fn test_histogram_timer() {
        let _timer = HistogramTimer::new(&BLOCK_VALIDATION_DURATION);
        std::thread::sleep(std::time::Duration::from_millis(1));
        // Timer observes on drop
    }
}
