//! # Event Bus Adapter
//!
//! Wires Subsystem 10 to the shared event bus for V2.3 choreography.
//!
//! ## Architecture Reference
//!
//! - Architecture.md Section 5.1: Event-Driven Choreography
//! - IPC-MATRIX.md Subsystem 10: Security Boundaries
//! - SPEC-10 Section 4: Event Schema
//!
//! ## Event Flow
//!
//! ```text
//! External Network ──SignedTransaction──→ [Signature Verification (10)]
//!                                                    │
//!                            ┌────────────────────────┴─────────────────────────┐
//!                            ↓                                                  ↓
//!                   [signature valid]                                [signature invalid]
//!                            │                                                  │
//!                            ↓                                                  ↓
//!            TransactionVerified ──→ [Event Bus]          TransactionInvalid ──→ [Event Bus]
//!                            │                                                  │
//!                            ↓                                                  │
//!                    [Mempool (6)]                                      [Logged/DLQ]
//! ```
//!
//! ## Security
//!
//! - Only authorized subsystems can request verification
//! - Envelope-Only Identity (sender_id from AuthenticatedMessage)
//! - Rate limiting per subsystem

use crate::domain::entities::{EcdsaSignature, VerificationResult};
use crate::domain::errors::SignatureError;
use crate::ports::inbound::SignatureVerificationApi;
use async_trait::async_trait;
use shared_bus::events::BlockchainEvent;
use shared_bus::publisher::EventPublisher;
use shared_types::entities::{Hash, ValidatedTransaction};
use std::sync::Arc;
use tracing::{debug, info, warn};

// =============================================================================
// BUS ADAPTER TRAIT
// =============================================================================

/// Adapter for publishing signature verification events to the event bus.
///
/// Per Architecture.md Section 5.1, this is the outbound adapter for
/// event-driven communication with other subsystems.
#[async_trait]
pub trait SignatureVerificationBusAdapter: Send + Sync {
    /// Verify a signature and publish the result to the event bus.
    ///
    /// # Arguments
    ///
    /// * `tx_hash` - The transaction hash (for event publishing)
    /// * `message_hash` - The message that was signed (typically tx hash)
    /// * `signature` - The ECDSA signature
    ///
    /// # Returns
    ///
    /// The verification result and number of subscribers that received the event.
    async fn verify_and_publish_result(
        &self,
        tx_hash: Hash,
        message_hash: &Hash,
        signature: &EcdsaSignature,
    ) -> Result<(VerificationResult, usize), SignatureError>;

    /// Publish a TransactionVerified event to the bus.
    ///
    /// Called when a transaction signature is successfully verified.
    async fn publish_verified(&self, tx: ValidatedTransaction) -> usize;

    /// Publish a TransactionInvalid event to the bus.
    ///
    /// Called when a transaction signature fails verification.
    async fn publish_invalid(&self, tx_hash: Hash, reason: String) -> usize;
}

// =============================================================================
// BUS ADAPTER IMPLEMENTATION
// =============================================================================

/// Event bus adapter for Subsystem 10.
///
/// Wires the signature verification service to the shared event bus,
/// enabling choreography-based communication with other subsystems.
pub struct EventBusAdapter<S, P>
where
    S: SignatureVerificationApi,
    P: EventPublisher,
{
    /// The signature verification service
    service: Arc<S>,

    /// The event publisher (shared bus)
    publisher: Arc<P>,
}

impl<S, P> EventBusAdapter<S, P>
where
    S: SignatureVerificationApi,
    P: EventPublisher,
{
    /// Create a new event bus adapter.
    ///
    /// # Arguments
    ///
    /// * `service` - The signature verification service
    /// * `publisher` - The event publisher (shared bus)
    pub fn new(service: Arc<S>, publisher: Arc<P>) -> Self {
        Self { service, publisher }
    }

    /// Get a reference to the underlying service.
    pub fn service(&self) -> &S {
        &self.service
    }

    /// Get a reference to the event publisher.
    pub fn publisher(&self) -> &P {
        &self.publisher
    }
}

