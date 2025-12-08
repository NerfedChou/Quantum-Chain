//! # IPC Security - Authorization & Validation
//!
//! Implements security boundaries per IPC-MATRIX.md for Subsystem 1.
//!
//! ## Security Rules
//!
//! Per IPC-MATRIX.md, Peer Discovery accepts requests ONLY from:
//! - Subsystem 5 (Block Propagation) - PeerListRequest
//! - Subsystem 7 (Bloom Filters) - PeerListRequest
//! - Subsystem 13 (Light Clients) - PeerListRequest, FullNodeListRequest
//!
//! All other senders are REJECTED.
//!
//! ## Message Validation Order (Architecture.md Section 3.5)
//!
//! 1. Timestamp check (bounds all operations)
//! 2. Version check (before deserialization)
//! 3. Sender check (authorization per IPC Matrix)
//! 4. Signature check (HMAC)
//! 5. Nonce check (replay prevention)
//! 6. Reply-to validation (forwarding attack prevention)

/// Subsystem IDs per Architecture.md Section 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SubsystemId {
    /// Subsystem 1: Peer Discovery & Routing
    PeerDiscovery = 1,
    /// Subsystem 2: Block Storage Engine
    BlockStorage = 2,
    /// Subsystem 3: Transaction Indexing
    TransactionIndexing = 3,
    /// Subsystem 4: State Management
    StateManagement = 4,
    /// Subsystem 5: Block Propagation
    BlockPropagation = 5,
    /// Subsystem 6: Mempool
    Mempool = 6,
    /// Subsystem 7: Bloom Filters
    BloomFilters = 7,
    /// Subsystem 8: Consensus
    Consensus = 8,
    /// Subsystem 9: Finality
    Finality = 9,
    /// Subsystem 10: Signature Verification
    SignatureVerification = 10,
    /// Subsystem 11: Smart Contracts
    SmartContracts = 11,
    /// Subsystem 12: Transaction Ordering
    TransactionOrdering = 12,
    /// Subsystem 13: Light Clients
    LightClients = 13,
    /// Subsystem 14: Sharding
    Sharding = 14,
    /// Subsystem 15: Cross-Chain
    CrossChain = 15,
}

impl SubsystemId {
    /// Convert from raw u8 value.
    #[must_use]
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::PeerDiscovery),
            2 => Some(Self::BlockStorage),
            3 => Some(Self::TransactionIndexing),
            4 => Some(Self::StateManagement),
            5 => Some(Self::BlockPropagation),
            6 => Some(Self::Mempool),
            7 => Some(Self::BloomFilters),
            8 => Some(Self::Consensus),
            9 => Some(Self::Finality),
            10 => Some(Self::SignatureVerification),
            11 => Some(Self::SmartContracts),
            12 => Some(Self::TransactionOrdering),
            13 => Some(Self::LightClients),
            14 => Some(Self::Sharding),
            15 => Some(Self::CrossChain),
            _ => None,
        }
    }

    /// Get the raw u8 value.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

impl std::fmt::Display for SubsystemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PeerDiscovery => write!(f, "PeerDiscovery(1)"),
            Self::BlockStorage => write!(f, "BlockStorage(2)"),
            Self::TransactionIndexing => write!(f, "TransactionIndexing(3)"),
            Self::StateManagement => write!(f, "StateManagement(4)"),
            Self::BlockPropagation => write!(f, "BlockPropagation(5)"),
            Self::Mempool => write!(f, "Mempool(6)"),
            Self::BloomFilters => write!(f, "BloomFilters(7)"),
            Self::Consensus => write!(f, "Consensus(8)"),
            Self::Finality => write!(f, "Finality(9)"),
            Self::SignatureVerification => write!(f, "SignatureVerification(10)"),
            Self::SmartContracts => write!(f, "SmartContracts(11)"),
            Self::TransactionOrdering => write!(f, "TransactionOrdering(12)"),
            Self::LightClients => write!(f, "LightClients(13)"),
            Self::Sharding => write!(f, "Sharding(14)"),
            Self::CrossChain => write!(f, "CrossChain(15)"),
        }
    }
}

