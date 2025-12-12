use crate::adapters::EventBusAdapter;
use crate::wiring::EventRouter;
use qc_10_signature_verification::domain::entities::VerifiedTransaction;
use qc_10_signature_verification::ports::outbound::{MempoolError, MempoolGateway};
use sha3::{Digest, Keccak256};
use shared_bus::{events::BlockchainEvent, EventPublisher, InMemoryEventBus};
use shared_types::entities::ValidatedTransaction;
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

/// Runtime implementation of MempoolGateway using the Event Bus.
///
/// This allows the Signature Verification service to "submit" transactions
/// by publishing events, decoupling it from the Mempool subsystem.
#[derive(Clone)]
pub struct RuntimeMempoolGateway {
    bus: Arc<InMemoryEventBus>,
}

impl RuntimeMempoolGateway {
    pub fn new(bus: Arc<InMemoryEventBus>) -> Self {
        Self { bus }
    }
}

#[async_trait::async_trait]
impl MempoolGateway for RuntimeMempoolGateway {
    async fn submit_verified_transaction(
        &self,
        transaction: VerifiedTransaction,
    ) -> Result<(), MempoolError> {
        let tx_hash = compute_transaction_hash(&transaction.transaction);
        let validated = ValidatedTransaction {
            inner: transaction.transaction,
            tx_hash,
        };
        let event = BlockchainEvent::TransactionVerified(validated);
        // Publish returns number of subscribers - we ignore it
        self.bus.publish(event).await;
        Ok(())
    }
}

/// Compute transaction hash compatible with qc-10/service.rs logic.
fn compute_transaction_hash(tx: &shared_types::Transaction) -> shared_types::Hash {
    let mut hasher = Keccak256::new();
    // Hash the transaction fields (excluding signature)
    hasher.update(tx.from);
    if let Some(ref to) = tx.to {
        hasher.update(to);
    }
    hasher.update(tx.value.to_le_bytes());
    hasher.update(tx.nonce.to_le_bytes());
    hasher.update(&tx.data);

    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}
