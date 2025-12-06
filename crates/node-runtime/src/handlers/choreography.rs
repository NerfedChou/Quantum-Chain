//! # Choreography Handlers
//!
//! Event handlers for V2.3 choreography pattern.
//!
//! ## Flow
//!
//! 1. Consensus validates block → publishes `BlockValidated`
//! 2. TxIndexing receives → computes Merkle root → publishes `MerkleRootComputed`
//! 3. StateMgmt receives → computes state root → publishes `StateRootComputed`
//! 4. BlockStorage assembles → performs atomic write → publishes `BlockStored`
//! 5. Finality monitors → checks finalization → publishes `BlockFinalized`

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use shared_types::SubsystemId;

use crate::adapters::BlockStorageAdapter;
use crate::wiring::ChoreographyEvent;

use crate::adapters::{StateAdapter, TransactionIndexingAdapter};

/// Handler for Transaction Indexing choreography events.
pub struct TxIndexingHandler {
    /// Subscriber for events.
    receiver: broadcast::Receiver<ChoreographyEvent>,
    /// Adapter wrapping qc-03 domain logic.
    adapter: Arc<TransactionIndexingAdapter>,
}

impl TxIndexingHandler {
    /// Create a new handler with adapter.
    pub fn new(
        receiver: broadcast::Receiver<ChoreographyEvent>,
        adapter: Arc<TransactionIndexingAdapter>,
    ) -> Self {
        Self { receiver, adapter }
    }

    /// Run the handler loop.
    pub async fn run(mut self, _publisher: Arc<crate::wiring::EventRouter>) {
        info!("[qc-03] Transaction Indexing handler started");

        loop {
            match self.receiver.recv().await {
                Ok(ChoreographyEvent::BlockValidated {
                    block_hash,
                    block_height,
                    sender_id,
                }) => {
                    if sender_id != SubsystemId::Consensus {
                        warn!("[qc-03] Ignoring BlockValidated from {:?}", sender_id);
                        continue;
                    }

                    info!(
                        "[qc-03] 🌳 Computing merkle tree for block #{}",
                        block_height
                    );

                    // JSON EVENT LOG
                    let start_time = chrono::Utc::now();
                    info!(
                        "EVENT_FLOW_JSON {}",
                        serde_json::json!({
                            "timestamp": start_time.to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
                            "subsystem_id": "qc-03",
                            "event_type": "MerkleComputationStarted",
                            "correlation_id": format!("{:x}", block_hash[0]),
                            "block_hash": hex::encode(&block_hash),
                            "block_height": block_height,
                            "metadata": {
                                "target": "qc-02",
                                "assembly_component": "1/3"
                            }
                        })
                    );

                    // Use the adapter to compute Merkle root with actual domain logic
                    // In production, transaction hashes would come from the block
                    let transaction_hashes: Vec<[u8; 32]> = vec![]; // Would be extracted from block

                    let computation_start = std::time::Instant::now();
                    match self.adapter.process_block_validated(
                        block_hash,
                        block_height,
                        transaction_hashes,
                    ) {
                        Ok(_) => {
                            let elapsed = computation_start.elapsed().as_millis();
                            info!("[qc-03] ✓ Merkle root computed for block #{}", block_height);
                            
                            // JSON EVENT LOG - Success
                            info!(
                                
                                "{}",
                                serde_json::json!({
                                    "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
                                    "subsystem_id": "qc-03",
                                    "event_type": "MerkleRootComputed",
                                    "correlation_id": format!("{:x}", block_hash[0]),
                                    "block_hash": hex::encode(&block_hash),
                                    "block_height": block_height,
                                    "processing_time_ms": elapsed,
                                    "metadata": {
                                        "target": "qc-02",
                                        "assembly_component": "1/3",
                                        "status": "sent_to_assembler"
                                    }
                                })
                            );
                        }
                        Err(e) => {
                            error!("[qc-03] ❌ Failed to compute merkle: {}", e);
                        }
                    }
                }
                Ok(_) => {
                    // Ignore other events
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("[qc-03] Lagged by {} messages", n);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!("[qc-03] Channel closed, exiting");
                    break;
                }
            }
        }
    }
}

