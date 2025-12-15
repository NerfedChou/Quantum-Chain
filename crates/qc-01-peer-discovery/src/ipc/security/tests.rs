//! Tests for IPC Security

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
    let cases = vec![
        (5, true),   // Block Propagation
        (7, true),   // Bloom Filters
        (13, true),  // Light Clients
        (1, false),  // Self
        (2, false),  // Block Storage
        (8, false),  // Consensus
        (10, false), // Signature Verification
    ];

    for (sender_id, expected) in cases {
        assert_eq!(
            AuthorizationRules::is_peer_list_authorized(sender_id),
            expected,
            "Failed for sender_id: {}",
            sender_id
        );
    }
}

#[test]
fn test_full_node_list_authorization() {
    let cases = vec![
        (13, true), // Light Clients
        (5, false), // Block Propagation
        (7, false), // Bloom Filters
        (8, false), // Consensus
    ];

    for (sender_id, expected) in cases {
        assert_eq!(
            AuthorizationRules::is_full_node_list_authorized(sender_id),
            expected,
            "Failed for sender_id: {}",
            sender_id
        );
    }
}

#[test]
fn test_recipient_allowed() {
    let cases = vec![
        (5, true),  // Block Propagation
        (7, true),  // Bloom Filters
        (10, true), // Signature Verification
        (13, true), // Light Clients
        (1, false), // Self
        (2, false), // Block Storage
        (8, false), // Consensus
    ];

    for (recipient_id, expected) in cases {
        assert_eq!(
            AuthorizationRules::is_recipient_allowed(recipient_id),
            expected,
            "Failed for recipient_id: {}",
            recipient_id
        );
    }
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
