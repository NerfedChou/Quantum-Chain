//! Crypto error types.

use thiserror::Error;

/// Cryptographic operation errors.
#[derive(Debug, Error)]
pub enum CryptoError {
    /// Encryption failed
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    /// Decryption failed
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    /// Invalid key length
    #[error("Invalid key length: expected {expected}, got {actual}")]
    InvalidKeyLength {
        /// Expected key length in bytes
        expected: usize,
        /// Actual key length in bytes
        actual: usize,
    },

    /// Invalid nonce length
    #[error("Invalid nonce length: expected {expected}, got {actual}")]
    InvalidNonceLength {
        /// Expected nonce length in bytes
        expected: usize,
        /// Actual nonce length in bytes
        actual: usize,
    },

    /// Signature verification failed
    #[error("Signature verification failed")]
    SignatureVerificationFailed,

    /// Invalid signature format
    #[error("Invalid signature format")]
    InvalidSignatureFormat,

    /// Invalid public key
    #[error("Invalid public key")]
    InvalidPublicKey,

    /// Invalid private key
    #[error("Invalid private key")]
    InvalidPrivateKey,

    /// Invalid signature
    #[error("Invalid signature")]
    InvalidSignature,

    /// Key generation failed
    #[error("Key generation failed: {0}")]
    KeyGenerationFailed(String),

    /// BLS aggregation failed
    #[error("BLS aggregation failed")]
    AggregationFailed,

    /// Invalid input for cryptographic operation
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}
