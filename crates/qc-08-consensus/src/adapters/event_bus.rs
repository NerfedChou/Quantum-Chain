//! Event Bus adapter
//!
//! Implements the EventBus port for publishing BlockValidated events

use crate::domain::{ValidatedBlock, ValidationProof};
use crate::events::BlockValidatedEvent;
use crate::ports::EventBus;
use async_trait::async_trait;
use shared_types::Hash;

/// In-memory event bus adapter for testing
pub struct InMemoryEventBus {
    events: parking_lot::RwLock<Vec<BlockValidatedEvent>>,
}

impl InMemoryEventBus {
    pub fn new() -> Self {
        Self {
            events: parking_lot::RwLock::new(Vec::new()),
        }
    }

    pub fn get_events(&self) -> Vec<BlockValidatedEvent> {
        self.events.read().clone()
    }

    pub fn event_count(&self) -> usize {
        self.events.read().len()
    }
}

impl Default for InMemoryEventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventBus for InMemoryEventBus {
    async fn publish_block_validated(
        &self,
        block_hash: Hash,
        block_height: u64,
        block: ValidatedBlock,
        consensus_proof: ValidationProof,
        validated_at: u64,
    ) -> Result<(), String> {
        let event = BlockValidatedEvent {
            block_hash,
            block_height,
            block,
            consensus_proof,
            validated_at,
        };

        self.events.write().push(event);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{BlockHeader, PoSProof};

    #[tokio::test]
    async fn test_in_memory_event_bus() {
        let bus = InMemoryEventBus::new();

        let block = ValidatedBlock {
            header: BlockHeader {
                version: 1,
                block_height: 1,
                parent_hash: [0u8; 32],
                timestamp: 1000,
                proposer: [0u8; 32],
                transactions_root: None,
                state_root: None,
                receipts_root: [0u8; 32],
                gas_limit: 30_000_000,
                gas_used: 0,
                extra_data: vec![],
            },
            transactions: vec![],
            validation_proof: ValidationProof::PoS(PoSProof {
                attestations: vec![],
                epoch: 1,
                slot: 0,
            }),
        };

        let result = bus
            .publish_block_validated(
                [1u8; 32],
                1,
                block,
                ValidationProof::PoS(PoSProof {
                    attestations: vec![],
                    epoch: 1,
                    slot: 0,
                }),
                1000,
            )
            .await;

        assert!(result.is_ok());
        assert_eq!(bus.event_count(), 1);
    }
}
