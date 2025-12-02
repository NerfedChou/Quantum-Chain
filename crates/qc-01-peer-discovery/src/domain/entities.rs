//! Core Domain Entities for Peer Discovery
//!
//! Reference: SPEC-01-PEER-DISCOVERY.md Section 2.1

use std::hash::Hash;

/// 256-bit node identifier derived from public key hash
///
/// # Security
/// NodeId is the unique identity of a peer in the Kademlia DHT.
/// It should be derived from a cryptographic hash of the peer's public key
/// to prevent Sybil attacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub [u8; 32]);

impl NodeId {
    /// Create a new NodeId from raw bytes
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get the raw bytes of the NodeId
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Create a NodeId with all zeros (for testing)
    pub fn zero() -> Self {
        Self([0u8; 32])
    }
}

impl AsRef<[u8]> for NodeId {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Complete peer information
///
/// Reference: SPEC-01 Section 2.1
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerInfo {
    /// Unique node identifier
    pub node_id: NodeId,
    /// Network address (IP + port)
    pub socket_addr: SocketAddr,
    /// Last time this peer was seen (Unix timestamp)
    pub last_seen: Timestamp,
    /// Reputation score (0-100, starts at 50)
    pub reputation_score: u8,
}

impl PeerInfo {
    /// Create a new PeerInfo with default reputation
    pub fn new(node_id: NodeId, socket_addr: SocketAddr, last_seen: Timestamp) -> Self {
        Self {
            node_id,
            socket_addr,
            last_seen,
            reputation_score: 50, // Default starting reputation
        }
    }
}

/// Socket address (IP + Port) - abstraction over std::net::SocketAddr
///
/// Reference: SPEC-01 Section 2.1
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SocketAddr {
    pub ip: IpAddr,
    pub port: u16,
}

impl SocketAddr {
    pub fn new(ip: IpAddr, port: u16) -> Self {
        Self { ip, port }
    }
}

/// IP address enum supporting both IPv4 and IPv6
///
/// Reference: SPEC-01 Section 2.1
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IpAddr {
    V4([u8; 4]),
    V6([u8; 16]),
}

impl IpAddr {
    /// Create an IPv4 address
    pub fn v4(a: u8, b: u8, c: u8, d: u8) -> Self {
        IpAddr::V4([a, b, c, d])
    }

    /// Create an IPv6 address from bytes
    pub fn v6(bytes: [u8; 16]) -> Self {
        IpAddr::V6(bytes)
    }

    /// Check if this is an IPv4 address
    pub fn is_ipv4(&self) -> bool {
        matches!(self, IpAddr::V4(_))
    }

    /// Check if this is an IPv6 address
    pub fn is_ipv6(&self) -> bool {
        matches!(self, IpAddr::V6(_))
    }
}

/// Unix timestamp in seconds
///
/// Reference: SPEC-01 Section 2.1
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(pub u64);

impl Timestamp {
    pub fn new(secs: u64) -> Self {
        Self(secs)
    }

    pub fn as_secs(&self) -> u64 {
        self.0
    }

    /// Add seconds to timestamp
    pub fn add_secs(&self, secs: u64) -> Self {
        Self(self.0.saturating_add(secs))
    }

    /// Subtract seconds from timestamp (saturating)
    pub fn sub_secs(&self, secs: u64) -> Self {
        Self(self.0.saturating_sub(secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_equality() {
        let id1 = NodeId::new([1u8; 32]);
        let id2 = NodeId::new([1u8; 32]);
        let id3 = NodeId::new([2u8; 32]);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_peer_info_default_reputation() {
        let node_id = NodeId::new([1u8; 32]);
        let addr = SocketAddr::new(IpAddr::v4(127, 0, 0, 1), 8080);
        let peer = PeerInfo::new(node_id, addr, Timestamp::new(1000));

        assert_eq!(peer.reputation_score, 50);
    }

    #[test]
    fn test_timestamp_arithmetic() {
        let ts = Timestamp::new(100);
        assert_eq!(ts.add_secs(50).as_secs(), 150);
        assert_eq!(ts.sub_secs(50).as_secs(), 50);
        assert_eq!(ts.sub_secs(200).as_secs(), 0); // Saturating
    }
}
