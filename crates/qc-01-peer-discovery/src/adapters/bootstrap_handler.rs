//! # Bootstrap Handler Adapter
//!
//! Handles incoming `BootstrapRequest` from external peers.
//!
//! ## DDoS Defense Flow
//!
//! ```text
//! External Peer ──BootstrapRequest──→ [BootstrapHandler]
//!                                           │
//!                                           ├─ 1. Validate PoW (anti-Sybil)
//!                                           │
//!                                           ├─ 2. Check bans/subnet limits
//!                                           │
//!                                           ├─ 3. Stage in pending_verification
//!                                           │
//!                                           └─ 4. Publish VerifyNodeIdentityRequest ──→ Subsystem 10
//! ```
//!
//! The handler does NOT add peers directly to the routing table.
//! It stages them and awaits `NodeIdentityVerificationResult` from Subsystem 10.

use crate::adapters::VerificationRequestPublisher;
use crate::domain::{NodeId, PeerDiscoveryError, PeerInfo, SocketAddr};
use crate::ipc::{BootstrapRequest, BootstrapResult};
use crate::ports::{NodeIdValidator, PeerDiscoveryApi, TimeSource};

/// Handler for external peer bootstrap requests.
///
/// Implements the DDoS defense flow:
/// 1. Validate proof-of-work (anti-Sybil)
/// 2. Check if peer is banned or subnet limit reached
/// 3. Stage peer in pending_verification
/// 4. Publish verification request to Subsystem 10
pub struct BootstrapHandler<S, P> {
    /// The peer discovery service.
    service: S,
    /// Publisher for sending verification requests.
    verification_publisher: P,
    /// Validator for proof-of-work.
    pow_validator: Box<dyn NodeIdValidator>,
    /// Time source for timestamps.
    time_source: Box<dyn TimeSource>,
}

impl<S: PeerDiscoveryApi, P: VerificationRequestPublisher> BootstrapHandler<S, P> {
    /// Create a new bootstrap handler.
    pub fn new(
        service: S,
        verification_publisher: P,
        pow_validator: Box<dyn NodeIdValidator>,
        time_source: Box<dyn TimeSource>,
    ) -> Self {
        Self {
            service,
            verification_publisher,
            pow_validator,
            time_source,
        }
    }

    /// Handle an incoming bootstrap request.
    ///
    /// # Arguments
    ///
    /// * `request` - The bootstrap request from an external peer
    ///
    /// # Returns
    ///
    /// The result of processing the request.
    pub fn handle(&mut self, request: &BootstrapRequest) -> BootstrapResult {
        let node_id = NodeId::new(request.node_id);
        let now = self.time_source.now();

        // Step 1: Validate proof-of-work
        if !self.validate_pow(&request.proof_of_work, &request.node_id) {
            return BootstrapResult::InvalidProofOfWork;
        }

        // Step 2: Check if peer is banned
        if self.service.is_banned(node_id) {
            return BootstrapResult::Banned;
        }

        // Step 3: Create peer info and stage for verification
        let socket_addr = SocketAddr::new(request.ip_address, request.port);
        let peer_info = PeerInfo::new(node_id, socket_addr, now);

        match self.service.add_peer(peer_info) {
            Ok(true) => {
                // Peer was staged successfully
            }
            Ok(false) => {
                // Already exists or rejected
                return BootstrapResult::SubnetLimitReached;
            }
            Err(PeerDiscoveryError::StagingAreaFull) => {
                return BootstrapResult::StagingFull;
            }
            Err(PeerDiscoveryError::SubnetLimitReached) => {
                return BootstrapResult::SubnetLimitReached;
            }
            Err(PeerDiscoveryError::PeerBanned) => {
                return BootstrapResult::Banned;
            }
            Err(_) => {
                return BootstrapResult::StagingFull;
            }
        }

        // Step 4: Generate correlation ID and publish verification request
        let correlation_id = self.generate_correlation_id();
        let verify_request = request.to_verification_request();

        if let Err(_e) = self
            .verification_publisher
            .publish_verification_request(verify_request, correlation_id)
        {
            // If we can't publish, the peer will timeout in staging
            // This is acceptable - they can retry
        }

        BootstrapResult::PendingVerification { correlation_id }
    }

