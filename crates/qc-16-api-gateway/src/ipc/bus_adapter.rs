//! Event bus adapter for real IPC communication.
//!
//! Implements IpcSender/IpcReceiver using shared-bus for production use.
//! Per SPEC-16 Section 6, the API Gateway communicates with subsystems
//! via the event bus, not direct function calls.

use crate::CorrelationId;
use crate::ipc::handler::{IpcError, IpcReceiver, IpcSender};
use crate::ipc::requests::{IpcRequest, RequestPayload};
use crate::ipc::responses::IpcResponse;
use async_trait::async_trait;
use futures::StreamExt;
use shared_bus::{BlockchainEvent, EventFilter, EventPublisher, InMemoryEventBus};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, warn};

/// Event bus adapter that implements IpcSender for production use.
///
/// Translates API Gateway requests into blockchain events and publishes
/// them to the shared event bus.
pub struct EventBusSender {
    /// Reference to the event bus
    bus: Arc<InMemoryEventBus>,
    /// Subsystem ID for this gateway instance
    pub subsystem_id: u8,
}

impl EventBusSender {
    /// Create a new event bus sender.
    ///
    /// # Arguments
    ///
    /// * `bus` - Reference to the shared event bus
    /// * `subsystem_id` - ID of this gateway (16 per IPC-MATRIX)
    pub fn new(bus: Arc<InMemoryEventBus>, subsystem_id: u8) -> Self {
        Self { bus, subsystem_id }
    }
}

#[async_trait]
impl IpcSender for EventBusSender {
    async fn send(&self, request: IpcRequest) -> Result<(), IpcError> {
        // Convert the internal request to an Event Bus ApiQuery event
        let method = payload_to_method(&request.payload);
        let params = payload_to_params(&request.payload);

        debug!(
            correlation_id = %request.correlation_id,
            target = %request.target,
            method = %method,
            "Publishing ApiQuery to event bus"
        );

        // Create the ApiQuery event per shared-bus event protocol
        let event = BlockchainEvent::ApiQuery {
            correlation_id: request.correlation_id.to_string(),
            target: request.target.clone(),
            method: method.to_string(),
            params,
        };

        // Publish to the event bus - the ApiQueryHandler in node-runtime
        // will receive this and dispatch to the appropriate subsystem
        let receivers = self.bus.publish(event).await;

        if receivers == 0 {
            warn!(
                correlation_id = %request.correlation_id,
                target = %request.target,
                "No subscribers for ApiQuery (ApiQueryHandler may not be running)"
            );
        } else {
            debug!(
                correlation_id = %request.correlation_id,
                receivers = receivers,
                "ApiQuery delivered to {} subscriber(s)",
                receivers
            );
        }

        Ok(())
    }
}

/// Convert RequestPayload to method name for event bus
fn payload_to_method(payload: &RequestPayload) -> &'static str {
    match payload {
        RequestPayload::GetBalance(_) => "get_balance",
        RequestPayload::GetCode(_) => "get_code",
        RequestPayload::GetStorageAt(_) => "get_storage_at",
        RequestPayload::GetTransactionCount(_) => "get_transaction_count",
        RequestPayload::GetBlockByHash(_) => "get_block_by_hash",
        RequestPayload::GetBlockByNumber(_) => "get_block_by_number",
        RequestPayload::GetBlockNumber(_) => "get_block_number",
        RequestPayload::GetFeeHistory(_) => "get_fee_history",
        RequestPayload::GetTransactionByHash(_) => "get_transaction_by_hash",
        RequestPayload::GetTransactionReceipt(_) => "get_transaction_receipt",
        RequestPayload::GetLogs(_) => "get_logs",
        RequestPayload::GetBlockReceipts(_) => "get_block_receipts",
        RequestPayload::Call(_) => "call",
        RequestPayload::EstimateGas(_) => "estimate_gas",
        RequestPayload::SubmitTransaction(_) => "submit_transaction",
        RequestPayload::GetGasPrice(_) => "get_gas_price",
        RequestPayload::GetMaxPriorityFeePerGas(_) => "get_max_priority_fee_per_gas",
        RequestPayload::GetTxPoolStatus(_) => "get_txpool_status",
        RequestPayload::GetTxPoolContent(_) => "get_txpool_content",
        RequestPayload::GetPeers(_) => "get_peers",
        RequestPayload::GetNodeInfo(_) => "get_node_info",
        RequestPayload::GetSyncStatus(_) => "get_sync_status",
        RequestPayload::AddPeer(_) => "add_peer",
        RequestPayload::RemovePeer(_) => "remove_peer",
        RequestPayload::Ping => "ping",
        RequestPayload::GetSubsystemMetrics(_) => "get_subsystem_metrics",
    }
}

