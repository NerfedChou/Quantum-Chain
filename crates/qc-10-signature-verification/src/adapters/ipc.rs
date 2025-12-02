//! # IPC Message Handler
//!
//! Handles incoming IPC messages with security boundary enforcement.
//!
//! Reference:
//! - SPEC-10 Section 4 (Event Schema)
//! - IPC-MATRIX.md Subsystem 10 (Security Boundaries)
//! - Architecture.md Section 3.2.1 (Envelope-Only Identity)
//!
//! ## Security Boundaries (IPC-MATRIX.md)
//!
//! **AUTHORIZED Consumers:**
//! - Subsystem 1 (Peer Discovery) - VerifyNodeIdentityRequest ONLY
//! - Subsystem 5 (Block Propagation) - VerifySignatureRequest
//! - Subsystem 6 (Mempool) - VerifyTransactionRequest
//! - Subsystem 8 (Consensus) - All verification requests + BatchVerify
//! - Subsystem 9 (Finality) - VerifySignatureRequest
//!
//! **FORBIDDEN Consumers:**
//! - Subsystems 2, 3, 4, 7, 11, 12, 13, 14, 15
//!
//! ## Rate Limiting
//!
//! - Subsystem 1: Max 100/sec (network edge protection)
//! - Subsystems 5, 6: Max 1000/sec (internal traffic)
//! - Subsystems 8, 9: No limit (consensus-critical)

use crate::domain::entities::{
    BatchVerificationRequest, BatchVerificationResult, EcdsaSignature, VerificationRequest,
};
use crate::domain::errors::SignatureError;
use crate::ports::inbound::SignatureVerificationApi;
use shared_types::envelope::AuthenticatedMessage;
use shared_types::ipc::{
    VerifyNodeIdentityPayload, VerifyNodeIdentityResponse, VerifySignatureRequestPayload,
    VerifySignatureResponsePayload,
};
use shared_types::Hash;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use thiserror::Error;

// =============================================================================
// SUBSYSTEM ID CONSTANTS (per IPC-MATRIX.md)
// =============================================================================

/// Our subsystem ID
pub const SUBSYSTEM_ID: u8 = 10;

/// Authorized sender subsystems
pub mod authorized {
    pub const PEER_DISCOVERY: u8 = 1;
    pub const BLOCK_PROPAGATION: u8 = 5;
    pub const MEMPOOL: u8 = 6;
    pub const CONSENSUS: u8 = 8;
    pub const FINALITY: u8 = 9;
}

/// Forbidden sender subsystems
pub mod forbidden {
    pub const BLOCK_STORAGE: u8 = 2;
    pub const TRANSACTION_INDEXING: u8 = 3;
    pub const STATE_MANAGEMENT: u8 = 4;
    pub const BLOOM_FILTERS: u8 = 7;
    pub const SMART_CONTRACTS: u8 = 11;
    pub const TRANSACTION_ORDERING: u8 = 12;
    pub const LIGHT_CLIENTS: u8 = 13;
    pub const SHARDING: u8 = 14;
    pub const CROSS_CHAIN: u8 = 15;
}

// =============================================================================
// ERROR TYPES
// =============================================================================

/// IPC handling errors.
#[derive(Debug, Error)]
pub enum IpcError {
    /// Sender is not authorized to send this message type
    #[error("Unauthorized sender: subsystem {sender_id} is not authorized for this operation")]
    UnauthorizedSender { sender_id: u8 },

    /// Sender is in the forbidden list
    #[error("Forbidden sender: subsystem {sender_id} is explicitly forbidden from accessing SignatureVerification")]
    ForbiddenSender { sender_id: u8 },

    /// Rate limit exceeded
    #[error("Rate limit exceeded for subsystem {sender_id}: {current}/sec exceeds {limit}/sec")]
    RateLimitExceeded {
        sender_id: u8,
        current: u64,
        limit: u64,
    },