/// Security errors that can occur during IPC processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityError {
    /// Message version is unsupported.
    UnsupportedVersion {
        received: u16,
        min_supported: u16,
        max_supported: u16,
    },
    /// Message timestamp is outside valid window.
    TimestampOutOfRange { timestamp: u64, now: u64 },
    /// Sender is not authorized for this request type.
    UnauthorizedSender {
        sender_id: u8,
        allowed_senders: &'static [u8],
    },
    /// Message signature is invalid.
    InvalidSignature,

    /// Reply-to subsystem doesn't match sender (forwarding attack).
    ReplyToMismatch {
        sender_id: u8,
        reply_to_subsystem: u8,
    },
    /// Unknown subsystem ID.
    UnknownSubsystem { id: u8 },
    /// Missing required reply_to for request.
    MissingReplyTo,
    /// Nonce replay detected (UUID-based from shared-types).
    ReplayDetectedUuid { nonce: uuid::Uuid },
}

impl SecurityError {
    /// Convert from shared-types VerificationResult.
    ///
    /// Bridges the centralized `MessageVerifier` result type to subsystem-specific
    /// error types. Callers must verify `!result.is_valid()` before calling.
    ///
    /// Reference: Architecture.md Section 3.5 (Centralized Security Module)
    pub fn from_verification_result(result: shared_types::envelope::VerificationResult) -> Self {
        use shared_types::envelope::VerificationResult;
        match result {
            VerificationResult::Valid => {
                // SAFETY: Precondition violated - callers MUST check !result.is_valid() before conversion.
                // Using unreachable!() instead of panic!() for clearer intent and potential optimization.
                // If this is reached, the caller has a bug in their validation logic.
                unreachable!(
                    "SecurityError::from_verification_result() called with Valid result. \
                     Caller must verify !result.is_valid() before calling."
                )
            }
            VerificationResult::UnsupportedVersion {
                received,
                supported,
            } => SecurityError::UnsupportedVersion {
                received,
                min_supported: supported,
                max_supported: supported,
            },
            VerificationResult::TimestampOutOfRange { timestamp, now } => {
                SecurityError::TimestampOutOfRange { timestamp, now }
            }
            VerificationResult::ReplayDetected { nonce } => {
                SecurityError::ReplayDetectedUuid { nonce }
            }
            VerificationResult::InvalidSignature => SecurityError::InvalidSignature,
            VerificationResult::ReplyToMismatch {
                reply_to_subsystem,
                sender_id,
            } => SecurityError::ReplyToMismatch {
                sender_id,
                reply_to_subsystem,
            },
        }
    }
}

impl std::fmt::Display for SecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedVersion {
                received,
                min_supported,
                max_supported,
            } => write!(
                f,
                "unsupported protocol version {} (supported: {}-{})",
                received, min_supported, max_supported
            ),
            Self::TimestampOutOfRange { timestamp, now } => {
                write!(
                    f,
                    "timestamp {} is outside valid window (now: {})",
                    timestamp, now
                )
            }
            Self::UnauthorizedSender {
                sender_id,
                allowed_senders,
            } => {
                write!(
                    f,
                    "unauthorized sender {} (allowed: {:?})",
                    sender_id, allowed_senders
                )
            }
            Self::InvalidSignature => write!(f, "message signature is invalid"),

            Self::ReplayDetectedUuid { nonce } => write!(f, "replay detected for nonce {}", nonce),
            Self::ReplyToMismatch {
                sender_id,
                reply_to_subsystem,
            } => write!(
                f,
                "reply_to subsystem {} doesn't match sender {}",
                reply_to_subsystem, sender_id
            ),
            Self::UnknownSubsystem { id } => write!(f, "unknown subsystem ID: {}", id),
            Self::MissingReplyTo => write!(f, "missing reply_to for request message"),
        }
    }
}

impl std::error::Error for SecurityError {}

/// Authorization rules for Peer Discovery (Subsystem 1).
///
/// Per IPC-MATRIX.md:
/// - PeerListRequest: Subsystems 5, 7, 13
/// - FullNodeListRequest: Subsystem 13 only
pub struct AuthorizationRules;

impl AuthorizationRules {
    /// Subsystems allowed to send PeerListRequest.
    pub const PEER_LIST_ALLOWED: &'static [u8] = &[5, 7, 13];

    /// Subsystems allowed to send FullNodeListRequest.
    pub const FULL_NODE_LIST_ALLOWED: &'static [u8] = &[13];

    /// Subsystems that Peer Discovery can send TO.
    pub const ALLOWED_RECIPIENTS: &'static [u8] = &[5, 7, 10, 13];

    /// Protocol version bounds.
    pub const MIN_SUPPORTED_VERSION: u16 = 1;
    pub const MAX_SUPPORTED_VERSION: u16 = 1;

