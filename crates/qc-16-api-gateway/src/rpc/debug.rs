//! Debug JSON-RPC methods per SPEC-16 Section 3.3 (Admin tier).

use crate::{ApiError, ApiResult};
use crate::domain::types::{BlockId, Hash};
use crate::ipc::handler::IpcHandler;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tracing::instrument;

/// Debug RPC methods handler
///
/// ADMIN TIER ONLY - These methods expose internal state and can be resource-intensive.
pub struct DebugRpc {
    ipc: Arc<IpcHandler>,
    start_time: Instant,
}

impl DebugRpc {
    pub fn new(ipc: Arc<IpcHandler>) -> Self {
        Self {
            ipc,
            start_time: Instant::now(),
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // SUBSYSTEM HEALTH (Admin Panel Support)
    // ═══════════════════════════════════════════════════════════════════════

    /// debug_subsystemHealth - Returns health status of all subsystems
    /// Used by qc-admin panel for monitoring
    #[instrument(skip(self))]
    pub async fn subsystem_health(&self) -> ApiResult<SubsystemHealthResponse> {
        let mut subsystems = Vec::new();
        let uptime_ms = self.start_time.elapsed().as_millis() as u64;

        // Query each subsystem for health via IPC
        // For now, we return the gateway's view of subsystem connectivity
        for (id, name, code) in SUBSYSTEM_INFO.iter() {
            let health = self.query_subsystem_health(*id, name, code).await;
            subsystems.push(health);
        }

        Ok(SubsystemHealthResponse {
            subsystems,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            gateway_uptime_ms: uptime_ms,
        })
    }

    /// Query health for a single subsystem
    async fn query_subsystem_health(&self, id: u8, name: &str, code: &str) -> SubsystemHealth {
        // First check if this is a stub/unimplemented subsystem
        if is_stub_subsystem(id) {
            return SubsystemHealth {
                id: format!("qc-{:02}", id),
                name: name.to_string(),
                status: SubsystemStatus::NotImplemented,
                implemented: false,
                uptime_ms: None,
                memory_bytes: None,
                ipc_sent: 0,
                ipc_received: 0,
                pending_requests: 0,
                avg_latency_ms: 0,
                connections: vec![],
                last_heartbeat_ms: None,
                specific_metrics: None,
            };
        }

        // Try to ping the subsystem via IPC
        let target = format!("qc-{:02}-{}", id, code);

        // Attempt a lightweight health check
        let (status, latency_ms) = match self.ipc.health_check(&target).await {
            Ok(latency) => (SubsystemStatus::Running, Some(latency)),
            Err(_) => (SubsystemStatus::Stopped, None),
        };

        // Query subsystem-specific metrics via IPC
        let specific_metrics = self.query_specific_metrics(id).await;

        SubsystemHealth {
            id: format!("qc-{:02}", id),
            name: name.to_string(),
            status,
            implemented: true,
            uptime_ms: if status == SubsystemStatus::Running {
                Some(self.start_time.elapsed().as_millis() as u64)
            } else {
                None
            },
            memory_bytes: None,
            ipc_sent: 0,
            ipc_received: 0,
            pending_requests: 0,
            avg_latency_ms: latency_ms.unwrap_or(0) as u32,
            connections: vec![],
            last_heartbeat_ms: latency_ms,
            specific_metrics,
        }
    }

    /// Query subsystem-specific metrics via IPC
    async fn query_specific_metrics(&self, subsystem_id: u8) -> Option<serde_json::Value> {
        use crate::ipc::requests::{GetSubsystemMetricsRequest, RequestPayload};
        use std::time::Duration;

        let payload =
            RequestPayload::GetSubsystemMetrics(GetSubsystemMetricsRequest { subsystem_id });

        (self
            .ipc
            .request("admin", payload, Some(Duration::from_millis(500)))
            .await)
            .ok()
    }

    /// debug_ipcMetrics - Returns IPC metrics for subsystem communication
    #[instrument(skip(self))]
    pub async fn ipc_metrics(&self) -> ApiResult<IpcMetricsResponse> {
        let metrics = self.ipc.get_metrics();

        Ok(IpcMetricsResponse {
            metrics: IpcMetrics {
                total_sent: metrics.total_sent,
                total_received: metrics.total_received,
                total_errors: metrics.total_errors,
                total_timeouts: metrics.total_timeouts,
                requests_per_sec: metrics.requests_per_sec,
                errors_per_sec: metrics.errors_per_sec,
                p50_latency_ms: metrics.p50_latency_ms,
                p99_latency_ms: metrics.p99_latency_ms,
                by_subsystem: metrics
                    .by_subsystem
                    .into_iter()
                    .map(|(k, v)| SubsystemIpcMetrics {
                        subsystem_id: k,
                        sent: v.sent,
                        received: v.received,
                        errors: v.errors,
                        timeouts: v.timeouts,
                        avg_latency_ms: v.avg_latency_ms,
                    })
                    .collect(),
            },
            window_seconds: 60,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        })
    }

    /// debug_subsystemStatus - Returns detailed status for a specific subsystem
    #[instrument(skip(self))]
    pub async fn subsystem_status(
        &self,
        subsystem_id: String,
    ) -> ApiResult<SubsystemDetailedStatus> {
        // Parse subsystem ID (e.g., "qc-01" or "01")
        let id_num: u8 = subsystem_id
            .trim_start_matches("qc-")
            .parse()
            .map_err(|_| ApiError::invalid_params("Invalid subsystem ID format"))?;

        let (_, name, code) = SUBSYSTEM_INFO
            .iter()
            .find(|(id, _, _)| *id == id_num)
            .ok_or_else(|| ApiError::invalid_params("Unknown subsystem ID"))?;

        let health = self.query_subsystem_health(id_num, name, code).await;

        Ok(SubsystemDetailedStatus {
            id: health.id,
            name: health.name,
            status: health.status,
            uptime_ms: health.uptime_ms.unwrap_or(0),
            memory_bytes: health.memory_bytes.unwrap_or(0),
            connections: health
                .connections
                .into_iter()
                .map(|c| ConnectionInfo {
                    target_id: c,
                    status: SubsystemStatus::Unknown,
                    last_message_ms: 0,
                    message_count: 0,
                })
                .collect(),
        })
    }

    /// debug_traceTransaction - Trace transaction execution
    #[instrument(skip(self))]
    pub async fn trace_transaction(
        &self,
        hash: Hash,
        options: Option<TraceOptions>,
    ) -> ApiResult<serde_json::Value> {
        // This would require deep integration with EVM execution
        // For now, return a placeholder structure
        Err(ApiError::method_not_supported(
            "debug_traceTransaction requires EVM tracing integration",
        ))
    }

    /// debug_traceBlockByHash - Trace all transactions in block
    #[instrument(skip(self))]
    pub async fn trace_block_by_hash(
        &self,
        hash: Hash,
        options: Option<TraceOptions>,
    ) -> ApiResult<Vec<serde_json::Value>> {
        Err(ApiError::method_not_supported(
            "debug_traceBlockByHash requires EVM tracing integration",
        ))
    }

    /// debug_traceBlockByNumber - Trace all transactions in block
    #[instrument(skip(self))]
    pub async fn trace_block_by_number(
        &self,
        block_id: BlockId,
        options: Option<TraceOptions>,
    ) -> ApiResult<Vec<serde_json::Value>> {
        Err(ApiError::method_not_supported(
            "debug_traceBlockByNumber requires EVM tracing integration",
        ))
    }

    /// debug_traceCall - Trace call without transaction
    #[instrument(skip(self))]
    pub async fn trace_call(
        &self,
        call: serde_json::Value,
        block_id: BlockId,
        options: Option<TraceOptions>,
    ) -> ApiResult<serde_json::Value> {
        Err(ApiError::method_not_supported(
            "debug_traceCall requires EVM tracing integration",
        ))
    }

    /// debug_getRawBlock - Returns raw block bytes
    #[instrument(skip(self))]
    pub async fn get_raw_block(&self, block_id: BlockId) -> ApiResult<String> {
        // Would query block storage for RLP-encoded block
        Err(ApiError::method_not_supported(
            "debug_getRawBlock not implemented",
        ))
    }

    /// debug_getRawHeader - Returns raw header bytes
    #[instrument(skip(self))]
    pub async fn get_raw_header(&self, block_id: BlockId) -> ApiResult<String> {
        Err(ApiError::method_not_supported(
            "debug_getRawHeader not implemented",
        ))
    }

    /// debug_getRawTransaction - Returns raw transaction bytes
    #[instrument(skip(self))]
    pub async fn get_raw_transaction(&self, hash: Hash) -> ApiResult<String> {
        Err(ApiError::method_not_supported(
            "debug_getRawTransaction not implemented",
        ))
    }

    /// debug_getRawReceipts - Returns raw receipts for block
    #[instrument(skip(self))]
    pub async fn get_raw_receipts(&self, block_id: BlockId) -> ApiResult<Vec<String>> {
        Err(ApiError::method_not_supported(
            "debug_getRawReceipts not implemented",
        ))
    }

    /// debug_setHead - Set chain head (DANGEROUS)
    #[instrument(skip(self))]
    pub async fn set_head(&self, block_number: u64) -> ApiResult<bool> {
        Err(ApiError::method_not_supported(
            "debug_setHead is disabled for safety",
        ))
    }

    /// debug_storageRangeAt - Returns storage range at block
    #[instrument(skip(self))]
    pub async fn storage_range_at(
        &self,
        block_hash: Hash,
        tx_index: u64,
        contract_address: Hash,
        key_start: Hash,
        max_result: u64,
    ) -> ApiResult<serde_json::Value> {
        Err(ApiError::method_not_supported(
            "debug_storageRangeAt not implemented",
        ))
    }
}

/// Trace options for debug methods
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceOptions {
    /// Tracer to use (callTracer, prestateTracer, etc.)
    #[serde(default)]
    pub tracer: Option<String>,
    /// Tracer configuration
    #[serde(default)]
    pub tracer_config: Option<serde_json::Value>,
    /// Timeout for trace operation
    #[serde(default)]
    pub timeout: Option<String>,
    /// Disable storage capture
    #[serde(default)]
    pub disable_storage: bool,
    /// Disable stack capture
    #[serde(default)]
    pub disable_stack: bool,
    /// Enable memory capture
    #[serde(default)]
    pub enable_memory: bool,
    /// Enable return data capture
    #[serde(default)]
    pub enable_return_data: bool,
}

// ═══════════════════════════════════════════════════════════════════════════
// SUBSYSTEM HEALTH TYPES (Admin Panel Support)
// ═══════════════════════════════════════════════════════════════════════════

/// All 16 subsystems with their info
const SUBSYSTEM_INFO: &[(u8, &str, &str)] = &[
    (1, "Peer Discovery", "peer-discovery"),
    (2, "Block Storage", "block-storage"),
    (3, "Transaction Indexing", "transaction-indexing"),
    (4, "State Management", "state-management"),
    (5, "Block Propagation", "block-propagation"),
    (6, "Mempool", "mempool"),
    (7, "Bloom Filters", "bloom-filters"),
    (8, "Consensus", "consensus"),
    (9, "Finality", "finality"),
    (10, "Signature Verification", "signature-verification"),
    (11, "Smart Contracts", "smart-contracts"),
    (12, "Transaction Ordering", "transaction-ordering"),
    (13, "Light Client Sync", "light-client-sync"),
    (14, "Sharding", "sharding"),
    (15, "Cross-Chain", "cross-chain"),
    (16, "API Gateway", "api-gateway"),
    (17, "Block Production", "block-production"),
];

/// Check if a subsystem is a stub (not yet implemented)
fn is_stub_subsystem(id: u8) -> bool {
    matches!(id, 7 | 11 | 12 | 13 | 14 | 15)
}

/// Status of a subsystem
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubsystemStatus {
    Running,
    Stopped,
    Degraded,
    Error,
    Unknown,
    NotImplemented,
}

/// Health info for a single subsystem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemHealth {
    pub id: String,
    pub name: String,
    pub status: SubsystemStatus,
    /// Whether this subsystem is implemented (false for stubs)
    pub implemented: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_bytes: Option<u64>,
    pub ipc_sent: u64,
    pub ipc_received: u64,
    pub pending_requests: u32,
    pub avg_latency_ms: u32,
    pub connections: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_heartbeat_ms: Option<u64>,
    /// Subsystem-specific metrics (different for each subsystem)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub specific_metrics: Option<serde_json::Value>,
}

/// Response from debug_subsystemHealth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemHealthResponse {
    pub subsystems: Vec<SubsystemHealth>,
    pub timestamp_ms: u64,
    pub gateway_uptime_ms: u64,
}