    /// Invalid message version
    #[error("Unsupported message version: {version}")]
    UnsupportedVersion { version: u16 },

    /// Message timestamp out of range
    #[error("Message timestamp {timestamp} is outside valid range")]
    TimestampOutOfRange { timestamp: u64 },

    /// Recipient mismatch
    #[error("Message recipient {recipient_id} does not match our subsystem ID {expected}")]
    RecipientMismatch { recipient_id: u8, expected: u8 },

    /// Signature verification failed
    #[error("Signature verification failed: {0}")]
    SignatureError(#[from] SignatureError),

    /// Batch too large
    #[error("Batch size {size} exceeds maximum {max}")]
    BatchTooLarge { size: usize, max: usize },
}

// =============================================================================
// RATE LIMITER
// =============================================================================

/// Per-subsystem rate limits (requests per second).
/// Reference: IPC-MATRIX.md Subsystem 10 Rate Limiting
#[derive(Debug, Clone, Copy)]
pub struct RateLimits {
    /// Subsystem 1 (Peer Discovery): 100/sec
    pub peer_discovery: u64,
    /// Subsystems 5, 6: 1000/sec
    pub internal: u64,
    /// Subsystems 8, 9: No limit (u64::MAX)
    pub consensus_critical: u64,
}

impl Default for RateLimits {
    fn default() -> Self {
        Self {
            peer_discovery: 100,
            internal: 1000,
            consensus_critical: u64::MAX, // No limit
        }
    }
}

/// Token bucket rate limiter per subsystem.
struct RateLimiter {
    limits: RateLimits,
    /// Counters per subsystem: (count, last_reset_time)
    counters: Mutex<HashMap<u8, (AtomicU64, Instant)>>,
}

impl RateLimiter {
    fn new(limits: RateLimits) -> Self {
        Self {
            limits,
            counters: Mutex::new(HashMap::new()),
        }
    }

    /// Check if a request from the given subsystem is allowed.
    fn check(&self, sender_id: u8) -> Result<(), IpcError> {
        let limit = self.get_limit(sender_id);

        // No limit for consensus-critical path
        if limit == u64::MAX {
            return Ok(());
        }

        let mut counters = self.counters.lock().unwrap();
        let now = Instant::now();

        let entry = counters
            .entry(sender_id)
            .or_insert_with(|| (AtomicU64::new(0), now));

        // Reset counter if more than 1 second has passed
        if now.duration_since(entry.1) >= Duration::from_secs(1) {
            entry.0.store(0, Ordering::Relaxed);
            entry.1 = now;
        }

        let current = entry.0.fetch_add(1, Ordering::Relaxed);
        if current >= limit {
            return Err(IpcError::RateLimitExceeded {
                sender_id,
                current,
                limit,
            });
        }

        Ok(())
    }

