//! Routing table constants and configuration.

/// Number of k-buckets (one per bit of NodeId)
pub const NUM_BUCKETS: usize = 256;

/// Maximum total peers across all buckets
pub const MAX_TOTAL_PEERS: usize = 5120; // 256 * 20