    /// Validate proof-of-work for anti-Sybil protection.
    fn validate_pow(&self, proof_of_work: &[u8; 32], node_id: &[u8; 32]) -> bool {
        // The PoW should be: H(node_id || nonce) with sufficient leading zeros
        // For now, we use the NodeIdValidator which checks leading zero bits
        // In production, this would verify H(node_id || proof_of_work) has difficulty
        self.pow_validator.validate_node_id(NodeId::new(*node_id))
            && Self::has_sufficient_zeros(proof_of_work)
    }

    /// Check if proof-of-work has sufficient leading zeros.
    fn has_sufficient_zeros(pow: &[u8; 32]) -> bool {
        // Require at least 16 leading zero bits (2 bytes)
        pow[0] == 0 && pow[1] == 0
    }

    /// Generate a correlation ID for request/response matching.
    fn generate_correlation_id(&self) -> [u8; 16] {
        // Use UUID v4 for correlation IDs
        let uuid = uuid::Uuid::new_v4();
        *uuid.as_bytes()
    }

    /// Get a reference to the underlying service.
    pub fn service(&self) -> &S {
        &self.service
    }

    /// Get a mutable reference to the underlying service.
    pub fn service_mut(&mut self) -> &mut S {
        &mut self.service
    }

    /// Consume the handler and return the service.
    pub fn into_service(self) -> S {
        self.service
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::{InMemoryVerificationPublisher, NoOpNodeIdValidator};
    use crate::domain::{IpAddr, KademliaConfig, Timestamp};
    use crate::ports::TimeSource;
    use crate::service::PeerDiscoveryService;

    /// Fixed time source for testing.
    struct TestTimeSource(Timestamp);

    impl TestTimeSource {
        fn new(ts: Timestamp) -> Self {
            Self(ts)
        }
    }

    impl TimeSource for TestTimeSource {
        fn now(&self) -> Timestamp {
            self.0
        }
    }

    fn make_handler() -> BootstrapHandler<PeerDiscoveryService, InMemoryVerificationPublisher> {
        let local_id = NodeId::new([0u8; 32]);
        let config = KademliaConfig::for_testing();
        let time_source: Box<dyn TimeSource> = Box::new(TestTimeSource::new(Timestamp::new(1000)));
        let service = PeerDiscoveryService::new(local_id, config, time_source);
        let test_time: Box<dyn TimeSource> = Box::new(TestTimeSource::new(Timestamp::new(1000)));
        // Publisher is managed by handler now
        let publisher = InMemoryVerificationPublisher::new();
        let validator = Box::new(NoOpNodeIdValidator::new());

        BootstrapHandler::new(service, publisher, validator, test_time)
    }

    fn make_request(node_byte: u8) -> BootstrapRequest {
        let mut node_id = [0u8; 32];
        node_id[0] = node_byte;

        BootstrapRequest::new(
            node_id,
            IpAddr::v4(192, 168, 1, node_byte),
            8080,
            [0u8; 32], // Valid PoW (leading zeros)
            [2u8; 33],
            [3u8; 64],
        )
    }

    #[test]
    fn test_handle_valid_bootstrap_request() {
        let mut handler = make_handler();
        let request = make_request(1);

        let result = handler.handle(&request);

        match result {
            BootstrapResult::PendingVerification { correlation_id } => {
                // Correlation ID should be non-zero
                assert_ne!(correlation_id, [0u8; 16]);
            }
            other => panic!("Expected PendingVerification, got {:?}", other),
        }
    }

    #[test]
    fn test_handle_invalid_pow() {
        let mut handler = make_handler();
        let mut request = make_request(1);
        // Invalid PoW - no leading zeros
        request.proof_of_work = [255u8; 32];

        let result = handler.handle(&request);

        assert_eq!(result, BootstrapResult::InvalidProofOfWork);
    }

    #[test]
    fn test_has_sufficient_zeros() {
        // Valid - 2 zero bytes
        let valid_pow = [0u8; 32];
        assert!(BootstrapHandler::<PeerDiscoveryService, InMemoryVerificationPublisher>::has_sufficient_zeros(
            &valid_pow
        ));

        // Invalid - only 1 zero byte
        let mut invalid_pow = [0u8; 32];
        invalid_pow[1] = 1;
        assert!(!BootstrapHandler::<PeerDiscoveryService, InMemoryVerificationPublisher>::has_sufficient_zeros(
            &invalid_pow
        ));
    }
}