    /// Get the rate limit for a subsystem.
    fn get_limit(&self, sender_id: u8) -> u64 {
        match sender_id {
            authorized::PEER_DISCOVERY => self.limits.peer_discovery,
            authorized::BLOCK_PROPAGATION | authorized::MEMPOOL => self.limits.internal,
            authorized::CONSENSUS | authorized::FINALITY => self.limits.consensus_critical,
            _ => 0, // Forbidden subsystems get zero limit
        }
    }
}

// =============================================================================
// SECURITY BOUNDARY CHECKS
// =============================================================================

/// Check if a sender is authorized for signature verification.
///
/// Reference: IPC-MATRIX.md Subsystem 10 Security Boundaries
fn check_authorized_sender(sender_id: u8) -> Result<(), IpcError> {
    // Check forbidden list first (explicit rejection)
    if is_forbidden(sender_id) {
        return Err(IpcError::ForbiddenSender { sender_id });
    }

    // Check authorized list
    if !is_authorized(sender_id) {
        return Err(IpcError::UnauthorizedSender { sender_id });
    }

    Ok(())
}

/// Check if sender is in the authorized list.
fn is_authorized(sender_id: u8) -> bool {
    matches!(
        sender_id,
        authorized::PEER_DISCOVERY
            | authorized::BLOCK_PROPAGATION
            | authorized::MEMPOOL
            | authorized::CONSENSUS
            | authorized::FINALITY
    )
}

/// Check if sender is in the forbidden list.
fn is_forbidden(sender_id: u8) -> bool {
    matches!(
        sender_id,
        forbidden::BLOCK_STORAGE
            | forbidden::TRANSACTION_INDEXING
            | forbidden::STATE_MANAGEMENT
            | forbidden::BLOOM_FILTERS
            | forbidden::SMART_CONTRACTS
            | forbidden::TRANSACTION_ORDERING
            | forbidden::LIGHT_CLIENTS
            | forbidden::SHARDING
            | forbidden::CROSS_CHAIN
    )
}

/// Check if sender is authorized for VerifyNodeIdentityRequest.
///
/// Reference: IPC-MATRIX.md - Only Subsystem 1 (Peer Discovery) can request node identity verification.
fn check_node_identity_authorized(sender_id: u8) -> Result<(), IpcError> {
    if sender_id != authorized::PEER_DISCOVERY {
        return Err(IpcError::UnauthorizedSender { sender_id });
    }
    Ok(())
}

/// Check if sender is authorized for BatchVerifyRequest.
///
/// Reference: IPC-MATRIX.md - Only Subsystem 8 (Consensus) can request batch verification.
fn check_batch_verify_authorized(sender_id: u8) -> Result<(), IpcError> {
    if sender_id != authorized::CONSENSUS {
        return Err(IpcError::UnauthorizedSender { sender_id });
    }
    Ok(())
}

// =============================================================================
// MESSAGE VALIDATION
// =============================================================================

/// Validate the message envelope.
///
/// Reference: Architecture.md Section 3.2.2 (Time-Bounded Nonce)
fn validate_envelope<T>(msg: &AuthenticatedMessage<T>) -> Result<(), IpcError> {
    // Check version
    if msg.version != AuthenticatedMessage::<T>::CURRENT_VERSION {
        return Err(IpcError::UnsupportedVersion {
            version: msg.version,
        });
    }

    // Check recipient
    if msg.recipient_id != SUBSYSTEM_ID {
        return Err(IpcError::RecipientMismatch {
            recipient_id: msg.recipient_id,
            expected: SUBSYSTEM_ID,
        });
    }

    // Check timestamp (within valid window)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let max_age = AuthenticatedMessage::<T>::MAX_AGE;
    let max_future = AuthenticatedMessage::<T>::MAX_FUTURE_SKEW;

    if msg.timestamp + max_age < now || msg.timestamp > now + max_future {
        return Err(IpcError::TimestampOutOfRange {
            timestamp: msg.timestamp,
        });
    }

    Ok(())
}

// =============================================================================
// IPC MESSAGE HANDLER
// =============================================================================

/// Maximum batch size for batch verification (DoS protection).
pub const MAX_BATCH_SIZE: usize = 1000;

/// IPC message handler for Subsystem 10.
///
/// This handler enforces security boundaries and rate limits as specified in IPC-MATRIX.md.
pub struct IpcHandler<S: SignatureVerificationApi> {
    service: S,
    rate_limiter: RateLimiter,
}

impl<S: SignatureVerificationApi> IpcHandler<S> {
    /// Create a new IPC handler with default rate limits.
    pub fn new(service: S) -> Self {
        Self {
            service,
            rate_limiter: RateLimiter::new(RateLimits::default()),
        }
    }

    /// Create a new IPC handler with custom rate limits.
    pub fn with_rate_limits(service: S, limits: RateLimits) -> Self {
        Self {
            service,
            rate_limiter: RateLimiter::new(limits),
        }
    }

