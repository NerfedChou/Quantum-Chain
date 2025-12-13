//! # Block Storage Adapter
//!
//! Implements the Stateful Assembler pattern for Block Storage (qc-02).
//!
//! ## V2.3 Choreography
//!
//! This adapter buffers three independent events:
//! 1. `BlockValidated` from Consensus (8)
//! 2. `MerkleRootComputed` from Transaction Indexing (3)
//! 3. `StateRootComputed` from State Management (4)
//!
//! When all three arrive for the same block_hash, performs atomic write.
//!
//! ## Architecture (Push Logic Down)
//!
//! All assembly logic is delegated to the domain `BlockAssemblyBuffer`.
//! This adapter is a thin wrapper that:
//! - Receives events from choreography
//! - Delegates to domain for state management
//! - Publishes BlockStored events when assembly completes

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;
use tracing::{debug, info, warn};

use qc_02_block_storage::{AssemblyConfig, BlockAssemblyBuffer, PendingBlockAssembly};
use shared_types::{BlockHeader, ConsensusProof, SubsystemId, ValidatedBlock};

use crate::adapters::EventBusAdapter;
use crate::wiring::{ChoreographyEvent, EventRouter};

/// Block Storage adapter implementing Stateful Assembler pattern.
///
/// This is a thin wrapper around the domain `BlockAssemblyBuffer`.
/// All assembly logic (completeness checking, timeout, buffer limits)
/// is handled by the domain layer.
pub struct BlockStorageAdapter {
    /// Event bus adapter for publishing.
    event_bus: EventBusAdapter,
    /// Domain assembly buffer (contains all logic).
    assembly_buffer: Arc<RwLock<BlockAssemblyBuffer>>,
    /// Assembly timeout for GC logging.
    assembly_timeout: Duration,
}

impl BlockStorageAdapter {
    /// Create a new block storage adapter.
    pub fn new(router: Arc<EventRouter>, assembly_timeout: Duration, max_pending: usize) -> Self {
        let event_bus = EventBusAdapter::new(router, SubsystemId::BlockStorage);
        let config = AssemblyConfig {
            assembly_timeout_secs: assembly_timeout.as_secs(),
            max_pending_assemblies: max_pending,
        };
        let assembly_buffer = Arc::new(RwLock::new(BlockAssemblyBuffer::new(config)));

        Self {
            event_bus,
            assembly_buffer,
            assembly_timeout,
        }
    }

    /// Get current timestamp in seconds.
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Handle BlockValidated event from Consensus.
    pub async fn on_block_validated(
        &self,
        block_hash: [u8; 32],
        block_height: u64,
    ) -> Result<(), BlockStorageError> {
        let now = Self::current_timestamp();

        // Create a minimal validated block (actual block would come from consensus)
        // In production, this would be passed with the event
        let validated_block = ValidatedBlock {
            header: BlockHeader {
                version: 1,
                height: block_height,
                parent_hash: [0; 32],
                merkle_root: [0; 32],
                state_root: [0; 32],
                timestamp: now,
                proposer: [0; 32],
                difficulty: primitive_types::U256::from(2).pow(primitive_types::U256::from(252)),
                nonce: 0,
            },
            transactions: vec![],
            consensus_proof: ConsensusProof::default(),
        };

        {
            let mut buffer = self.assembly_buffer.write();

            // Enforce max pending (INVARIANT-8) - domain handles logic
            let purged = buffer.enforce_max_pending();
            if !purged.is_empty() {
                warn!(
                    "Assembly buffer full, purged {} oldest entries",
                    purged.len()
                );
            }

            // Add block validated to domain buffer
            buffer.add_block_validated(block_hash, validated_block, now);
            debug!("BlockValidated received for height {}", block_height);
        }

        // Try to complete (check outside lock to avoid deadlock)
        self.try_complete_assembly(block_hash).await
    }

    /// Handle MerkleRootComputed event from Transaction Indexing.
    pub async fn on_merkle_root(
        &self,
        block_hash: [u8; 32],
        merkle_root: [u8; 32],
    ) -> Result<(), BlockStorageError> {
        let now = Self::current_timestamp();

        {
            let mut buffer = self.assembly_buffer.write();
            buffer.add_merkle_root(block_hash, merkle_root, now);
            debug!("MerkleRootComputed received for {:?}", &block_hash[..4]);
        }

        self.try_complete_assembly(block_hash).await
    }

    /// Handle StateRootComputed event from State Management.
    pub async fn on_state_root(
        &self,
        block_hash: [u8; 32],
        state_root: [u8; 32],
    ) -> Result<(), BlockStorageError> {
        let now = Self::current_timestamp();

        {
            let mut buffer = self.assembly_buffer.write();
            buffer.add_state_root(block_hash, state_root, now);
            debug!("StateRootComputed received for {:?}", &block_hash[..4]);
        }

        self.try_complete_assembly(block_hash).await
    }

