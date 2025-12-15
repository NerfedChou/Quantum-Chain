//! Connection slots types.

use crate::domain::NodeId;

/// Direction of a connection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionDirection {
    /// We initiated this connection (from Tried table)
    Outbound,
    /// Peer initiated this connection
    Inbound,
}

/// Result of trying to accept an inbound connection
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptResult {
    /// Connection accepted (slot was available)
    Accepted,
    /// Connection rejected (no slots, eviction failed)
    Rejected,
    /// Connection accepted after evicting another peer
    Evicted(NodeId),
}

/// Connection statistics.
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    /// Current number of outbound connections.
    pub outbound_count: usize,
    /// Current number of inbound connections.
    pub inbound_count: usize,
    /// Maximum outbound connections allowed.
    pub max_outbound: usize,
    /// Maximum inbound connections allowed.
    pub max_inbound: usize,
}
