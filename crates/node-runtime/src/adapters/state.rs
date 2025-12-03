//! # State Management Adapter
//! Stub adapter for State Management (qc-04) subsystem.

use crate::adapters::EventBusAdapter;
use crate::wiring::EventRouter;
use shared_types::SubsystemId;
use std::sync::Arc;

/// State management adapter - Patricia trie operations.
pub struct StateAdapter {
    #[allow(dead_code)]
    event_bus: EventBusAdapter,
}

impl StateAdapter {
    pub fn new(router: Arc<EventRouter>) -> Self {
        let event_bus = EventBusAdapter::new(router, SubsystemId::StateManagement);
        Self { event_bus }
    }
}
