//! # Mempool Adapter
//! Stub adapter for Mempool (qc-06) subsystem.

use crate::adapters::EventBusAdapter;
use crate::wiring::EventRouter;
use shared_types::SubsystemId;
use std::sync::Arc;

/// Mempool adapter - transaction pool management.
pub struct MempoolAdapter {
    #[allow(dead_code)]
    event_bus: EventBusAdapter,
}

impl MempoolAdapter {
    pub fn new(router: Arc<EventRouter>) -> Self {
        let event_bus = EventBusAdapter::new(router, SubsystemId::Mempool);
        Self { event_bus }
    }
}
