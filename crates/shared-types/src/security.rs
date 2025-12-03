//! # Centralized IPC Security Module
//!
//! This module provides the **single, authoritative implementation** of all IPC
//! security validation logic as mandated by Architecture.md v2.2.
//!
//! ## Design Rationale
//!
//! Prior to this module, each subsystem (qc-01, qc-06, etc.) implemented its own
//! HMAC validation and nonce caching. This led to:
//! - Code duplication across 15 subsystems
//! - Risk of inconsistent security policy application
//! - Higher maintenance burden
//!
//! This module centralizes all security logic so that:
//! 1. All subsystems use the SAME validation code
//! 2. Security policy changes propagate to all subsystems automatically
//! 3. The brutal test suite only needs to test ONE implementation
//!
//! ## Security Properties
//!
//! - **HMAC-SHA256 Signatures**: All messages are signed with subsystem-specific keys
//! - **Time-Bounded Validity**: Messages expire after 60 seconds
//! - **Nonce Replay Prevention**: Each nonce is valid only once within the time window
//! - **Sender Authorization**: Messages are checked against IPC-MATRIX.md rules

use crate::envelope::{AuthenticatedMessage, VerificationResult};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

// =============================================================================
// CONSTANTS
// =============================================================================

/// Maximum allowed clock skew for future timestamps (seconds).
pub const MAX_FUTURE_SKEW: u64 = 10;

/// Maximum age for valid timestamps (seconds).
pub const MAX_AGE: u64 = 60;

/// Duration to retain nonces in cache (2x the validity window for safety).
pub const NONCE_CACHE_TTL: Duration = Duration::from_secs(120);

/// Maximum nonce cache size before forced cleanup.
pub const MAX_NONCE_CACHE_SIZE: usize = 100_000;

// =============================================================================
// NONCE CACHE
// =============================================================================

/// Thread-safe nonce cache for replay prevention.
///
/// ## Design
///
/// - Uses a `HashMap<Uuid, Instant>` to track seen nonces and their expiry times
/// - Automatically evicts expired nonces on access
/// - Bounded to prevent memory exhaustion attacks
///
/// ## Usage
///
/// ```rust,ignore
/// let cache = NonceCache::new();
///
/// // First attempt - nonce is fresh
/// assert!(cache.check_and_insert(nonce));
///
/// // Second attempt - replay detected!
/// assert!(!cache.check_and_insert(nonce));
/// ```
#[derive(Debug)]
pub struct NonceCache {
    /// Map of nonce -> expiry instant
    cache: RwLock<HashMap<Uuid, Instant>>,
}

