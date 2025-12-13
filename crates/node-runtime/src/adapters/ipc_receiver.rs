//! # Event Bus IPC Receiver
//!
//! Receives `ApiQueryResponse` events from the Event Bus and completes
//! pending requests in the `PendingRequestStore`.
//!
//! ## Architecture (per SPEC-16 Section 6)
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │  ApiQueryHandler (node-runtime)                                             │
//! │  - Receives ApiQuery events                                                 │
//! │  - Calls subsystem APIs                                                     │
//! │  - Publishes ApiQueryResponse                                               │
//! └─────────────────────────────────────────────────────────────────────────────┘
//!                                     │
//!                                     │ ApiQueryResponse
//!                                     ▼
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │  Event Bus                                                                  │
//! └─────────────────────────────────────────────────────────────────────────────┘
//!                                     │
//!                                     │ EventBusIpcReceiver subscribes
//!                                     ▼
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │  EventBusIpcReceiver (this module)                                          │
//! │  - Subscribes to ApiQueryResponse events                                    │
//! │  - Parses correlation ID                                                    │
//! │  - Calls pending_store.complete()                                           │
//! └─────────────────────────────────────────────────────────────────────────────┘
//!                                     │
//!                                     │ oneshot::Sender
//!                                     ▼
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │  PendingRequestStore (qc-16)                                                │
//! │  - Waiting HTTP handlers receive response                                   │
//! │  - Returns JSON-RPC response to client                                      │
//! └─────────────────────────────────────────────────────────────────────────────┘
//! ```

use qc_16_api_gateway::adapters::pending::{PendingRequestStore, ResponseError};
use qc_16_api_gateway::domain::CorrelationId;
use shared_bus::{BlockchainEvent, EventFilter, EventTopic, InMemoryEventBus, Subscription};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Receiver that listens for `ApiQueryResponse` events and completes pending requests.
pub struct EventBusIpcReceiver {
    /// Event bus subscription
    subscription: Subscription,
    /// Pending request store to complete requests
    pending_store: Arc<PendingRequestStore>,
}

impl EventBusIpcReceiver {
    /// Create a new receiver.
    ///
    /// Subscribes to `ApiGateway` topic events on the event bus.
    pub fn new(bus: &InMemoryEventBus, pending_store: Arc<PendingRequestStore>) -> Self {
        // Subscribe to ApiGateway topic to receive ApiQueryResponse events
        let filter = EventFilter::topics(vec![EventTopic::ApiGateway]);
        let subscription = bus.subscribe(filter);

        Self {
            subscription,
            pending_store,
        }
    }

    /// Start receiving responses.
    ///
    /// This runs in a loop, receiving ApiQueryResponse events and completing
    /// pending requests. Should be spawned as a background task.
    pub async fn run(mut self) {
        info!("[EventBusIpcReceiver] Started listening for API query responses");

        loop {
            let event = match self.subscription.recv().await {
                Some(e) => e,
                None => {
                    error!("[EventBusIpcReceiver] Event bus closed, shutting down");
                    break;
                }
            };

            let BlockchainEvent::ApiQueryResponse {
                correlation_id,
                source,
                result,
            } = event
            else {
                continue;
            };

            debug!(
                correlation_id = %correlation_id,
                source = source,
                success = result.is_ok(),
                "Received API query response"
            );

            // Parse correlation ID
            let Ok(cid) = CorrelationId::parse(&correlation_id) else {
                error!(
                    correlation_id = %correlation_id,
                    "Failed to parse correlation ID"
                );
                continue;
            };

            // Convert result to ResponseError format
            let response_result = match result {
                Ok(value) => Ok(value),
                Err(api_error) => Err(ResponseError {
                    code: api_error.code,
                    message: api_error.message,
                    data: None,
                }),
            };

            // Complete the pending request
            let completed = self.pending_store.complete(cid, response_result);

            if completed {
                debug!(correlation_id = %correlation_id, "Completed pending request");
            } else {
                warn!(
                    correlation_id = %correlation_id,
                    "No pending request found for correlation ID (may have timed out)"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_receiver_creation() {
        let bus = InMemoryEventBus::new();
        let store = Arc::new(PendingRequestStore::new(Duration::from_secs(30)));

        let _receiver = EventBusIpcReceiver::new(&bus, store);
        // No panic = success
    }
}
