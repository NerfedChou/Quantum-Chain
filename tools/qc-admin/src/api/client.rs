//! Admin API client for communicating with qc-16 gateway.

use std::time::Duration;

use reqwest::Client;
use thiserror::Error;

use super::types::*;

/// Errors that can occur when communicating with the admin API.
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JSON-RPC error: {0}")]
    Rpc(String),
    #[error("Failed to parse response: {0}")]
    Parse(String),
    #[error("Connection failed: {0}")]
    Connection(String),
}

/// Admin API client.
pub struct AdminApiClient {
    client: Client,
    base_url: String,
    request_id: std::sync::atomic::AtomicU64,
}

impl AdminApiClient {
    /// Create a new admin API client.
    pub fn new(base_url: impl Into<String>) -> Result<Self, ApiError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .connect_timeout(Duration::from_secs(2))
            .build()
            .map_err(ApiError::Http)?;

        Ok(Self {
            client,
            base_url: base_url.into(),
            request_id: std::sync::atomic::AtomicU64::new(1),
        })
    }

    /// Get the next request ID.
    fn next_id(&self) -> u64 {
        self.request_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    /// Call a JSON-RPC method.
    async fn call<P: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: P,
    ) -> Result<R, ApiError> {
        let request = JsonRpcRequest::new(method, params, self.next_id());

        let response = self
            .client
            .post(&self.base_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    ApiError::Connection(format!("Cannot connect to {}", self.base_url))
                } else {
                    ApiError::Http(e)
                }
            })?;

        let rpc_response: JsonRpcResponse<R> = response
            .json()
            .await
            .map_err(|e| ApiError::Parse(e.to_string()))?;

        if let Some(error) = rpc_response.error {
            return Err(ApiError::Rpc(error.to_string()));
        }

        rpc_response
            .result
            .ok_or_else(|| ApiError::Parse("Missing result in response".to_string()))
    }

    /// Get health status of all subsystems.
    pub async fn get_subsystem_health(&self) -> Result<SubsystemHealthResponse, ApiError> {
        self.call::<[(); 0], SubsystemHealthResponse>("debug_subsystemHealth", [])
            .await
    }

    /// Get list of connected peers from qc-01.
    pub async fn get_peers(&self) -> Result<Vec<PeerInfo>, ApiError> {
        self.call::<[(); 0], Vec<PeerInfo>>("admin_peers", [])
            .await
    }

    /// Get system resource metrics (CPU, memory).
    /// Note: This may need to be implemented separately or pulled from system.
    pub async fn get_system_metrics(&self) -> Result<SystemMetrics, ApiError> {
        // Try to get from admin API first
        // If not available, fall back to local system metrics
        match self
            .call::<[(); 0], SystemMetrics>("debug_systemMetrics", [])
            .await
        {
            Ok(metrics) => Ok(metrics),
            Err(_) => {
                // Fall back to reading from /proc on Linux
                Ok(get_local_system_metrics())
            }
        }
    }

    /// Check if the admin API is reachable.
    pub async fn is_connected(&self) -> bool {
        self.get_subsystem_health().await.is_ok()
    }
}

/// Get local system metrics from /proc filesystem (Linux).
fn get_local_system_metrics() -> SystemMetrics {
    let mut metrics = SystemMetrics::default();

    // Read CPU usage from /proc/stat
    if let Ok(stat) = std::fs::read_to_string("/proc/stat") {
        if let Some(cpu_line) = stat.lines().next() {
            let parts: Vec<&str> = cpu_line.split_whitespace().collect();
            if parts.len() >= 5 {
                // Parse CPU times: user, nice, system, idle, iowait...
                let user: u64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                let nice: u64 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
                let system: u64 = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
                let idle: u64 = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
                let iowait: u64 = parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);

                let total = user + nice + system + idle + iowait;
                let active = user + nice + system;

                if total > 0 {
                    // This is cumulative, so we'll show a rough percentage
                    // In a real implementation, we'd track deltas over time
                    metrics.cpu_percent = (active as f32 / total as f32) * 100.0;
                }
            }
        }
    }

    // Read memory usage from /proc/meminfo
    if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
        let mut mem_total: u64 = 0;
        let mut mem_available: u64 = 0;

        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                mem_total = parse_meminfo_value(line);
            } else if line.starts_with("MemAvailable:") {
                mem_available = parse_meminfo_value(line);
            }
        }

        if mem_total > 0 {
            let mem_used = mem_total.saturating_sub(mem_available);
            metrics.memory_total_bytes = mem_total * 1024; // kB to bytes
            metrics.memory_used_bytes = mem_used * 1024;
            metrics.memory_percent = (mem_used as f32 / mem_total as f32) * 100.0;
        }
    }

    metrics
}

/// Parse a value from /proc/meminfo (e.g., "MemTotal:       16384 kB")
fn parse_meminfo_value(line: &str) -> u64 {
    line.split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_system_metrics() {
        let metrics = get_local_system_metrics();
        // Just verify it doesn't panic and returns something
        assert!(metrics.memory_total_bytes > 0 || cfg!(not(target_os = "linux")));
    }
}
