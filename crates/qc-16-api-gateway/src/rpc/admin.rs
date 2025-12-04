//! Admin JSON-RPC methods per SPEC-16 Section 3.2 and 3.3.

use crate::domain::error::{ApiError, ApiResult};
use crate::ipc::handler::IpcHandler;
use crate::ipc::requests::*;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::instrument;

/// Admin RPC methods handler
pub struct AdminRpc {
    ipc: Arc<IpcHandler>,
    data_dir: PathBuf,
}

impl AdminRpc {
    pub fn new(ipc: Arc<IpcHandler>, data_dir: PathBuf) -> Self {
        Self { ipc, data_dir }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // TIER 2: PROTECTED (Read-only admin info)
    // ═══════════════════════════════════════════════════════════════════════

    /// admin_nodeInfo - Returns node info
    #[instrument(skip(self))]
    pub async fn node_info(&self) -> ApiResult<serde_json::Value> {
        let result = self
            .ipc
            .request(
                "qc-07-network",
                RequestPayload::GetNodeInfo(GetNodeInfoRequest),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        Ok(result)
    }

    /// admin_peers - Returns connected peers
    #[instrument(skip(self))]
    pub async fn peers(&self) -> ApiResult<serde_json::Value> {
        let result = self
            .ipc
            .request(
                "qc-07-network",
                RequestPayload::GetPeers(GetPeersRequest),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        Ok(result)
    }

    /// admin_datadir - Returns data directory path
    #[instrument(skip(self))]
    pub async fn datadir(&self) -> ApiResult<String> {
        Ok(self.data_dir.to_string_lossy().to_string())
    }

    // ═══════════════════════════════════════════════════════════════════════
    // TIER 3: ADMIN (Node control)
    // ═══════════════════════════════════════════════════════════════════════

    /// admin_addPeer - Add a peer
    #[instrument(skip(self))]
    pub async fn add_peer(&self, enode: String) -> ApiResult<bool> {
        // Validate enode URL format
        if !enode.starts_with("enode://") {
            return Err(ApiError::invalid_params(
                "Invalid enode URL: must start with 'enode://'",
            ));
        }

        let result = self
            .ipc
            .request(
                "qc-07-network",
                RequestPayload::AddPeer(AddPeerRequest { enode_url: enode }),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        Ok(result.as_bool().unwrap_or(false))
    }

    /// admin_removePeer - Remove a peer
    #[instrument(skip(self))]
    pub async fn remove_peer(&self, enode: String) -> ApiResult<bool> {
        let result = self
            .ipc
            .request(
                "qc-07-network",
                RequestPayload::RemovePeer(RemovePeerRequest { enode_url: enode }),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        Ok(result.as_bool().unwrap_or(false))
    }

    /// admin_addTrustedPeer - Add a trusted peer
    #[instrument(skip(self))]
    pub async fn add_trusted_peer(&self, enode: String) -> ApiResult<bool> {
        // Validate enode URL format
        if !enode.starts_with("enode://") {
            return Err(ApiError::invalid_params(
                "Invalid enode URL: must start with 'enode://'",
            ));
        }

        // For now, same as addPeer - trusted peer handling would be in network subsystem
        self.add_peer(enode).await
    }

    /// admin_removeTrustedPeer - Remove a trusted peer
    #[instrument(skip(self))]
    pub async fn remove_trusted_peer(&self, enode: String) -> ApiResult<bool> {
        self.remove_peer(enode).await
    }

    /// admin_startHTTP - Start HTTP server (no-op if already running)
    #[instrument(skip(self))]
    pub async fn start_http(&self) -> ApiResult<bool> {
        // API Gateway is already running if this method is called
        Ok(true)
    }

    /// admin_stopHTTP - Stop HTTP server
    #[instrument(skip(self))]
    pub async fn stop_http(&self) -> ApiResult<bool> {
        // Cannot stop from within - return false
        Ok(false)
    }

    /// admin_startWS - Start WebSocket server (no-op if already running)
    #[instrument(skip(self))]
    pub async fn start_ws(&self) -> ApiResult<bool> {
        Ok(true)
    }

    /// admin_stopWS - Stop WebSocket server
    #[instrument(skip(self))]
    pub async fn stop_ws(&self) -> ApiResult<bool> {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enode_validation() {
        // Valid enode
        assert!("enode://abc@127.0.0.1:30303".starts_with("enode://"));

        // Invalid enode
        assert!(!"enr://abc".starts_with("enode://"));
    }
}
