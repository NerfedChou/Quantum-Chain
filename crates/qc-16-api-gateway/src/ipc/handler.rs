//! IPC handler for event bus communication.

use crate::domain::pending::{PendingRequestStore, ResponseError};
use crate::ipc::requests::{IpcRequest, RequestPayload};
use crate::ipc::responses::{IpcResponse, ResponsePayload, SuccessData};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

/// IPC handler trait for sending requests to subsystems
#[async_trait]
pub trait IpcSender: Send + Sync {
    /// Send a request to a subsystem
    async fn send(&self, request: IpcRequest) -> Result<(), IpcError>;
}

/// IPC receiver trait for receiving responses from subsystems
#[async_trait]
pub trait IpcReceiver: Send + Sync {
    /// Receive next response (blocks until available)
    async fn receive(&self) -> Result<IpcResponse, IpcError>;
}

/// IPC error types
#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    #[error("channel closed")]
    ChannelClosed,
    #[error("send failed: {0}")]
    SendFailed(String),
    #[error("receive failed: {0}")]
    ReceiveFailed(String),
    #[error("timeout")]
    Timeout,
    #[error("subsystem unavailable: {0}")]
    SubsystemUnavailable(String),
}

/// IPC handler that bridges API Gateway to event bus
pub struct IpcHandler {
    /// Pending request store for correlation
    pending: Arc<PendingRequestStore>,
    /// Sender for outgoing requests
    sender: Arc<dyn IpcSender>,
    /// Default timeout
    default_timeout: Duration,
}

impl IpcHandler {
    pub fn new(
        pending: Arc<PendingRequestStore>,
        sender: Arc<dyn IpcSender>,
        default_timeout: Duration,
    ) -> Self {
        Self {
            pending,
            sender,
            default_timeout,
        }
    }

    /// Send request and wait for response
    pub async fn request(
        &self,
        target: &str,
        payload: RequestPayload,
        timeout: Option<Duration>,
    ) -> Result<serde_json::Value, ResponseError> {
        let method = payload_method_name(&payload);
        let timeout = timeout.unwrap_or(self.default_timeout);

        // Register pending request
        let (correlation_id, rx) = self.pending.register(method, Some(timeout));

        // Create and send IPC request
        let request = IpcRequest::with_correlation_id(correlation_id, target, payload);

        if let Err(e) = self.sender.send(request).await {
            // Remove from pending if send fails
            self.pending.cancel(&correlation_id);
            return Err(ResponseError {
                code: -32603,
                message: format!("IPC send failed: {}", e),
                data: None,
            });
        }

        debug!(
            correlation_id = %correlation_id,
            target = target,
            method = method,
            "Sent IPC request"
        );

        // Wait for response
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(response)) => response.result,
            Ok(Err(_)) => {
                // Channel was dropped
                Err(ResponseError {
                    code: -32603,
                    message: "Response channel closed".into(),
                    data: None,
                })
            }
            Err(_) => {
                // Timeout
                self.pending.cancel(&correlation_id);
                Err(ResponseError {
                    code: -32006,
                    message: format!("Request timed out after {}s", timeout.as_secs()),
                    data: None,
                })
            }
        }
    }

    /// Get pending request count
    pub fn pending_count(&self) -> usize {
        self.pending.pending_count()
    }
}

/// Response listener that processes incoming IPC responses
pub struct ResponseListener {
    pending: Arc<PendingRequestStore>,
    receiver: Arc<dyn IpcReceiver>,
}

impl ResponseListener {
    pub fn new(pending: Arc<PendingRequestStore>, receiver: Arc<dyn IpcReceiver>) -> Self {
        Self { pending, receiver }
    }

    /// Run the listener loop
    pub async fn run(self) {
        loop {
            match self.receiver.receive().await {
                Ok(response) => {
                    self.handle_response(response);
                }
                Err(IpcError::ChannelClosed) => {
                    warn!("IPC receiver channel closed, stopping listener");
                    break;
                }
                Err(e) => {
                    error!(error = %e, "Error receiving IPC response");
                }
            }
        }
    }

    fn handle_response(&self, response: IpcResponse) {
        let result = match response.payload {
            ResponsePayload::Success(data) => Ok(success_to_json(data)),
            ResponsePayload::Error(e) => Err(ResponseError {
                code: e.code,
                message: e.message,
                data: e.data,
            }),
        };

        if !self.pending.complete(response.correlation_id, result) {
            debug!(
                correlation_id = %response.correlation_id,
                "Response for unknown or expired correlation ID"
            );
        }
    }
}

