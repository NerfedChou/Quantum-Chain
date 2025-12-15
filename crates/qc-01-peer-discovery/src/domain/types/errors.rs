//! Domain Errors for Peer Discovery
//!
//! Reference: SPEC-01-PEER-DISCOVERY.md Section 3.1

use std::fmt;

/// Errors that can occur during peer discovery operations
///
/// Reference: SPEC-01 Section 3.1
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerDiscoveryError {
    /// Peer not found in routing table
    PeerNotFound,
    /// Peer is currently banned
    PeerBanned,
    /// K-bucket is at capacity (K=20)
    BucketFull,
    /// Invalid node identifier
    InvalidNodeId,
    /// Too many peers from the same subnet
    SubnetLimitReached,
    /// Attempted to add local node to routing table
    SelfConnection,
    /// V2.3: Staging area is at capacity (Memory Bomb Defense)
    /// Request immediately dropped; no memory allocated.
    /// See INVARIANT-9 for Tail Drop Strategy.
    StagingAreaFull,
    /// V2.4: An eviction challenge is already in progress for this bucket
    ChallengeInProgress,
    /// Routing table is at maximum capacity
    RoutingTableFull,
}

impl fmt::Display for PeerDiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PeerNotFound => write!(f, "Peer not found in routing table"),
            Self::PeerBanned => write!(f, "Peer is currently banned"),
            Self::BucketFull => write!(f, "K-bucket is at capacity"),
            Self::InvalidNodeId => write!(f, "Invalid node identifier"),
            Self::SubnetLimitReached => write!(f, "Too many peers from the same subnet"),
            Self::SelfConnection => write!(f, "Cannot add local node to routing table"),
            Self::StagingAreaFull => write!(f, "Staging area at capacity (tail drop)"),
            Self::ChallengeInProgress => write!(f, "Eviction challenge already in progress"),
            Self::RoutingTableFull => write!(f, "Routing table at maximum capacity"),
        }
    }
}

impl std::error::Error for PeerDiscoveryError {}

/// Reasons for banning a peer from the routing table
///
/// # Security Note (SPEC-01 Section 2.2)
/// `InvalidSignature` is intentionally EXCLUDED from this enum.
/// In UDP contexts, IP addresses can be trivially spoofed. If we banned IPs
/// for bad signatures, an attacker could spoof a legitimate peer's IP,
/// send a bad signature, and trick us into banning the victim.
///
/// Instead, failed signature verification results in a SILENT DROP:
/// - Remove from `pending_verification`
/// - Do NOT add to `banned_peers`
/// - Log at DEBUG level only (no alerting)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BanReason {
    /// Peer sent malformed protocol messages
    MalformedMessage,
    /// Peer exceeded request rate limits
    ExcessiveRequests,
    /// Manually banned by operator
    ManualBan,
    // NOTE: InvalidSignature is INTENTIONALLY NOT included
    // See security note above
}

impl fmt::Display for BanReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedMessage => write!(f, "Malformed message"),
            Self::ExcessiveRequests => write!(f, "Excessive requests"),
            Self::ManualBan => write!(f, "Manual ban"),
        }
    }
}

/// Reasons why a peer was disconnected
///
/// Reference: SPEC-01 Section 4.1
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectReason {
    /// Peer timed out (no response to PING)
    Timeout,
    /// Explicitly removed by application
    ExplicitRemoval,
    /// Replaced by another peer in bucket (only when dead)
    BucketReplacement,
    /// Network error during communication
    NetworkError,
}

impl fmt::Display for DisconnectReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout => write!(f, "Timeout"),
            Self::ExplicitRemoval => write!(f, "Explicit removal"),
            Self::BucketReplacement => write!(f, "Bucket replacement"),
            Self::NetworkError => write!(f, "Network error"),
        }
    }
}

/// Types of routing table health warnings
///
/// Reference: SPEC-01 Section 4.1
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningType {
    /// Too few peers in routing table
    TooFewPeers,
    /// No recent activity detected
    NoRecentActivity,
    /// High rate of peer churn
    HighChurnRate,
}

impl fmt::Display for WarningType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewPeers => write!(f, "Too few peers"),
            Self::NoRecentActivity => write!(f, "No recent activity"),
            Self::HighChurnRate => write!(f, "High churn rate"),
        }
    }
}
