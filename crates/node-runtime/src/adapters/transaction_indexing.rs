//! # Transaction Indexing Adapter
//! Stub adapter for Transaction Indexing (qc-03) subsystem.

use crate::adapters::EventBusAdapter;
use crate::wiring::EventRouter;
use shared_types::SubsystemId;
use std::sync::Arc;

/// Transaction indexing adapter - Merkle tree operations.
pub struct TransactionIndexingAdapter {
    #[allow(dead_code)]
    event_bus: EventBusAdapter,
}

impl TransactionIndexingAdapter {
    pub fn new(router: Arc<EventRouter>) -> Self {
        let event_bus = EventBusAdapter::new(router, SubsystemId::TransactionIndexing);
        Self { event_bus }
    }
}