#[async_trait]
impl<S, P> SignatureVerificationBusAdapter for EventBusAdapter<S, P>
where
    S: SignatureVerificationApi + Send + Sync,
    P: EventPublisher + Send + Sync,
{
    async fn verify_and_publish_result(
        &self,
        tx_hash: Hash,
        message_hash: &Hash,
        signature: &EcdsaSignature,
    ) -> Result<(VerificationResult, usize), SignatureError> {
        // Step 1: Verify the signature using the domain service
        let result = self.service.verify_ecdsa(message_hash, signature);

        // Step 2: Publish appropriate event based on result
        let receivers = if result.valid {
            debug!(
                tx_hash = ?tx_hash,
                signer = ?result.recovered_address,
                "Transaction signature verified"
            );

            // Note: In production, the full ValidatedTransaction should be passed
            // to publish_verified. Here we just publish the invalid event since
            // we don't have the full transaction data.
            // The caller (runtime/orchestrator) should call publish_verified with
            // the actual ValidatedTransaction.
            0 // No event published - caller should use publish_verified with full tx
        } else {
            let reason = result
                .error
                .as_ref()
                .map_or_else(|| "Unknown error".to_string(), ToString::to_string);

            warn!(
                tx_hash = ?tx_hash,
                reason = %reason,
                "Transaction signature invalid"
            );

            self.publish_invalid(tx_hash, reason).await
        };

        Ok((result, receivers))
    }

    async fn publish_verified(&self, tx: ValidatedTransaction) -> usize {
        let event = BlockchainEvent::TransactionVerified(tx.clone());

        info!(
            tx_hash = ?tx.tx_hash,
            "Publishing TransactionVerified event"
        );

        self.publisher.publish(event).await
    }

    async fn publish_invalid(&self, tx_hash: Hash, reason: String) -> usize {
        let event = BlockchainEvent::TransactionInvalid {
            hash: tx_hash,
            reason: reason.clone(),
        };

        info!(
            tx_hash = ?tx_hash,
            reason = %reason,
            "Publishing TransactionInvalid event"
        );

        self.publisher.publish(event).await
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::SignatureVerificationService;
    use shared_bus::events::{EventFilter, EventTopic};
    use shared_bus::publisher::InMemoryEventBus;
    use shared_types::entities::Transaction;
    use std::time::Duration;
    use tokio::time::timeout;

    fn create_test_adapter(
    ) -> EventBusAdapter<SignatureVerificationService<DummyMempool>, InMemoryEventBus> {
        let mempool = DummyMempool;
        let service = Arc::new(SignatureVerificationService::new(mempool));
        let publisher = Arc::new(InMemoryEventBus::new());
        EventBusAdapter::new(service, publisher)
    }

    /// Create a dummy transaction for testing
    fn create_test_transaction(tx_hash: Hash) -> ValidatedTransaction {
        ValidatedTransaction {
            inner: Transaction {
                from: [0u8; 32],
                to: None,
                value: 0,
                nonce: 0,
                data: vec![],
                signature: [0u8; 64],
            },
            tx_hash,
        }
    }

    /// Dummy mempool for testing (doesn't actually forward)
    #[derive(Clone)]
    struct DummyMempool;

    #[async_trait::async_trait]
    impl crate::ports::outbound::MempoolGateway for DummyMempool {
        async fn submit_verified_transaction(
            &self,
            _tx: crate::domain::entities::VerifiedTransaction,
        ) -> Result<(), crate::ports::outbound::MempoolError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_publish_invalid_transaction() {
        let mempool = DummyMempool;
        let service = Arc::new(SignatureVerificationService::new(mempool));
        let bus = Arc::new(InMemoryEventBus::new());

        // Subscribe to SignatureVerification events
        let mut sub = bus.subscribe(EventFilter::topics(vec![EventTopic::SignatureVerification]));

        let adapter = EventBusAdapter::new(service, bus);

        // Publish invalid transaction event
        let tx_hash = [1u8; 32];
        let reason = "Invalid signature".to_string();
        let receivers = adapter.publish_invalid(tx_hash, reason.clone()).await;

        assert_eq!(receivers, 1);

        // Verify the event was received
        let event = timeout(Duration::from_millis(100), sub.recv())
            .await
            .expect("timeout")
            .expect("event");

        match event {
            BlockchainEvent::TransactionInvalid { hash, reason: r } => {
                assert_eq!(hash, tx_hash);
                assert_eq!(r, reason);
            }
            _ => panic!("Unexpected event type"),
        }
    }

    #[tokio::test]
    async fn test_publish_verified_transaction() {
        let mempool = DummyMempool;
        let service = Arc::new(SignatureVerificationService::new(mempool));
        let bus = Arc::new(InMemoryEventBus::new());

        // Subscribe to SignatureVerification events
        let mut sub = bus.subscribe(EventFilter::topics(vec![EventTopic::SignatureVerification]));

        let adapter = EventBusAdapter::new(service, bus);

        // Create and publish verified transaction event
        let tx = create_test_transaction([2u8; 32]);
        let receivers = adapter.publish_verified(tx.clone()).await;

        assert_eq!(receivers, 1);

        // Verify the event was received
        let event = timeout(Duration::from_millis(100), sub.recv())
            .await
            .expect("timeout")
            .expect("event");

        match event {
            BlockchainEvent::TransactionVerified(received_tx) => {
                assert_eq!(received_tx.tx_hash, tx.tx_hash);
            }
            _ => panic!("Unexpected event type"),
        }
    }

    #[tokio::test]
    async fn test_verify_and_publish_invalid_signature() {
        let adapter = create_test_adapter();

        // Subscribe to events
        let mut sub = adapter
            .publisher()
            .subscribe(EventFilter::topics(vec![EventTopic::SignatureVerification]));

        // Create invalid signature
        let tx_hash = [4u8; 32];
        let message_hash = [5u8; 32];
        let invalid_signature = EcdsaSignature {
            r: [0xFF; 32],
            s: [0xFF; 32],
            v: 27,
        };

        // Verify and publish
        let (result, receivers) = adapter
            .verify_and_publish_result(tx_hash, &message_hash, &invalid_signature)
            .await
            .expect("should succeed");

        assert!(!result.valid);
        assert_eq!(receivers, 1);

        // Verify TransactionInvalid event was published
        let event = timeout(Duration::from_millis(100), sub.recv())
            .await
            .expect("timeout")
            .expect("event");

        assert!(matches!(event, BlockchainEvent::TransactionInvalid { .. }));
    }

    #[tokio::test]
    async fn test_event_filtering() {
        let adapter = create_test_adapter();

        // Subscribe only to Consensus events (should NOT receive SignatureVerification events)
        let mut consensus_sub = adapter
            .publisher()
            .subscribe(EventFilter::topics(vec![EventTopic::Consensus]));

        // Publish invalid transaction
        let tx_hash = [6u8; 32];
        adapter.publish_invalid(tx_hash, "test".to_string()).await;

        // Should NOT receive the event (filtered out)
        let result = consensus_sub.try_recv();
        assert!(matches!(result, Ok(None)));
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let mempool = DummyMempool;
        let service = Arc::new(SignatureVerificationService::new(mempool));
        let bus = Arc::new(InMemoryEventBus::new());

        // Create multiple subscribers
        let _sub1 = bus.subscribe(EventFilter::all());
        let _sub2 = bus.subscribe(EventFilter::topics(vec![EventTopic::SignatureVerification]));
        let _sub3 = bus.subscribe(EventFilter::all());

        let adapter = EventBusAdapter::new(service, bus);

        // Publish event
        let tx_hash = [7u8; 32];
        let receivers = adapter.publish_invalid(tx_hash, "test".to_string()).await;

        // All 3 subscribers should receive the event
        assert_eq!(receivers, 3);
    }
}