/// Handler for State Management choreography events.
pub struct StateMgmtHandler {
    /// Subscriber for events.
    receiver: broadcast::Receiver<ChoreographyEvent>,
    /// Adapter wrapping qc-04 domain logic.
    adapter: Arc<StateAdapter>,
}

impl StateMgmtHandler {
    /// Create a new handler with adapter.
    pub fn new(
        receiver: broadcast::Receiver<ChoreographyEvent>,
        adapter: Arc<StateAdapter>,
    ) -> Self {
        Self { receiver, adapter }
    }

    /// Run the handler loop.
    pub async fn run(mut self, _publisher: Arc<crate::wiring::EventRouter>) {
        info!("[qc-04] State Management handler started");

        loop {
            match self.receiver.recv().await {
                Ok(ChoreographyEvent::BlockValidated {
                    block_hash,
                    block_height,
                    sender_id,
                }) => {
                    if sender_id != SubsystemId::Consensus {
                        warn!("[qc-04] Ignoring BlockValidated from {:?}", sender_id);
                        continue;
                    }

                    info!(
                        "[qc-04] 💾 Computing state root for block #{}",
                        block_height
                    );

                    // JSON EVENT LOG
                    info!(
                        
                        "{}",
                        serde_json::json!({
                            "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
                            "subsystem_id": "qc-04",
                            "event_type": "StateComputationStarted",
                            "correlation_id": format!("{:x}", block_hash[0]),
                            "block_hash": hex::encode(&block_hash),
                            "block_height": block_height,
                            "metadata": {
                                "target": "qc-02",
                                "assembly_component": "2/3"
                            }
                        })
                    );

                    // Use the adapter to compute state root with actual domain logic
                    // In production, transactions would come from the block
                    let transactions = vec![]; // Would be extracted from block

                    let computation_start = std::time::Instant::now();
                    match self.adapter
                            .process_block_validated(block_hash, block_height, transactions)
                    {
                        Ok(_) => {
                            let elapsed = computation_start.elapsed().as_millis();
                            info!("[qc-04] ✓ State root computed for block #{}", block_height);
                            
                            // JSON EVENT LOG - Success
                            info!(
                                
                                "{}",
                                serde_json::json!({
                                    "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
                                    "subsystem_id": "qc-04",
                                    "event_type": "StateRootComputed",
                                    "correlation_id": format!("{:x}", block_hash[0]),
                                    "block_hash": hex::encode(&block_hash),
                                    "block_height": block_height,
                                    "processing_time_ms": elapsed,
                                    "metadata": {
                                        "target": "qc-02",
                                        "assembly_component": "2/3",
                                        "status": "sent_to_assembler"
                                    }
                                })
                            );
                        }
                        Err(e) => {
                            error!("[qc-04] ❌ Failed to compute state: {}", e);
                        }
                    }
                }
                Ok(_) => {
                    // Ignore other events
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("[qc-04] Lagged by {} messages", n);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!("[qc-04] Channel closed, exiting");
                    break;
                }
            }
        }
    }
}

/// Handler for Block Storage assembly events.
pub struct BlockStorageHandler {
    /// The adapter that does the assembly.
    adapter: Arc<BlockStorageAdapter>,
    /// Subscriber for events.
    receiver: broadcast::Receiver<ChoreographyEvent>,
}

impl BlockStorageHandler {
    /// Create a new handler.
    pub fn new(
        adapter: Arc<BlockStorageAdapter>,
        receiver: broadcast::Receiver<ChoreographyEvent>,
    ) -> Self {
        Self { adapter, receiver }
    }

