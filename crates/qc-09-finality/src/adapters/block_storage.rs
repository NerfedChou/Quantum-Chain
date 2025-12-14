//! Block Storage Adapter
//!
//! Implements `BlockStorageGateway` port using the event bus.
//! Reference: SPEC-09-FINALITY.md Section 3.2, IPC-MATRIX.md

use crate::error::FinalityResult;
use crate::ports::outbound::{BlockStorageGateway, MarkFinalizedRequest};
use async_trait::async_trait;
use shared_bus::{BlockchainEvent, EventPublisher, InMemoryEventBus};
use std::sync::Arc;
use tracing::info;

/// Event bus adapter for Block Storage (Subsystem 2).
///
/// Instead of calling qc-02 directly, publishes `BlockFinalized` events
/// that qc-02 subscribes to via choreography (EDA pattern).
pub struct EventBusBlockStorageAdapter {
    event_bus: Arc<InMemoryEventBus>,
}

impl EventBusBlockStorageAdapter {
    /// Create a new adapter with the given event bus.
    pub fn new(event_bus: Arc<InMemoryEventBus>) -> Self {
        Self { event_bus }
    }
}

#[async_trait]
impl BlockStorageGateway for EventBusBlockStorageAdapter {
    async fn mark_finalized(&self, request: MarkFinalizedRequest) -> FinalityResult<()> {
        info!(
            "[qc-09] 📤 Publishing BlockFinalized event for block #{} (epoch: {})",
            request.block_height, request.finalized_epoch
        );

        // Publish event for qc-02 to consume (EDA choreography)
        let event = BlockchainEvent::BlockFinalized {
            block_height: request.block_height,
            block_hash: request.block_hash,
            finalized_epoch: request.finalized_epoch,
        };

        let receivers = self.event_bus.publish(event).await;

        if receivers == 0 {
            // No subscribers yet - this is acceptable during bootstrap
            info!(
                "[qc-09] ⚠️ No subscribers for BlockFinalized (bootstrap phase)"
            );
        }

        Ok(())
    }
}

/// In-memory mock adapter for testing.
#[derive(Default)]
pub struct MockBlockStorageAdapter {
    finalized_blocks: parking_lot::RwLock<Vec<MarkFinalizedRequest>>,
}

impl MockBlockStorageAdapter {
    /// Create a new mock adapter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all finalized block requests.
    pub fn get_finalized(&self) -> Vec<MarkFinalizedRequest> {
        self.finalized_blocks.read().clone()
    }
}

#[async_trait]
impl BlockStorageGateway for MockBlockStorageAdapter {
    async fn mark_finalized(&self, request: MarkFinalizedRequest) -> FinalityResult<()> {
        self.finalized_blocks.write().push(request);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::proof::FinalityProof;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_mock_adapter_stores_requests() {
        let adapter = MockBlockStorageAdapter::new();

        let request = MarkFinalizedRequest {
            correlation_id: Uuid::new_v4(),
            block_hash: [1u8; 32],
            block_height: 100,
            finalized_epoch: 10,
            finality_proof: FinalityProof::default(),
        };

        adapter.mark_finalized(request.clone()).await.unwrap();

        let finalized = adapter.get_finalized();
        assert_eq!(finalized.len(), 1);
        assert_eq!(finalized[0].block_height, 100);
    }

    #[tokio::test]
    async fn test_event_bus_adapter_publishes() {
        let event_bus = Arc::new(InMemoryEventBus::new());
        let adapter = EventBusBlockStorageAdapter::new(Arc::clone(&event_bus));

        let request = MarkFinalizedRequest {
            correlation_id: Uuid::new_v4(),
            block_hash: [2u8; 32],
            block_height: 200,
            finalized_epoch: 20,
            finality_proof: FinalityProof::default(),
        };

        let result = adapter.mark_finalized(request).await;
        assert!(result.is_ok());
    }
}
