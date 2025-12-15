//! Peer sorting and selection.

use super::distance::xor_distance;
use crate::domain::{NodeId, PeerInfo};

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
