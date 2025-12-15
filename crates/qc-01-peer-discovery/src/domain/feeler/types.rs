//! Feeler data types.

use crate::domain::{NodeId, SocketAddr, Timestamp};

/// State of a pending feeler probe
#[derive(Debug, Clone)]
pub struct FeelerProbe {
    /// Target address being probed
    pub target: SocketAddr,
    /// Target node ID (if known)
    pub node_id: Option<NodeId>,
    /// When the probe started
    pub started_at: Timestamp,
    /// Deadline for connection
    pub deadline: Timestamp,
}

impl FeelerProbe {
    /// Create a new feeler probe
    pub fn new(
        target: SocketAddr,
        node_id: Option<NodeId>,
        now: Timestamp,
        timeout_secs: u64,
    ) -> Self {
        Self {
            target,
            node_id,
            started_at: now,
            deadline: Timestamp::new(now.as_secs() + timeout_secs),
        }
    }

    /// Check if probe has timed out
    pub fn is_timed_out(&self, now: Timestamp) -> bool {
        now.as_secs() >= self.deadline.as_secs()
    }
}

/// Result of a feeler probe
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeelerResult {
    /// Probe successful - chain compatible, promote to Tried
    Success,
    /// Probe failed - connection error or timeout
    ConnectionFailed,
    /// Probe failed - wrong chain (different genesis)
    WrongChain,
    /// Probe failed - peer too far behind
    TooFarBehind,
    /// No address available to probe
    NoAddressAvailable,
}
