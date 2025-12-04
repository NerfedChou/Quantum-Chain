//! # Signature Errors
//!
//! Error types for signature verification operations.
//!
//! Reference: SPEC-10 Section 6 (Error Handling)

use thiserror::Error;

/// Errors that can occur during signature verification.
///
/// Reference: SPEC-10 Section 6
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum SignatureError {
    /// The signature format is invalid (wrong length, invalid encoding)
    #[error("Invalid signature format")]
    InvalidFormat,

    /// Signature verification failed (signature doesn't match message/signer)
    #[error("Signature verification failed")]
    VerificationFailed,

    /// Signature has high S value (EIP-2 malleability protection)
    ///
    /// Reference: SPEC-10 Section 2.2, Invariant 3
    #[error("Malleable signature (high S value)")]
    MalleableSignature,

    /// Invalid recovery ID (v must be 0, 1, 27, or 28)
    #[error("Invalid recovery ID: {0}")]
    InvalidRecoveryId(u8),

    /// Failed to recover public key from signature
    #[error("Failed to recover public key")]
    RecoveryFailed,

    /// BLS pairing check failed
    #[error("BLS pairing check failed")]
    BlsPairingFailed,

    /// Cannot aggregate an empty list of signatures
    #[error("Cannot aggregate empty signature list")]
    EmptyAggregation,

    /// Recovered signer does not match expected signer
    #[error("Signer mismatch: expected {expected:?}, got {actual:?}")]
    SignerMismatch {
        expected: [u8; 20],
        actual: [u8; 20],
    },

    /// Failed to submit verified transaction to mempool
    #[error("Submission to mempool failed: {0}")]
    SubmissionFailed(String),
}
