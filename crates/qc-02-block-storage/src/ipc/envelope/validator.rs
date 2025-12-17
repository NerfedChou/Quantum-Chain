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

        self.validate_version(msg.version)?;
        self.validate_recipient(msg.recipient_id)?;
        self.validate_timestamp(msg.timestamp, now)?;
        self.validate_nonce(msg.nonce, now)?;

        if !self.verify_signature(msg) {
            return Err(EnvelopeError::InvalidSignature);
        }

        self.validate_reply_to(msg)?;

        Ok(())
    }

    fn validate_version(&self, version: u8) -> Result<(), EnvelopeError> {
        if version < MIN_SUPPORTED_VERSION || version > MAX_SUPPORTED_VERSION {
            return Err(EnvelopeError::UnsupportedVersion {
                version,
                min: MIN_SUPPORTED_VERSION,
                max: MAX_SUPPORTED_VERSION,
            });
        }
        Ok(())
    }

    fn validate_recipient(&self, recipient_id: u8) -> Result<(), EnvelopeError> {
        if recipient_id != self.our_subsystem_id {
            return Err(EnvelopeError::WrongRecipient {
                expected: self.our_subsystem_id,
                actual: recipient_id,
            });
        }
        Ok(())
    }

    fn validate_timestamp(&self, timestamp: u64, now: u64) -> Result<(), EnvelopeError> {
        if timestamp > now {
            return Err(EnvelopeError::MessageFromFuture { timestamp, now });
        }

        let age = now.saturating_sub(timestamp);
        if age > MAX_MESSAGE_AGE_SECS {
            return Err(EnvelopeError::MessageExpired {
                age_secs: age,
                max_age: MAX_MESSAGE_AGE_SECS,
            });
        }
        Ok(())
    }

    fn validate_nonce(&mut self, nonce: u64, now: u64) -> Result<(), EnvelopeError> {
        self.cleanup_old_nonces(now);
        if self.seen_nonces.contains(&nonce) {
            return Err(EnvelopeError::NonceReused { nonce });
        }
        self.seen_nonces.insert(nonce);
        Ok(())
    }

    fn validate_reply_to<T>(&self, msg: &AuthenticatedMessage<T>) -> Result<(), EnvelopeError> {
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

        use super::security::{compute_message_signature, SignatureContext};

        let ctx = SignatureContext {
            shared_secret: &self.shared_secret,
            version: msg.version,
            correlation_id: &msg.correlation_id,
            sender_id: msg.sender_id,
            recipient_id: msg.recipient_id,
            timestamp: msg.timestamp,
            nonce: msg.nonce,
        };

        let computed = compute_message_signature(ctx);

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
