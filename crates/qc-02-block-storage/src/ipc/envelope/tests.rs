//! # Envelope Tests

use super::*;

#[test]
fn test_envelope_version_validation() {
    let mut validator = EnvelopeValidator::new(subsystem_ids::BLOCK_STORAGE, [0u8; 32]);

    let msg = AuthenticatedMessage {
        version: 99, // Invalid version
        correlation_id: [0; 16],
        reply_to: None,
        sender_id: subsystem_ids::CONSENSUS,
        recipient_id: subsystem_ids::BLOCK_STORAGE,
        timestamp: current_timestamp(),
        nonce: 1,
        signature: [0; 32],
        payload: (),
    };

    let result = validator.validate(&msg);
    assert!(matches!(
        result,
        Err(EnvelopeError::UnsupportedVersion { .. })
    ));
}

#[test]
fn test_envelope_recipient_validation() {
    let mut validator = EnvelopeValidator::new(subsystem_ids::BLOCK_STORAGE, [0u8; 32]);

    let msg = AuthenticatedMessage {
        version: 1,
        correlation_id: [0; 16],
        reply_to: None,
        sender_id: subsystem_ids::CONSENSUS,
        recipient_id: subsystem_ids::MEMPOOL, // Wrong recipient
        timestamp: current_timestamp(),
        nonce: 1,
        signature: [0; 32],
        payload: (),
    };

    let result = validator.validate(&msg);
    assert!(matches!(result, Err(EnvelopeError::WrongRecipient { .. })));
}

#[test]
fn test_envelope_expired_message() {
    let mut validator = EnvelopeValidator::new(subsystem_ids::BLOCK_STORAGE, [0u8; 32]);

    let msg = AuthenticatedMessage {
        version: 1,
        correlation_id: [0; 16],
        reply_to: None,
        sender_id: subsystem_ids::CONSENSUS,
        recipient_id: subsystem_ids::BLOCK_STORAGE,
        timestamp: current_timestamp() - 120, // 2 minutes old
        nonce: 1,
        signature: [0; 32],
        payload: (),
    };

    let result = validator.validate(&msg);
    assert!(matches!(result, Err(EnvelopeError::MessageExpired { .. })));
}

#[test]
fn test_envelope_nonce_reuse() {
    let mut validator = EnvelopeValidator::new(subsystem_ids::BLOCK_STORAGE, [0u8; 32]);

    let msg1 = AuthenticatedMessage {
        version: 1,
        correlation_id: [0; 16],
        reply_to: None,
        sender_id: subsystem_ids::CONSENSUS,
        recipient_id: subsystem_ids::BLOCK_STORAGE,
        timestamp: current_timestamp(),
        nonce: 12345,
        signature: [0; 32],
        payload: (),
    };

    assert!(validator.validate(&msg1).is_ok());

    let msg2 = AuthenticatedMessage {
        version: 1,
        correlation_id: [1; 16],
        reply_to: None,
        sender_id: subsystem_ids::CONSENSUS,
        recipient_id: subsystem_ids::BLOCK_STORAGE,
        timestamp: current_timestamp(),
        nonce: 12345, // Same nonce!
        signature: [0; 32],
        payload: (),
    };

    let result = validator.validate(&msg2);
    assert!(matches!(result, Err(EnvelopeError::NonceReused { .. })));
}

#[test]
fn test_envelope_reply_to_mismatch() {
    let mut validator = EnvelopeValidator::new(subsystem_ids::BLOCK_STORAGE, [0u8; 32]);

    let msg = AuthenticatedMessage {
        version: 1,
        correlation_id: [0; 16],
        reply_to: Some(Topic::new(subsystem_ids::MEMPOOL, "responses")), // Mismatch!
        sender_id: subsystem_ids::CONSENSUS,
        recipient_id: subsystem_ids::BLOCK_STORAGE,
        timestamp: current_timestamp(),
        nonce: 1,
        signature: [0; 32],
        payload: (),
    };

    let result = validator.validate(&msg);
    assert!(matches!(result, Err(EnvelopeError::ReplyToMismatch { .. })));
}

#[test]
fn test_envelope_valid_message() {
    let mut validator = EnvelopeValidator::new(subsystem_ids::BLOCK_STORAGE, [0u8; 32]);

    let msg = AuthenticatedMessage {
        version: 1,
        correlation_id: [0; 16],
        reply_to: Some(Topic::new(subsystem_ids::CONSENSUS, "responses")),
        sender_id: subsystem_ids::CONSENSUS,
        recipient_id: subsystem_ids::BLOCK_STORAGE,
        timestamp: current_timestamp(),
        nonce: 99999,
        signature: [0; 32],
        payload: (),
    };

    assert!(validator.validate(&msg).is_ok());
}

#[test]
fn test_sender_authorization() {
    let validator = EnvelopeValidator::new(subsystem_ids::BLOCK_STORAGE, [0u8; 32]);

    assert!(validator
        .validate_sender(subsystem_ids::CONSENSUS, &[subsystem_ids::CONSENSUS])
        .is_ok());

    assert!(validator
        .validate_sender(subsystem_ids::MEMPOOL, &[subsystem_ids::CONSENSUS])
        .is_err());
}
