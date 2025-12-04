//! # IPC Message Handler
//!
//! Handles incoming IPC messages with security validation.
//!
//! ## Security Integration
//!
//! This handler uses the **centralized security module** from `shared-types`
//! as mandated by Architecture.md v2.2. This ensures:
//! - Consistent security policy across all subsystems
//! - Single source of truth for HMAC and nonce validation
//! - Reduced code duplication and maintenance burden
//!
//! ## Validation Order (Architecture.md Section 3.5)
//!
//! 1. Timestamp check (bounds all operations, prevents DoS)
//! 2. Version check (before any deserialization)
//! 3. Sender check (authorization per IPC Matrix)
//! 4. Signature check (HMAC via shared-types MessageVerifier)
//! 5. Nonce check (replay prevention via shared-types NonceCache)
//! 6. Reply-to validation (forwarding attack prevention)

use crate::ipc::payloads::{
    FullNodeListRequestPayload, PeerFilter, PeerListRequestPayload, PeerListResponsePayload,
};
use crate::ipc::security::{AuthorizationRules, SecurityError, SubsystemId};
use crate::ports::PeerDiscoveryApi;

use shared_types::security::{KeyProvider, MessageVerifier, NonceCache};
use shared_types::AuthenticatedMessage;
use std::collections::HashMap;
use std::sync::Arc;

/// Correlation ID for request/response tracking.
pub type CorrelationId = [u8; 16];

/// Pending request awaiting response.
#[derive(Debug, Clone)]
pub struct PendingRequest {
    /// When the request was sent.
    pub sent_at: u64,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
    /// The correlation ID.
    pub correlation_id: CorrelationId,
}

/// Static key provider using pre-configured shared secrets.
///
/// Maps each subsystem ID (1-15) to its HMAC shared secret for message
/// authentication per Architecture.md Section 3.5. Production deployments
/// load secrets from environment variables via `NodeConfig`.
///
/// Reference: Architecture.md Section 7.1 (Defense in Depth - Layer 3: IPC Security)
#[derive(Clone)]
pub struct StaticKeyProvider {
    /// HMAC-SHA256 shared secrets indexed by subsystem ID (1-15).
    secrets: HashMap<u8, Vec<u8>>,
}

impl StaticKeyProvider {
    /// Create a new key provider with a default shared secret for all subsystems.
    #[must_use]
    pub fn new(default_secret: &[u8]) -> Self {
        let mut secrets = HashMap::new();
        // Pre-populate with secrets for authorized senders per IPC-MATRIX
        for id in 1..=15 {
            secrets.insert(id, default_secret.to_vec());
        }
        Self { secrets }
    }

    /// Create a key provider with specific per-subsystem secrets.
    #[must_use]
    pub fn with_secrets(secrets: HashMap<u8, Vec<u8>>) -> Self {
        Self { secrets }
    }
}

impl KeyProvider for StaticKeyProvider {
    fn get_shared_secret(&self, sender_id: u8) -> Option<Vec<u8>> {
        self.secrets.get(&sender_id).cloned()
    }
}

/// IPC Handler for Peer Discovery subsystem.
///
/// Uses the centralized `MessageVerifier` from `shared-types` for all
/// security validation, ensuring consistent application of the security
/// policy defined in Architecture.md and IPC-MATRIX.md.
pub struct IpcHandler<K: KeyProvider> {
    /// Our subsystem ID.
    subsystem_id: u8,
    /// Pending outbound requests awaiting responses.
    pending_requests: HashMap<CorrelationId, PendingRequest>,
    /// Default timeout for requests (seconds).
    default_timeout: u64,
    /// Centralized message verifier from shared-types.
    verifier: MessageVerifier<K>,
}

impl IpcHandler<StaticKeyProvider> {
    /// Create a new IPC handler with a default secret.
    ///
    /// # Arguments
    ///
    /// * `secret` - The shared secret for HMAC validation
    ///
    /// # Note
    ///
    /// In production, use `with_key_provider` with a proper key management system.
    #[must_use]
    pub fn new(secret: &[u8]) -> Self {
        let key_provider = StaticKeyProvider::new(secret);
        let nonce_cache = Arc::new(NonceCache::new());
        let verifier = MessageVerifier::new(
            SubsystemId::PeerDiscovery.as_u8(),
            nonce_cache,
            key_provider,
        );

        Self {
            subsystem_id: SubsystemId::PeerDiscovery.as_u8(),
            pending_requests: HashMap::new(),
            default_timeout: Self::DEFAULT_TIMEOUT_SECS,
            verifier,
        }
    }
}

impl<K: KeyProvider> IpcHandler<K> {
    /// Default request timeout in seconds.
    pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

    /// Create a new IPC handler with a custom key provider.
    #[must_use]
    pub fn with_key_provider(key_provider: K, nonce_cache: Arc<NonceCache>) -> Self {
        let verifier = MessageVerifier::new(
            SubsystemId::PeerDiscovery.as_u8(),
            nonce_cache,
            key_provider,
        );

        Self {
            subsystem_id: SubsystemId::PeerDiscovery.as_u8(),
            pending_requests: HashMap::new(),
            default_timeout: Self::DEFAULT_TIMEOUT_SECS,
            verifier,
        }
    }

