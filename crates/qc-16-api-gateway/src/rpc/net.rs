//! Net JSON-RPC methods per SPEC-16 Section 3.1.

use crate::domain::error::{ApiError, ApiResult};
use crate::ipc::handler::IpcHandler;
use crate::ipc::requests::*;
use std::sync::Arc;
use tracing::instrument;

/// Net RPC methods handler
pub struct NetRpc {
    ipc: Arc<IpcHandler>,
    chain_id: u64,
}

impl NetRpc {
    pub fn new(ipc: Arc<IpcHandler>, chain_id: u64) -> Self {
        Self { ipc, chain_id }
    }

    /// net_version - Returns network ID (same as chain ID for most networks)
    #[instrument(skip(self))]
    pub async fn version(&self) -> ApiResult<String> {
        Ok(self.chain_id.to_string())
    }

    /// net_listening - Returns true if node is listening for connections
    /// Routes to qc-01 Peer Discovery per SPEC-16 Section 3.1
    #[instrument(skip(self))]
    pub async fn listening(&self) -> ApiResult<bool> {
        // Query peer discovery subsystem per SPEC-16
        let result = self
            .ipc
            .request(
                "qc-01-peer-discovery",
                RequestPayload::GetNodeInfo(GetNodeInfoRequest),
                None,
            )
            .await;

        // If peer discovery subsystem responds, we're listening
        Ok(result.is_ok())
    }

    /// net_peerCount - Returns number of connected peers
    /// Routes to qc-01 Peer Discovery per SPEC-16 Section 3.1
    #[instrument(skip(self))]
    pub async fn peer_count(&self) -> ApiResult<String> {
        let result = self
            .ipc
            .request(
                "qc-01-peer-discovery",
                RequestPayload::GetPeers(GetPeersRequest),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        // Parse result as array and count
        let count = if let Some(arr) = result.as_array() {
            arr.len()
        } else {
            0
        };

        Ok(format!("0x{:x}", count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_net_version() {
        // Would need mock IPC
    }
}