    /// Try to complete assembly if all components present.
    /// Delegates completeness check to domain.
    async fn try_complete_assembly(
        &self,
        block_hash: [u8; 32],
    ) -> Result<(), BlockStorageError> {
        // Check if complete and take assembly atomically
        let completed: Option<PendingBlockAssembly> = {
            let mut buffer = self.assembly_buffer.write();
            buffer.take_complete(&block_hash)
        };

        if let Some(assembly) = completed {
            info!(
                "Block {} assembly complete - performing atomic write",
                assembly.block_height
            );

            // Extract components (domain guarantees they exist)
            let merkle_root = assembly
                .merkle_root
                .ok_or_else(|| BlockStorageError::WriteFailed("Missing merkle_root".into()))?;
            let state_root = assembly
                .state_root
                .ok_or_else(|| BlockStorageError::WriteFailed("Missing state_root".into()))?;

            // Publish BlockStored event
            let event = ChoreographyEvent::BlockStored {
                block_hash,
                block_height: assembly.block_height,
                merkle_root,
                state_root,
                sender_id: SubsystemId::BlockStorage,
            };

            self.event_bus
                .publish(event)
                .map_err(|e| BlockStorageError::PublishFailed(e.to_string()))?;

            info!(
                "[qc-02] 📤 Published BlockStored #{} to choreography",
                assembly.block_height
            );
        }

        Ok(())
    }

    /// Garbage collect timed-out assemblies (INVARIANT-7).
    /// Delegates to domain and publishes timeout events.
    pub async fn gc_stale_assemblies(&self) {
        let now = Self::current_timestamp();

        let expired = {
            let mut buffer = self.assembly_buffer.write();
            buffer.gc_expired_with_data(now)
        };

        for (block_hash, assembly) in expired {
            let missing = Self::get_missing_components(&assembly);
            warn!(
                "Assembly timeout for {:?}, missing: {:?}",
                &block_hash[..4],
                missing
            );

            // Publish AssemblyTimeout event
            let event = ChoreographyEvent::AssemblyTimeout {
                block_hash,
                missing_components: missing,
                sender_id: SubsystemId::BlockStorage,
            };
            let _ = self.event_bus.publish(event);
        }
    }

    /// Get missing components for a pending assembly.
    fn get_missing_components(assembly: &PendingBlockAssembly) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if assembly.validated_block.is_none() {
            missing.push("BlockValidated");
        }
        if assembly.merkle_root.is_none() {
            missing.push("MerkleRootComputed");
        }
        if assembly.state_root.is_none() {
            missing.push("StateRootComputed");
        }
        missing
    }

    /// Get the assembly timeout duration.
    pub fn assembly_timeout(&self) -> Duration {
        self.assembly_timeout
    }

    /// Get the number of pending assemblies.
    pub fn pending_count(&self) -> usize {
        self.assembly_buffer.read().len()
    }
}

/// Block storage errors.
#[derive(Debug)]
pub enum BlockStorageError {
    /// Assembly buffer is full.
    BufferFull,
    /// Failed to publish event.
    PublishFailed(String),
    /// Storage write failed.
    WriteFailed(String),
}

impl std::fmt::Display for BlockStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockStorageError::BufferFull => write!(f, "Assembly buffer full"),
            BlockStorageError::PublishFailed(msg) => write!(f, "Publish failed: {}", msg),
            BlockStorageError::WriteFailed(msg) => write!(f, "Write failed: {}", msg),
        }
    }
}

impl std::error::Error for BlockStorageError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_adapter() -> BlockStorageAdapter {
        let router = Arc::new(EventRouter::default());
        BlockStorageAdapter::new(router, Duration::from_secs(30), 100)
    }

    #[test]
    fn test_adapter_creation() {
        let adapter = create_test_adapter();
        assert_eq!(adapter.pending_count(), 0);
    }

    #[tokio::test]
    async fn test_single_component_not_complete() {
        let adapter = create_test_adapter();
        let block_hash = [0xAB; 32];

        // Only BlockValidated - not complete
        adapter.on_block_validated(block_hash, 1).await.unwrap();
        assert_eq!(adapter.pending_count(), 1);
    }

    #[tokio::test]
    async fn test_all_components_completes() {
        let adapter = create_test_adapter();
        let block_hash = [0xCD; 32];

        adapter.on_block_validated(block_hash, 1).await.unwrap();
        adapter.on_merkle_root(block_hash, [0x11; 32]).await.unwrap();
        adapter.on_state_root(block_hash, [0x22; 32]).await.unwrap();

        // Assembly should be taken and removed
        assert_eq!(adapter.pending_count(), 0);
    }
}
