//! Core Domain Entities for Peer Discovery
//!
//! Reference: SPEC-01-PEER-DISCOVERY.md Section 2.1

use std::hash::Hash;

/// 256-bit node identifier derived from public key hash.
///
/// NodeId uniquely identifies a peer in the Kademlia DHT. Generated from
/// SHA-256(public_key) to cryptographically bind identity to key ownership,
/// providing Sybil attack resistance per SPEC-01 Section 6.1.
///
/// # Security (V2.5 - Timing Attack Prevention)
///
/// This type implements constant-time comparison to prevent timing attacks.
/// Standard `PartialEq` for byte arrays short-circuits on first difference,
/// allowing attackers to recover NodeIds via timing measurements.
///
/// Reference: SPEC-01 Section 2.1 (`NodeId`)
// SAFETY: derived_hash_with_manual_eq is intentionally allowed here.
// The manual PartialEq provides constant-time comparison for security,
// but Hash using the underlying bytes is semantically correct since
// equal NodeIds (same bytes) will have the same hash.
#[allow(clippy::derived_hash_with_manual_eq)]
#[derive(Debug, Clone, Copy, Hash)]
pub struct NodeId(pub [u8; 32]);

impl PartialEq for NodeId {
    /// Constant-time comparison to prevent timing attacks.
    ///
    /// This implementation XORs all bytes and accumulates differences,
    /// ensuring comparison time is independent of where bytes differ.
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        let mut result = 0u8;
        for (a, b) in self.0.iter().zip(other.0.iter()) {
            result |= a ^ b;
        }
        result == 0
    }
}

impl Eq for NodeId {}

impl NodeId {
    /// Create a NodeId from raw 32-byte array.
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get the underlying bytes for XOR distance calculation.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Create a zero-initialized NodeId for testing bucket index 255.
    pub fn zero() -> Self {
        Self([0u8; 32])
    }
}

impl AsRef<[u8]> for NodeId {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Complete peer information stored in routing table.
///
/// Reference: SPEC-01 Section 2.1 (`PeerInfo`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerInfo {
    /// Unique node identifier (SHA-256 of public key).
    pub node_id: NodeId,
    /// Network address for P2P communication.
    pub socket_addr: SocketAddr,
    /// Unix timestamp of last successful communication.
    pub last_seen: Timestamp,
    /// Reputation score (0-100) for peer selection priority.
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

/// Socket address (IP + Port) - abstraction over std::net::SocketAddr.
///
/// Reference: SPEC-01 Section 2.1
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SocketAddr {
    /// IP address (v4 or v6).
    pub ip: IpAddr,
    /// Port number.
    pub port: u16,
}

impl SocketAddr {
    /// Create a new socket address from IP and port.
    pub fn new(ip: IpAddr, port: u16) -> Self {
        Self { ip, port }
    }
}

/// IP address enum supporting both IPv4 and IPv6.
///
/// Reference: SPEC-01 Section 2.1
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IpAddr {
    /// IPv4 address (4 bytes).
    V4([u8; 4]),
    /// IPv6 address (16 bytes).
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
/// # Security (V2.5 - Timestamp Bounds)
///
/// Timestamps are clamped to a reasonable maximum to prevent overflow
/// attacks in sorting and comparison operations.
///
/// Reference: SPEC-01 Section 2.1
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(u64);

impl Timestamp {
    /// Maximum reasonable timestamp (year 9999).
    ///
    /// Prevents attackers from using u64::MAX to corrupt eviction logic.
    pub const MAX_REASONABLE: u64 = 253_402_300_799;

    /// Create a new timestamp, clamping to MAX_REASONABLE.
    ///
    /// # Security
    ///
    /// Values above MAX_REASONABLE are silently clamped to prevent
    /// overflow attacks in comparison/sorting logic.
    pub fn new(secs: u64) -> Self {
        Self(secs.min(Self::MAX_REASONABLE))
    }

    /// Create a timestamp with explicit validation.
    ///
    /// # Returns
    ///
    /// `None` if `secs > MAX_REASONABLE`, `Some(Timestamp)` otherwise.
    /// Optimized: uses guard pattern for cleaner conditional.
    #[inline]
    pub fn try_new(secs: u64) -> Option<Self> {
        (secs <= Self::MAX_REASONABLE).then_some(Self(secs))
    }

    /// Get the underlying seconds value.
    pub fn as_secs(&self) -> u64 {
        self.0
    }

    /// Add seconds to timestamp (saturating at MAX_REASONABLE).
    pub fn add_secs(&self, secs: u64) -> Self {
        Self(self.0.saturating_add(secs).min(Self::MAX_REASONABLE))
    }

    /// Subtract seconds from timestamp (saturating at 0).
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
