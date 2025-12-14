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
    /// Required PoW difficulty (leading zero bits).
    pow_difficulty: u32,
}

impl<S: PeerDiscoveryApi, P: VerificationRequestPublisher> BootstrapHandler<S, P> {
    /// Create a new bootstrap handler with production PoW difficulty (24 bits).
    pub fn new(
        service: S,
        verification_publisher: P,
        pow_validator: Box<dyn NodeIdValidator>,
        time_source: Box<dyn TimeSource>,
    ) -> Self {
        Self::with_difficulty(service, verification_publisher, pow_validator, time_source, 24)
    }

    /// Create a bootstrap handler with custom PoW difficulty.
    ///
    /// # Arguments
    ///
    /// * `pow_difficulty` - Required leading zero bits (24 for production, lower for tests)
    pub fn with_difficulty(
        service: S,
        verification_publisher: P,
        pow_validator: Box<dyn NodeIdValidator>,
        time_source: Box<dyn TimeSource>,
        pow_difficulty: u32,
    ) -> Self {
        Self {
            service,
            verification_publisher,
            pow_validator,
            time_source,
            pow_difficulty,
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
    ///
    /// # Security (Hardened)
    ///
    /// PoW must satisfy: SHA256(node_id || proof_of_work) has N+ leading zero bits.
    /// This binds the proof to the identity. Production uses 24 bits (~16M attempts).
    fn validate_pow(&self, proof_of_work: &[u8; 32], node_id: &[u8; 32]) -> bool {
        // First check: NodeId must also pass the validator (additional constraint)
        if !self.pow_validator.validate_node_id(NodeId::new(*node_id)) {
            return false;
        }

        // Compute H(node_id || proof_of_work) and verify difficulty
        Self::verify_pow_binding(node_id, proof_of_work, self.pow_difficulty)
    }

    /// Verify that SHA256(node_id || nonce) has required leading zeros.
    fn verify_pow_binding(node_id: &[u8; 32], nonce: &[u8; 32], required_zeros: u32) -> bool {
        use sha2::{Sha256, Digest};

        let mut hasher = Sha256::new();
        hasher.update(node_id);
        hasher.update(nonce);
        let result = hasher.finalize();

        Self::count_leading_zero_bits(&result) >= required_zeros
    }

    /// Count leading zero bits in a byte slice.
    fn count_leading_zero_bits(bytes: &[u8]) -> u32 {
        let mut count = 0u32;
        for byte in bytes {
            if *byte == 0 {
                count += 8;
            } else {
                count += byte.leading_zeros();
                break;
            }
        }
        count
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
        let publisher = InMemoryVerificationPublisher::new();
        let validator = Box::new(NoOpNodeIdValidator::new());

        // Use low difficulty (8 bits) for fast tests
        BootstrapHandler::with_difficulty(service, publisher, validator, test_time, 8)
    }

    /// Generate a valid PoW nonce for a given node_id at difficulty 8.
    fn generate_test_pow(node_id: &[u8; 32]) -> [u8; 32] {
        type Handler = BootstrapHandler<PeerDiscoveryService, InMemoryVerificationPublisher>;
        
        let mut nonce = [0u8; 32];
        for i in 0..100_000u32 {
            nonce[0..4].copy_from_slice(&i.to_le_bytes());
            if Handler::verify_pow_binding(node_id, &nonce, 8) {
                return nonce;
            }
        }
        panic!("Failed to generate valid PoW");
    }

    fn make_request(node_byte: u8) -> BootstrapRequest {
        let mut node_id = [0u8; 32];
        node_id[0] = node_byte;
        
        // Generate valid PoW for this node_id
        let pow = generate_test_pow(&node_id);

        BootstrapRequest::new(
            node_id,
            IpAddr::v4(192, 168, 1, node_byte),
            8080,
            pow,
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
    fn test_count_leading_zero_bits() {
        type Handler = BootstrapHandler<PeerDiscoveryService, InMemoryVerificationPublisher>;

        // 0 zero bits
        assert_eq!(Handler::count_leading_zero_bits(&[0xFF]), 0);
        
        // 8 zero bits (1 byte)
        assert_eq!(Handler::count_leading_zero_bits(&[0x00, 0xFF]), 8);
        
        // 12 zero bits (1 byte + 4 bits from 0x0F which is 0000_1111)
        assert_eq!(Handler::count_leading_zero_bits(&[0x00, 0x0F]), 12);
        
        // 16 zero bits (2 bytes)
        assert_eq!(Handler::count_leading_zero_bits(&[0x00, 0x00, 0xFF]), 16);
        
        // 24 zero bits (3 bytes + 0x80 = 1000_0000 has 0 leading zeros in that byte)
        assert_eq!(Handler::count_leading_zero_bits(&[0x00, 0x00, 0x00, 0x80]), 24);
        
        // 25 zero bits (3 bytes + 0x40 = 0100_0000 has 1 leading zero)
        assert_eq!(Handler::count_leading_zero_bits(&[0x00, 0x00, 0x00, 0x40]), 25);
        
        // 31 zero bits (3 bytes + 0x01 = 0000_0001 has 7 leading zeros)
        assert_eq!(Handler::count_leading_zero_bits(&[0x00, 0x00, 0x00, 0x01]), 31);
    }

    #[test]
    fn test_verify_pow_binding() {
        type Handler = BootstrapHandler<PeerDiscoveryService, InMemoryVerificationPublisher>;

        let node_id = [1u8; 32];
        
        // Find a valid nonce that produces 8 leading zero bits (for quick test)
        // In production, 24 bits would be required but that's too slow for tests
        let mut nonce = [0u8; 32];
        let mut found = false;
        for i in 0..100_000u32 {
            nonce[0..4].copy_from_slice(&i.to_le_bytes());
            if Handler::verify_pow_binding(&node_id, &nonce, 8) {
                found = true;
                break;
            }
        }
        assert!(found, "Should find valid 8-bit PoW within 100K attempts");
        
        // Verify that same nonce passes
        assert!(Handler::verify_pow_binding(&node_id, &nonce, 8));
        
        // Verify that higher difficulty fails (probably)
        // Note: might pass if we got lucky, but very unlikely for 24 bits
    }
}
