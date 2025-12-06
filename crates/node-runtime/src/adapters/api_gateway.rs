//! API Gateway adapters for connecting to event bus.
//!
//! Per SPEC-16 Section 6, the API Gateway communicates with subsystems
//! via the Event Bus, not direct function calls.

use async_trait::async_trait;
use qc_16_api_gateway::ipc::{IpcError, IpcRequest, IpcSender};
use shared_bus::{BlockchainEvent, EventPublisher, InMemoryEventBus};
use std::sync::Arc;
use tracing::debug;

/// Event bus adapter that implements IpcSender for API Gateway.
///
/// Translates API Gateway requests into blockchain events and publishes
/// them to the shared event bus.
pub struct EventBusIpcSender {
    /// Reference to the event bus
    bus: Arc<InMemoryEventBus>,
}

impl EventBusIpcSender {
    /// Create a new event bus IPC sender.
    pub fn new(bus: Arc<InMemoryEventBus>) -> Self {
        Self { bus }
    }
}

#[async_trait]
impl IpcSender for EventBusIpcSender {
    async fn send(&self, request: IpcRequest) -> Result<(), IpcError> {
        debug!(
            correlation_id = %request.correlation_id,
            target = %request.target,
            "Forwarding API request to event bus"
        );

        // Convert IPC request to ApiQuery blockchain event
        let event = BlockchainEvent::ApiQuery {
            correlation_id: request.correlation_id.to_string(),
            target: request.target.clone(),
            method: request.method_name(),
            params: request.payload_as_json(),
        };

        // Publish to event bus
        let receivers = self.bus.publish(event).await;

        debug!(
            correlation_id = %request.correlation_id,
            receivers = receivers,
            "ApiQuery published to event bus"
        );

        if receivers == 0 {
            // No subscribers - this is a warning but not an error
            // The pending request store will timeout if no response comes
            debug!("No subscribers received the ApiQuery event");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_bus_ipc_sender_creation() {
        let bus = Arc::new(InMemoryEventBus::new());
        let sender = EventBusIpcSender::new(bus);
        assert!(Arc::strong_count(&sender.bus) >= 1);
    }
}
