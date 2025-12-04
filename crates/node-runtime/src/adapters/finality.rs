//! # Finality Adapter
//! Stub adapter for Finality (qc-09) subsystem.

use crate::adapters::EventBusAdapter;
use crate::wiring::EventRouter;
use shared_types::SubsystemId;
use std::sync::Arc;

/// Finality adapter - monitors blocks for finalization.
/// 
/// Reference: SPEC-09 Section 4 (Event Schema)
pub struct FinalityAdapter {
    event_bus: EventBusAdapter,
}

impl FinalityAdapter {
    pub fn new(router: Arc<EventRouter>) -> Self {
        let event_bus = EventBusAdapter::new(router, SubsystemId::Finality);
        Self { event_bus }
    }

    /// Get the event bus adapter for subscription.
    /// 
    /// Used to subscribe to attestation events from Consensus (Subsystem 8).
    pub fn event_bus(&self) -> &EventBusAdapter {
        &self.event_bus
    }
}
