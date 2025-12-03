//! Value Objects for Peer Discovery
//!
//! Reference: SPEC-01-PEER-DISCOVERY.md Section 2.3

/// Result of XOR distance calculation between two nodes
///
/// The distance is measured as the index of the first differing bit
/// when comparing two NodeIds via XOR. Range is 0-255 (for 256-bit NodeIds).
///
/// Reference: SPEC-01 Section 2.3
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Distance(pub u8);

impl Distance {
    /// Create a new Distance value
    pub fn new(bucket_index: u8) -> Self {
        Self(bucket_index)
    }

    /// Get the bucket index (0-255)
    pub fn bucket_index(&self) -> u8 {
        self.0
    }

    /// Maximum possible distance (used as sentinel value)
    pub fn max() -> Self {
        Self(255)
    }
}

/// Configuration constants for Kademlia DHT
///
/// # Security Notes (SPEC-01 Section 2.3)
///
/// - `max_pending_peers`: Limits the size of the pending_verification staging area.
///   This prevents attackers from exhausting node memory by flooding connection
///   requests faster than signatures can be verified. (V2.3 Memory Bomb Defense)
///
/// - `eviction_challenge_timeout_secs`: Controls how long we wait for an oldest
///   peer to respond before declaring it dead and allowing eviction.
///   (V2.4 Eclipse Attack Defense)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KademliaConfig {
    /// Bucket size (default: 20)
    pub k: usize,
    /// Parallelism factor for lookups (default: 3)
    pub alpha: usize,
    /// Maximum peers from the same /24 subnet (default: 2)
    pub max_peers_per_subnet: usize,
    /// Maximum peers allowed in pending_verification staging area.
    /// Incoming requests beyond this limit are immediately dropped (Tail Drop).
    /// Default: 1024 (bounded memory: ~128KB worst case)
    pub max_pending_peers: usize,
    /// Timeout for eviction challenge PING (V2.4 Eclipse Defense).
    /// If oldest peer doesn't respond within this time, it's considered dead.
    /// Default: 5 seconds
    pub eviction_challenge_timeout_secs: u64,
    /// Default verification timeout for new peers (default: 10 seconds)
    pub verification_timeout_secs: u64,
}

impl Default for KademliaConfig {
    fn default() -> Self {
        Self {
            k: 20,
            alpha: 3,
            max_peers_per_subnet: 2,
            max_pending_peers: 1024,
            eviction_challenge_timeout_secs: 5,
            verification_timeout_secs: 10,
        }
    }
}

impl KademliaConfig {
    /// Create a config suitable for testing (smaller values)
    pub fn for_testing() -> Self {
        Self {
            k: 3, // Smaller buckets for easier testing
            alpha: 2,
            max_peers_per_subnet: 2,
            max_pending_peers: 10,              // Small staging for testing
            eviction_challenge_timeout_secs: 1, // Fast timeout for tests
            verification_timeout_secs: 2,
        }
    }
}

/// Subnet mask for IP diversity checks
///
/// Used to enforce INVARIANT-3: max_peers_per_subnet limit
///
/// Reference: SPEC-01 Section 2.3
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubnetMask {
    /// Prefix length in bits (e.g., 24 for /24)
    pub prefix_length: u8,
}

impl SubnetMask {
    pub fn new(prefix_length: u8) -> Self {
        Self { prefix_length }
    }

    /// Default /24 subnet mask for IPv4
    pub fn ipv4_default() -> Self {
        Self { prefix_length: 24 }
    }

    /// Default /48 subnet mask for IPv6
    pub fn ipv6_default() -> Self {
        Self { prefix_length: 48 }
    }
}

impl Default for SubnetMask {
    fn default() -> Self {
        Self::ipv4_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_ordering() {
        let d1 = Distance::new(0);
        let d2 = Distance::new(128);
        let d3 = Distance::new(255);

        assert!(d1 < d2);
        assert!(d2 < d3);
        assert!(d1 < d3);
    }

    #[test]
    fn test_kademlia_config_defaults() {
        let config = KademliaConfig::default();
        assert_eq!(config.k, 20);
        assert_eq!(config.alpha, 3);
        assert_eq!(config.max_peers_per_subnet, 2);
        assert_eq!(config.max_pending_peers, 1024);
        assert_eq!(config.eviction_challenge_timeout_secs, 5);
    }

    #[test]
    fn test_subnet_mask_defaults() {
        let ipv4 = SubnetMask::ipv4_default();
        let ipv6 = SubnetMask::ipv6_default();

        assert_eq!(ipv4.prefix_length, 24);
        assert_eq!(ipv6.prefix_length, 48);
    }
}
