//! # Quantum Telemetry
//!
//! LGTM Stack integration for Quantum-Chain observability.
//!
//! ## Components
//!
//! - **L**oki: Structured log aggregation
//! - **G**rafana: Unified dashboards (configured separately)
//! - **T**empo: Distributed tracing via OpenTelemetry
//! - **M**etrics: Prometheus metrics for Mimir
//!
//! ## Usage
//!
//! ```rust,ignore
//! use quantum_telemetry::{TelemetryConfig, init_telemetry};
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = TelemetryConfig::from_env();
//!     let _guard = init_telemetry(config).await.expect("Failed to init telemetry");
//!     
//!     // Your application code here
//!     // Traces, logs, and metrics are now being collected
//! }
//! ```
//!
//! ## Environment Variables

// Allow dead code for API functions that may be used by consumers
#![allow(dead_code)]

//!
//! | Variable | Default | Description |
//! |----------|---------|-------------|
//! | `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4317` | Tempo OTLP endpoint |
//! | `OTEL_SERVICE_NAME` | `quantum-chain` | Service name in traces |
//! | `LOKI_ENDPOINT` | `http://localhost:3100` | Loki push endpoint |
//! | `QC_LOG_LEVEL` | `info` | Log level filter |
//! | `QC_SUBSYSTEM_ID` | `00` | Subsystem identifier |

#![warn(missing_docs)]
#![allow(missing_docs)] // TODO: Add documentation for all public items

mod config;
mod context;
mod logging;
mod metrics;
mod tracing_setup;

pub use config::TelemetryConfig;
pub use context::{PropagatedContext, TraceContext};
pub use logging::StructuredLogger;
pub use metrics::{
    register_metrics, MetricsHandle, BLOCKS_FINALIZED, BLOCKS_STORED, BLOCKS_VALIDATED,
    CONSENSUS_ROUNDS, EVENT_BUS_MESSAGES_RECEIVED, EVENT_BUS_MESSAGES_SENT, FINALITY_EPOCHS,
    MEMPOOL_BYTES, MEMPOOL_SIZE, PEERS_CONNECTED, PEERS_DISCOVERED, SIGNATURE_FAILURES,
    SIGNATURE_VERIFICATIONS, SUBSYSTEM_ERRORS, TRANSACTIONS_INDEXED, TRANSACTIONS_RECEIVED,
};
pub use tracing_setup::TracingGuard;

use thiserror::Error;

/// Telemetry initialization errors
#[derive(Error, Debug)]
pub enum TelemetryError {
    #[error("Failed to initialize OpenTelemetry tracer: {0}")]
    TracerInit(String),

    #[error("Failed to initialize Loki logger: {0}")]
    LokiInit(String),

    #[error("Failed to initialize Prometheus metrics: {0}")]
    MetricsInit(String),

    #[error("Invalid configuration: {0}")]
    Config(String),
}

/// Initialize the complete LGTM telemetry stack.
///
/// Returns a guard that must be held for the lifetime of the application.
/// When dropped, it flushes all pending traces and logs.
///
/// # Example
///
/// ```rust,ignore
/// let config = TelemetryConfig::from_env();
/// let _guard = init_telemetry(config).await?;
///
/// // Application runs here...
/// // Guard is dropped on exit, flushing telemetry
/// ```
pub async fn init_telemetry(config: TelemetryConfig) -> Result<TelemetryGuard, TelemetryError> {
    // Initialize metrics first (synchronous)
    let metrics_handle = register_metrics()?;

    // Initialize tracing (OpenTelemetry -> Tempo)
    let tracing_guard = tracing_setup::init_tracing(&config).await?;

    // Initialize structured logging (-> Loki)
    let _logging_guard = logging::init_logging(&config)?;

    Ok(TelemetryGuard {
        _tracing: tracing_guard,
        _metrics: metrics_handle,
    })
}

/// Guard that keeps telemetry active. Drop to flush and shutdown.
pub struct TelemetryGuard {
    _tracing: TracingGuard,
    _metrics: MetricsHandle,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        tracing::info!("Shutting down telemetry...");
        // TracingGuard handles OpenTelemetry shutdown
        // MetricsHandle handles Prometheus shutdown
    }
}

/// Convenience macro for creating a span with subsystem context.
///
/// # Example
///
/// ```rust,ignore
/// use quantum_telemetry::subsystem_span;
///
/// fn validate_block() {
///     let _span = subsystem_span!("validate_block", subsystem = "consensus", block_height = 12345);
///     // ... validation logic
/// }
/// ```
#[macro_export]
macro_rules! subsystem_span {
    ($name:expr, $($field:tt)*) => {
        tracing::info_span!($name, $($field)*)
    };
}

/// Convenience macro for recording a metric increment.
#[macro_export]
macro_rules! metric_inc {
    ($metric:expr) => {
        $metric.inc()
    };
    ($metric:expr, $labels:expr) => {
        $metric.with_label_values($labels).inc()
    };
}

/// Convenience macro for recording a metric with a value.
#[macro_export]
macro_rules! metric_observe {
    ($metric:expr, $value:expr) => {
        $metric.observe($value)
    };
    ($metric:expr, $labels:expr, $value:expr) => {
        $metric.with_label_values($labels).observe($value)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_env() {
        let config = TelemetryConfig::default();
        assert_eq!(config.service_name, "quantum-chain");
    }
}
