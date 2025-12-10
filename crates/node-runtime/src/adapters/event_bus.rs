//! # Event Bus Adapter
//!
//! Provides the event bus adapter that subsystems use for choreography.
//!
//! ## V2.3 Choreography Pattern
//!
//! This adapter wraps the event router and provides:
//! - Authenticated message publishing
//! - Topic-based subscription filtering
//! - Sender authorization validation

use std::sync::Arc;

use tokio::sync::broadcast;
use tracing::{debug, warn};

use shared_types::SubsystemId;

use crate::wiring::{ChoreographyEvent, EventRouter};

/// Event bus adapter for subsystem communication.
pub struct EventBusAdapter {
    /// The underlying event router.
    router: Arc<EventRouter>,
    /// This adapter's subsystem ID.
    subsystem_id: SubsystemId,
}

impl EventBusAdapter {
    /// Create a new event bus adapter for a specific subsystem.
    pub fn new(router: Arc<EventRouter>, subsystem_id: SubsystemId) -> Self {
        Self {
            router,
            subsystem_id,
        }
    }

    /// Publish an event to the bus.
    ///
    /// The event's sender_id must match this adapter's subsystem_id.
    pub fn publish(&self, event: ChoreographyEvent) -> Result<(), EventBusError> {
        // Verify sender matches this adapter
        let event_sender = match &event {
            ChoreographyEvent::BlockValidated { sender_id, .. } => *sender_id,
            ChoreographyEvent::MerkleRootComputed { sender_id, .. } => *sender_id,
            ChoreographyEvent::StateRootComputed { sender_id, .. } => *sender_id,
            ChoreographyEvent::BlockStored { sender_id, .. } => *sender_id,
            ChoreographyEvent::BlockFinalized { sender_id, .. } => *sender_id,
            ChoreographyEvent::TransactionsOrdered { sender_id, .. } => *sender_id,
            ChoreographyEvent::AssemblyTimeout { sender_id, .. } => *sender_id,
        };

        if event_sender != self.subsystem_id {
            warn!(
                "Subsystem {:?} attempted to publish event with sender_id {:?}",
                self.subsystem_id, event_sender
            );
            return Err(EventBusError::SenderMismatch {
                expected: self.subsystem_id,
                actual: event_sender,
            });
        }

        // Publish through the router (which does authorization check)
        self.router
            .publish(event)
            .map_err(|e| EventBusError::AuthorizationFailed(e.to_string()))
    }

    /// Subscribe to choreography events.
    pub fn subscribe(&self) -> broadcast::Receiver<ChoreographyEvent> {
        debug!(
            "Subsystem {:?} subscribing to choreography events",
            self.subsystem_id
        );
        self.router.subscribe()
    }

    /// Get the subsystem ID for this adapter.
    pub fn subsystem_id(&self) -> SubsystemId {
        self.subsystem_id
    }
}

/// Event bus errors.
#[derive(Debug)]
pub enum EventBusError {
    /// Sender ID in event doesn't match adapter's subsystem.
    SenderMismatch {
        expected: SubsystemId,
        actual: SubsystemId,
    },
    /// Authorization check failed.
    AuthorizationFailed(String),
    /// Failed to send message.
    SendFailed(String),
}

impl std::fmt::Display for EventBusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventBusError::SenderMismatch { expected, actual } => {
                write!(
                    f,
                    "Sender mismatch: adapter is {:?} but event has sender {:?}",
                    expected, actual
                )
            }
            EventBusError::AuthorizationFailed(msg) => {
                write!(f, "Authorization failed: {}", msg)
            }
            EventBusError::SendFailed(msg) => {
                write!(f, "Send failed: {}", msg)
            }
        }
    }
}

impl std::error::Error for EventBusError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiring::EventRouter;

    fn create_test_adapter(subsystem_id: SubsystemId) -> EventBusAdapter {
        let router = Arc::new(EventRouter::new(16));
        EventBusAdapter::new(router, subsystem_id)
    }

    #[test]
    fn test_publish_with_correct_sender() {
        let adapter = create_test_adapter(SubsystemId::Consensus);

        let event = ChoreographyEvent::BlockValidated {
            block_hash: [0u8; 32],
            block_height: 1,
            sender_id: SubsystemId::Consensus,
        };

        // Should succeed - sender matches adapter
        assert!(adapter.publish(event).is_ok());
    }

    #[test]
    fn test_publish_with_wrong_sender() {
        let adapter = create_test_adapter(SubsystemId::Mempool);

        let event = ChoreographyEvent::BlockValidated {
            block_hash: [0u8; 32],
            block_height: 1,
            sender_id: SubsystemId::Consensus, // Wrong! Adapter is Mempool
        };

        // Should fail - sender doesn't match adapter
        assert!(adapter.publish(event).is_err());
    }

    #[tokio::test]
    async fn test_subscribe_and_receive() {
        let router = Arc::new(EventRouter::new(16));

        let consensus_adapter = EventBusAdapter::new(Arc::clone(&router), SubsystemId::Consensus);

        let tx_indexing_adapter =
            EventBusAdapter::new(Arc::clone(&router), SubsystemId::TransactionIndexing);

        // TxIndexing subscribes
        let mut receiver = tx_indexing_adapter.subscribe();

        // Consensus publishes
        let event = ChoreographyEvent::BlockValidated {
            block_hash: [42u8; 32],
            block_height: 100,
            sender_id: SubsystemId::Consensus,
        };
        consensus_adapter.publish(event).unwrap();

        // TxIndexing receives
        let received = receiver.recv().await.unwrap();
        match received {
            ChoreographyEvent::BlockValidated { block_height, .. } => {
                assert_eq!(block_height, 100);
            }
            _ => panic!("Wrong event type"),
        }
    }
}