    /// Timestamp validity window (seconds).
    pub const TIMESTAMP_MAX_AGE: u64 = 60;
    pub const TIMESTAMP_MAX_FUTURE: u64 = 10;

    /// Check if a sender is authorized for PeerListRequest.
    #[must_use]
    pub fn is_peer_list_authorized(sender_id: u8) -> bool {
        Self::PEER_LIST_ALLOWED.contains(&sender_id)
    }

    /// Check if a sender is authorized for FullNodeListRequest.
    #[must_use]
    pub fn is_full_node_list_authorized(sender_id: u8) -> bool {
        Self::FULL_NODE_LIST_ALLOWED.contains(&sender_id)
    }

    /// Check if we can send to a given subsystem.
    #[must_use]
    pub fn is_recipient_allowed(recipient_id: u8) -> bool {
        Self::ALLOWED_RECIPIENTS.contains(&recipient_id)
    }

    /// Validate protocol version.
    pub fn validate_version(version: u16) -> Result<(), SecurityError> {
        if version < Self::MIN_SUPPORTED_VERSION || version > Self::MAX_SUPPORTED_VERSION {
            return Err(SecurityError::UnsupportedVersion {
                received: version,
                min_supported: Self::MIN_SUPPORTED_VERSION,
                max_supported: Self::MAX_SUPPORTED_VERSION,
            });
        }
        Ok(())
    }

    /// Validate timestamp is within valid window.
    ///
    /// Valid window: `now - 60s <= timestamp <= now + 10s`
    pub fn validate_timestamp(timestamp: u64, now: u64) -> Result<(), SecurityError> {
        let min_valid = now.saturating_sub(Self::TIMESTAMP_MAX_AGE);
        let max_valid = now.saturating_add(Self::TIMESTAMP_MAX_FUTURE);

        if timestamp < min_valid || timestamp > max_valid {
            return Err(SecurityError::TimestampOutOfRange { timestamp, now });
        }
        Ok(())
    }

    /// Validate sender is authorized for PeerListRequest.
    pub fn validate_peer_list_sender(sender_id: u8) -> Result<(), SecurityError> {
        if !Self::is_peer_list_authorized(sender_id) {
            return Err(SecurityError::UnauthorizedSender {
                sender_id,
                allowed_senders: Self::PEER_LIST_ALLOWED,
            });
        }
        Ok(())
    }

    /// Validate sender is authorized for FullNodeListRequest.
    pub fn validate_full_node_list_sender(sender_id: u8) -> Result<(), SecurityError> {
        if !Self::is_full_node_list_authorized(sender_id) {
            return Err(SecurityError::UnauthorizedSender {
                sender_id,
                allowed_senders: Self::FULL_NODE_LIST_ALLOWED,
            });
        }
        Ok(())
    }

