//! OpenTelemetry tracing setup for Tempo integration.
//!
//! This module configures distributed tracing that sends spans to Tempo
//! via the OpenTelemetry Protocol (OTLP).

use opentelemetry::trace::TracerProvider;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    runtime,
    trace::{self, RandomIdGenerator, Sampler},
    Resource,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

use crate::{TelemetryConfig, TelemetryError};

/// Guard that shuts down the tracer provider on drop.
pub struct TracingGuard {
    provider: opentelemetry_sdk::trace::TracerProvider,
}

impl Drop for TracingGuard {
    fn drop(&mut self) {
        if let Err(e) = self.provider.shutdown() {
            eprintln!("Error shutting down tracer provider: {:?}", e);
        }
    }
}

/// Initialize OpenTelemetry tracing with OTLP export to Tempo.
pub async fn init_tracing(config: &TelemetryConfig) -> Result<TracingGuard, TelemetryError> {
    // Create OTLP exporter
    let otlp_exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(&config.otlp_endpoint);

    // Build the tracer provider
    let provider = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(otlp_exporter)
        .with_trace_config(
            trace::Config::default()
                .with_sampler(Sampler::AlwaysOn)
                .with_id_generator(RandomIdGenerator::default())
                .with_resource(Resource::new(vec![
                    KeyValue::new("service.name", config.full_service_name()),
                    KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
                    KeyValue::new("deployment.environment", config.network.clone()),
                    KeyValue::new("qc.subsystem_id", config.subsystem_id.clone()),
                ])),
        )
        .install_batch(runtime::Tokio)
        .map_err(|e| TelemetryError::TracerInit(e.to_string()))?;

    // Create OpenTelemetry tracing layer
    let tracer = provider.tracer(config.full_service_name());
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    // Create env filter
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&config.log_level))
        .map_err(|e| TelemetryError::TracerInit(e.to_string()))?;

    // Build subscriber based on configuration
    if config.json_logs {
        // JSON output for containers/production
        let json_layer = tracing_subscriber::fmt::layer()
            .json()
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true);

        if config.console_output {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(otel_layer)
                .with(json_layer)
                .try_init()
                .map_err(|e| TelemetryError::TracerInit(e.to_string()))?;
        } else {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(otel_layer)
                .try_init()
                .map_err(|e| TelemetryError::TracerInit(e.to_string()))?;
        }
    } else {
        // Pretty output for development
        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_target(true)
            .with_thread_ids(false)
            .with_file(false)
            .with_line_number(false)
            .with_ansi(true);

        if config.console_output {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(otel_layer)
                .with(fmt_layer)
                .try_init()
                .map_err(|e| TelemetryError::TracerInit(e.to_string()))?;
        } else {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(otel_layer)
                .try_init()
                .map_err(|e| TelemetryError::TracerInit(e.to_string()))?;
        }
    }

    tracing::info!(
        service = %config.full_service_name(),
        otlp_endpoint = %config.otlp_endpoint,
        "OpenTelemetry tracing initialized"
    );

    Ok(TracingGuard { provider })
}

/// Create a span that will be sent to Tempo.
///
/// Note: Due to tracing macro requirements, use the `subsystem_span!` macro
/// for dynamic span creation, or use this for fixed span names.
pub fn create_span_with_attrs(attributes: &[(&str, &str)]) -> tracing::Span {
    let span = tracing::info_span!("operation");

    for (key, value) in attributes {
        span.record(*key, *value);
    }

    span
}

#[cfg(test)]
mod tests {
    // Tracing tests require async runtime and would conflict with global state
    // These are better tested in integration tests
}
