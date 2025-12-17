//! # Envelope Tests

use super::*;
use crate::test_utils::MessageBuilder;

#[test]
fn test_envelope_version_validation() {
    let mut validator = EnvelopeValidator::new(subsystem_ids::BLOCK_STORAGE, [0u8; 32]);

    let msg = MessageBuilder::new().version(99).build();

    let result = validator.validate(&msg);
    assert!(matches!(
        result,
        Err(EnvelopeError::UnsupportedVersion { .. })
    ));
}

#[test]
fn test_envelope_recipient_validation() {
    let mut validator = EnvelopeValidator::new(subsystem_ids::BLOCK_STORAGE, [0u8; 32]);

    let msg = MessageBuilder::new()
        .recipient(subsystem_ids::MEMPOOL)
        .build();

    let result = validator.validate(&msg);
    assert!(matches!(result, Err(EnvelopeError::WrongRecipient { .. })));
}

#[test]
fn test_envelope_expired_message() {
    let mut validator = EnvelopeValidator::new(subsystem_ids::BLOCK_STORAGE, [0u8; 32]);

    let msg = MessageBuilder::new()
        .timestamp(current_timestamp() - 120)
        .build();

    let result = validator.validate(&msg);
    assert!(matches!(result, Err(EnvelopeError::MessageExpired { .. })));
}

#[test]
fn test_envelope_nonce_reuse() {
    let mut validator = EnvelopeValidator::new(subsystem_ids::BLOCK_STORAGE, [0u8; 32]);

    let msg1 = MessageBuilder::new().nonce(12345).build();

    assert!(validator.validate(&msg1).is_ok());

    let msg2 = MessageBuilder::new().nonce(12345).build();

    let result = validator.validate(&msg2);
    assert!(matches!(result, Err(EnvelopeError::NonceReused { .. })));
}

#[test]
fn test_envelope_reply_to_mismatch() {
    let mut validator = EnvelopeValidator::new(subsystem_ids::BLOCK_STORAGE, [0u8; 32]);

    let msg = MessageBuilder::new()
        .reply_to(Topic::new(subsystem_ids::MEMPOOL, "responses"))
        .build();

    let result = validator.validate(&msg);
    assert!(matches!(result, Err(EnvelopeError::ReplyToMismatch { .. })));
}

#[test]
fn test_envelope_valid_message() {
    let mut validator = EnvelopeValidator::new(subsystem_ids::BLOCK_STORAGE, [0u8; 32]);

    let msg = MessageBuilder::new()
        .reply_to(Topic::new(subsystem_ids::CONSENSUS, "responses"))
        .nonce(99999)
        .build();

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
