//! Kademlia distance calculations.

use crate::domain::{Distance, NodeId};

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
            return i * 8 + xor.leading_zeros() as usize;
        }
    }

    // Identical nodes - return max bucket (255)
    255
}
