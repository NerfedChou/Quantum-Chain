//! # Consensus Adapter
//! Stub adapter for Consensus (qc-08) subsystem.

use crate::adapters::EventBusAdapter;
use crate::wiring::EventRouter;
use shared_types::SubsystemId;
use std::sync::Arc;

/// Consensus adapter - publishes BlockValidated events.
pub struct ConsensusAdapter {
    #[allow(dead_code)]
    event_bus: EventBusAdapter,
}

impl ConsensusAdapter {
    pub fn new(router: Arc<EventRouter>) -> Self {
        let event_bus = EventBusAdapter::new(router, SubsystemId::Consensus);
        Self { event_bus }
    }
}