/// Convert RequestPayload to JSON params for event bus
fn payload_to_params(payload: &RequestPayload) -> serde_json::Value {
    // Serialize the payload data, handling errors gracefully
    serde_json::to_value(payload).unwrap_or_else(|_| serde_json::Value::Object(Default::default()))
}

/// Event bus receiver that listens for response events.
pub struct EventBusReceiver {
    /// Channel to receive responses
    response_rx: RwLock<mpsc::Receiver<IpcResponse>>,
}

impl EventBusReceiver {
    /// Create a new event bus receiver.
    pub fn new(response_rx: mpsc::Receiver<IpcResponse>) -> Self {
        Self {
            response_rx: RwLock::new(response_rx),
        }
    }
}

#[async_trait]
impl IpcReceiver for EventBusReceiver {
    async fn receive(&self) -> Result<IpcResponse, IpcError> {
        let mut rx = self.response_rx.write().await;
        rx.recv().await.ok_or(IpcError::ChannelClosed)
    }
}

/// Response router that routes blockchain events to pending requests.
///
/// Subscribes to ApiGateway topic and converts ApiQueryResponse events
/// to IpcResponse messages for the pending request store.
pub struct ResponseRouter {
    /// Event bus subscription
    bus: Arc<InMemoryEventBus>,
    /// Channel to send responses to the receiver
    response_tx: mpsc::Sender<IpcResponse>,
}

impl ResponseRouter {
    /// Create a new response router.
    pub fn new(bus: Arc<InMemoryEventBus>, response_tx: mpsc::Sender<IpcResponse>) -> Self {
        Self { bus, response_tx }
    }

    /// Start listening for ApiQueryResponse events and routing them.
    ///
    /// This should be spawned as a background task.
    pub async fn run(self) {
        use tracing::info;

        info!("[ResponseRouter] Started listening for ApiQueryResponse events");

        // Subscribe to ApiGateway topic to receive ApiQueryResponse events
        let filter = EventFilter::topics(vec![shared_bus::EventTopic::ApiGateway]);
        let mut stream = self.bus.event_stream(filter);

        loop {
            match stream.next().await {
                Some(event) => {
                    if let Some(response) = self.event_to_response(&event) {
                        debug!(
                            correlation_id = %response.correlation_id,
                            "Routing ApiQueryResponse to pending request"
                        );
                        if self.response_tx.send(response).await.is_err() {
                            warn!("Response channel closed, stopping router");
                            break;
                        }
                    }
                }
                None => {
                    // Stream ended
                    warn!("[ResponseRouter] Event stream ended, shutting down");
                    break;
                }
            }
        }
    }

    /// Convert a blockchain event to an IPC response if applicable.
    fn event_to_response(&self, event: &BlockchainEvent) -> Option<IpcResponse> {
        use crate::ipc::responses::{ErrorData, ResponsePayload, SuccessData};

        match event {
            BlockchainEvent::ApiQueryResponse {
                correlation_id,
                source,
                result,
            } => {
                // Parse correlation ID, or generate new one if parsing fails
                let correlation = CorrelationId::parse(correlation_id).unwrap_or_else(|_| {
                    warn!(
                        correlation_id = %correlation_id,
                        "Failed to parse correlation ID from response"
                    );
                    CorrelationId::new()
                });

                let payload = match result {
                    Ok(data) => ResponsePayload::Success(SuccessData::Json(data.clone())),
                    Err(e) => ResponsePayload::Error(ErrorData {
                        code: e.code,
                        message: e.message.clone(),
                        data: None,
                    }),
                };

                Some(IpcResponse {
                    correlation_id: correlation,
                    source: *source,
                    payload,
                })
            }
            _ => None, // Ignore non-response events
        }
    }
}

