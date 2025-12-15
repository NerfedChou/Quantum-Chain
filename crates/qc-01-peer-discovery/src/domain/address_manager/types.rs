//! Address manager type definitions.

use super::security::SubnetKey;
use crate::domain::{PeerInfo, Timestamp};

/// An address entry in the address manager
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressEntry {
    /// Full peer information
    pub peer_info: PeerInfo,
    /// When we first learned about this address
    pub first_seen: Timestamp,
    /// When we last attempted connection
    pub last_attempt: Option<Timestamp>,
    /// When we last successfully connected
    pub last_success: Option<Timestamp>,
    /// Number of connection attempts
    pub attempts: u32,
    /// Source that told us about this address (/16 subnet key)
    pub source_subnet: SubnetKey,
}

impl AddressEntry {
    /// Create a new address entry
    pub fn new(peer_info: PeerInfo, now: Timestamp, source_subnet: SubnetKey) -> Self {
        Self {
            first_seen: now,
            last_attempt: None,
            last_success: None,
            attempts: 0,
            source_subnet,
            peer_info,
        }
    }
}

/// Statistics about the address manager.
#[derive(Debug, Clone, Default)]
pub struct AddressManagerStats {
    /// Number of addresses in the New table.
    pub new_count: usize,
    /// Number of addresses in the Tried table.
    pub tried_count: usize,
    /// Number of buckets in the New table.
    pub new_bucket_count: usize,
    /// Number of buckets in the Tried table.
    pub tried_bucket_count: usize,
}
