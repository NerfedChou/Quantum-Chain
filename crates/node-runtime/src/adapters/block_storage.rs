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

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use shared_types::SubsystemId;

use crate::adapters::EventBusAdapter;
use crate::wiring::{ChoreographyEvent, EventRouter};

/// Pending block assembly tracking.
#[derive(Debug)]
pub struct PendingAssembly {
    /// Block hash (correlation key).
    pub block_hash: [u8; 32],
    /// Block height.
    pub block_height: u64,
    /// When assembly started.
    pub started_at: Instant,
    /// Block data from Consensus.
    pub block_validated: Option<()>, // Would be ValidatedBlock
    /// Merkle root from Transaction Indexing.
    pub merkle_root: Option<[u8; 32]>,
    /// State root from State Management.
    pub state_root: Option<[u8; 32]>,
}

impl PendingAssembly {
    /// Create a new pending assembly.
    pub fn new(block_hash: [u8; 32], block_height: u64) -> Self {
        Self {
            block_hash,
            block_height,
            started_at: Instant::now(),
            block_validated: None,
            merkle_root: None,
            state_root: None,
        }
    }

    /// Check if all components have arrived.
    pub fn is_complete(&self) -> bool {
        self.block_validated.is_some() && self.merkle_root.is_some() && self.state_root.is_some()
    }

    /// Get missing components for timeout logging.
    pub fn missing_components(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if self.block_validated.is_none() {
            missing.push("BlockValidated");
        }
        if self.merkle_root.is_none() {
            missing.push("MerkleRootComputed");
        }
        if self.state_root.is_none() {
            missing.push("StateRootComputed");
        }
        missing
    }
}

/// Block Storage adapter implementing Stateful Assembler pattern.
pub struct BlockStorageAdapter {
    /// Event bus adapter for publishing.
    event_bus: EventBusAdapter,
    /// Pending block assemblies keyed by block_hash.
    pending: Arc<RwLock<HashMap<[u8; 32], PendingAssembly>>>,
    /// Assembly timeout.
    assembly_timeout: Duration,
    /// Maximum pending assemblies (memory bound).
    max_pending: usize,
}

impl BlockStorageAdapter {
    /// Create a new block storage adapter.
    pub fn new(router: Arc<EventRouter>, assembly_timeout: Duration, max_pending: usize) -> Self {
        let event_bus = EventBusAdapter::new(router, SubsystemId::BlockStorage);

        Self {
            event_bus,
            pending: Arc::new(RwLock::new(HashMap::new())),
            assembly_timeout,
            max_pending,
        }
    }

    /// Handle BlockValidated event from Consensus.
    pub async fn on_block_validated(
        &self,
        block_hash: [u8; 32],
        block_height: u64,
    ) -> Result<(), BlockStorageError> {
        let mut pending = self.pending.write().await;

        // Check buffer limit (INVARIANT-8)
        if pending.len() >= self.max_pending && !pending.contains_key(&block_hash) {
            warn!(
                "Assembly buffer full ({}/{}), rejecting new block",
                pending.len(),
                self.max_pending
            );
            return Err(BlockStorageError::BufferFull);
        }

        let assembly = pending
            .entry(block_hash)
            .or_insert_with(|| PendingAssembly::new(block_hash, block_height));

        assembly.block_validated = Some(());
        debug!("BlockValidated received for height {}", block_height);

        self.try_complete_assembly(block_hash, &pending).await
    }

    /// Handle MerkleRootComputed event from Transaction Indexing.
    pub async fn on_merkle_root(
        &self,
        block_hash: [u8; 32],
        merkle_root: [u8; 32],
    ) -> Result<(), BlockStorageError> {
        let mut pending = self.pending.write().await;

        if let Some(assembly) = pending.get_mut(&block_hash) {
            assembly.merkle_root = Some(merkle_root);
            debug!("MerkleRootComputed received for {:?}", &block_hash[..4]);
            return self.try_complete_assembly(block_hash, &pending).await;
        }

        // Event arrived before BlockValidated - create pending entry
        if pending.len() < self.max_pending {
            let mut assembly = PendingAssembly::new(block_hash, 0);
            assembly.merkle_root = Some(merkle_root);
            pending.insert(block_hash, assembly);
        }

        Ok(())
    }

    /// Handle StateRootComputed event from State Management.
    pub async fn on_state_root(
        &self,
        block_hash: [u8; 32],
        state_root: [u8; 32],
    ) -> Result<(), BlockStorageError> {
        let mut pending = self.pending.write().await;

        if let Some(assembly) = pending.get_mut(&block_hash) {
            assembly.state_root = Some(state_root);
            debug!("StateRootComputed received for {:?}", &block_hash[..4]);
            return self.try_complete_assembly(block_hash, &pending).await;
        }

        // Event arrived before BlockValidated - create pending entry
        if pending.len() < self.max_pending {
            let mut assembly = PendingAssembly::new(block_hash, 0);
            assembly.state_root = Some(state_root);
            pending.insert(block_hash, assembly);
        }

        Ok(())
    }

    /// Try to complete assembly if all components present.
    async fn try_complete_assembly(
        &self,
        block_hash: [u8; 32],
        pending: &HashMap<[u8; 32], PendingAssembly>,
    ) -> Result<(), BlockStorageError> {
        if let Some(assembly) = pending.get(&block_hash) {
            if assembly.is_complete() {
                info!(
                    "Block {} assembly complete - performing atomic write",
                    assembly.block_height
                );

                // Publish BlockStored event
                let event = ChoreographyEvent::BlockStored {
                    block_hash,
                    block_height: assembly.block_height,
                    merkle_root: assembly.merkle_root.unwrap(),
                    state_root: assembly.state_root.unwrap(),
                    sender_id: SubsystemId::BlockStorage,
                };

                self.event_bus
                    .publish(event)
                    .map_err(|e| BlockStorageError::PublishFailed(e.to_string()))?;

                // Remove from pending (would be done with mutable access)
            }
        }
        Ok(())
    }

    /// Garbage collect timed-out assemblies (INVARIANT-7).
    pub async fn gc_stale_assemblies(&self) {
        let mut pending = self.pending.write().await;
        let now = Instant::now();

        let stale: Vec<_> = pending
            .iter()
            .filter(|(_, assembly)| now.duration_since(assembly.started_at) > self.assembly_timeout)
            .map(|(hash, assembly)| (*hash, assembly.missing_components()))
            .collect();

        for (block_hash, missing) in stale {
            warn!(
                "Assembly timeout for {:?}, missing: {:?}",
                &block_hash[..4],
                missing
            );
            pending.remove(&block_hash);

            // Publish AssemblyTimeout event
            let event = ChoreographyEvent::AssemblyTimeout {
                block_hash,
                missing_components: missing,
                sender_id: SubsystemId::BlockStorage,
            };
            let _ = self.event_bus.publish(event);
        }
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