/// Query router that dispatches requests to appropriate subsystems.
///
/// Per SPEC-16 Section 6, different methods route to different subsystems:
/// - eth_getBalance → qc-04-state-management
/// - eth_getBlock* → qc-02-block-storage
/// - eth_sendRawTransaction → qc-06-mempool
/// - etc.
#[derive(Default)]
pub struct QueryRouter {
    /// State management queries (qc-04)
    pub state_tx: Option<mpsc::Sender<StateQuery>>,
    /// Block storage queries (qc-02)
    pub block_tx: Option<mpsc::Sender<BlockQuery>>,
    /// Transaction indexing queries (qc-03)
    pub tx_index_tx: Option<mpsc::Sender<TxIndexQuery>>,
    /// Mempool queries (qc-06)
    pub mempool_tx: Option<mpsc::Sender<MempoolQuery>>,
    /// Peer discovery queries (qc-01)
    pub peer_discovery_tx: Option<mpsc::Sender<PeerDiscoveryQuery>>,
}

/// Query to state management subsystem
#[derive(Debug)]
pub struct StateQuery {
    pub correlation_id: CorrelationId,
    pub payload: RequestPayload,
    pub response_tx: mpsc::Sender<IpcResponse>,
}

/// Query to block storage subsystem
#[derive(Debug)]
pub struct BlockQuery {
    pub correlation_id: CorrelationId,
    pub payload: RequestPayload,
    pub response_tx: mpsc::Sender<IpcResponse>,
}

/// Query to transaction indexing subsystem
#[derive(Debug)]
pub struct TxIndexQuery {
    pub correlation_id: CorrelationId,
    pub payload: RequestPayload,
    pub response_tx: mpsc::Sender<IpcResponse>,
}

/// Query to mempool subsystem
#[derive(Debug)]
pub struct MempoolQuery {
    pub correlation_id: CorrelationId,
    pub payload: RequestPayload,
    pub response_tx: mpsc::Sender<IpcResponse>,
}

/// Query to peer discovery subsystem (qc-01)
#[derive(Debug)]
pub struct PeerDiscoveryQuery {
    pub correlation_id: CorrelationId,
    pub payload: RequestPayload,
    pub response_tx: mpsc::Sender<IpcResponse>,
}

impl QueryRouter {
    /// Create an empty query router (for testing or standalone mode)
    pub fn empty() -> Self {
        Self::default()
    }

