//! Domain Services - Pure functions for Kademlia operations
//!
//! Reference: SPEC-01-PEER-DISCOVERY.md Section 2
//!
//! All functions in this module are pure (no I/O, no state mutation)
//! and deterministic (same inputs → same outputs).

use crate::domain::{Distance, IpAddr, NodeId, PeerInfo, SubnetMask};

/// Calculate the XOR distance between two NodeIds
///
/// # Properties (SPEC-01 Section 5.1 - Test Group 1)
/// - Symmetric: `xor_distance(a, b) == xor_distance(b, a)`
/// - Self is zero: `xor_distance(a, a)` returns bucket 255 (closest)
/// - Identifies correct bucket based on first differing bit
///
/// # Returns
/// Distance value representing the bucket index (0-255)
/// Lower values mean the nodes are "farther" in XOR space (more bits differ early)
/// Higher values mean the nodes are "closer" (more leading bits are the same)
pub fn xor_distance(a: &NodeId, b: &NodeId) -> Distance {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();

    // XOR each byte and find the first non-zero byte
    for i in 0..32 {
        let xor = a_bytes[i] ^ b_bytes[i];
        if xor != 0 {
            // Find the position of the first set bit in this byte
            let leading_zeros = xor.leading_zeros() as u8;
            // Calculate bucket index: (byte_index * 8) + leading_zeros_in_byte
            // This gives us 0-255 range
            let bucket = (i as u8) * 8 + leading_zeros;
            return Distance::new(bucket);
        }
    }

    // All bytes are identical (same node) - return max distance (closest bucket)
    Distance::new(255)
}

/// Calculate the bucket index for a remote node relative to local node.
///
/// Bucket index equals the XOR distance, determining which k-bucket
/// stores peers at that distance range per Kademlia specification.
///
/// Reference: SPEC-01 Section 2.4 (INVARIANT-6: Distance Ordering)
pub fn calculate_bucket_index(local: &NodeId, remote: &NodeId) -> usize {
    xor_distance(local, remote).bucket_index() as usize
}

/// Optimized fused function: Calculate bucket index directly without intermediate Distance.
///
/// This is an optimized version of `calculate_bucket_index` that avoids creating
/// an intermediate `Distance` struct. Use this in hot paths where bucket lookup
/// performance is critical.
///
/// # Performance
/// - Avoids Distance struct allocation
/// - Single pass through node ID bytes
/// - O(1) for identical nodes, O(32) worst case
#[inline]
pub fn bucket_for_peer(local: &NodeId, remote: &NodeId) -> usize {
    let local_bytes = local.as_bytes();
    let remote_bytes = remote.as_bytes();

    // XOR each byte and find the first non-zero byte
    for i in 0..32 {
        let xor = local_bytes[i] ^ remote_bytes[i];
        if xor != 0 {
            // Calculate bucket index directly: (byte_index * 8) + leading_zeros_in_byte
            return (i * 8 + xor.leading_zeros() as usize) as usize;
        }
    }

    // Identical nodes - return max bucket (255)
    255
}

/// Check if two IP addresses share the same subnet prefix.
///
/// Used to enforce INVARIANT-3 (IP Diversity) for Sybil attack resistance.
/// Compares addresses using the specified prefix length (e.g., /24 for IPv4).
///
/// Reference: SPEC-01 Section 6.1 (Sybil Attack Resistance)
pub fn is_same_subnet(a: &IpAddr, b: &IpAddr, mask: &SubnetMask) -> bool {
    match (a, b) {
        (IpAddr::V4(a_bytes), IpAddr::V4(b_bytes)) => {
            let prefix_bytes = (mask.prefix_length / 8) as usize;
            let remaining_bits = mask.prefix_length % 8;

            // Compare full bytes within prefix
            for i in 0..prefix_bytes.min(4) {
                if a_bytes[i] != b_bytes[i] {
                    return false;
                }
            }

            // Compare partial byte if prefix doesn't align to byte boundary
            if remaining_bits > 0 && prefix_bytes < 4 {
                let mask_byte = 0xFF << (8 - remaining_bits);
                if (a_bytes[prefix_bytes] & mask_byte) != (b_bytes[prefix_bytes] & mask_byte) {
                    return false;
                }
            }

            true
        }
        (IpAddr::V6(a_bytes), IpAddr::V6(b_bytes)) => {
            let prefix_bytes = (mask.prefix_length / 8) as usize;
            let remaining_bits = mask.prefix_length % 8;

            for i in 0..prefix_bytes.min(16) {
                if a_bytes[i] != b_bytes[i] {
                    return false;
                }
            }

            if remaining_bits > 0 && prefix_bytes < 16 {
                let mask_byte = 0xFF << (8 - remaining_bits);
                if (a_bytes[prefix_bytes] & mask_byte) != (b_bytes[prefix_bytes] & mask_byte) {
                    return false;
                }
            }

            true
        }
        // IPv4 and IPv6 addresses are in disjoint address spaces
        _ => false,
    }
}

