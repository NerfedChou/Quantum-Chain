use super::error::SecurityError;

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

    /// Minimum supported protocol version.
    pub const MIN_SUPPORTED_VERSION: u16 = 1;
    /// Maximum supported protocol version.
    pub const MAX_SUPPORTED_VERSION: u16 = 1;

    /// Maximum age of a valid timestamp in seconds.
    pub const TIMESTAMP_MAX_AGE: u64 = 60;
    /// Maximum future timestamp allowed in seconds.
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