    /// Handle a VerifySignatureRequest message.
    ///
    /// Reference: SPEC-10 Section 4, IPC-MATRIX.md
    ///
    /// # Security
    /// - Validates sender is authorized (1, 5, 8, 9)
    /// - Enforces rate limits
    pub fn handle_verify_signature(
        &self,
        msg: AuthenticatedMessage<VerifySignatureRequestPayload>,
    ) -> Result<VerifySignatureResponsePayload, IpcError> {
        // Step 1: Validate envelope
        validate_envelope(&msg)?;

        // Step 2: Check sender authorization
        check_authorized_sender(msg.sender_id)?;

        // Step 3: Check rate limit
        self.rate_limiter.check(msg.sender_id)?;

        // Step 4: Extract signature components
        let payload = &msg.payload;

        // Convert shared_types::Signature ([u8; 64]) to EcdsaSignature
        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&payload.signature[..32]);
        s.copy_from_slice(&payload.signature[32..]);

        let ecdsa_sig = EcdsaSignature { r, s, v: 27 }; // Try v=27 first

        // Step 5: Compute message hash
        let message_hash = compute_message_hash(&payload.message);

        // Step 6: Verify signature
        let result = self.service.verify_ecdsa(&message_hash, &ecdsa_sig);

        // If failed with v=27, try v=28
        let final_result = if !result.valid {
            let ecdsa_sig_28 = EcdsaSignature { r, s, v: 28 };
            self.service.verify_ecdsa(&message_hash, &ecdsa_sig_28)
        } else {
            result
        };

        Ok(VerifySignatureResponsePayload {
            valid: final_result.valid,
        })
    }

    /// Handle a VerifyNodeIdentityRequest message.
    ///
    /// Reference: IPC-MATRIX.md - Peer Discovery DDoS defense
    ///
    /// # Security
    /// - ONLY Subsystem 1 (Peer Discovery) is authorized
    pub fn handle_verify_node_identity(
        &self,
        msg: AuthenticatedMessage<VerifyNodeIdentityPayload>,
    ) -> Result<VerifyNodeIdentityResponse, IpcError> {
        // Step 1: Validate envelope
        validate_envelope(&msg)?;

        // Step 2: Check sender is Peer Discovery ONLY
        check_node_identity_authorized(msg.sender_id)?;

        // Step 3: Check rate limit
        self.rate_limiter.check(msg.sender_id)?;

        // Step 4: Extract payload
        let payload = &msg.payload;

        // Step 5: Verify the signature over the challenge
        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&payload.signature[..32]);
        s.copy_from_slice(&payload.signature[32..]);

        let ecdsa_sig = EcdsaSignature { r, s, v: 27 };
        let result = self.service.verify_ecdsa(&payload.challenge, &ecdsa_sig);

        // Try v=28 if v=27 failed
        let final_result = if !result.valid {
            let ecdsa_sig_28 = EcdsaSignature { r, s, v: 28 };
            self.service.verify_ecdsa(&payload.challenge, &ecdsa_sig_28)
        } else {
            result
        };

