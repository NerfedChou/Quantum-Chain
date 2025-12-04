//! # Signature Verification Adapter
//! Stub adapter for Signature Verification (qc-10) subsystem.

use crate::adapters::EventBusAdapter;
use crate::wiring::EventRouter;
use shared_types::SubsystemId;
use std::sync::Arc;

/// Signature verification adapter - ECDSA operations.
///
/// Reference: SPEC-10 Section 4 (Event Schema)
pub struct SignatureAdapter {
    event_bus: EventBusAdapter,
}

impl SignatureAdapter {
    pub fn new(router: Arc<EventRouter>) -> Self {
        let event_bus = EventBusAdapter::new(router, SubsystemId::SignatureVerification);
        Self { event_bus }
    }

    /// Get the event bus adapter for publishing verification results.
    ///
    /// Used to publish TransactionVerified events to Mempool (Subsystem 6).
    pub fn event_bus(&self) -> &EventBusAdapter {
        &self.event_bus
    }
}
