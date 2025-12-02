//! # IPC Message Handler
//!
//! Handles incoming IPC messages with security validation.
//!
//! ## Validation Order (Architecture.md Section 3.5)
//!
//! 1. Timestamp check (bounds all operations, prevents DoS)
//! 2. Version check (before any deserialization)
//! 3. Sender check (authorization per IPC Matrix)
//! 4. Signature check (HMAC)
//! 5. Nonce check (replay prevention via TimeBoundedNonceCache)
//! 6. Reply-to validation (forwarding attack prevention)

use crate::ipc::payloads::{
    FullNodeListRequestPayload, PeerFilter, PeerListRequestPayload, PeerListResponsePayload,
};
use crate::ipc::security::{AuthorizationRules, SecurityError, SubsystemId};
use crate::ports::PeerDiscoveryApi;

use std::collections::HashMap;

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

/// IPC Handler for Peer Discovery subsystem.
///
/// Processes incoming requests with security validation per IPC-MATRIX.md.
pub struct IpcHandler {
    /// Our subsystem ID.
    subsystem_id: u8,
    /// Pending outbound requests awaiting responses.
    pending_requests: HashMap<CorrelationId, PendingRequest>,
    /// Default timeout for requests (seconds).
    default_timeout: u64,
}

impl Default for IpcHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl IpcHandler {
    /// Default request timeout in seconds.
    pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

    /// Create a new IPC handler for Peer Discovery.
    #[must_use]
    pub fn new() -> Self {
        Self {
            subsystem_id: SubsystemId::PeerDiscovery.as_u8(),
            pending_requests: HashMap::new(),
            default_timeout: Self::DEFAULT_TIMEOUT_SECS,
        }
    }

    /// Get our subsystem ID.
    #[must_use]
    pub const fn subsystem_id(&self) -> u8 {
        self.subsystem_id
    }