        Ok(VerifyNodeIdentityResponse {
            valid: final_result.valid,
            reason: if final_result.valid {
                None
            } else {
                Some(format!(
                    "Signature verification failed: {:?}",
                    final_result.error
                ))
            },
        })
    }

    /// Handle a BatchVerifyRequest.
    ///
    /// Reference: IPC-MATRIX.md - Only Consensus (Subsystem 8) can batch verify
    ///
    /// # Security
    /// - ONLY Subsystem 8 (Consensus) is authorized
    /// - Maximum 1000 signatures per batch (DoS protection)
    pub fn handle_batch_verify(
        &self,
        sender_id: u8,
        requests: Vec<VerificationRequest>,
    ) -> Result<BatchVerificationResult, IpcError> {
        // Step 1: Check sender is Consensus ONLY
        check_batch_verify_authorized(sender_id)?;

        // Step 2: Check batch size
        if requests.len() > MAX_BATCH_SIZE {
            return Err(IpcError::BatchTooLarge {
                size: requests.len(),
                max: MAX_BATCH_SIZE,
            });
        }

        // Step 3: Verify batch
        let batch_request = BatchVerificationRequest { requests };
        let result = self.service.batch_verify_ecdsa(&batch_request);

        Ok(result)
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Compute message hash using keccak256.
fn compute_message_hash(message: &[u8]) -> Hash {
    use sha3::{Digest, Keccak256};

    let mut hasher = Keccak256::new();
    hasher.update(message);
    let result = hasher.finalize();

    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authorized_senders() {
        assert!(is_authorized(authorized::PEER_DISCOVERY));
        assert!(is_authorized(authorized::BLOCK_PROPAGATION));
        assert!(is_authorized(authorized::MEMPOOL));
        assert!(is_authorized(authorized::CONSENSUS));
        assert!(is_authorized(authorized::FINALITY));
    }

    #[test]
    fn test_forbidden_senders() {
        assert!(is_forbidden(forbidden::BLOCK_STORAGE));
        assert!(is_forbidden(forbidden::TRANSACTION_INDEXING));
        assert!(is_forbidden(forbidden::STATE_MANAGEMENT));
        assert!(is_forbidden(forbidden::BLOOM_FILTERS));
        assert!(is_forbidden(forbidden::SMART_CONTRACTS));
        assert!(is_forbidden(forbidden::TRANSACTION_ORDERING));
        assert!(is_forbidden(forbidden::LIGHT_CLIENTS));
        assert!(is_forbidden(forbidden::SHARDING));
        assert!(is_forbidden(forbidden::CROSS_CHAIN));
    }

    #[test]
    fn test_check_authorized_sender_success() {
        assert!(check_authorized_sender(authorized::PEER_DISCOVERY).is_ok());
        assert!(check_authorized_sender(authorized::CONSENSUS).is_ok());
    }

    #[test]
    fn test_check_authorized_sender_forbidden() {
        let result = check_authorized_sender(forbidden::BLOCK_STORAGE);
        assert!(matches!(result, Err(IpcError::ForbiddenSender { .. })));
    }

    #[test]
    fn test_check_authorized_sender_unknown() {
        let result = check_authorized_sender(99); // Unknown subsystem
        assert!(matches!(result, Err(IpcError::UnauthorizedSender { .. })));
    }

    #[test]
    fn test_check_node_identity_authorized() {
        assert!(check_node_identity_authorized(authorized::PEER_DISCOVERY).is_ok());
        assert!(check_node_identity_authorized(authorized::CONSENSUS).is_err());
    }

    #[test]
    fn test_check_batch_verify_authorized() {
        assert!(check_batch_verify_authorized(authorized::CONSENSUS).is_ok());
        assert!(check_batch_verify_authorized(authorized::PEER_DISCOVERY).is_err());
    }

    #[test]
    fn test_rate_limiter_default_limits() {
        let limits = RateLimits::default();
        assert_eq!(limits.peer_discovery, 100);
        assert_eq!(limits.internal, 1000);
        assert_eq!(limits.consensus_critical, u64::MAX);
    }

    #[test]
    fn test_rate_limiter_get_limit() {
        let limiter = RateLimiter::new(RateLimits::default());

        assert_eq!(limiter.get_limit(authorized::PEER_DISCOVERY), 100);
        assert_eq!(limiter.get_limit(authorized::BLOCK_PROPAGATION), 1000);
        assert_eq!(limiter.get_limit(authorized::MEMPOOL), 1000);
        assert_eq!(limiter.get_limit(authorized::CONSENSUS), u64::MAX);
        assert_eq!(limiter.get_limit(authorized::FINALITY), u64::MAX);
        assert_eq!(limiter.get_limit(forbidden::BLOCK_STORAGE), 0);
    }
}