    /// Get our subsystem ID.
    #[must_use]
    pub const fn subsystem_id(&self) -> u8 {
        self.subsystem_id
    }

    /// Handle an incoming PeerListRequest using the centralized security module.
    ///
    /// # Arguments
    ///
    /// * `message` - The authenticated message wrapper
    /// * `message_bytes` - Raw serialized bytes for signature verification
    /// * `service` - The peer discovery service
    ///
    /// # Returns
    ///
    /// The response payload, or a security error.
    pub fn handle_peer_list_request<S: PeerDiscoveryApi>(
        &self,
        message: &AuthenticatedMessage<PeerListRequestPayload>,
        message_bytes: &[u8],
        service: &S,
    ) -> Result<PeerListResponsePayload, SecurityError> {
        // Step 1: Verify message using centralized security module
        let verification_result = self.verifier.verify(message, message_bytes);
        if !verification_result.is_valid() {
            return Err(SecurityError::from_verification_result(verification_result));
        }

        // Step 2: Validate sender is authorized for this specific message type
        AuthorizationRules::validate_peer_list_sender(message.sender_id)?;

        // Step 3: Process the request
        let payload = &message.payload;
        let peers = if let Some(ref filter) = payload.filter {
            // Filter peers by reputation
            service
                .get_random_peers(payload.max_peers * 2) // Get extra to account for filtering
                .into_iter()
                .filter(|p| p.reputation_score >= filter.min_reputation)
                .take(payload.max_peers)
                .collect()
        } else {
            service.get_random_peers(payload.max_peers)
        };

        let total_available = service.get_stats().total_peers;

        Ok(PeerListResponsePayload {
            peers,
            total_available,
        })
    }

    /// Handle an incoming FullNodeListRequest using the centralized security module.
    ///
    /// # Arguments
    ///
    /// * `message` - The authenticated message wrapper
    /// * `message_bytes` - Raw serialized bytes for signature verification
    /// * `service` - The peer discovery service
    ///
    /// # Returns
    ///
    /// The response payload, or a security error.
    pub fn handle_full_node_list_request<S: PeerDiscoveryApi>(
        &self,
        message: &AuthenticatedMessage<FullNodeListRequestPayload>,
        message_bytes: &[u8],
        service: &S,
    ) -> Result<PeerListResponsePayload, SecurityError> {
        // Step 1: Verify message using centralized security module
        let verification_result = self.verifier.verify(message, message_bytes);
        if !verification_result.is_valid() {
            return Err(SecurityError::from_verification_result(verification_result));
        }

        // Step 2: Validate sender is authorized (Subsystem 13 only)
        AuthorizationRules::validate_full_node_list_sender(message.sender_id)?;

        // Step 3: Process the request
        let payload = &message.payload;
        let filter = PeerFilter {
            min_reputation: 50, // Full nodes require baseline reputation per SPEC-01 Section 6.1
            exclude_subnets: vec![],
        };

        let peers = service
            .get_random_peers(payload.max_nodes * 2)
            .into_iter()
            .filter(|p| p.reputation_score >= filter.min_reputation)
            .take(payload.max_nodes)
            .collect();

        let total_available = service.get_stats().total_peers;

        Ok(PeerListResponsePayload {
            peers,
            total_available,
        })
    }

    /// Register a pending outbound request.
    ///
    /// # Arguments
    ///
    /// * `correlation_id` - The unique correlation ID for this request
    /// * `now` - Current timestamp
    pub fn register_pending_request(&mut self, correlation_id: CorrelationId, now: u64) {
        self.pending_requests.insert(
            correlation_id,
            PendingRequest {
                sent_at: now,
                timeout_secs: self.default_timeout,
                correlation_id,
            },
        );
    }

    /// Handle a response by matching correlation ID.
    ///
    /// # Arguments
    ///
    /// * `correlation_id` - The correlation ID from the response
    ///
    /// # Returns
    ///
    /// The pending request if found, None otherwise.
    pub fn match_response(&mut self, correlation_id: &CorrelationId) -> Option<PendingRequest> {
        self.pending_requests.remove(correlation_id)
    }

    /// Remove expired pending requests.
    ///
    /// # Arguments
    ///
    /// * `now` - Current timestamp
    ///
    /// # Returns
    ///
    /// Number of expired requests removed.
    pub fn gc_expired_requests(&mut self, now: u64) -> usize {
        let before_count = self.pending_requests.len();
        self.pending_requests.retain(|_, req| {
            let deadline = req.sent_at.saturating_add(req.timeout_secs);
            now <= deadline
        });
        before_count - self.pending_requests.len()
    }