    /// Handle an incoming PeerListRequest.
    ///
    /// # Arguments
    ///
    /// * `sender_id` - The sender's subsystem ID (from envelope)
    /// * `timestamp` - Message timestamp (from envelope)
    /// * `now` - Current time
    /// * `reply_to_subsystem` - The reply_to subsystem ID (from envelope)
    /// * `payload` - The request payload
    /// * `service` - The peer discovery service
    ///
    /// # Returns
    ///
    /// The response payload, or a security error.
    pub fn handle_peer_list_request<S: PeerDiscoveryApi>(
        &self,
        sender_id: u8,
        timestamp: u64,
        now: u64,
        reply_to_subsystem: Option<u8>,
        payload: &PeerListRequestPayload,
        service: &S,
    ) -> Result<PeerListResponsePayload, SecurityError> {
        // Step 1: Validate timestamp (MUST be first per Architecture.md)
        AuthorizationRules::validate_timestamp(timestamp, now)?;

        // Step 2: Validate sender is authorized
        AuthorizationRules::validate_peer_list_sender(sender_id)?;

        // Step 3: Validate reply_to matches sender (forwarding attack prevention)
        AuthorizationRules::validate_reply_to(sender_id, reply_to_subsystem)?;

        // Step 4: Process the request
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

    /// Handle an incoming FullNodeListRequest.
    ///
    /// # Arguments
    ///
    /// * `sender_id` - The sender's subsystem ID (from envelope)
    /// * `timestamp` - Message timestamp (from envelope)
    /// * `now` - Current time
    /// * `reply_to_subsystem` - The reply_to subsystem ID (from envelope)
    /// * `payload` - The request payload
    /// * `service` - The peer discovery service
    ///
    /// # Returns
    ///
    /// The response payload, or a security error.
    pub fn handle_full_node_list_request<S: PeerDiscoveryApi>(
        &self,
        sender_id: u8,
        timestamp: u64,
        now: u64,
        reply_to_subsystem: Option<u8>,
        payload: &FullNodeListRequestPayload,
        service: &S,
    ) -> Result<PeerListResponsePayload, SecurityError> {
        // Step 1: Validate timestamp
        AuthorizationRules::validate_timestamp(timestamp, now)?;

        // Step 2: Validate sender is authorized (Subsystem 13 only)
        AuthorizationRules::validate_full_node_list_sender(sender_id)?;

        // Step 3: Validate reply_to matches sender
        AuthorizationRules::validate_reply_to(sender_id, reply_to_subsystem)?;

        // Step 4: Process the request
        // For full nodes, we prioritize high-reputation peers
        let filter = PeerFilter {
            min_reputation: 50, // Full nodes should have good reputation
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

    /// Mock service for testing
    struct MockPeerDiscoveryService {
        routing_table: RoutingTable,
    }

    impl MockPeerDiscoveryService {
        fn new() -> Self {
            let local_id = NodeId::new([0u8; 32]);
            let config = KademliaConfig::for_testing();
            Self {
                routing_table: RoutingTable::new(local_id, config),
            }
        }

        fn with_peers(peer_count: usize) -> Self {
            let mut service = Self::new();
            let now = Timestamp::new(1000);

            for i in 1..=peer_count {
                let mut id_bytes = [0u8; 32];
                id_bytes[0] = i as u8;
                // Use different /24 subnets to avoid subnet limit (max 2 per subnet)
                // Each peer gets its own /24 subnet
                let peer = PeerInfo::new(
                    NodeId::new(id_bytes),
                    SocketAddr::new(IpAddr::v4(10, (i / 256) as u8, (i % 256) as u8, 1), 8080),
                    now,
                );
                // Stage and verify immediately for testing
                if let Ok(true) = service.routing_table.stage_peer(peer.clone(), now) {
                    let _ = service
                        .routing_table
                        .on_verification_result(&peer.node_id, true, now);
                }
            }
            service
        }
    }

    impl PeerDiscoveryApi for MockPeerDiscoveryService {
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
            self.routing_table.ban_peer(node_id, duration_seconds, reason, now)
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
        let handler = IpcHandler::new();
        assert_eq!(handler.subsystem_id(), SubsystemId::PeerDiscovery.as_u8());
        assert_eq!(handler.pending_request_count(), 0);
    }

    #[test]
    fn test_handle_peer_list_request_authorized() {
        let handler = IpcHandler::new();
        let service = MockPeerDiscoveryService::with_peers(10);
        let payload = PeerListRequestPayload::new(5);
        let now = 1000u64;

        // Block Propagation (5) is authorized
        let result = handler.handle_peer_list_request(
            5,
            now,
            now,
            Some(5),
            &payload,
            &service,
        );
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.peers.len() <= 5);
        // total_available may be less than 10 due to subnet limits in testing
        assert!(response.total_available > 0);
    }

    #[test]
    fn test_handle_peer_list_request_unauthorized() {
        let handler = IpcHandler::new();
        let service = MockPeerDiscoveryService::new();
        let payload = PeerListRequestPayload::new(5);
        let now = 1000u64;

        // Consensus (8) is NOT authorized for PeerListRequest
        let result = handler.handle_peer_list_request(
            8,
            now,
            now,
            Some(8),
            &payload,
            &service,
        );
        assert!(matches!(
            result,
            Err(SecurityError::UnauthorizedSender { .. })
        ));
    }

    #[test]
    fn test_handle_peer_list_request_timestamp_invalid() {
        let handler = IpcHandler::new();
        let service = MockPeerDiscoveryService::new();
        let payload = PeerListRequestPayload::new(5);
        let now = 1000u64;

        // Old timestamp (100 seconds ago)
        let result = handler.handle_peer_list_request(
            5,
            now - 100,
            now,
            Some(5),
            &payload,
            &service,
        );
        assert!(matches!(
            result,
            Err(SecurityError::TimestampOutOfRange { .. })
        ));
    }

    #[test]
    fn test_handle_peer_list_request_reply_to_mismatch() {
        let handler = IpcHandler::new();
        let service = MockPeerDiscoveryService::new();
        let payload = PeerListRequestPayload::new(5);
        let now = 1000u64;

        // sender_id=5 but reply_to=13 (forwarding attack)
        let result = handler.handle_peer_list_request(
            5,
            now,
            now,
            Some(13),
            &payload,
            &service,
        );
        assert!(matches!(result, Err(SecurityError::ReplyToMismatch { .. })));
    }

    #[test]
    fn test_handle_full_node_list_request_authorized() {
        let handler = IpcHandler::new();
        let service = MockPeerDiscoveryService::with_peers(10);
        let payload = FullNodeListRequestPayload::new(5);
        let now = 1000u64;

        // Light Clients (13) is authorized
        let result = handler.handle_full_node_list_request(
            13,
            now,
            now,
            Some(13),
            &payload,
            &service,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_full_node_list_request_unauthorized() {
        let handler = IpcHandler::new();
        let service = MockPeerDiscoveryService::new();
        let payload = FullNodeListRequestPayload::new(5);
        let now = 1000u64;

        // Block Propagation (5) is NOT authorized for FullNodeListRequest
        let result = handler.handle_full_node_list_request(
            5,
            now,
            now,
            Some(5),
            &payload,
            &service,
        );
        assert!(matches!(
            result,
            Err(SecurityError::UnauthorizedSender { .. })
        ));
    }

    #[test]
    fn test_pending_request_tracking() {
        let mut handler = IpcHandler::new();
        let correlation_id = [1u8; 16];
        let now = 1000u64;

        handler.register_pending_request(correlation_id, now);
        assert_eq!(handler.pending_request_count(), 1);

        // Match the response
        let matched = handler.match_response(&correlation_id);
        assert!(matched.is_some());
        assert_eq!(handler.pending_request_count(), 0);

        // Second match should fail
        let matched_again = handler.match_response(&correlation_id);
        assert!(matched_again.is_none());
    }

    #[test]
    fn test_gc_expired_requests() {
        let mut handler = IpcHandler::new();
        let correlation_id = [1u8; 16];
        let now = 1000u64;

        handler.register_pending_request(correlation_id, now);
        assert_eq!(handler.pending_request_count(), 1);

        // After timeout, request should be removed
        let expired_time = now + IpcHandler::DEFAULT_TIMEOUT_SECS + 1;
        let removed = handler.gc_expired_requests(expired_time);
        assert_eq!(removed, 1);
        assert_eq!(handler.pending_request_count(), 0);
    }

    #[test]
    fn test_peer_list_with_filter() {
        let handler = IpcHandler::new();
        let service = MockPeerDiscoveryService::with_peers(10);
        let filter = PeerFilter {
            min_reputation: 50,
            exclude_subnets: vec![],
        };
        let payload = PeerListRequestPayload::with_filter(5, filter);
        let now = 1000u64;

        let result = handler.handle_peer_list_request(
            5,
            now,
            now,
            Some(5),
            &payload,
            &service,
        );
        assert!(result.is_ok());
        let response = result.unwrap();
        // All returned peers should meet minimum reputation
        for peer in &response.peers {
            assert!(peer.reputation_score >= 50);
        }
    }
}