/// IPC metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcMetrics {
    pub total_sent: u64,
    pub total_received: u64,
    pub total_errors: u64,
    pub total_timeouts: u64,
    pub requests_per_sec: f64,
    pub errors_per_sec: f64,
    pub p50_latency_ms: u32,
    pub p99_latency_ms: u32,
    pub by_subsystem: Vec<SubsystemIpcMetrics>,
}

/// Per-subsystem IPC metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemIpcMetrics {
    pub subsystem_id: String,
    pub sent: u64,
    pub received: u64,
    pub errors: u64,
    pub timeouts: u64,
    pub avg_latency_ms: u32,
}

/// Response from debug_ipcMetrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcMetricsResponse {
    pub metrics: IpcMetrics,
    pub window_seconds: u32,
    pub timestamp_ms: u64,
}

/// Detailed status for a single subsystem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemDetailedStatus {
    pub id: String,
    pub name: String,
    pub status: SubsystemStatus,
    pub uptime_ms: u64,
    pub memory_bytes: u64,
    pub connections: Vec<ConnectionInfo>,
}

/// Connection info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub target_id: String,
    pub status: SubsystemStatus,
    pub last_message_ms: u64,
    pub message_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_options_default() {
        let opts = TraceOptions::default();
        assert!(opts.tracer.is_none());
        assert!(!opts.disable_storage);
    }

    #[test]
    fn test_subsystem_info_complete() {
        assert_eq!(SUBSYSTEM_INFO.len(), 17);
    }

    #[test]
    fn test_stub_subsystems() {
        assert!(is_stub_subsystem(7));
        assert!(is_stub_subsystem(11));
        assert!(!is_stub_subsystem(1));
        assert!(!is_stub_subsystem(16));
    }
}