/// Sort peers by XOR distance from a target node (closest first).
///
/// Higher bucket index indicates closer distance in Kademlia XOR metric.
/// Used for iterative lookups per SPEC-01 Section 3.1 (`find_closest_peers`).
pub fn sort_peers_by_distance(peers: &[PeerInfo], target: &NodeId) -> Vec<PeerInfo> {
    let mut sorted = peers.to_vec();
    sorted.sort_by(|a, b| {
        let dist_a = xor_distance(&a.node_id, target);
        let dist_b = xor_distance(&b.node_id, target);
        // Higher bucket index = closer in XOR space = sort first
        dist_b.cmp(&dist_a)
    });
    sorted
}

/// Find the k closest peers to a target from a list
///
/// # Arguments
/// * `peers` - List of all available peers
/// * `target` - Target NodeId to measure distance from
/// * `k` - Maximum number of peers to return
///
/// # Returns
/// Up to k peers sorted by distance (closest first)
pub fn find_k_closest(peers: &[PeerInfo], target: &NodeId, k: usize) -> Vec<PeerInfo> {
    let sorted = sort_peers_by_distance(peers, target);
    sorted.into_iter().take(k).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{SocketAddr, Timestamp};

    fn make_node_id(first_byte: u8) -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = first_byte;
        NodeId::new(bytes)
    }

    fn make_peer(first_byte: u8) -> PeerInfo {
        PeerInfo::new(
            make_node_id(first_byte),
            SocketAddr::new(IpAddr::v4(127, 0, 0, 1), 8080),
            Timestamp::new(1000),
        )
    }

    // =========================================================================
    // Test Group 1: XOR Distance Calculation (SPEC-01 Section 5.1)
    // =========================================================================

    #[test]
    fn test_xor_distance_calculation_is_symmetric() {
        let a = make_node_id(0b1010_0000);
        let b = make_node_id(0b0101_0000);

        let dist_ab = xor_distance(&a, &b);
        let dist_ba = xor_distance(&b, &a);

        assert_eq!(dist_ab, dist_ba, "XOR distance must be symmetric");
    }

    #[test]
    fn test_xor_distance_to_self_is_max() {
        let a = make_node_id(0b1010_1010);

        let dist = xor_distance(&a, &a);

        assert_eq!(
            dist,
            Distance::new(255),
            "Distance to self should be max (255 = closest bucket)"
        );
    }

    #[test]
    fn test_xor_distance_identifies_correct_bucket() {
        let local = NodeId::new([0u8; 32]);

        // Differ in first bit of first byte → bucket 0
        let mut remote1 = [0u8; 32];
        remote1[0] = 0b1000_0000;
        assert_eq!(
            xor_distance(&local, &NodeId::new(remote1)),
            Distance::new(0),
            "First bit different → bucket 0"
        );

        // Differ in second bit of first byte → bucket 1
        let mut remote2 = [0u8; 32];
        remote2[0] = 0b0100_0000;
        assert_eq!(
            xor_distance(&local, &NodeId::new(remote2)),
            Distance::new(1),
            "Second bit different → bucket 1"
        );

        // Differ in first bit of second byte → bucket 8
        let mut remote3 = [0u8; 32];
        remote3[1] = 0b1000_0000;
        assert_eq!(
            xor_distance(&local, &NodeId::new(remote3)),
            Distance::new(8),
            "First bit of second byte → bucket 8"
        );
    }

    #[test]
    fn test_xor_distance_ordering_for_closest_peers() {
        let target = NodeId::new([0u8; 32]);

        // Create peers at different distances
        let mut far = [0u8; 32];
        far[0] = 0b1000_0000; // Bucket 0 (farthest)
        let peer_far = make_peer(0b1000_0000);

        let mut mid = [0u8; 32];
        mid[1] = 0b1000_0000; // Bucket 8 (middle)
        let mut peer_mid = make_peer(0);
        peer_mid.node_id = NodeId::new(mid);

        let mut close = [0u8; 32];
        close[31] = 0b0000_0001; // Bucket 255 (closest)
        let mut peer_close = make_peer(0);
        peer_close.node_id = NodeId::new(close);

        let peers = vec![peer_far.clone(), peer_mid.clone(), peer_close.clone()];
        let sorted = sort_peers_by_distance(&peers, &target);

        // XOR metric: higher bucket index = closer = sorted first
        assert_eq!(
            sorted[0].node_id, peer_close.node_id,
            "Closest peer (highest bucket index) first"
        );
        assert_eq!(
            sorted[1].node_id, peer_mid.node_id,
            "Middle distance peer second"
        );
        assert_eq!(
            sorted[2].node_id, peer_far.node_id,
            "Farthest peer (lowest bucket index) last"
        );
    }

    // =========================================================================
    // Test Group 3: IP Diversity
    // Reference: SPEC-01 Section 5.1 (Sybil Attack Resistance Tests)
    // =========================================================================

    #[test]
    fn test_same_subnet_ipv4() {
        let a = IpAddr::v4(192, 168, 1, 100);
        let b = IpAddr::v4(192, 168, 1, 200);
        let c = IpAddr::v4(192, 168, 2, 100);
        let mask = SubnetMask::new(24);

        assert!(
            is_same_subnet(&a, &b, &mask),
            "192.168.1.x should be same /24"
        );
        assert!(
            !is_same_subnet(&a, &c, &mask),
            "192.168.1.x and 192.168.2.x should be different /24"
        );
    }

    #[test]
    fn test_different_subnets_ipv4() {
        let a = IpAddr::v4(10, 0, 0, 1);
        let b = IpAddr::v4(10, 0, 1, 1);
        let mask = SubnetMask::new(24);

        assert!(
            !is_same_subnet(&a, &b, &mask),
            "10.0.0.x and 10.0.1.x should be different /24"
        );
    }

    #[test]
    fn test_subnet_check_works_for_ipv6() {
        let mut a_bytes = [0u8; 16];
        let mut b_bytes = [0u8; 16];
        let mut c_bytes = [0u8; 16];

        // Same /48 prefix
        a_bytes[0..6].copy_from_slice(&[0x20, 0x01, 0x0d, 0xb8, 0x85, 0xa3]);
        a_bytes[6] = 0x00;
        b_bytes[0..6].copy_from_slice(&[0x20, 0x01, 0x0d, 0xb8, 0x85, 0xa3]);
        b_bytes[6] = 0xFF;

        // Different /48 prefix
        c_bytes[0..6].copy_from_slice(&[0x20, 0x01, 0x0d, 0xb8, 0x85, 0xa4]);

        let a = IpAddr::v6(a_bytes);
        let b = IpAddr::v6(b_bytes);
        let c = IpAddr::v6(c_bytes);
        let mask = SubnetMask::new(48);

        assert!(is_same_subnet(&a, &b, &mask), "Same /48 should match");
        assert!(
            !is_same_subnet(&a, &c, &mask),
            "Different /48 should not match"
        );
    }

    #[test]
    fn test_ipv4_ipv6_never_same_subnet() {
        let v4 = IpAddr::v4(192, 168, 1, 1);
        let v6 = IpAddr::v6([0u8; 16]);
        let mask = SubnetMask::new(0); // Even with /0 mask

        assert!(
            !is_same_subnet(&v4, &v6, &mask),
            "IPv4 and IPv6 are never in same subnet"
        );
    }

    // =========================================================================
    // Test: find_k_closest
    // =========================================================================

    #[test]
    fn test_find_k_closest_returns_correct_count() {
        let target = NodeId::new([0u8; 32]);

        // Create 10 peers
        let peers: Vec<PeerInfo> = (1..=10).map(|i| make_peer(i as u8)).collect();

        let closest_3 = find_k_closest(&peers, &target, 3);
        assert_eq!(closest_3.len(), 3, "Should return exactly k peers");

        let closest_20 = find_k_closest(&peers, &target, 20);
        assert_eq!(closest_20.len(), 10, "Should return all peers if k > len");
    }
}
