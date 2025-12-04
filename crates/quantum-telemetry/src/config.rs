//! Telemetry configuration from environment variables.

use std::env;

/// Configuration for the LGTM telemetry stack.
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// Service name for traces and logs
    pub service_name: String,

    /// Subsystem identifier (01-15)
    pub subsystem_id: String,

    /// OpenTelemetry OTLP endpoint for Tempo
    pub otlp_endpoint: String,

    /// Loki push endpoint
    pub loki_endpoint: String,

    /// Log level filter (trace, debug, info, warn, error)
    pub log_level: String,

    /// Whether to enable console output (for development)
    pub console_output: bool,

    /// Whether to enable JSON formatted logs
    pub json_logs: bool,

    /// Prometheus metrics port
    pub metrics_port: u16,

    /// Network identifier (testnet, mainnet, devnet)
    pub network: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            service_name: "quantum-chain".to_string(),
            subsystem_id: "00".to_string(),
            otlp_endpoint: "http://localhost:4317".to_string(),
            loki_endpoint: "http://localhost:3100".to_string(),
            log_level: "info".to_string(),
            console_output: true,
            json_logs: false,
            metrics_port: 9100,
            network: "testnet".to_string(),
        }
    }
}

impl TelemetryConfig {
    /// Create configuration from environment variables.
    ///
    /// # Environment Variables
    ///
    /// - `OTEL_SERVICE_NAME`: Service name (default: quantum-chain)
    /// - `QC_SUBSYSTEM_ID`: Subsystem ID (default: 00)
    /// - `OTEL_EXPORTER_OTLP_ENDPOINT`: Tempo endpoint (default: http://localhost:4317)
    /// - `LOKI_ENDPOINT`: Loki endpoint (default: http://localhost:3100)
    /// - `QC_LOG_LEVEL` or `RUST_LOG`: Log level (default: info)
    /// - `QC_CONSOLE_OUTPUT`: Enable console output (default: true)
    /// - `QC_JSON_LOGS`: Enable JSON logs (default: false in dev, true in containers)
    /// - `QC_METRICS_PORT`: Prometheus metrics port (default: 9100)
    /// - `QC_NETWORK`: Network name (default: testnet)
    pub fn from_env() -> Self {
        let is_container =
            env::var("KUBERNETES_SERVICE_HOST").is_ok() || env::var("DOCKER_CONTAINER").is_ok();

        Self {
            service_name: env::var("OTEL_SERVICE_NAME")
                .unwrap_or_else(|_| "quantum-chain".to_string()),

            subsystem_id: env::var("QC_SUBSYSTEM_ID").unwrap_or_else(|_| "00".to_string()),

            otlp_endpoint: env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:4317".to_string()),

            loki_endpoint: env::var("LOKI_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:3100".to_string()),

            log_level: env::var("QC_LOG_LEVEL")
                .or_else(|_| env::var("RUST_LOG"))
                .unwrap_or_else(|_| "info".to_string()),

            console_output: env::var("QC_CONSOLE_OUTPUT")
                .map(|v| v.to_lowercase() != "false" && v != "0")
                .unwrap_or(true),

            json_logs: env::var("QC_JSON_LOGS")
                .map(|v| v.to_lowercase() == "true" || v == "1")
                .unwrap_or(is_container),

            metrics_port: env::var("QC_METRICS_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(9100),

            network: env::var("QC_NETWORK").unwrap_or_else(|_| "testnet".to_string()),
        }
    }

    /// Create configuration for a specific subsystem.
    pub fn for_subsystem(subsystem_id: &str, subsystem_name: &str) -> Self {
        let mut config = Self::from_env();
        config.subsystem_id = subsystem_id.to_string();
        config.service_name = format!("qc-{}-{}", subsystem_id, subsystem_name);
        config
    }

    /// Get the full service name including subsystem.
    pub fn full_service_name(&self) -> String {
        if self.subsystem_id == "00" {
            self.service_name.clone()
        } else {
            format!("{}-{}", self.service_name, self.subsystem_id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TelemetryConfig::default();
        assert_eq!(config.service_name, "quantum-chain");
        assert_eq!(config.log_level, "info");
        assert_eq!(config.metrics_port, 9100);
    }

    #[test]
    fn test_for_subsystem() {
        let config = TelemetryConfig::for_subsystem("08", "consensus");
        assert_eq!(config.subsystem_id, "08");
        assert_eq!(config.service_name, "qc-08-consensus");
    }

    #[test]
    fn test_full_service_name() {
        let mut config = TelemetryConfig::default();
        assert_eq!(config.full_service_name(), "quantum-chain");

        config.subsystem_id = "10".to_string();
        assert_eq!(config.full_service_name(), "quantum-chain-10");
    }
}
