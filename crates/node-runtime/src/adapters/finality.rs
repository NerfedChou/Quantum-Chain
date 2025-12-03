//! # Finality Adapter
//! Stub adapter for Finality (qc-09) subsystem.

use crate::adapters::EventBusAdapter;
use crate::wiring::EventRouter;
use shared_types::SubsystemId;
use std::sync::Arc;

/// Finality adapter - monitors blocks for finalization.
pub struct FinalityAdapter {
    #[allow(dead_code)]
    event_bus: EventBusAdapter,
}

impl FinalityAdapter {
    pub fn new(router: Arc<EventRouter>) -> Self {
        let event_bus = EventBusAdapter::new(router, SubsystemId::Finality);
        Self { event_bus }
    }
}
