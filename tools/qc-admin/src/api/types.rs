//! API response types matching qc-16 debug.rs

use serde::{Deserialize, Serialize};

/// Status of a subsystem (mirrors qc-16 SubsystemStatus)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiSubsystemStatus {
    Running,
    Stopped,
    Degraded,
    Error,
    Unknown,
    NotImplemented,
}

/// Health info for a single subsystem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiSubsystemHealth {
    pub id: String,
    pub name: String,
    pub status: ApiSubsystemStatus,
    pub implemented: bool,
    #[serde(default)]
    pub uptime_ms: Option<u64>,
    #[serde(default)]
    pub memory_bytes: Option<u64>,
    #[serde(default)]
    pub ipc_sent: u64,
    #[serde(default)]
    pub ipc_received: u64,
    #[serde(default)]
    pub pending_requests: u32,
    #[serde(default)]
    pub avg_latency_ms: u32,
    #[serde(default)]
    pub connections: Vec<String>,
    #[serde(default)]
    pub last_heartbeat_ms: Option<u64>,
    /// Subsystem-specific metrics (different for each subsystem)
    #[serde(default)]
    pub specific_metrics: Option<serde_json::Value>,
}

/// Response from debug_subsystemHealth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemHealthResponse {
    pub subsystems: Vec<ApiSubsystemHealth>,
    pub timestamp_ms: u64,
    pub gateway_uptime_ms: u64,
}

/// Peer information from admin_peers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Peer's node ID (hex string)
    #[serde(default)]
    pub id: String,
    /// Peer's enode URL
    #[serde(default)]
    pub enode: String,
    /// Peer's name/client info
    #[serde(default)]
    pub name: String,
    /// Remote address
    #[serde(default, alias = "remoteAddress")]
    pub remote_address: String,
    /// Local address
    #[serde(default, alias = "localAddress")]
    pub local_address: String,
    /// Capabilities
    #[serde(default)]
    pub caps: Vec<String>,
    /// Network info
    #[serde(default)]
    pub network: Option<PeerNetwork>,
}

/// Network info for a peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerNetwork {
    #[serde(default, alias = "localAddress")]
    pub local_address: String,
    #[serde(default, alias = "remoteAddress")]
    pub remote_address: String,
    #[serde(default)]
    pub inbound: bool,
    #[serde(default)]
    pub trusted: bool,
    #[serde(default)]
    pub static_node: bool,
}

/// System resource metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu_percent: f32,
    pub memory_percent: f32,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
}

/// JSON-RPC request structure
#[derive(Debug, Serialize)]
pub struct JsonRpcRequest<T> {
    pub jsonrpc: &'static str,
    pub method: String,
    pub params: T,
    pub id: u64,
}

impl<T> JsonRpcRequest<T> {
    pub fn new(method: impl Into<String>, params: T, id: u64) -> Self {
        Self {
            jsonrpc: "2.0",
            method: method.into(),
            params,
            id,
        }
    }
}

/// JSON-RPC response structure
#[derive(Debug, Deserialize)]
pub struct JsonRpcResponse<T> {
    pub jsonrpc: String,
    pub id: u64,
    pub result: Option<T>,
    #[serde(default)]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC error
#[derive(Debug, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RPC Error {}: {}", self.code, self.message)
    }
}