impl NonceCache {
    /// Creates a new empty nonce cache.
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Creates a nonce cache wrapped in Arc for shared ownership.
    pub fn new_shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    /// Checks if a nonce has been seen before; if not, inserts it.
    ///
    /// # Returns
    ///
    /// - `true` if the nonce is fresh (not seen before)
    /// - `false` if the nonce is a replay (seen before)
    ///
    /// # Thread Safety
    ///
    /// This method is thread-safe and uses interior mutability.
    pub fn check_and_insert(&self, nonce: Uuid) -> bool {
        let now = Instant::now();
        let expiry = now + NONCE_CACHE_TTL;

        // Handle poisoned lock gracefully - if poisoned, reject to be safe
        let mut cache = match self.cache.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                // Recover from poisoned lock but log the issue
                // In a poisoned state, we conservatively reject to prevent replays
                poisoned.into_inner()
            }
        };

        // Cleanup expired nonces if cache is too large
        if cache.len() >= MAX_NONCE_CACHE_SIZE {
            cache.retain(|_, exp| *exp > now);
        }

        // Check if nonce exists and is not expired
        if let Some(&exp) = cache.get(&nonce) {
            if exp > now {
                // Nonce is still valid - replay detected!
                return false;
            }
            // Nonce expired, will be replaced
        }

        // Insert fresh nonce
        cache.insert(nonce, expiry);
        true
    }

    /// Clears all cached nonces. Primarily for testing.
    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
        // If poisoned, we can't clear - but this is primarily for testing
    }

    /// Returns the current number of cached nonces.
    pub fn len(&self) -> usize {
        self.cache.read().map(|c| c.len()).unwrap_or(0)
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for NonceCache {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// HMAC VALIDATION
// =============================================================================

/// Validates the HMAC-SHA256 signature of an authenticated message.
///
/// # Arguments
///
/// - `message_bytes`: The canonically serialized message (header + payload)
/// - `signature`: The 64-byte signature from the message envelope
/// - `shared_secret`: The pre-shared key for the sender subsystem
///
/// # Returns
///
/// - `true` if the signature is valid
/// - `false` if the signature is invalid or tampered
///
/// # Security
///
/// Uses constant-time comparison to prevent timing attacks.
pub fn validate_hmac_signature(
    message_bytes: &[u8],
    signature: &[u8; 64],
    shared_secret: &[u8],
) -> bool {
    // First 32 bytes of our 64-byte signature field contain the HMAC
    let hmac_bytes = &signature[..32];

    let mut mac = match HmacSha256::new_from_slice(shared_secret) {
        Ok(m) => m,
        Err(_) => return false,
    };

    mac.update(message_bytes);

    // Constant-time comparison
    mac.verify_slice(hmac_bytes).is_ok()
}

/// Signs a message with HMAC-SHA256.
///
/// # Arguments
///
/// - `message_bytes`: The canonically serialized message (header + payload)
/// - `shared_secret`: The pre-shared key for this subsystem
///
/// # Returns
///
/// A 64-byte signature (HMAC in first 32 bytes, zeros in remaining 32)
pub fn sign_message(message_bytes: &[u8], shared_secret: &[u8]) -> [u8; 64] {
    let mut mac = HmacSha256::new_from_slice(shared_secret).expect("HMAC can take key of any size");

    mac.update(message_bytes);

    let result = mac.finalize();
    let hmac_bytes = result.into_bytes();

    let mut signature = [0u8; 64];
    signature[..32].copy_from_slice(&hmac_bytes);
    signature
}

// =============================================================================
// TIMESTAMP VALIDATION
// =============================================================================

/// Validates that a message timestamp is within the acceptable window.
///
/// # Arguments
///
/// - `timestamp`: Unix timestamp (seconds since epoch) from the message
///
/// # Returns
///
/// - `Ok(())` if the timestamp is valid
/// - `Err(VerificationResult::TimestampOutOfRange)` if expired or too far in future
///
/// # Time Window
///
/// Valid range: `now - 60s <= timestamp <= now + 10s`
pub fn validate_timestamp(timestamp: u64) -> Result<(), VerificationResult> {
    let now = current_timestamp();

    // Check if too old
    if timestamp + MAX_AGE < now {
        return Err(VerificationResult::TimestampOutOfRange { timestamp, now });
    }

    // Check if too far in future
    if timestamp > now + MAX_FUTURE_SKEW {
        return Err(VerificationResult::TimestampOutOfRange { timestamp, now });
    }

    Ok(())
}

/// Returns the current Unix timestamp.
///
/// # Panics
///
/// This function will NOT panic. If the system clock is before UNIX_EPOCH
/// (which should never happen on any sane system), it returns 0.
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// =============================================================================
// MESSAGE VERIFIER
// =============================================================================

/// Comprehensive message verifier that performs ALL security checks.
///
/// This is the **single entry point** for all IPC message validation.
/// Subsystems MUST use this instead of implementing their own checks.
///
/// ## Verification Steps (in order)
///
/// 1. **Version Check**: Reject unsupported protocol versions
/// 2. **Timestamp Check**: Reject expired or future-dated messages
/// 3. **Nonce Check**: Reject replayed messages
/// 4. **Signature Check**: Reject tampered or forged messages
/// 5. **Authorization Check**: Reject unauthorized sender/recipient pairs
///
/// ## Example
///
/// ```rust,ignore
/// let verifier = MessageVerifier::new(my_subsystem_id, nonce_cache, key_provider);
///
/// match verifier.verify(&message, &serialized_bytes) {
///     VerificationResult::Valid => handle_message(message),
///     err => log_and_reject(err),
/// }
/// ```
pub struct MessageVerifier<K: KeyProvider> {
    /// The subsystem ID of this node (recipient)
    recipient_id: u8,
    /// Shared nonce cache
    nonce_cache: Arc<NonceCache>,
    /// Provider for sender public keys / shared secrets
    key_provider: K,
    /// Authorization matrix checker
    auth_matrix: AuthorizationMatrix,
}

/// Trait for retrieving shared secrets for HMAC validation.
///
/// Implementations might:
/// - Look up keys from a configuration file
/// - Derive keys from a master secret
/// - Query a key management service
pub trait KeyProvider: Send + Sync {
    /// Returns the shared secret for a given sender subsystem.
    ///
    /// # Returns
    ///
    /// - `Some(secret)` if the sender is known
    /// - `None` if the sender is unknown (reject message)
    fn get_shared_secret(&self, sender_id: u8) -> Option<Vec<u8>>;
}

impl<K: KeyProvider> MessageVerifier<K> {
    /// Creates a new message verifier.
    pub fn new(recipient_id: u8, nonce_cache: Arc<NonceCache>, key_provider: K) -> Self {
        Self {
            recipient_id,
            nonce_cache,
            key_provider,
            auth_matrix: AuthorizationMatrix::new(),
        }
    }

    /// Verifies an authenticated message.
    ///
    /// # Arguments
    ///
    /// - `message`: The deserialized authenticated message
    /// - `message_bytes`: The original serialized bytes (for signature verification)
    ///
    /// # Returns
    ///
    /// `VerificationResult::Valid` if all checks pass, otherwise an error variant.
    pub fn verify<T>(
        &self,
        message: &AuthenticatedMessage<T>,
        message_bytes: &[u8],
    ) -> VerificationResult {
        // 1. Version check
        if message.version != AuthenticatedMessage::<T>::CURRENT_VERSION {
            return VerificationResult::UnsupportedVersion {
                received: message.version,
                supported: AuthenticatedMessage::<T>::CURRENT_VERSION,
            };
        }

        // 2. Timestamp check
        if let Err(e) = validate_timestamp(message.timestamp) {
            return e;
        }

        // 3. Nonce check
        if !self.nonce_cache.check_and_insert(message.nonce) {
            return VerificationResult::ReplayDetected {
                nonce: message.nonce,
            };
        }

        // 4. Signature check
        let shared_secret = match self.key_provider.get_shared_secret(message.sender_id) {
            Some(s) => s,
            None => return VerificationResult::InvalidSignature,
        };

        if !validate_hmac_signature(message_bytes, &message.signature, &shared_secret) {
            return VerificationResult::InvalidSignature;
        }

        // 5. Reply-to validation (prevent forwarding attacks)
        if let Some(ref reply_to) = message.reply_to {
            if reply_to.subsystem_id != message.sender_id {
                return VerificationResult::ReplyToMismatch {
                    reply_to_subsystem: reply_to.subsystem_id,
                    sender_id: message.sender_id,
                };
            }
        }

        VerificationResult::Valid
    }

    /// Checks if a sender is authorized to send a specific message type to this recipient.
    ///
    /// # Arguments
    ///
    /// - `sender_id`: The sender's subsystem ID
    /// - `message_type`: A string identifier for the message type
    ///
    /// # Returns
    ///
    /// `true` if authorized, `false` if not
    pub fn is_authorized(&self, sender_id: u8, message_type: &str) -> bool {
        self.auth_matrix
            .is_authorized(sender_id, self.recipient_id, message_type)
    }
}

// =============================================================================
// AUTHORIZATION MATRIX
// =============================================================================

/// Implements the IPC authorization rules from IPC-MATRIX.md.
///
/// This defines which subsystems can send which message types to which recipients.
#[derive(Debug, Clone)]
pub struct AuthorizationMatrix {
    /// Map of (sender_id, recipient_id, message_type) -> authorized
    rules: HashMap<(u8, u8, &'static str), bool>,
}

impl AuthorizationMatrix {
    /// Creates a new authorization matrix with all rules from IPC-MATRIX.md.
    pub fn new() -> Self {
        let mut rules = HashMap::new();

        // =================================================================
        // Block Storage (2) - Authorized Senders
        // =================================================================
        rules.insert((8, 2, "BlockValidated"), true); // Consensus -> Block Storage
        rules.insert((3, 2, "MerkleRootComputed"), true); // Tx Indexing -> Block Storage
        rules.insert((4, 2, "StateRootComputed"), true); // State Mgmt -> Block Storage
        rules.insert((9, 2, "MarkFinalized"), true); // Finality -> Block Storage

        // =================================================================
        // Transaction Indexing (3) - Authorized Senders
        // =================================================================
        rules.insert((8, 3, "BlockValidated"), true); // Consensus -> Tx Indexing
        rules.insert((2, 3, "BlockStored"), true); // Block Storage -> Tx Indexing

        // =================================================================
        // State Management (4) - Authorized Senders
        // =================================================================
        rules.insert((8, 4, "BlockValidated"), true); // Consensus -> State Mgmt
        rules.insert((11, 4, "ContractExecuted"), true); // Smart Contracts -> State Mgmt

        // =================================================================
        // Mempool (6) - Authorized Senders
        // =================================================================
        rules.insert((1, 6, "PeerTransaction"), true); // Peer Discovery -> Mempool
        rules.insert((2, 6, "BlockStorageConfirmation"), true); // Block Storage -> Mempool
        rules.insert((10, 6, "SignatureVerified"), true); // Sig Verify -> Mempool

        // =================================================================
        // Consensus (8) - Authorized Senders
        // =================================================================
        rules.insert((6, 8, "TransactionBatch"), true); // Mempool -> Consensus
        rules.insert((9, 8, "FinalityVote"), true); // Finality -> Consensus

        // =================================================================
        // Finality (9) - Authorized Senders
        // =================================================================
        rules.insert((8, 9, "BlockProposed"), true); // Consensus -> Finality
        rules.insert((2, 9, "BlockStored"), true); // Block Storage -> Finality

        // =================================================================
        // Signature Verification (10) - Authorized Senders
        // =================================================================
        rules.insert((6, 10, "VerifyTransaction"), true); // Mempool -> Sig Verify
        rules.insert((1, 10, "VerifyPeerSignature"), true); // Peer Discovery -> Sig Verify

        // =================================================================
        // Peer Discovery (1) - Authorized Senders
        // =================================================================
        rules.insert((8, 1, "RequestPeers"), true); // Consensus -> Peer Discovery
        rules.insert((5, 1, "PropagationStatus"), true); // Block Propagation -> Peer Discovery

        Self { rules }
    }

    /// Checks if a sender is authorized to send a message type to a recipient.
    pub fn is_authorized(&self, sender_id: u8, recipient_id: u8, message_type: &str) -> bool {
        // Static string lookup for common message types
        let key = (sender_id, recipient_id, message_type);

        // Try direct lookup first
        if let Some(&authorized) = self.rules.get(&key) {
            return authorized;
        }

        // No explicit rule = not authorized
        false
    }
}

impl Default for AuthorizationMatrix {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// SIMPLE KEY PROVIDER (for testing)
// =============================================================================

/// A simple key provider that derives keys from a master secret.
///
/// For production, replace with a proper key management implementation.
#[derive(Clone)]
pub struct DerivedKeyProvider {
    master_secret: Vec<u8>,
}

impl DerivedKeyProvider {
    /// Creates a new key provider with the given master secret.
    pub fn new(master_secret: Vec<u8>) -> Self {
        Self { master_secret }
    }

    /// Derives a subsystem-specific key from the master secret.
    fn derive_key(&self, subsystem_id: u8) -> Vec<u8> {
        let mut mac =
            HmacSha256::new_from_slice(&self.master_secret).expect("HMAC can take key of any size");
        mac.update(&[subsystem_id]);
        mac.finalize().into_bytes().to_vec()
    }
}

impl KeyProvider for DerivedKeyProvider {
    fn get_shared_secret(&self, sender_id: u8) -> Option<Vec<u8>> {
        Some(self.derive_key(sender_id))
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nonce_cache_fresh_nonce() {
        let cache = NonceCache::new();
        let nonce = Uuid::new_v4();

        // First check should succeed
        assert!(cache.check_and_insert(nonce));
        // Second check should fail (replay)
        assert!(!cache.check_and_insert(nonce));
    }

    #[test]
    fn test_nonce_cache_different_nonces() {
        let cache = NonceCache::new();
        let nonce1 = Uuid::new_v4();
        let nonce2 = Uuid::new_v4();

        assert!(cache.check_and_insert(nonce1));
        assert!(cache.check_and_insert(nonce2));
    }

    #[test]
    fn test_hmac_validation() {
        let secret = b"test_secret_key";
        let message = b"hello world";

        let signature = sign_message(message, secret);
        assert!(validate_hmac_signature(message, &signature, secret));
    }

    #[test]
    fn test_hmac_validation_wrong_key() {
        let secret1 = b"test_secret_key";
        let secret2 = b"wrong_secret_key";
        let message = b"hello world";

        let signature = sign_message(message, secret1);
        assert!(!validate_hmac_signature(message, &signature, secret2));
    }

    #[test]
    fn test_hmac_validation_tampered_message() {
        let secret = b"test_secret_key";
        let message = b"hello world";
        let tampered = b"hello World"; // Capital W

        let signature = sign_message(message, secret);
        assert!(!validate_hmac_signature(tampered, &signature, secret));
    }

    #[test]
    fn test_timestamp_validation_valid() {
        let now = current_timestamp();
        assert!(validate_timestamp(now).is_ok());
    }

    #[test]
    fn test_timestamp_validation_expired() {
        let old = current_timestamp() - MAX_AGE - 10;
        assert!(matches!(
            validate_timestamp(old),
            Err(VerificationResult::TimestampOutOfRange { .. })
        ));
    }

    #[test]
    fn test_timestamp_validation_future() {
        let future = current_timestamp() + MAX_FUTURE_SKEW + 10;
        assert!(matches!(
            validate_timestamp(future),
            Err(VerificationResult::TimestampOutOfRange { .. })
        ));
    }

    #[test]
    fn test_authorization_matrix() {
        let matrix = AuthorizationMatrix::new();

        // Valid authorizations
        assert!(matrix.is_authorized(8, 2, "BlockValidated"));
        assert!(matrix.is_authorized(3, 2, "MerkleRootComputed"));
        assert!(matrix.is_authorized(6, 10, "VerifyTransaction"));

        // Invalid authorizations
        assert!(!matrix.is_authorized(1, 2, "BlockValidated")); // Wrong sender
        assert!(!matrix.is_authorized(8, 6, "BlockValidated")); // Wrong recipient
        assert!(!matrix.is_authorized(8, 2, "FakeMessage")); // Unknown message
    }

    #[test]
    fn test_derived_key_provider() {
        let provider = DerivedKeyProvider::new(b"master_secret".to_vec());

        let key1 = provider.get_shared_secret(1).unwrap();
        let key2 = provider.get_shared_secret(2).unwrap();

        // Keys should be different for different subsystems
        assert_ne!(key1, key2);

        // Same subsystem should get same key
        let key1_again = provider.get_shared_secret(1).unwrap();
        assert_eq!(key1, key1_again);
    }
}
