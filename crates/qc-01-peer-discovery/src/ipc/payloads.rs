//! # IPC Payloads for Peer Discovery
//!
//! Defines all IPC message payloads as specified in SPEC-01 Section 4.
//!
//! ## Design Rules (Architecture.md v2.2)
//!
//! - All payloads are wrapped in `AuthenticatedMessage<T>`.
//! - Payloads MUST NOT contain `requester_id` fields (envelope authority).
//! - Request/response pairs use the envelope's `correlation_id`.

use crate::domain::{BanReason, DisconnectReason, NodeId, PeerInfo, SubnetMask, WarningType};

// =============================================================================
// EVENT PAYLOADS (Published to Event Bus)
// =============================================================================

/// Events emitted by the Peer Discovery subsystem.
///
/// USAGE: These are payloads wrapped in `AuthenticatedMessage<T>`.
/// Example: `AuthenticatedMessage<PeerConnectedPayload>`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerDiscoveryEventPayload {
    /// A new peer was successfully added to routing table.
    PeerConnected(PeerConnectedPayload),
    /// A peer was removed from routing table.
    PeerDisconnected(PeerDisconnectedPayload),
    /// A peer was banned.
    PeerBanned(PeerBannedPayload),
    /// Bootstrap process completed.
    BootstrapCompleted(BootstrapCompletedPayload),
    /// Routing table health warning.
    RoutingTableWarning(RoutingTableWarningPayload),
    /// Response to a peer list request (correlated via correlation_id).
    PeerListResponse(PeerListResponsePayload),
}

/// Payload for peer connected event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerConnectedPayload {
    /// Information about the connected peer.
    pub peer_info: PeerInfo,
    /// The k-bucket index where the peer was added.
    pub bucket_index: u8,
}

/// Payload for peer disconnected event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerDisconnectedPayload {
    /// The disconnected peer's node ID.
    pub node_id: NodeId,
    /// Reason for disconnection.
    pub reason: DisconnectReason,
}

/// Payload for peer banned event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerBannedPayload {
    /// The banned peer's node ID.
    pub node_id: NodeId,
    /// Reason for banning.
    pub reason: BanReason,
    /// Duration of ban in seconds.
    pub duration_seconds: u64,
}

/// Payload for bootstrap completed event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapCompletedPayload {
    /// Number of peers discovered during bootstrap.
    pub peer_count: usize,
    /// Time taken for bootstrap in milliseconds.
    pub duration_ms: u64,
}

/// Payload for routing table warning event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingTableWarningPayload {
    /// Type of warning.
    pub warning_type: WarningType,
    /// Additional details about the warning.
    pub details: String,
}

/// Response payload for peer list requests.
/// The `correlation_id` in the envelope links this to the original request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerListResponsePayload {
    /// The requested peers.
    pub peers: Vec<PeerInfo>,
    /// Total peers available in routing table.
    pub total_available: usize,
}

// =============================================================================
// REQUEST PAYLOADS (Subscribed from Event Bus)
// =============================================================================

/// Request payloads this subsystem handles.
///
/// CRITICAL: These payloads arrive wrapped in `AuthenticatedMessage<T>`.
/// The envelope's `correlation_id` and `reply_to` fields MUST be used for responses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerDiscoveryRequestPayload {
    /// Request for a list of known peers.
    /// Allowed senders: Subsystems 5, 7, 13 ONLY.
    PeerListRequest(PeerListRequestPayload),
    /// Request for full node connections (for light clients).
    /// Allowed senders: Subsystem 13 ONLY.
    FullNodeListRequest(FullNodeListRequestPayload),
}

/// Request payload for peer list.
///
/// SECURITY (Envelope-Only Identity - Architecture.md v2.2 Amendment 4.2):
/// This payload contains NO identity fields (e.g., `requester_id`).
/// The sender's identity is derived SOLELY from the `AuthenticatedMessage`
/// envelope's `sender_id` field, which is cryptographically signed.
///
/// PRIVACY NOTE: The `filter` field can have privacy implications.
/// Complex or unique filter combinations may enable request fingerprinting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerListRequestPayload {
    /// Maximum number of peers to return.
    pub max_peers: usize,
    /// Optional filter for peer selection.
    pub filter: Option<PeerFilter>,
}

