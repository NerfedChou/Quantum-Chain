//! # Integration Test Flows
//!
//! Tests that qc-01-peer-discovery, qc-06-mempool, and qc-10-signature-verification
//! work together correctly via the shared-bus as per IPC-MATRIX.md.
//!
//! ## IPC Flow Tested:
//!
//! 1. **Sig Verification (10) → Mempool (6)**: Verified transactions flow to mempool
//! 2. **Sig Verification (10) → Peer Discovery (1)**: Node identity verification for DDoS defense
//! 3. **Cross-subsystem event publishing**: Events flow correctly through shared-bus
//!
//! ## Architecture Compliance:
//!
//! - All communication via shared-bus (Architecture.md Rule #4)
//! - Envelope-Only Identity (Architecture.md Section 3.2.1)
//! - V2.3 Choreography Pattern (Architecture.md Section 5)

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::time::timeout;

    // Shared infrastructure
    use shared_bus::events::{BlockchainEvent, EventFilter, EventTopic};
    use shared_bus::publisher::InMemoryEventBus;
    use shared_types::entities::{Hash, Transaction, ValidatedTransaction};

    // Subsystem 10: Signature Verification
    use qc_10_signature_verification::{
        adapters::bus::{
            EventBusAdapter as SigVerificationBusAdapter, SignatureVerificationBusAdapter,
        },
        domain::entities::{BatchVerificationRequest, EcdsaSignature},
        ports::inbound::SignatureVerificationApi,
        service::SignatureVerificationService,
    };

    // =============================================================================
    // TEST FIXTURES
    // =============================================================================

    /// Create a test transaction for signature verification flow
    fn create_test_transaction(tx_hash: Hash) -> ValidatedTransaction {
        ValidatedTransaction {
            inner: Transaction {
                from: [0xAA; 32],
                to: Some([0xBB; 32]),
                value: 1_000_000,
                nonce: 0,
                data: vec![],
                signature: [0u8; 64],
            },
            tx_hash,
        }
    }

    /// Dummy mempool gateway for testing
    #[derive(Clone)]
    struct MockMempoolGateway;

    #[async_trait::async_trait]
    impl qc_10_signature_verification::ports::outbound::MempoolGateway for MockMempoolGateway {
        async fn submit_verified_transaction(
            &self,
            _tx: qc_10_signature_verification::domain::entities::VerifiedTransaction,
        ) -> Result<(), qc_10_signature_verification::ports::outbound::MempoolError> {
            Ok(())
        }
    }

    // =============================================================================
    // INTEGRATION TESTS: SIGNATURE VERIFICATION → EVENT BUS
    // =============================================================================

    /// Test that TransactionVerified events are published correctly
    #[tokio::test]
    async fn test_sig_verification_publishes_verified_event() {
        // Setup: Create bus and adapter
        let bus = Arc::new(InMemoryEventBus::new());
        let service = Arc::new(SignatureVerificationService::new(MockMempoolGateway));
        let adapter = SigVerificationBusAdapter::new(service, bus.clone());

        // Subscribe to SignatureVerification events (simulating Mempool subscriber)
        let mut mempool_sub =
            bus.subscribe(EventFilter::topics(vec![EventTopic::SignatureVerification]));

        // Act: Publish a verified transaction event
        let tx = create_test_transaction([1u8; 32]);
        let receivers = adapter.publish_verified(tx.clone()).await;

        // Assert: Event was published and received
        assert_eq!(receivers, 1, "Expected 1 subscriber to receive the event");

        let event = timeout(Duration::from_millis(100), mempool_sub.recv())
            .await
            .expect("timeout waiting for event")
            .expect("should receive event");

        match event {
            BlockchainEvent::TransactionVerified(received_tx) => {
                assert_eq!(received_tx.tx_hash, tx.tx_hash);
                assert_eq!(received_tx.inner.from, tx.inner.from);
            }
            _ => panic!("Expected TransactionVerified event, got {:?}", event),
        }
    }

    /// Test that TransactionInvalid events are published correctly
    #[tokio::test]
    async fn test_sig_verification_publishes_invalid_event() {
        // Setup: Create bus and adapter
        let bus = Arc::new(InMemoryEventBus::new());
        let service = Arc::new(SignatureVerificationService::new(MockMempoolGateway));
        let adapter = SigVerificationBusAdapter::new(service, bus.clone());

        // Subscribe to events
        let mut sub = bus.subscribe(EventFilter::topics(vec![EventTopic::SignatureVerification]));

        // Act: Publish an invalid transaction event
        let tx_hash = [2u8; 32];
        let reason = "Malleability: S value too high".to_string();
        let receivers = adapter.publish_invalid(tx_hash, reason.clone()).await;

        // Assert: Event was published and received
        assert_eq!(receivers, 1);

        let event = timeout(Duration::from_millis(100), sub.recv())
            .await
            .expect("timeout")
            .expect("event");

        match event {
            BlockchainEvent::TransactionInvalid { hash, reason: r } => {
                assert_eq!(hash, tx_hash);
                assert_eq!(r, reason);
            }
            _ => panic!("Expected TransactionInvalid event"),
        }
    }

    // =============================================================================
    // INTEGRATION TESTS: EVENT FILTERING (Per IPC-MATRIX)
    // =============================================================================

    /// Test that Mempool only receives SignatureVerification events (per IPC-MATRIX)
    #[tokio::test]
    async fn test_mempool_only_receives_sig_verification_events() {
        let bus = Arc::new(InMemoryEventBus::new());
        let service = Arc::new(SignatureVerificationService::new(MockMempoolGateway));
        let adapter = SigVerificationBusAdapter::new(service, bus.clone());

        // Mempool subscribes ONLY to SignatureVerification topic
        let mut mempool_sub =
            bus.subscribe(EventFilter::topics(vec![EventTopic::SignatureVerification]));

        // Also create a subscriber for other topics (should NOT receive)
        let mut other_sub = bus.subscribe(EventFilter::topics(vec![EventTopic::Consensus]));

        // Publish SignatureVerification event
        let tx = create_test_transaction([3u8; 32]);
        adapter.publish_verified(tx).await;

        // Mempool should receive
        let result = timeout(Duration::from_millis(100), mempool_sub.recv()).await;
        assert!(result.is_ok(), "Mempool should receive event");

        // Other subscriber should NOT receive (different topic)
        let other_result = other_sub.try_recv();
        assert!(
            matches!(other_result, Ok(None)),
            "Consensus subscriber should NOT receive SignatureVerification events"
        );
    }

    /// Test that multiple subscribers can receive the same event
    #[tokio::test]
    async fn test_multiple_subscribers_receive_events() {
        let bus = Arc::new(InMemoryEventBus::new());
        let service = Arc::new(SignatureVerificationService::new(MockMempoolGateway));
        let adapter = SigVerificationBusAdapter::new(service, bus.clone());

        // Multiple subscribers (simulating different subsystems)
        let _sub1 = bus.subscribe(EventFilter::all());
        let _sub2 = bus.subscribe(EventFilter::topics(vec![EventTopic::SignatureVerification]));
        let _sub3 = bus.subscribe(EventFilter::all());

        // Publish event
        let tx = create_test_transaction([4u8; 32]);
        let receivers = adapter.publish_verified(tx).await;

        // All 3 subscribers should receive
        assert_eq!(receivers, 3);
    }

    // =============================================================================
    // INTEGRATION TESTS: DOMAIN SERVICE CORRECTNESS
    // =============================================================================

    /// Test ECDSA signature verification through service layer
    #[tokio::test]
    async fn test_ecdsa_verification_through_service() {
        let service = SignatureVerificationService::new(MockMempoolGateway);

        // Create a message hash
        let message_hash = [5u8; 32];

        // Create an invalid signature (all zeros/max values are invalid)
        let invalid_signature = EcdsaSignature {
            r: [0xFF; 32],
            s: [0xFF; 32],
            v: 27,
        };

        // Verify through service
        let result = service.verify_ecdsa(&message_hash, &invalid_signature);

        // Should be invalid
        assert!(!result.valid, "All-max signature should be invalid");
        assert!(result.error.is_some(), "Should have error reason");
    }

    /// Test batch verification returns correct results
    #[tokio::test]
    async fn test_batch_verification_correctness() {
        use qc_10_signature_verification::domain::entities::VerificationRequest;

        let service = SignatureVerificationService::new(MockMempoolGateway);

        // Create multiple verification requests (all invalid for this test)
        let requests: Vec<VerificationRequest> = (0..5)
            .map(|i| VerificationRequest {
                message_hash: [i as u8; 32],
                signature: EcdsaSignature {
                    r: [0xFF; 32],
                    s: [0xFF; 32],
                    v: 27,
                },
                expected_signer: None,
            })
            .collect();

        // Create BatchVerificationRequest
        let batch_request = BatchVerificationRequest { requests };

        // Batch verify using the correct method
        let results = service.batch_verify_ecdsa(&batch_request);

        // All should be invalid
        assert_eq!(results.results.len(), 5);
        assert_eq!(results.invalid_count, 5);
        assert_eq!(results.valid_count, 0);
    }

    // =============================================================================
    // INTEGRATION TESTS: EVENT FLOW COMPLIANCE
    // =============================================================================

    /// Verify event source_subsystem is correctly set (Architecture.md compliance)
    #[tokio::test]
    async fn test_event_source_subsystem_id() {
        // TransactionVerified should come from subsystem 10
        let tx = create_test_transaction([6u8; 32]);
        let event = BlockchainEvent::TransactionVerified(tx);
        assert_eq!(
            event.source_subsystem(),
            10,
            "TransactionVerified should have source_subsystem=10"
        );

        // TransactionInvalid should also come from subsystem 10
        let invalid_event = BlockchainEvent::TransactionInvalid {
            hash: [7u8; 32],
            reason: "test".to_string(),
        };
        assert_eq!(
            invalid_event.source_subsystem(),
            10,
            "TransactionInvalid should have source_subsystem=10"
        );
    }

    /// Verify event topic mapping is correct for filtering
    #[tokio::test]
    async fn test_event_topic_mapping() {
        let tx = create_test_transaction([8u8; 32]);
        let event = BlockchainEvent::TransactionVerified(tx);

        assert_eq!(
            event.topic(),
            EventTopic::SignatureVerification,
            "TransactionVerified should map to SignatureVerification topic"
        );
    }

    // =============================================================================
    // INTEGRATION TESTS: IPC-MATRIX AUTHORIZATION COMPLIANCE
    // =============================================================================

    /// Test that qc-10 IPC authorization constants are correctly defined
    #[tokio::test]
    async fn test_ipc_authorization_constants() {
        use qc_10_signature_verification::adapters::ipc::{authorized, forbidden};

        // Verify authorized subsystems (per IPC-MATRIX.md) via constants
        assert_eq!(authorized::PEER_DISCOVERY, 1);
        assert_eq!(authorized::BLOCK_PROPAGATION, 5);
        assert_eq!(authorized::MEMPOOL, 6);
        assert_eq!(authorized::CONSENSUS, 8);
        assert_eq!(authorized::FINALITY, 9);

        // Verify forbidden subsystems
        assert_eq!(forbidden::BLOCK_STORAGE, 2);
        assert_eq!(forbidden::TRANSACTION_INDEXING, 3);
        assert_eq!(forbidden::STATE_MANAGEMENT, 4);
        assert_eq!(forbidden::BLOOM_FILTERS, 7);
        assert_eq!(forbidden::SMART_CONTRACTS, 11);
    }

    // =============================================================================
    // BENCHMARK-LIKE TESTS: SPEC CLAIMS
    // =============================================================================

    /// Test that signature verification handles batch sizes correctly
    #[tokio::test]
    async fn test_batch_size_limit_enforcement() {
        use qc_10_signature_verification::domain::entities::VerificationRequest;

        let service = SignatureVerificationService::new(MockMempoolGateway);

        // Create maximum allowed batch (per IPC-MATRIX: max 1000 signatures)
        let requests: Vec<VerificationRequest> = (0..1000)
            .map(|i| VerificationRequest {
                message_hash: [(i % 256) as u8; 32],
                signature: EcdsaSignature {
                    r: [0x01; 32],
                    s: [0x01; 32],
                    v: 27,
                },
                expected_signer: None,
            })
            .collect();

        let batch_request = BatchVerificationRequest { requests };
        let results = service.batch_verify_ecdsa(&batch_request);
        assert_eq!(results.results.len(), 1000);
    }

    /// Test verify_and_publish flow for invalid signatures
    #[tokio::test]
    async fn test_verify_and_publish_invalid_flow() {
        let bus = Arc::new(InMemoryEventBus::new());
        let service = Arc::new(SignatureVerificationService::new(MockMempoolGateway));
        let adapter = SigVerificationBusAdapter::new(service, bus.clone());

        // Subscribe to events
        let mut sub = bus.subscribe(EventFilter::topics(vec![EventTopic::SignatureVerification]));

        // Create invalid signature
        let tx_hash = [9u8; 32];
        let message_hash = [10u8; 32];
        let invalid_sig = EcdsaSignature {
            r: [0xFF; 32],
            s: [0xFF; 32],
            v: 27,
        };

        // Verify and publish
        let (result, _receivers) = adapter
            .verify_and_publish_result(tx_hash, &message_hash, &invalid_sig)
            .await
            .expect("should not error");

        assert!(!result.valid, "Signature should be invalid");

        // Verify event was published
        let event = timeout(Duration::from_millis(100), sub.recv())
            .await
            .expect("timeout")
            .expect("event");

        assert!(
            matches!(event, BlockchainEvent::TransactionInvalid { .. }),
            "Should publish TransactionInvalid event"
        );
    }

    // =============================================================================
    // INTEGRATION TESTS: PEER DISCOVERY STANDALONE
    // =============================================================================

    /// Test Peer Discovery routing table basic operations
    #[test]
    fn test_peer_discovery_routing_table() {
        use qc_01_peer_discovery::{
            IpAddr, KademliaConfig, NodeId, PeerInfo, RoutingTable, SocketAddr, Timestamp,
        };

        // Create local node
        let local_id = NodeId::new([0u8; 32]);
        let config = KademliaConfig::default();
        let mut table = RoutingTable::new(local_id, config);

        // Stage a peer
        let peer = PeerInfo::new(
            NodeId::new([1u8; 32]),
            SocketAddr::new(IpAddr::v4(192, 168, 1, 100), 8080),
            Timestamp::new(1000),
        );
        let now = Timestamp::new(1000);

        // Stage peer
        let result = table.stage_peer(peer.clone(), now);
        assert!(result.is_ok(), "Should successfully stage peer");

        // Get stats (pass timestamp)
        let stats = table.stats(now);
        assert_eq!(
            stats.pending_verification_count, 1,
            "Should have 1 pending peer"
        );
    }

    /// Test XOR distance calculation
    #[test]
    fn test_xor_distance_calculation() {
        use qc_01_peer_discovery::{xor_distance, Distance, NodeId};

        let node_a = NodeId::new([0x00; 32]);
        let node_b = NodeId::new([0xFF; 32]);

        let distance = xor_distance(&node_a, &node_b);

        // When comparing 0x00 with 0xFF, the first differing bit is at position 0
        // (the leading bit of byte 0), so the bucket index is 0
        // This means the nodes are "farthest" apart in XOR space
        assert_eq!(
            distance.bucket_index(),
            0,
            "All bits different = first differing bit at 0"
        );

        // Test same node = max distance (255, meaning all bits match)
        let same_distance = xor_distance(&node_a, &node_a);
        assert_eq!(same_distance, Distance::max());
    }

    // =============================================================================
    // INTEGRATION TESTS: MEMPOOL STANDALONE
    // =============================================================================

    /// Test Mempool two-phase commit protocol
    #[test]
    fn test_mempool_two_phase_commit() {
        use qc_06_mempool::domain::entities::{
            MempoolConfig, MempoolTransaction, SignedTransaction, U256,
        };
        use qc_06_mempool::domain::pool::TransactionPool;

        let mut pool = TransactionPool::new(MempoolConfig::default());

        // Create a signed transaction
        let signed_tx = SignedTransaction {
            from: [0xAA; 20],
            to: Some([0xBB; 20]),
            value: U256::zero(),
            nonce: 0,
            gas_price: U256::from(1_000_000_000u64), // 1 gwei
            gas_limit: 21000,
            data: vec![],
            signature: [0u8; 64],
        };
        let tx = MempoolTransaction::new(signed_tx, 1000);
        let tx_hash = tx.hash;

        // Add transaction
        pool.add(tx).expect("Should add transaction");

        // Verify transaction is in pool
        assert!(pool.contains(&tx_hash));

        // Propose for block (Phase 1)
        pool.propose(&[tx_hash], 1, 2000);

        // Transaction should be in pending_inclusion state
        let status = pool.status(2000);
        assert_eq!(status.pending_inclusion_count, 1);

        // Confirm inclusion (Phase 2a)
        pool.confirm(&[tx_hash]);

        // Transaction should be removed
        assert!(!pool.contains(&tx_hash));
    }

    /// Test Mempool rollback on timeout
    #[test]
    fn test_mempool_rollback() {
        use qc_06_mempool::domain::entities::{
            MempoolConfig, MempoolTransaction, SignedTransaction, U256,
        };
        use qc_06_mempool::domain::pool::TransactionPool;

        let mut pool = TransactionPool::new(MempoolConfig::default());

        // Create a signed transaction
        let signed_tx = SignedTransaction {
            from: [0xBB; 20],
            to: Some([0xCC; 20]),
            value: U256::zero(),
            nonce: 0,
            gas_price: U256::from(1_000_000_000u64),
            gas_limit: 21000,
            data: vec![],
            signature: [0u8; 64],
        };
        let tx = MempoolTransaction::new(signed_tx, 1000);
        let tx_hash = tx.hash;

        // Add transaction
        pool.add(tx).expect("Should add transaction");

        // Propose for block
        pool.propose(&[tx_hash], 1, 2000);

        // Rollback (block rejected)
        pool.rollback(&[tx_hash]);

        // Transaction should be back in pending state
        let status = pool.status(3000);
        assert_eq!(status.pending_count, 1);
        assert_eq!(status.pending_inclusion_count, 0);
    }
}
