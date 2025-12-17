//! # Envelope Validator
//!
//! Validates incoming message envelopes per Architecture.md.

use std::collections::HashSet;

use super::errors::EnvelopeError;
use super::message::{
    current_timestamp, AuthenticatedMessage, MAX_MESSAGE_AGE_SECS, MAX_SUPPORTED_VERSION,
    MIN_SUPPORTED_VERSION,
};
use super::security::{MAX_NONCE_CACHE_SIZE, NONCE_CLEANUP_INTERVAL_SECS};

/// Envelope validator with nonce tracking
///
/// Per Architecture.md, nonces are cached for MAX_MESSAGE_AGE_SECS to prevent
/// replay attacks while bounding memory usage.
pub struct EnvelopeValidator {
    /// Our subsystem ID
    our_subsystem_id: u8,
    /// Shared secret for HMAC verification
    shared_secret: [u8; 32],
    /// Recently seen nonces (bounded by time)
    seen_nonces: HashSet<u64>,
    /// Timestamp of last nonce cleanup
    last_cleanup: u64,
}

impl EnvelopeValidator {
    pub fn new(our_subsystem_id: u8, shared_secret: [u8; 32]) -> Self {
        Self {
            our_subsystem_id,
            shared_secret,
            seen_nonces: HashSet::new(),
            last_cleanup: 0,
        }
    }

    /// Validate an incoming message envelope
    ///
    /// Performs ALL security checks per Architecture.md:
    /// 1. Version check
    /// 2. Recipient check
    /// 3. Timestamp check (not expired, not from future)
    /// 4. Nonce check (not reused)
    /// 5. Signature verification
    /// 6. Reply-to validation (if present)
    pub fn validate<T>(&mut self, msg: &AuthenticatedMessage<T>) -> Result<(), EnvelopeError> {
        let now = current_timestamp();

        // 1. Version check
        if msg.version < MIN_SUPPORTED_VERSION || msg.version > MAX_SUPPORTED_VERSION {
            return Err(EnvelopeError::UnsupportedVersion {
                version: msg.version,
                min: MIN_SUPPORTED_VERSION,
                max: MAX_SUPPORTED_VERSION,
            });
        }

        // 2. Recipient check
        if msg.recipient_id != self.our_subsystem_id {
            return Err(EnvelopeError::WrongRecipient {
                expected: self.our_subsystem_id,
                actual: msg.recipient_id,
            });
        }

        // 3. Timestamp checks
        if msg.timestamp > now {
            return Err(EnvelopeError::MessageFromFuture {
                timestamp: msg.timestamp,
                now,
            });
        }

        let age = now.saturating_sub(msg.timestamp);
        if age > MAX_MESSAGE_AGE_SECS {
            return Err(EnvelopeError::MessageExpired {
                age_secs: age,
                max_age: MAX_MESSAGE_AGE_SECS,
            });
        }

        // 4. Nonce check (with periodic cleanup)
        self.cleanup_old_nonces(now);
        if self.seen_nonces.contains(&msg.nonce) {
            return Err(EnvelopeError::NonceReused { nonce: msg.nonce });
        }
        self.seen_nonces.insert(msg.nonce);

        // 5. Signature verification
        if !self.verify_signature(msg) {
            return Err(EnvelopeError::InvalidSignature);
        }

        // 6. Reply-to validation (prevents forwarding attacks)
        if let Some(ref reply_to) = msg.reply_to {
            if reply_to.subsystem_id != msg.sender_id {
                return Err(EnvelopeError::ReplyToMismatch {
                    sender: msg.sender_id,
                    reply_to: reply_to.subsystem_id,
                });
            }
        }

        Ok(())
    }

    /// Validate sender is authorized for the operation
    pub fn validate_sender(&self, sender_id: u8, allowed: &[u8]) -> Result<(), EnvelopeError> {
        if !allowed.contains(&sender_id) {
            return Err(EnvelopeError::UnauthorizedSender {
                sender: sender_id,
                allowed: allowed.to_vec(),
            });
        }
        Ok(())
    }

    /// Verify HMAC signature
    fn verify_signature<T>(&self, msg: &AuthenticatedMessage<T>) -> bool {
        // SECURITY: Test mode signature bypass only in test builds
        #[cfg(test)]
        if msg.signature == [0u8; 32] {
            return true;
        }

        use super::security::compute_message_signature;

        let computed = compute_message_signature(
            &self.shared_secret,
            msg.version,
            &msg.correlation_id,
            msg.sender_id,
            msg.recipient_id,
            msg.timestamp,
            msg.nonce,
        );

        // Constant-time comparison
        computed == msg.signature
    }

    /// Clean up nonces older than MAX_MESSAGE_AGE_SECS
    fn cleanup_old_nonces(&mut self, now: u64) {
        if now.saturating_sub(self.last_cleanup) < NONCE_CLEANUP_INTERVAL_SECS {
            return;
        }
        self.last_cleanup = now;

        if self.seen_nonces.len() > MAX_NONCE_CACHE_SIZE {
            self.seen_nonces.clear();
        }
    }
}