    /// Route a request to the appropriate subsystem
    pub async fn route(
        &self,
        correlation_id: CorrelationId,
        payload: RequestPayload,
        response_tx: mpsc::Sender<IpcResponse>,
    ) -> Result<(), IpcError> {
        match &payload {
            // State management (qc-04)
            RequestPayload::GetBalance(_)
            | RequestPayload::GetCode(_)
            | RequestPayload::GetStorageAt(_)
            | RequestPayload::GetTransactionCount(_) => {
                if let Some(tx) = &self.state_tx {
                    let query = StateQuery {
                        correlation_id,
                        payload,
                        response_tx,
                    };
                    tx.send(query).await.map_err(|_| IpcError::ChannelClosed)?;
                } else {
                    return Err(IpcError::SubsystemUnavailable(
                        "qc-04-state-management".into(),
                    ));
                }
            }

            // Block storage (qc-02)
            RequestPayload::GetBlockByHash(_)
            | RequestPayload::GetBlockByNumber(_)
            | RequestPayload::GetBlockNumber(_)
            | RequestPayload::GetFeeHistory(_) => {
                if let Some(tx) = &self.block_tx {
                    let query = BlockQuery {
                        correlation_id,
                        payload,
                        response_tx,
                    };
                    tx.send(query).await.map_err(|_| IpcError::ChannelClosed)?;
                } else {
                    return Err(IpcError::SubsystemUnavailable("qc-02-block-storage".into()));
                }
            }

            // Transaction indexing (qc-03)
            RequestPayload::GetTransactionByHash(_)
            | RequestPayload::GetTransactionReceipt(_)
            | RequestPayload::GetLogs(_)
            | RequestPayload::GetBlockReceipts(_) => {
                if let Some(tx) = &self.tx_index_tx {
                    let query = TxIndexQuery {
                        correlation_id,
                        payload,
                        response_tx,
                    };
                    tx.send(query).await.map_err(|_| IpcError::ChannelClosed)?;
                } else {
                    return Err(IpcError::SubsystemUnavailable(
                        "qc-03-transaction-indexing".into(),
                    ));
                }
            }

            // Mempool (qc-06)
            RequestPayload::SubmitTransaction(_)
            | RequestPayload::GetGasPrice(_)
            | RequestPayload::GetMaxPriorityFeePerGas(_)
            | RequestPayload::GetTxPoolStatus(_)
            | RequestPayload::GetTxPoolContent(_) => {
                if let Some(tx) = &self.mempool_tx {
                    let query = MempoolQuery {
                        correlation_id,
                        payload,
                        response_tx,
                    };
                    tx.send(query).await.map_err(|_| IpcError::ChannelClosed)?;
                } else {
                    return Err(IpcError::SubsystemUnavailable("qc-06-mempool".into()));
                }
            }

            // Peer Discovery (qc-01)
            RequestPayload::GetPeers(_)
            | RequestPayload::GetNodeInfo(_)
            | RequestPayload::AddPeer(_)
            | RequestPayload::RemovePeer(_) => {
                if let Some(tx) = &self.peer_discovery_tx {
                    let query = PeerDiscoveryQuery {
                        correlation_id,
                        payload,
                        response_tx,
                    };
                    tx.send(query).await.map_err(|_| IpcError::ChannelClosed)?;
                } else {
                    return Err(IpcError::SubsystemUnavailable(
                        "qc-01-peer-discovery".into(),
                    ));
                }
            }

            // Sync status (node-runtime)
            RequestPayload::GetSyncStatus(_) => {
                // Sync status is handled by node-runtime, not a subsystem channel
                return Err(IpcError::SubsystemUnavailable("node-runtime".into()));
            }

            // Contract execution (qc-11)
            RequestPayload::Call(_) | RequestPayload::EstimateGas(_) => {
                return Err(IpcError::SubsystemUnavailable(
                    "qc-11-smart-contracts".into(),
                ));
            }

            // Ping - lightweight health check (returns immediately)
            RequestPayload::Ping => {
                // Ping doesn't need routing - just acknowledge receipt
                let response = IpcResponse {
                    correlation_id,
                    source: 16, // API Gateway
                    payload: crate::ipc::responses::ResponsePayload::Success(
                        crate::ipc::responses::SuccessData::Bool(true),
                    ),
                };
                response_tx
                    .send(response)
                    .await
                    .map_err(|_| IpcError::ChannelClosed)?;
            }

            // Admin metrics query - routed to admin handler
            RequestPayload::GetSubsystemMetrics(_) => {
                // Route to admin target via event bus
                // The target is set to "admin" in the request
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_bus_sender_creation() {
        let bus = Arc::new(InMemoryEventBus::new());
        let sender = EventBusSender::new(bus, 16);
        assert_eq!(sender.subsystem_id, 16);
    }

    #[test]
    fn test_query_router_empty() {
        let router = QueryRouter::empty();
        assert!(router.state_tx.is_none());
        assert!(router.block_tx.is_none());
    }

    #[tokio::test]
    async fn test_query_router_unavailable() {
        use crate::ipc::requests::GetBlockNumberRequest;

        let router = QueryRouter::empty();
        let (tx, _rx) = mpsc::channel(1);
        let correlation_id = CorrelationId::new();

        let result = router
            .route(
                correlation_id,
                RequestPayload::GetBlockNumber(GetBlockNumberRequest),
                tx,
            )
            .await;

        assert!(matches!(result, Err(IpcError::SubsystemUnavailable(_))));
    }
}
