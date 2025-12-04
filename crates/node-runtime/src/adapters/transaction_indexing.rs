//! # Transaction Indexing Adapter
//!
//! Adapter that wraps qc-03-transaction-indexing domain logic and connects
//! it to the choreography event bus.
//!
//! ## V2.3 Choreography
//!
//! - Subscribes to: BlockValidated (from Consensus 8)
//! - Publishes: MerkleRootComputed (to Block Storage 2)

use parking_lot::RwLock;
use std::sync::Arc;
use tracing::{debug, error};

use qc_03_transaction_indexing::{
    IndexConfig, MerkleTree, TransactionIndex, TransactionLocation,
};
use shared_types::{Hash, SubsystemId};

use crate::adapters::EventBusAdapter;
use crate::wiring::{ChoreographyEvent, EventRouter};

/// Transaction indexing adapter - wraps qc-03 domain logic.
///
/// Computes Merkle roots for validated blocks and indexes transactions.
pub struct TransactionIndexingAdapter {
    /// Event bus for publishing MerkleRootComputed
    event_bus: EventBusAdapter,
    /// Transaction index (domain logic from qc-03)
    index: Arc<RwLock<TransactionIndex>>,
}

impl TransactionIndexingAdapter {
    /// Create a new adapter with default configuration.
    pub fn new(router: Arc<EventRouter>) -> Self {
        let event_bus = EventBusAdapter::new(router, SubsystemId::TransactionIndexing);
        let config = IndexConfig::default();
        let index = Arc::new(RwLock::new(TransactionIndex::new(config)));
        
        Self { event_bus, index }
    }

    /// Create with custom configuration.
    pub fn with_config(router: Arc<EventRouter>, config: IndexConfig) -> Self {
        let event_bus = EventBusAdapter::new(router, SubsystemId::TransactionIndexing);
        let index = Arc::new(RwLock::new(TransactionIndex::new(config)));
        
        Self { event_bus, index }
    }

    /// Process a BlockValidated event - compute Merkle root and publish.
    ///
    /// This is the main choreography trigger for Transaction Indexing.
    ///
    /// ## Returns
    ///
    /// - The computed Merkle root
    /// - Publishes `MerkleRootComputed` event on success
    pub fn process_block_validated(
        &self,
        block_hash: Hash,
        block_height: u64,
        transaction_hashes: Vec<Hash>,
    ) -> Result<Hash, TransactionIndexingError> {
        debug!(
            "[qc-03] Processing BlockValidated: height={}, txs={}",
            block_height,
            transaction_hashes.len()
        );

        // Step 1: Build Merkle tree (INVARIANT-1: power of two padding)
        let tree = MerkleTree::build(transaction_hashes.clone());
        let merkle_root = tree.root();

        // Step 2: Index all transactions
        {
            let mut index = self.index.write();
            for (idx, tx_hash) in transaction_hashes.iter().enumerate() {
                let location = TransactionLocation {
                    block_height,
                    block_hash,
                    tx_index: idx,
                    merkle_root,
                };
                index.put_location(*tx_hash, location);
            }

            // Step 3: Cache the Merkle tree (INVARIANT-5: LRU eviction)
            index.cache_tree(block_hash, tree);
        }

        // Step 4: Publish MerkleRootComputed event
        let event = ChoreographyEvent::MerkleRootComputed {
            block_hash,
            merkle_root,
            sender_id: SubsystemId::TransactionIndexing,
        };

        if let Err(e) = self.event_bus.publish(event) {
            error!("[qc-03] Failed to publish MerkleRootComputed: {}", e);
            return Err(TransactionIndexingError::PublishFailed(e.to_string()));
        }

        debug!(
            "[qc-03] MerkleRootComputed published: {:?}",
            &merkle_root[..4]
        );

        Ok(merkle_root)
    }

    /// Get the transaction index for querying.
    pub fn index(&self) -> Arc<RwLock<TransactionIndex>> {
        Arc::clone(&self.index)
    }
}

/// Transaction indexing errors.
#[derive(Debug)]
pub enum TransactionIndexingError {
    /// Failed to publish event.
    PublishFailed(String),
    /// Indexing error from domain.
    IndexError(String),
}

impl std::fmt::Display for TransactionIndexingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PublishFailed(msg) => write!(f, "Publish failed: {}", msg),
            Self::IndexError(msg) => write!(f, "Index error: {}", msg),
        }
    }
}

impl std::error::Error for TransactionIndexingError {}
