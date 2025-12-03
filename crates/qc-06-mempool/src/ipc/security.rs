//! Security boundaries and authorization for IPC messages.
//!
//! Implements the security rules defined in IPC-MATRIX.md for Subsystem 6.
//!
//! # Authorization Rules
//!
//! | Message Type | Authorized Sender(s) |
//! |--------------|---------------------|
//! | `AddTransactionRequest` | Subsystem 10 ONLY |
//! | `GetTransactionsRequest` | Subsystem 8 ONLY |
//! | `RemoveTransactionsRequest` | Subsystem 8 ONLY |
//! | `BlockStorageConfirmation` | Subsystem 2 ONLY |
//! | `BlockRejectedNotification` | Subsystems 2, 8 |
//!
//! # Security Validation Order (Architecture.md Section 3.5)
//!
//! 1. Timestamp check (bounds all operations, prevents DoS)
//! 2. Version check (before any deserialization)
//! 3. Sender check (authorization per IPC Matrix)
//! 4. Signature check (HMAC)
//! 5. Nonce check (replay prevention via TimeBoundedNonceCache)
//! 6. Reply-to validation (forwarding attack prevention)

use crate::domain::MempoolError;
use std::collections::HashSet;
use std::sync::Mutex;

/// Subsystem IDs as defined in Architecture.md.
pub mod subsystem_id {
    /// Peer Discovery
    pub const PEER_DISCOVERY: u8 = 1;
    /// Block Storage
    pub const BLOCK_STORAGE: u8 = 2;
    /// Transaction Indexing
    pub const TX_INDEXING: u8 = 3;
    /// State Management
    pub const STATE_MANAGEMENT: u8 = 4;
    /// Block Propagation
    pub const BLOCK_PROPAGATION: u8 = 5;
    /// Mempool (this subsystem)
    pub const MEMPOOL: u8 = 6;
    /// Bloom Filters
    pub const BLOOM_FILTERS: u8 = 7;
    /// Consensus
    pub const CONSENSUS: u8 = 8;
    /// Finality
    pub const FINALITY: u8 = 9;
    /// Signature Verification
    pub const SIGNATURE_VERIFICATION: u8 = 10;
}

/// Authorization rules for IPC messages.
#[derive(Debug, Clone)]
pub struct AuthorizationRules;

impl AuthorizationRules {
    /// Validates that a sender is authorized to send AddTransactionRequest.
    ///
    /// Only Subsystem 10 (Signature Verification) is allowed.
    pub fn validate_add_transaction(sender_id: u8) -> Result<(), MempoolError> {
        if sender_id != subsystem_id::SIGNATURE_VERIFICATION {
            return Err(MempoolError::UnauthorizedSender {
                sender_id,
                allowed: vec![subsystem_id::SIGNATURE_VERIFICATION],
            });
        }
        Ok(())
    }

    /// Validates that a sender is authorized to send GetTransactionsRequest.
    ///
    /// Only Subsystem 8 (Consensus) is allowed.
    pub fn validate_get_transactions(sender_id: u8) -> Result<(), MempoolError> {
        if sender_id != subsystem_id::CONSENSUS {
            return Err(MempoolError::UnauthorizedSender {
                sender_id,
                allowed: vec![subsystem_id::CONSENSUS],
            });
        }
        Ok(())
    }

    /// Validates that a sender is authorized to send RemoveTransactionsRequest.
    ///
    /// Only Subsystem 8 (Consensus) is allowed.
    pub fn validate_remove_transactions(sender_id: u8) -> Result<(), MempoolError> {
        if sender_id != subsystem_id::CONSENSUS {
            return Err(MempoolError::UnauthorizedSender {
                sender_id,
                allowed: vec![subsystem_id::CONSENSUS],
            });
        }
        Ok(())
    }

    /// Validates that a sender is authorized to send BlockStorageConfirmation.
    ///
    /// Only Subsystem 2 (Block Storage) is allowed.
    pub fn validate_storage_confirmation(sender_id: u8) -> Result<(), MempoolError> {
        if sender_id != subsystem_id::BLOCK_STORAGE {
            return Err(MempoolError::UnauthorizedSender {
                sender_id,
                allowed: vec![subsystem_id::BLOCK_STORAGE],
            });
        }
        Ok(())
    }

    /// Validates that a sender is authorized to send BlockRejectedNotification.
    ///
    /// Subsystems 2 (Block Storage) and 8 (Consensus) are allowed.
    pub fn validate_block_rejected(sender_id: u8) -> Result<(), MempoolError> {
        if sender_id != subsystem_id::BLOCK_STORAGE && sender_id != subsystem_id::CONSENSUS {
            return Err(MempoolError::UnauthorizedSender {
                sender_id,
                allowed: vec![subsystem_id::BLOCK_STORAGE, subsystem_id::CONSENSUS],
            });
        }
        Ok(())
    }
}

/// Validates message timestamp is within acceptable window.
///
/// Per Architecture.md:
/// - Valid window: `now - 60s <= timestamp <= now + 10s`
///
/// Returns `Ok(())` if valid, `Err` with reason if invalid.
pub fn validate_timestamp(msg_timestamp: u64, now: u64) -> Result<(), MempoolError> {
    let max_age = 60; // seconds
    let max_future = 10; // seconds

    if msg_timestamp > now + max_future {
        return Err(MempoolError::TimestampTooFuture {
            timestamp: msg_timestamp,
            now,
        });
    }
    if now > msg_timestamp && now - msg_timestamp > max_age {
        return Err(MempoolError::TimestampTooOld {
            timestamp: msg_timestamp,
            now,
        });
    }
    Ok(())
}

