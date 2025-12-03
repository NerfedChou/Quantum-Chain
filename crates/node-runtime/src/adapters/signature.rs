//! # Signature Verification Adapter
//! Stub adapter for Signature Verification (qc-10) subsystem.

use crate::adapters::EventBusAdapter;
use crate::wiring::EventRouter;
use shared_types::SubsystemId;
use std::sync::Arc;

/// Signature verification adapter - ECDSA operations.
pub struct SignatureAdapter {
    #[allow(dead_code)]
    event_bus: EventBusAdapter,
}

impl SignatureAdapter {
    pub fn new(router: Arc<EventRouter>) -> Self {
        let event_bus = EventBusAdapter::new(router, SubsystemId::SignatureVerification);
        Self { event_bus }
    }
}