/// Convert success data to JSON
fn success_to_json(data: SuccessData) -> serde_json::Value {
    match data {
        SuccessData::Balance(v) => serde_json::to_value(v).unwrap_or_default(),
        SuccessData::Code(v) => serde_json::to_value(v).unwrap_or_default(),
        SuccessData::StorageValue(v) => serde_json::to_value(v).unwrap_or_default(),
        SuccessData::TransactionCount(v) => serde_json::json!(format!("0x{:x}", v)),
        SuccessData::BlockNumber(v) => serde_json::json!(format!("0x{:x}", v)),
        SuccessData::GasPrice(v) => serde_json::to_value(v).unwrap_or_default(),
        SuccessData::GasEstimate(v) => serde_json::to_value(v).unwrap_or_default(),
        SuccessData::TransactionHash(v) => serde_json::to_value(v).unwrap_or_default(),
        SuccessData::Block(v) => serde_json::to_value(v).unwrap_or_default(),
        SuccessData::Transaction(v) => serde_json::to_value(v).unwrap_or_default(),
        SuccessData::Receipt(v) => serde_json::to_value(v).unwrap_or_default(),
        SuccessData::Logs(v) => serde_json::to_value(v).unwrap_or_default(),
        SuccessData::SyncStatus(v) => serde_json::to_value(v).unwrap_or_default(),
        SuccessData::TxPoolStatus(v) => serde_json::to_value(v).unwrap_or_default(),
        SuccessData::TxPoolContent(v) => serde_json::to_value(v).unwrap_or_default(),
        SuccessData::Peers(v) => serde_json::to_value(v).unwrap_or_default(),
        SuccessData::NodeInfo(v) => serde_json::to_value(v).unwrap_or_default(),
        SuccessData::Bool(v) => serde_json::json!(v),
        SuccessData::Null => serde_json::Value::Null,
    }
}

/// Get method name from payload for logging
fn payload_method_name(payload: &RequestPayload) -> &'static str {
    match payload {
        RequestPayload::GetBalance(_) => "eth_getBalance",
        RequestPayload::GetCode(_) => "eth_getCode",
        RequestPayload::GetStorageAt(_) => "eth_getStorageAt",
        RequestPayload::GetTransactionCount(_) => "eth_getTransactionCount",
        RequestPayload::GetBlockByHash(_) => "eth_getBlockByHash",
        RequestPayload::GetBlockByNumber(_) => "eth_getBlockByNumber",
        RequestPayload::GetBlockNumber(_) => "eth_blockNumber",
        RequestPayload::GetFeeHistory(_) => "eth_feeHistory",
        RequestPayload::GetTransactionByHash(_) => "eth_getTransactionByHash",
        RequestPayload::GetTransactionReceipt(_) => "eth_getTransactionReceipt",
        RequestPayload::GetLogs(_) => "eth_getLogs",
        RequestPayload::GetBlockReceipts(_) => "eth_getBlockReceipts",
        RequestPayload::Call(_) => "eth_call",
        RequestPayload::EstimateGas(_) => "eth_estimateGas",
        RequestPayload::SubmitTransaction(_) => "eth_sendRawTransaction",
        RequestPayload::GetGasPrice(_) => "eth_gasPrice",
        RequestPayload::GetMaxPriorityFeePerGas(_) => "eth_maxPriorityFeePerGas",
        RequestPayload::GetTxPoolStatus(_) => "txpool_status",
        RequestPayload::GetTxPoolContent(_) => "txpool_content",
        RequestPayload::GetPeers(_) => "admin_peers",
        RequestPayload::GetNodeInfo(_) => "admin_nodeInfo",
        RequestPayload::GetSyncStatus(_) => "eth_syncing",
        RequestPayload::AddPeer(_) => "admin_addPeer",
        RequestPayload::RemovePeer(_) => "admin_removePeer",
    }
}

/// In-memory IPC channel for testing
pub mod channel {
    use super::*;

    pub struct ChannelSender(pub mpsc::Sender<IpcRequest>);
    pub struct ChannelReceiver(pub mpsc::Receiver<IpcResponse>);

    #[async_trait]
    impl IpcSender for ChannelSender {
        async fn send(&self, request: IpcRequest) -> Result<(), IpcError> {
            self.0
                .send(request)
                .await
                .map_err(|_| IpcError::ChannelClosed)
        }
    }

    #[async_trait]
    impl IpcReceiver for tokio::sync::Mutex<ChannelReceiver> {
        async fn receive(&self) -> Result<IpcResponse, IpcError> {
            let mut guard = self.lock().await;
            guard.0.recv().await.ok_or(IpcError::ChannelClosed)
        }
    }

    /// Create a test IPC channel pair
    pub fn create_test_channel(
        buffer: usize,
    ) -> (
        mpsc::Sender<IpcRequest>,
        mpsc::Receiver<IpcRequest>,
        mpsc::Sender<IpcResponse>,
        mpsc::Receiver<IpcResponse>,
    ) {
        let (req_tx, req_rx) = mpsc::channel(buffer);
        let (resp_tx, resp_rx) = mpsc::channel(buffer);
        (req_tx, req_rx, resp_tx, resp_rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::requests::GetBlockNumberRequest;

    #[tokio::test]
    async fn test_payload_method_name() {
        assert_eq!(
            payload_method_name(&RequestPayload::GetBlockNumber(GetBlockNumberRequest)),
            "eth_blockNumber"
        );
    }
}
