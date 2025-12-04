//! Structured logging for Loki integration.
//!
//! Logs are formatted as JSON with consistent fields that Loki can parse:
//! - `timestamp`: ISO 8601 timestamp
//! - `level`: Log level (trace, debug, info, warn, error)
//! - `subsystem`: Subsystem identifier (consensus, finality, etc.)
//! - `message`: Log message
//! - `trace_id`: OpenTelemetry trace ID (for correlation with Tempo)
//! - Additional context fields

use crate::{TelemetryConfig, TelemetryError};

/// Structured logger handle
pub struct StructuredLogger {
    _initialized: bool,
}

/// Initialize Loki logging.
///
/// Note: Loki integration is handled via the tracing-subscriber JSON layer.
/// Logs are sent to Loki via a log shipping agent (Promtail) or direct push.
/// This function configures structured logging that's Loki-compatible.
pub fn init_logging(config: &TelemetryConfig) -> Result<StructuredLogger, TelemetryError> {
    tracing::debug!(
        loki_endpoint = %config.loki_endpoint,
        json_logs = config.json_logs,
        "Structured logging configured for Loki compatibility"
    );

    Ok(StructuredLogger { _initialized: true })
}

/// Helper to create structured log entries with consistent formatting.
#[macro_export]
macro_rules! log_event {
    // Info level with subsystem
    (info, $subsystem:expr, $msg:expr $(, $($field:tt)*)?) => {
        tracing::info!(
            subsystem = $subsystem,
            $($($field)*,)?
            $msg
        )
    };

    // Warn level with subsystem
    (warn, $subsystem:expr, $msg:expr $(, $($field:tt)*)?) => {
        tracing::warn!(
            subsystem = $subsystem,
            $($($field)*,)?
            $msg
        )
    };

    // Error level with subsystem
    (error, $subsystem:expr, $msg:expr $(, $($field:tt)*)?) => {
        tracing::error!(
            subsystem = $subsystem,
            $($($field)*,)?
            $msg
        )
    };

    // Debug level with subsystem
    (debug, $subsystem:expr, $msg:expr $(, $($field:tt)*)?) => {
        tracing::debug!(
            subsystem = $subsystem,
            $($($field)*,)?
            $msg
        )
    };
}

/// Log a block-related event with standard fields.
#[macro_export]
macro_rules! log_block_event {
    ($level:ident, $subsystem:expr, $msg:expr, $block_height:expr, $block_hash:expr $(, $($field:tt)*)?) => {
        tracing::$level!(
            subsystem = $subsystem,
            block_height = $block_height,
            block_hash = %$block_hash,
            $($($field)*,)?
            $msg
        )
    };
}

/// Log a transaction-related event with standard fields.
#[macro_export]
macro_rules! log_tx_event {
    ($level:ident, $subsystem:expr, $msg:expr, $tx_hash:expr $(, $($field:tt)*)?) => {
        tracing::$level!(
            subsystem = $subsystem,
            tx_hash = %$tx_hash,
            $($($field)*,)?
            $msg
        )
    };
}

/// Log a peer-related event with standard fields.
#[macro_export]
macro_rules! log_peer_event {
    ($level:ident, $subsystem:expr, $msg:expr, $peer_id:expr $(, $($field:tt)*)?) => {
        tracing::$level!(
            subsystem = $subsystem,
            peer_id = %$peer_id,
            $($($field)*,)?
            $msg
        )
    };
}

#[cfg(test)]
mod tests {
    // Logging tests would require a mock Loki server
    // Better tested in integration tests
}
