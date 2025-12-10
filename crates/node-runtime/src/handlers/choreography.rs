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
//!
//! ## Plug-and-Play (v2.4)
//!
//! Handlers are conditionally compiled based on enabled subsystems.

use std::sync::Arc;

use tokio::sync::broadcast;
use tracing::{error, info, warn};

use shared_types::SubsystemId;

use crate::wiring::ChoreographyEvent;

// Conditional imports
#[cfg(feature = "qc-02")]
use crate::adapters::BlockStorageAdapter;
#[cfg(feature = "qc-02")]
use std::time::Duration;

#[cfg(feature = "qc-03")]
use crate::adapters::TransactionIndexingAdapter;

#[cfg(feature = "qc-04")]
use crate::adapters::StateAdapter;

#[cfg(feature = "qc-12")]
use crate::adapters::TransactionOrderingAdapter;

/// Handler for Transaction Indexing choreography events.
#[cfg(feature = "qc-03")]
pub struct TxIndexingHandler {
    /// Subscriber for events.
    receiver: broadcast::Receiver<ChoreographyEvent>,
    /// Adapter wrapping qc-03 domain logic.
    adapter: Arc<TransactionIndexingAdapter>,
}

#[cfg(feature = "qc-03")]
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
                    let transaction_hashes: Vec<[u8; 32]> = vec![];

                    let computation_start = std::time::Instant::now();
                    match self.adapter.process_block_validated(
                        block_hash,
                        block_height,
                        transaction_hashes,
                    ) {
                        Ok(_) => {
                            let elapsed = computation_start.elapsed().as_millis();
                            info!("[qc-03] ✓ Merkle root computed for block #{}", block_height);

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
                Ok(_) => {}
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
#[cfg(feature = "qc-04")]
pub struct StateMgmtHandler {
    /// Subscriber for events.
    receiver: broadcast::Receiver<ChoreographyEvent>,
    /// Adapter wrapping qc-04 domain logic.
    adapter: Arc<StateAdapter>,
}

#[cfg(feature = "qc-04")]
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

                    let transactions = vec![];
                    let computation_start = std::time::Instant::now();

                    match self.adapter.process_block_validated(
                        block_hash,
                        block_height,
                        transactions,
                    ) {
                        Ok(_) => {
                            let elapsed = computation_start.elapsed().as_millis();
                            info!("[qc-04] ✓ State root computed for block #{}", block_height);

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
                Ok(_) => {}
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
#[cfg(feature = "qc-02")]
pub struct BlockStorageHandler {
    /// The adapter that does the assembly.
    adapter: Arc<BlockStorageAdapter>,
    /// Subscriber for events.
    receiver: broadcast::Receiver<ChoreographyEvent>,
}

#[cfg(feature = "qc-02")]
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

        // Spawn GC task for stale assemblies
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
                Ok(event) => match event {
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
                        if let Err(e) = self.adapter.on_merkle_root(block_hash, merkle_root).await {
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
                        if let Err(e) = self.adapter.on_state_root(block_hash, state_root).await {
                            error!("[qc-02] Failed to process StateRootComputed: {}", e);
                        }
                    }
                    _ => {}
                },
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
#[cfg(feature = "qc-09")]
pub struct FinalityHandler {
    /// Subscriber for events.
    receiver: broadcast::Receiver<ChoreographyEvent>,
}

#[cfg(feature = "qc-09")]
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

                    info!(
                        "[qc-09] 📥 Received BlockStored for block #{}",
                        block_height
                    );

                    // Finalize every 4 blocks (epoch boundary)
                    let epoch_size = 4;
                    if block_height % epoch_size == 0 || block_height <= 10 {
                        let epoch = block_height / epoch_size;
                        info!(
                            "[qc-09] 🔒 Block #{} at epoch {} boundary, finalizing...",
                            block_height, epoch
                        );

                        let event = ChoreographyEvent::BlockFinalized {
                            block_hash,
                            block_height,
                            finality_proof: vec![],
                            sender_id: SubsystemId::Finality,
                        };

                        match publisher.publish(event) {
                            Ok(_) => {
                                info!(
                                    "[qc-09] ✓ Block #{} FINALIZED at epoch {}",
                                    block_height, epoch
                                );
                            }
                            Err(e) => {
                                error!("[qc-09] ❌ Failed to finalize: {}", e);
                            }
                        }
                    }
                }
                Ok(_) => {}
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

/// Handler for Transaction Ordering choreography events.
///
/// ## IPC-MATRIX (Subsystem 12)
///
/// - Receives: BlockValidated from Consensus (8)
/// - Publishes: TransactionsOrdered to Smart Contracts (11)
/// - Queries: State Management (4) for conflict detection
#[cfg(feature = "qc-12")]
pub struct TransactionOrderingHandler {
    /// Subscriber for events.
    receiver: broadcast::Receiver<ChoreographyEvent>,
    /// Adapter wrapping qc-12 domain logic.
    adapter: Arc<TransactionOrderingAdapter>,
}

#[cfg(feature = "qc-12")]
impl TransactionOrderingHandler {
    /// Create a new handler with adapter.
    pub fn new(
        receiver: broadcast::Receiver<ChoreographyEvent>,
        adapter: Arc<TransactionOrderingAdapter>,
    ) -> Self {
        Self { receiver, adapter }
    }

    /// Run the handler loop.
    ///
    /// Listens for BlockValidated events from Consensus and orders
    /// transactions for parallel execution.
    pub async fn run(mut self) {
        info!("[qc-12] Transaction Ordering handler started");

        loop {
            match self.receiver.recv().await {
                Ok(ChoreographyEvent::BlockValidated {
                    block_hash,
                    block_height,
                    sender_id,
                }) => {
                    if sender_id != SubsystemId::Consensus {
                        warn!("[qc-12] Ignoring BlockValidated from {:?}", sender_id);
                        continue;
                    }

                    info!(
                        "[qc-12] Received BlockValidated for height {} from Consensus",
                        block_height
                    );

                    // In production, we would extract transactions from the validated block
                    // For now, create a minimal request to demonstrate the flow
                    let request = qc_12_transaction_ordering::OrderTransactionsRequest {
                        correlation_id: {
                            let mut id = [0u8; 16];
                            id[..8].copy_from_slice(&block_height.to_le_bytes());
                            id[8..16].copy_from_slice(&block_hash[..8]);
                            id
                        },
                        reply_to: format!("qc-12-ordering-{}", block_height),
                        transaction_hashes: vec![], // Would be populated from block
                        senders: vec![],
                        nonces: vec![],
                        read_sets: vec![],
                        write_sets: vec![],
                    };

                    // Process ordering (adapter publishes TransactionsOrdered on success)
                    let response = self
                        .adapter
                        .process_order_transactions(
                            SubsystemId::Consensus,
                            request,
                            block_hash,
                            block_height,
                        )
                        .await;

                    if response.success {
                        info!(
                            "[qc-12] ✓ Block {} ordering complete: {} groups, max parallelism {}",
                            block_height,
                            response.metrics.parallel_groups,
                            response.metrics.max_parallelism
                        );
                    } else {
                        error!(
                            "[qc-12] ❌ Block {} ordering failed: {:?}",
                            block_height, response.error
                        );
                    }
                }
                Ok(_) => {} // Ignore other events
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("[qc-12] Lagged by {} messages", n);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!("[qc-12] Channel closed, exiting");
                    break;
                }
            }
        }
    }
}
