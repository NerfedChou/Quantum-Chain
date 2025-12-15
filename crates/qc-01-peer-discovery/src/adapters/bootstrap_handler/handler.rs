use super::security::{generate_correlation_id, ProofOfWork};
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

/// Configuration for the BootstrapHandler.
pub struct BootstrapHandlerConfig {
    /// Validator for proof-of-work.
    pub pow_validator: Box<dyn NodeIdValidator>,
    /// Time source for timestamps.
    pub time_source: Box<dyn TimeSource>,
    /// Required PoW difficulty (leading zero bits).
    pub pow_difficulty: u32,
}

impl<S: PeerDiscoveryApi, P: VerificationRequestPublisher> BootstrapHandler<S, P> {
    /// Create a new bootstrap handler with production PoW difficulty (24 bits).
    pub fn new(
        service: S,
        verification_publisher: P,
        pow_validator: Box<dyn NodeIdValidator>,
        time_source: Box<dyn TimeSource>,
    ) -> Self {
        Self::with_config(
            service,
            verification_publisher,
            BootstrapHandlerConfig {
                pow_validator,
                time_source,
                pow_difficulty: 24,
            },
        )
    }

    /// Create a bootstrap handler with custom configuration.
    pub fn with_config(
        service: S,
        verification_publisher: P,
        config: BootstrapHandlerConfig,
    ) -> Self {
        Self {
            service,
            verification_publisher,
            pow_validator: config.pow_validator,
            time_source: config.time_source,
            pow_difficulty: config.pow_difficulty,
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
        // First check: NodeId must also pass the validator (additional constraint)
        if !self.pow_validator.validate_node_id(node_id) {
            return BootstrapResult::InvalidProofOfWork;
        }

        // Compute H(node_id || proof_of_work) and verify difficulty
        let pow = ProofOfWork::new(request.proof_of_work);
        if !pow.validate(&node_id, self.pow_difficulty) {
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
        let correlation_id = generate_correlation_id();
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