    /// Get count of pending requests.
    #[must_use]
    pub fn pending_request_count(&self) -> usize {
        self.pending_requests.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        BanReason, IpAddr, KademliaConfig, NodeId, PeerDiscoveryError, PeerInfo, RoutingTable,
        RoutingTableStats, SocketAddr, Timestamp,
    };
    use crate::ports::PeerDiscoveryApi;

    /// Test-only implementation of PeerDiscoveryApi for IPC handler unit tests.
    /// Uses a real RoutingTable with deterministic timestamps (epoch 1000).
    #[allow(dead_code)]
    struct TestPeerDiscoveryService {
        routing_table: RoutingTable,
    }

    impl TestPeerDiscoveryService {
        /// Creates an empty service with zero-initialized local NodeId.
        #[allow(dead_code)]
        fn new() -> Self {
            let local_id = NodeId::new([0u8; 32]);
            let config = KademliaConfig::for_testing();
            Self {
                routing_table: RoutingTable::new(local_id, config),
            }
        }

        /// Creates a service pre-populated with verified peers for testing.
        /// Each peer has unique NodeId and IP address in distinct /24 subnets.
        #[allow(dead_code)]
        fn with_peers(peer_count: usize) -> Self {
            let mut service = Self::new();
            let now = Timestamp::new(1000);

            for i in 1..=peer_count {
                let mut id_bytes = [0u8; 32];
                id_bytes[0] = i as u8;
                let peer = PeerInfo::new(
                    NodeId::new(id_bytes),
                    SocketAddr::new(IpAddr::v4(10, (i / 256) as u8, (i % 256) as u8, 1), 8080),
                    now,
                );
                if let Ok(true) = service.routing_table.stage_peer(peer.clone(), now) {
                    let _ = service
                        .routing_table
                        .on_verification_result(&peer.node_id, true, now);
                }
            }
            service
        }
    }

    impl PeerDiscoveryApi for TestPeerDiscoveryService {
        fn find_closest_peers(&self, target_id: NodeId, count: usize) -> Vec<PeerInfo> {
            self.routing_table.find_closest_peers(&target_id, count)
        }

        fn add_peer(&mut self, peer: PeerInfo) -> Result<bool, PeerDiscoveryError> {
            let now = Timestamp::new(1000);
            self.routing_table.stage_peer(peer, now)
        }

        fn get_random_peers(&self, count: usize) -> Vec<PeerInfo> {
            self.routing_table.get_random_peers(count)
        }

        fn ban_peer(
            &mut self,
            node_id: NodeId,
            duration_seconds: u64,
            reason: BanReason,
        ) -> Result<(), PeerDiscoveryError> {
            let now = Timestamp::new(1000);
            self.routing_table
                .ban_peer(node_id, duration_seconds, reason, now)
        }

        fn is_banned(&self, node_id: NodeId) -> bool {
            let now = Timestamp::new(1000);
            self.routing_table.is_banned(&node_id, now)
        }

        fn touch_peer(&mut self, node_id: NodeId) -> Result<(), PeerDiscoveryError> {
            let now = Timestamp::new(1000);
            self.routing_table.touch_peer(&node_id, now)
        }

        fn remove_peer(&mut self, node_id: NodeId) -> Result<(), PeerDiscoveryError> {
            self.routing_table.remove_peer(&node_id)
        }

        fn get_stats(&self) -> RoutingTableStats {
            let now = Timestamp::new(1000);
            self.routing_table.stats(now)
        }
    }

    #[test]
    fn test_handler_new() {
        let handler = IpcHandler::new(&[0u8; 32]);
        assert_eq!(handler.subsystem_id(), SubsystemId::PeerDiscovery.as_u8());
        assert_eq!(handler.pending_request_count(), 0);
    }

    #[test]
    fn test_pending_request_tracking() {
        let mut handler = IpcHandler::new(&[0u8; 32]);
        let correlation_id = [1u8; 16];
        let now = 1000u64;

        handler.register_pending_request(correlation_id, now);
        assert_eq!(handler.pending_request_count(), 1);

        // Correlation ID lookup removes the pending request (one-time use per Architecture.md 3.3)
        let matched = handler.match_response(&correlation_id);
        assert!(matched.is_some());
        assert_eq!(handler.pending_request_count(), 0);

        // Subsequent lookups return None - correlation IDs are single-use for replay prevention
        let matched_again = handler.match_response(&correlation_id);
        assert!(matched_again.is_none());
    }

    #[test]
    fn test_gc_expired_requests() {
        let mut handler = IpcHandler::new(&[0u8; 32]);
        let correlation_id = [1u8; 16];
        let now = 1000u64;

        handler.register_pending_request(correlation_id, now);
        assert_eq!(handler.pending_request_count(), 1);

        // GC removes requests past their deadline (default 30s per Architecture.md 3.3)
        let expired_time = now + IpcHandler::<StaticKeyProvider>::DEFAULT_TIMEOUT_SECS + 1;
        let removed = handler.gc_expired_requests(expired_time);
        assert_eq!(removed, 1);
        assert_eq!(handler.pending_request_count(), 0);
    }
}
