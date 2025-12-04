# Quantum Telemetry Integration Guide

This document explains how to integrate LGTM telemetry into Quantum-Chain subsystems.

## Overview

The `quantum-telemetry` crate provides:

- **Metrics** (Prometheus → Mimir): Counters, gauges, histograms
- **Tracing** (OpenTelemetry → Tempo): Distributed spans across subsystems
- **Logging** (Structured JSON → Loki): Searchable, filterable logs

## Quick Start

### 1. Add Dependency

```toml
# In your subsystem's Cargo.toml
[dependencies]
quantum-telemetry = { path = "../quantum-telemetry" }
```

### 2. Initialize in Node Runtime

The telemetry is initialized once in `node-runtime/main.rs`:

```rust
use quantum_telemetry::{TelemetryConfig, init_telemetry};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize LGTM stack
    let config = TelemetryConfig::from_env();
    let _guard = init_telemetry(config).await?;
    
    // ... rest of application
}
```

### 3. Add Tracing to Functions

```rust
use tracing::{info, info_span, warn, error, instrument};

// Option 1: Use #[instrument] attribute for automatic span creation
#[instrument(skip(self), fields(subsystem = "consensus", block_height = %height))]
async fn validate_block(&self, height: u64, block: &Block) -> Result<()> {
    info!("Starting block validation");
    
    // ... validation logic
    
    if validation_failed {
        warn!(reason = "invalid_signature", "Block validation failed");
    }
    
    info!("Block validation complete");
    Ok(())
}

// Option 2: Manual span creation
fn process_transaction(&self, tx: &Transaction) {
    let span = info_span!(
        "process_transaction",
        subsystem = "mempool",
        tx_hash = %tx.hash(),
    );
    let _guard = span.enter();
    
    // ... processing
}
```

### 4. Record Metrics

```rust
use quantum_telemetry::{
    BLOCKS_VALIDATED, MEMPOOL_SIZE, SIGNATURE_VERIFICATIONS,
    time_histogram, BLOCK_VALIDATION_DURATION,
};

fn validate_block(&self, block: &Block) {
    // Start timer (auto-records on drop)
    let _timer = time_histogram!(BLOCK_VALIDATION_DURATION);
    
    // ... validation
    
    // Increment counter
    BLOCKS_VALIDATED.inc();
    
    // Set gauge
    MEMPOOL_SIZE.set(self.pool.len() as f64);
    
    // Increment with labels
    SIGNATURE_VERIFICATIONS
        .with_label_values(&["ecdsa", "valid"])
        .inc();
}
```

### 5. Propagate Trace Context (Cross-Subsystem)

When sending events via the bus, include trace context:

```rust
use quantum_telemetry::{TraceContext, PropagatedContext};

// In sending subsystem (e.g., Consensus)
fn emit_block_validated(&self, block: &Block) {
    // Extract current trace context
    let trace_ctx = TraceContext::extract_current().to_propagated();
    
    let event = BlockValidatedEvent {
        block_hash: block.hash(),
        block_height: block.height,
        trace_context: trace_ctx,  // Include in event
    };
    
    self.bus.publish(event);
}

// In receiving subsystem (e.g., Transaction Indexing)
fn handle_block_validated(&self, event: BlockValidatedEvent) {
    // Continue the trace from the sending subsystem
    let parent = event.trace_context.to_context();
    let span = parent.child_span_for_subsystem("tx_indexing", "compute_merkle");
    let _guard = span.enter();
    
    // ... process event (this span will be linked to the parent)
}
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4317` | Tempo OTLP endpoint |
| `OTEL_SERVICE_NAME` | `quantum-chain` | Service name in traces |
| `LOKI_ENDPOINT` | `http://localhost:3100` | Loki endpoint |
| `QC_LOG_LEVEL` | `info` | Log level (trace/debug/info/warn/error) |
| `QC_SUBSYSTEM_ID` | `00` | Subsystem ID for labeling |
| `QC_JSON_LOGS` | auto | JSON logs (auto-detected in containers) |
| `QC_METRICS_PORT` | `9100` | Prometheus scrape port |

## Available Metrics

### Consensus (qc-08)
- `qc_consensus_blocks_validated_total` - Total blocks validated
- `qc_consensus_block_validation_duration_seconds` - Validation time histogram
- `qc_consensus_rounds_total{algorithm,outcome}` - Consensus rounds

### Finality (qc-09)
- `qc_finality_blocks_finalized_total` - Total blocks finalized
- `qc_finality_epochs_total` - Epochs processed
- `qc_finality_finalized_height` - Current finalized height
- `qc_finality_epochs_without_finality` - Circuit breaker indicator

### Mempool (qc-06)
- `qc_mempool_transactions_received_total` - Transactions received
- `qc_mempool_transactions_pending` - Current pool size
- `qc_mempool_size_bytes` - Pool size in bytes

### Signatures (qc-10)
- `qc_signature_verifications_total{type,result}` - Verification counts
- `qc_signature_failures_total` - Failed verifications
- `qc_signature_verification_duration_seconds{type}` - Verification time

### Storage (qc-02)
- `qc_storage_blocks_stored_total` - Blocks written
- `qc_storage_chain_height` - Current height
- `qc_storage_block_write_duration_seconds` - Write time

### Peers (qc-01)
- `qc_peers_connected` - Active connections
- `qc_peers_discovered_total` - Total discovered
- `qc_peers_connection_attempts_total{outcome}` - Connection attempts

### Event Bus (IPC)
- `qc_eventbus_messages_sent_total{event_type,source}` - Messages sent
- `qc_eventbus_messages_received_total{event_type,target}` - Messages received
- `qc_eventbus_delivery_latency_seconds` - Delivery latency

## Grafana Queries

### Loki (Logs)

```logql
# All errors in finality
{subsystem="finality"} |= "ERROR"

# Circuit breaker warnings
{job="quantum-chain"} |~ "circuit.?breaker"

# Logs with specific trace ID
{trace_id="abc123"}
```

### Prometheus (Metrics)

```promql
# Blocks per minute
rate(qc_consensus_blocks_validated_total[1m]) * 60

# p99 validation latency
histogram_quantile(0.99, rate(qc_consensus_block_validation_duration_seconds_bucket[5m]))

# Mempool size trend
qc_mempool_transactions_pending
```

### Tempo (Traces)

```
# Slow transactions (>500ms)
{ duration > 500ms }

# Traces through consensus
{ resource.service.name = "qc-08-consensus" }

# Failed operations
{ status = error }
```

## Docker Setup

```bash
# Start LGTM stack
cd docker
docker compose --profile monitoring up

# Access:
# - Grafana: http://localhost:3000 (admin/quantum)
# - Prometheus: http://localhost:9090
# - Tempo: http://localhost:3200
# - Loki: http://localhost:3100
```
