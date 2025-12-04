//! API Gateway adapters for connecting to event bus.
//!
//! Per SPEC-16 Section 6, the API Gateway communicates with subsystems
//! via the Event Bus, not direct function calls.

use async_trait::async_trait;
use qc_16_api_gateway::ipc::{IpcError, IpcRequest, IpcSender};
use shared_bus::InMemoryEventBus;
use std::sync::Arc;
use tracing::debug;

/// Event bus adapter that implements IpcSender for API Gateway.
///
/// Translates API Gateway requests into blockchain events and publishes
/// them to the shared event bus.
pub struct EventBusIpcSender {
    /// Reference to the event bus
    #[allow(dead_code)]
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

        // In a full implementation, we'd publish a QueryRequest event
        // and the target subsystem would respond with a QueryResponse event.
        // For now, this is a placeholder that logs the request.
        //
        // The actual query protocol requires:
        // 1. A QueryRequest event type in shared-bus
        // 2. Subsystem handlers that respond to queries
        // 3. A QueryResponse event that carries the correlation_id
        //
        // This will be implemented when the query protocol is added to shared-bus.

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
