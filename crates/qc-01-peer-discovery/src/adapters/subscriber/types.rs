use crate::ipc::security::SecurityError;

/// Response from Subsystem 10 for node identity verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeIdentityVerificationResult {
    /// The node ID that was verified.
    pub node_id: [u8; 32],
    /// Whether the identity is valid.
    pub identity_valid: bool,
    /// Timestamp of verification.
    pub verification_timestamp: u64,
}

/// Outcome of processing a verification result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationOutcome {
    /// Peer was promoted to routing table.
    PeerPromoted {
        /// The promoted peer's node ID.
        node_id: [u8; 32],
    },
    /// Peer was rejected (invalid signature).
    PeerRejected {
        /// The rejected peer's node ID.
        node_id: [u8; 32],
    },
    /// Bucket is full, need to challenge existing peer.
    ChallengeRequired {
        /// The new peer waiting to be added.
        new_peer: [u8; 32],
        /// The existing peer being challenged.
        challenged_peer: [u8; 32],
    },
    /// Node was not in pending verification (already processed or unknown).
    NotFound {
        /// The node ID that was not found.
        node_id: [u8; 32],
    },
}

/// Errors that can occur during event subscription processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionError {
    /// Message failed security validation.
    SecurityViolation(SecurityError),
    /// The event was for an unknown node.
    UnknownNode {
        /// The unknown node's ID.
        node_id: [u8; 32],
    },
    /// Processing error.
    ProcessingError(String),
}

impl std::fmt::Display for SubscriptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SecurityViolation(e) => write!(f, "security violation: {e}"),
            Self::UnknownNode { node_id } => {
                write!(f, "unknown node: {:?}", &node_id[..4])
            }
            Self::ProcessingError(msg) => write!(f, "processing error: {msg}"),
        }
    }
}

impl std::error::Error for SubscriptionError {}

impl From<SecurityError> for SubscriptionError {
    fn from(e: SecurityError) -> Self {
        Self::SecurityViolation(e)
    }
}
