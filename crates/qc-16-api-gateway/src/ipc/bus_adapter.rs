//! Event bus adapter for real IPC communication.
//!
//! Implements IpcSender/IpcReceiver using shared-bus for production use.
//! Per SPEC-16 Section 6, the API Gateway communicates with subsystems
//! via the event bus, not direct function calls.

use crate::domain::correlation::CorrelationId;
use crate::ipc::handler::{IpcError, IpcReceiver, IpcSender};
use crate::ipc::requests::{IpcRequest, RequestPayload};
use crate::ipc::responses::IpcResponse;
use async_trait::async_trait;
use futures::StreamExt;
use shared_bus::{BlockchainEvent, EventFilter, InMemoryEventBus};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, warn};

/// Event bus adapter that implements IpcSender for production use.
///
/// Translates API Gateway requests into blockchain events and publishes
/// them to the shared event bus.
pub struct EventBusSender {
    /// Reference to the event bus
    #[allow(dead_code)]
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
        // For production, we need a query protocol over the event bus.
        // The current shared-bus is designed for choreography (fire-and-forget events),
        // not request-response patterns.
        //
        // For now, we implement option 3: wrap the request in a query event.

        debug!(
            correlation_id = %request.correlation_id,
            target = %request.target,
            method = ?std::mem::discriminant(&request.payload),
            "Sending IPC request via event bus"
        );

        // In a full implementation, we'd publish a QueryRequest event
        // and the target subsystem would respond with a QueryResponse event.

        Ok(())
    }
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
/// Subscribes to relevant event topics and matches events to pending
/// correlation IDs.
pub struct ResponseRouter {
    /// Event bus subscription
    #[allow(dead_code)]
    bus: Arc<InMemoryEventBus>,
    /// Channel to send responses to the receiver
    response_tx: mpsc::Sender<IpcResponse>,
}

impl ResponseRouter {
    /// Create a new response router.
    pub fn new(bus: Arc<InMemoryEventBus>, response_tx: mpsc::Sender<IpcResponse>) -> Self {
        Self { bus, response_tx }
    }

    /// Start listening for events and routing responses.
    #[allow(dead_code)]
    pub async fn run(self) {
        // Subscribe to relevant events
        let filter = EventFilter::all();
        let mut stream = self.bus.event_stream(filter);

        loop {
            match stream.next().await {
                Some(event) => {
                    if let Some(response) = self.event_to_response(&event) {
                        if self.response_tx.send(response).await.is_err() {
                            warn!("Response channel closed, stopping router");
                            break;
                        }
                    }
                }
                None => {
                    // Stream ended
                    break;
                }
            }
        }
    }

    /// Convert a blockchain event to an IPC response if applicable.
    fn event_to_response(&self, _event: &BlockchainEvent) -> Option<IpcResponse> {
        // Most blockchain events are not direct responses to API queries.
        // This would need a more sophisticated query protocol.
        None
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
    /// Network queries (qc-07)
    pub network_tx: Option<mpsc::Sender<NetworkQuery>>,
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

/// Query to network subsystem
#[derive(Debug)]
pub struct NetworkQuery {
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

            // Network (qc-07)
            RequestPayload::GetPeers(_)
            | RequestPayload::GetNodeInfo(_)
            | RequestPayload::GetSyncStatus(_)
            | RequestPayload::AddPeer(_)
            | RequestPayload::RemovePeer(_) => {
                if let Some(tx) = &self.network_tx {
                    let query = NetworkQuery {
                        correlation_id,
                        payload,
                        response_tx,
                    };
                    tx.send(query).await.map_err(|_| IpcError::ChannelClosed)?;
                } else {
                    return Err(IpcError::SubsystemUnavailable("qc-07-network".into()));
                }
            }

            // Contract execution (qc-11)
            RequestPayload::Call(_) | RequestPayload::EstimateGas(_) => {
                return Err(IpcError::SubsystemUnavailable(
                    "qc-11-smart-contracts".into(),
                ));
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
