//! # Mempool Adapter
//! Stub adapter for Mempool (qc-06) subsystem.

use crate::adapters::EventBusAdapter;
use crate::wiring::EventRouter;
use shared_types::SubsystemId;
use std::sync::Arc;

/// Mempool adapter - transaction pool management.
/// 
/// Reference: SPEC-06 Section 4 (Event Schema)
pub struct MempoolAdapter {
    event_bus: EventBusAdapter,
}

impl MempoolAdapter {
    pub fn new(router: Arc<EventRouter>) -> Self {
        let event_bus = EventBusAdapter::new(router, SubsystemId::Mempool);
        Self { event_bus }
    }

    /// Get the event bus adapter for subscription.
    /// 
    /// Used to receive BlockStorageConfirmation from Block Storage (Subsystem 2).
    pub fn event_bus(&self) -> &EventBusAdapter {
        &self.event_bus
    }
}