    /// Validate reply_to matches sender_id.
    ///
    /// Prevents forwarding attacks where a compromised subsystem sets reply_to
    /// to a victim subsystem, causing responses to be sent to the wrong destination.
    ///
    /// Reference: Architecture.md Section 3.3.1 (Reply-To Validation)
    pub fn validate_reply_to(
        sender_id: u8,
        reply_to_subsystem: Option<u8>,
    ) -> Result<(), SecurityError> {
        if let Some(reply_to) = reply_to_subsystem {
            if reply_to != sender_id {
                return Err(SecurityError::ReplyToMismatch {
                    sender_id,
                    reply_to_subsystem: reply_to,
                });
            }
        }
        Ok(())
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subsystem_id_from_u8() {
        assert_eq!(SubsystemId::from_u8(1), Some(SubsystemId::PeerDiscovery));
        assert_eq!(SubsystemId::from_u8(5), Some(SubsystemId::BlockPropagation));
        assert_eq!(SubsystemId::from_u8(13), Some(SubsystemId::LightClients));
        assert_eq!(SubsystemId::from_u8(0), None);
        assert_eq!(SubsystemId::from_u8(16), None);
    }

    #[test]
    fn test_subsystem_id_as_u8() {
        assert_eq!(SubsystemId::PeerDiscovery.as_u8(), 1);
        assert_eq!(SubsystemId::BlockPropagation.as_u8(), 5);
        assert_eq!(SubsystemId::LightClients.as_u8(), 13);
    }

    #[test]
    fn test_peer_list_authorization() {
        // Authorized senders
        assert!(AuthorizationRules::is_peer_list_authorized(5)); // Block Propagation
        assert!(AuthorizationRules::is_peer_list_authorized(7)); // Bloom Filters
        assert!(AuthorizationRules::is_peer_list_authorized(13)); // Light Clients

        // Unauthorized senders
        assert!(!AuthorizationRules::is_peer_list_authorized(1)); // Self
        assert!(!AuthorizationRules::is_peer_list_authorized(2)); // Block Storage
        assert!(!AuthorizationRules::is_peer_list_authorized(8)); // Consensus
        assert!(!AuthorizationRules::is_peer_list_authorized(10)); // Signature Verification
    }

    #[test]
    fn test_full_node_list_authorization() {
        // Only Light Clients (13) is authorized
        assert!(AuthorizationRules::is_full_node_list_authorized(13));

        // All others unauthorized
        assert!(!AuthorizationRules::is_full_node_list_authorized(5));
        assert!(!AuthorizationRules::is_full_node_list_authorized(7));
        assert!(!AuthorizationRules::is_full_node_list_authorized(8));
    }

    #[test]
    fn test_recipient_allowed() {
        // Allowed recipients
        assert!(AuthorizationRules::is_recipient_allowed(5)); // Block Propagation
        assert!(AuthorizationRules::is_recipient_allowed(7)); // Bloom Filters
        assert!(AuthorizationRules::is_recipient_allowed(10)); // Signature Verification
        assert!(AuthorizationRules::is_recipient_allowed(13)); // Light Clients

        // Disallowed recipients
        assert!(!AuthorizationRules::is_recipient_allowed(1)); // Self
        assert!(!AuthorizationRules::is_recipient_allowed(2)); // Block Storage
        assert!(!AuthorizationRules::is_recipient_allowed(8)); // Consensus
    }

    #[test]
    fn test_validate_version() {
        assert!(AuthorizationRules::validate_version(1).is_ok());

        let result = AuthorizationRules::validate_version(0);
        assert!(matches!(
            result,
            Err(SecurityError::UnsupportedVersion { .. })
        ));

        let result = AuthorizationRules::validate_version(2);
        assert!(matches!(
            result,
            Err(SecurityError::UnsupportedVersion { .. })
        ));
    }

    #[test]
    fn test_validate_timestamp() {
        let now = 1000u64;

        // Valid timestamps
        assert!(AuthorizationRules::validate_timestamp(now, now).is_ok());
        assert!(AuthorizationRules::validate_timestamp(now - 30, now).is_ok()); // 30s ago
        assert!(AuthorizationRules::validate_timestamp(now + 5, now).is_ok()); // 5s future

        // Invalid timestamps
        let result = AuthorizationRules::validate_timestamp(now - 100, now);
        assert!(matches!(
            result,
            Err(SecurityError::TimestampOutOfRange { .. })
        ));

        let result = AuthorizationRules::validate_timestamp(now + 100, now);
        assert!(matches!(
            result,
            Err(SecurityError::TimestampOutOfRange { .. })
        ));
    }

    #[test]
    fn test_validate_peer_list_sender() {
        assert!(AuthorizationRules::validate_peer_list_sender(5).is_ok());
        assert!(AuthorizationRules::validate_peer_list_sender(7).is_ok());
        assert!(AuthorizationRules::validate_peer_list_sender(13).is_ok());

        let result = AuthorizationRules::validate_peer_list_sender(8);
        assert!(matches!(
            result,
            Err(SecurityError::UnauthorizedSender { .. })
        ));
    }

    #[test]
    fn test_validate_reply_to() {
        // Valid: reply_to matches sender
        assert!(AuthorizationRules::validate_reply_to(5, Some(5)).is_ok());

        // Valid: no reply_to
        assert!(AuthorizationRules::validate_reply_to(5, None).is_ok());

        // Invalid: reply_to doesn't match sender (forwarding attack)
        let result = AuthorizationRules::validate_reply_to(5, Some(13));
        assert!(matches!(result, Err(SecurityError::ReplyToMismatch { .. })));
    }

    #[test]
    fn test_security_error_display() {
        let err = SecurityError::UnauthorizedSender {
            sender_id: 8,
            allowed_senders: AuthorizationRules::PEER_LIST_ALLOWED,
        };
        let msg = err.to_string();
        assert!(msg.contains("unauthorized sender"));
        assert!(msg.contains("8"));
    }
}