    /// Run the handler loop.
    pub async fn run(mut self) {
        info!("[qc-02] Block Storage handler started (Stateful Assembler)");

        // Also spawn GC task for stale assemblies
        let gc_adapter = Arc::clone(&self.adapter);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                gc_adapter.gc_stale_assemblies().await;
            }
        });

        loop {
            match self.receiver.recv().await {
                Ok(event) => {
                    match event {
                        ChoreographyEvent::BlockValidated {
                            block_hash,
                            block_height,
                            sender_id,
                        } => {
                            if sender_id != SubsystemId::Consensus {
                                warn!("[qc-02] Ignoring BlockValidated from {:?}", sender_id);
                                continue;
                            }
                            info!("[qc-02] 📦 Starting assembly for block #{}", block_height);
                            
                            // JSON EVENT LOG - Assembly started
                            info!(
                                
                                "{}",
                                serde_json::json!({
                                    "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
                                    "subsystem_id": "qc-02",
                                    "event_type": "AssemblyStarted",
                                    "correlation_id": format!("{:x}", block_hash[0]),
                                    "block_hash": hex::encode(&block_hash),
                                    "block_height": block_height,
                                    "metadata": {
                                        "components_waiting": ["BlockValidated", "MerkleRootComputed", "StateRootComputed"],
                                        "components_received": ["BlockValidated"],
                                        "assembly_state": "1/3"
                                    }
                                })
                            );
                            
                            match self
                                .adapter
                                .on_block_validated(block_hash, block_height)
                                .await
                            {
                                Ok(_) => {
                                    info!("[qc-02] ✓ Block #{} assembly initiated", block_height);
                                }
                                Err(e) => {
                                    error!("[qc-02] ❌ Assembly failed: {}", e);
                                }
                            }
                        }
                        ChoreographyEvent::MerkleRootComputed {
                            block_hash,
                            merkle_root,
                            sender_id,
                        } => {
                            if sender_id != SubsystemId::TransactionIndexing {
                                warn!("[qc-02] Ignoring MerkleRootComputed from {:?}", sender_id);
                                continue;
                            }
                            if let Err(e) =
                                self.adapter.on_merkle_root(block_hash, merkle_root).await
                            {
                                error!("[qc-02] Failed to process MerkleRootComputed: {}", e);
                            }
                        }
                        ChoreographyEvent::StateRootComputed {
                            block_hash,
                            state_root,
                            sender_id,
                        } => {
                            if sender_id != SubsystemId::StateManagement {
                                warn!("[qc-02] Ignoring StateRootComputed from {:?}", sender_id);
                                continue;
                            }
                            if let Err(e) = self.adapter.on_state_root(block_hash, state_root).await
                            {
                                error!("[qc-02] Failed to process StateRootComputed: {}", e);
                            }
                        }
                        _ => {
                            // Ignore other events
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("[qc-02] Lagged by {} messages", n);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!("[qc-02] Channel closed, exiting");
                    break;
                }
            }
        }
    }
}

/// Handler for Finality events.
pub struct FinalityHandler {
    /// Subscriber for events.
    receiver: broadcast::Receiver<ChoreographyEvent>,
}

impl FinalityHandler {
    /// Create a new handler.
    pub fn new(receiver: broadcast::Receiver<ChoreographyEvent>) -> Self {
        Self { receiver }
    }

    /// Run the handler loop.
    pub async fn run(mut self, publisher: Arc<crate::wiring::EventRouter>) {
        info!("[qc-09] Finality handler started (Casper-FFG)");

        loop {
            match self.receiver.recv().await {
                Ok(ChoreographyEvent::BlockStored {
                    block_hash,
                    block_height,
                    sender_id,
                    ..
                }) => {
                    if sender_id != SubsystemId::BlockStorage {
                        warn!("[qc-09] Ignoring BlockStored from {:?}", sender_id);
                        continue;
                    }
                    
                    info!("[qc-09] 📥 Received BlockStored for block #{}", block_height);

                    // In real impl: check attestations, verify 2/3 threshold
                    // For demo: finalize every 4 blocks (epoch boundary)
                    // In production this would be 32 blocks per epoch
                    let epoch_size = 4;
                    if block_height % epoch_size == 0 || block_height <= 10 {
                        let epoch = block_height / epoch_size;
                        info!(
                            "[qc-09] 🔒 Block #{} at epoch {} boundary, finalizing...",
                            block_height, epoch
                        );

                        // Publish BlockFinalized
                        let event = ChoreographyEvent::BlockFinalized {
                            block_hash,
                            block_height,
                            finality_proof: vec![],
                            sender_id: SubsystemId::Finality,
                        };

                        match publisher.publish(event) {
                            Ok(_) => {
                                info!("[qc-09] ✓ Block #{} FINALIZED at epoch {}", block_height, epoch);
                            }
                            Err(e) => {
                                error!("[qc-09] ❌ Failed to finalize: {}", e);
                            }
                        }
                    }
                }
                Ok(_) => {
                    // Ignore other events
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("[qc-09] Lagged by {} messages", n);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!("[qc-09] Channel closed, exiting");
                    break;
                }
            }
        }
    }
}
