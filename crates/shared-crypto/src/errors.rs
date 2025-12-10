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
    InvalidKeyLength { expected: usize, actual: usize },

    /// Invalid nonce length
    #[error("Invalid nonce length: expected {expected}, got {actual}")]
    InvalidNonceLength { expected: usize, actual: usize },

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
}