// =============================================================================
// HMAC SIGNATURE VERIFICATION (Architecture.md Section 3.5)
// =============================================================================

/// HMAC-SHA256 key for IPC message signing.
pub type HmacKey = [u8; 32];

/// Time-bounded nonce cache for replay prevention.
pub struct NonceCache {
    seen: Mutex<HashSet<(u64, u64)>>, // (nonce, timestamp)
    max_age: u64,
}

impl Default for NonceCache {
    fn default() -> Self {
        Self::new(120) // 2 minute window
    }
}

impl NonceCache {
    /// Create a new nonce cache with specified max age.
    #[must_use]
    pub fn new(max_age: u64) -> Self {
        Self {
            seen: Mutex::new(HashSet::new()),
            max_age,
        }
    }

    /// Check if a nonce has been seen, and if not, record it.
    pub fn check_and_record(&self, nonce: u64, timestamp: u64, now: u64) -> bool {
        let mut seen = self.seen.lock().unwrap();
        seen.retain(|(_, ts)| now.saturating_sub(*ts) <= self.max_age);

        if seen.contains(&(nonce, timestamp)) {
            return false;
        }
        seen.insert((nonce, timestamp));
        true
    }
}

/// Validates HMAC-SHA256 signature on IPC message.
pub fn validate_hmac_signature(
    message: &[u8],
    signature: &[u8; 32],
    key: &HmacKey,
) -> Result<(), MempoolError> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(message);

    if mac.verify_slice(signature).is_err() {
        return Err(MempoolError::InvalidSignature);
    }
    Ok(())
}

/// Validates nonce has not been seen before.
pub fn validate_nonce(
    nonce: u64,
    timestamp: u64,
    now: u64,
    cache: &NonceCache,
) -> Result<(), MempoolError> {
    if !cache.check_and_record(nonce, timestamp, now) {
        return Err(MempoolError::ReplayDetected { nonce });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_add_transaction_authorized() {
        // Subsystem 10 should be authorized
        assert!(
            AuthorizationRules::validate_add_transaction(subsystem_id::SIGNATURE_VERIFICATION)
                .is_ok()
        );
    }

    #[test]
    fn test_validate_add_transaction_unauthorized() {
        // Other subsystems should be rejected
        assert!(AuthorizationRules::validate_add_transaction(subsystem_id::CONSENSUS).is_err());
        assert!(AuthorizationRules::validate_add_transaction(subsystem_id::BLOCK_STORAGE).is_err());
        assert!(AuthorizationRules::validate_add_transaction(subsystem_id::MEMPOOL).is_err());
    }

    #[test]
    fn test_validate_get_transactions_authorized() {
        // Subsystem 8 should be authorized
        assert!(AuthorizationRules::validate_get_transactions(subsystem_id::CONSENSUS).is_ok());
    }

    #[test]
    fn test_validate_get_transactions_unauthorized() {
        // Other subsystems should be rejected
        assert!(AuthorizationRules::validate_get_transactions(
            subsystem_id::SIGNATURE_VERIFICATION
        )
        .is_err());
        assert!(
            AuthorizationRules::validate_get_transactions(subsystem_id::BLOCK_STORAGE).is_err()
        );
    }

    #[test]
    fn test_validate_storage_confirmation_authorized() {
        // Subsystem 2 should be authorized
        assert!(
            AuthorizationRules::validate_storage_confirmation(subsystem_id::BLOCK_STORAGE).is_ok()
        );
    }

    #[test]
    fn test_validate_storage_confirmation_unauthorized() {
        // Other subsystems should be rejected
        assert!(
            AuthorizationRules::validate_storage_confirmation(subsystem_id::CONSENSUS).is_err()
        );
        assert!(AuthorizationRules::validate_storage_confirmation(
            subsystem_id::SIGNATURE_VERIFICATION
        )
        .is_err());
    }

    #[test]
    fn test_validate_block_rejected_authorized() {
        // Subsystems 2 and 8 should be authorized
        assert!(AuthorizationRules::validate_block_rejected(subsystem_id::BLOCK_STORAGE).is_ok());
        assert!(AuthorizationRules::validate_block_rejected(subsystem_id::CONSENSUS).is_ok());
    }

    #[test]
    fn test_validate_block_rejected_unauthorized() {
        // Other subsystems should be rejected
        assert!(
            AuthorizationRules::validate_block_rejected(subsystem_id::SIGNATURE_VERIFICATION)
                .is_err()
        );
        assert!(AuthorizationRules::validate_block_rejected(subsystem_id::MEMPOOL).is_err());
    }

    #[test]
    fn test_validate_timestamp_valid() {
        let now = 1000;
        // Exactly now
        assert!(validate_timestamp(1000, now).is_ok());
        // 30 seconds ago (within 60s window)
        assert!(validate_timestamp(970, now).is_ok());
        // 5 seconds in future (within 10s window)
        assert!(validate_timestamp(1005, now).is_ok());
    }

    #[test]
    fn test_validate_timestamp_too_old() {
        let now = 1000;
        // 61 seconds ago (outside 60s window)
        assert!(validate_timestamp(939, now).is_err());
    }

    #[test]
    fn test_validate_timestamp_too_future() {
        let now = 1000;
        // 11 seconds in future (outside 10s window)
        assert!(validate_timestamp(1011, now).is_err());
    }
}
