//! # Consensus Adapter
//! Stub adapter for Consensus (qc-08) subsystem.

use crate::adapters::EventBusAdapter;
use crate::wiring::EventRouter;
use shared_types::SubsystemId;
use std::sync::Arc;

/// Consensus adapter - publishes BlockValidated events.
/// 
/// Reference: SPEC-08 Section 4 (Event Schema)
pub struct ConsensusAdapter {
    event_bus: EventBusAdapter,
}

impl ConsensusAdapter {
    pub fn new(router: Arc<EventRouter>) -> Self {
        let event_bus = EventBusAdapter::new(router, SubsystemId::Consensus);
        Self { event_bus }
    }

    /// Get the event bus adapter for publishing.
    /// 
    /// Used to publish BlockValidated events to the choreography flow.
    /// Subsystems 2, 3, 4 subscribe to this event.
    pub fn event_bus(&self) -> &EventBusAdapter {
        &self.event_bus
    }
}