/// Request payload for full node list (light clients).
///
/// NOTE: Identity derived from envelope.sender_id per Architecture.md v2.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullNodeListRequestPayload {
    /// Maximum number of nodes to return.
    pub max_nodes: usize,
    /// Optional preferred geographic region.
    pub preferred_region: Option<String>,
}

/// Filter for peer selection.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PeerFilter {
    /// Minimum reputation score (0-100).
    pub min_reputation: u8,
    /// Subnets to exclude from results.
    pub exclude_subnets: Vec<SubnetMask>,
}

impl PeerListRequestPayload {
    /// Create a new peer list request with default filter.
    #[must_use]
    pub fn new(max_peers: usize) -> Self {
        Self {
            max_peers,
            filter: None,
        }
    }

    /// Create a new peer list request with custom filter.
    #[must_use]
    pub fn with_filter(max_peers: usize, filter: PeerFilter) -> Self {
        Self {
            max_peers,
            filter: Some(filter),
        }
    }
}

impl FullNodeListRequestPayload {
    /// Create a new full node list request.
    #[must_use]
    pub fn new(max_nodes: usize) -> Self {
        Self {
            max_nodes,
            preferred_region: None,
        }
    }

    /// Create a request with preferred region.
    #[must_use]
    pub fn with_region(max_nodes: usize, region: String) -> Self {
        Self {
            max_nodes,
            preferred_region: Some(region),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{IpAddr, SocketAddr, Timestamp};

    fn make_peer_info(id_byte: u8) -> PeerInfo {
        let mut id_bytes = [0u8; 32];
        id_bytes[0] = id_byte;
        PeerInfo::new(
            NodeId::new(id_bytes),
            SocketAddr::new(IpAddr::v4(192, 168, 1, id_byte), 8080),
            Timestamp::new(1000),
        )
    }

    #[test]
    fn test_peer_list_request_new() {
        let req = PeerListRequestPayload::new(20);
        assert_eq!(req.max_peers, 20);
        assert!(req.filter.is_none());
    }

    #[test]
    fn test_peer_list_request_with_filter() {
        let filter = PeerFilter {
            min_reputation: 50,
            exclude_subnets: vec![],
        };
        let req = PeerListRequestPayload::with_filter(10, filter);
        assert_eq!(req.max_peers, 10);
        assert!(req.filter.is_some());
        assert_eq!(req.filter.unwrap().min_reputation, 50);
    }

    #[test]
    fn test_peer_connected_payload() {
        let peer = make_peer_info(1);
        let payload = PeerConnectedPayload {
            peer_info: peer.clone(),
            bucket_index: 5,
        };
        assert_eq!(payload.peer_info.node_id, peer.node_id);
        assert_eq!(payload.bucket_index, 5);
    }

    #[test]
    fn test_peer_disconnected_payload() {
        let node_id = NodeId::new([1u8; 32]);
        let payload = PeerDisconnectedPayload {
            node_id,
            reason: DisconnectReason::Timeout,
        };
        assert_eq!(payload.node_id, node_id);
        assert_eq!(payload.reason, DisconnectReason::Timeout);
    }

    #[test]
    fn test_peer_list_response_payload() {
        let peers = vec![make_peer_info(1), make_peer_info(2)];
        let payload = PeerListResponsePayload {
            peers: peers.clone(),
            total_available: 100,
        };
        assert_eq!(payload.peers.len(), 2);
        assert_eq!(payload.total_available, 100);
    }

    #[test]
    fn test_full_node_list_request() {
        let req = FullNodeListRequestPayload::new(5);
        assert_eq!(req.max_nodes, 5);
        assert!(req.preferred_region.is_none());

        let req_with_region = FullNodeListRequestPayload::with_region(10, "us-east".to_string());
        assert_eq!(req_with_region.max_nodes, 10);
        assert_eq!(
            req_with_region.preferred_region,
            Some("us-east".to_string())
        );
    }
}
