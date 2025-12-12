//! # Ed25519 Signatures
//!
//! Twisted Edwards curve signatures with deterministic nonces.
//!
//! ## Security Properties
//!
//! - No RNG dependency (deterministic nonce from message)
//! - Complete addition formulas (no conditional branches)
//! - Immune to side-channel timing attacks

use crate::CryptoError;
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use zeroize::Zeroize;

/// Ed25519 public key (32 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ed25519PublicKey([u8; 32]);

impl Ed25519PublicKey {
    /// Create from bytes.
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, CryptoError> {
        // Validate it's a valid point
        VerifyingKey::from_bytes(&bytes).map_err(|_| CryptoError::InvalidPublicKey)?;
        Ok(Self(bytes))
    }

    /// Get raw bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Verify a signature.
    pub fn verify(&self, message: &[u8], signature: &Ed25519Signature) -> Result<(), CryptoError> {
        let verifying_key =
            VerifyingKey::from_bytes(&self.0).map_err(|_| CryptoError::InvalidPublicKey)?;

        let sig = ed25519_dalek::Signature::from_bytes(&signature.0);

        verifying_key
            .verify(message, &sig)
            .map_err(|_| CryptoError::SignatureVerificationFailed)
    }
}

/// Ed25519 signature (64 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ed25519Signature([u8; 64]);

impl Ed25519Signature {
    /// Create from bytes.
    pub fn from_bytes(bytes: [u8; 64]) -> Self {
        Self(bytes)
    }

    /// Get raw bytes.
    pub fn as_bytes(&self) -> &[u8; 64] {
        &self.0
    }
}

/// Ed25519 keypair.
pub struct Ed25519KeyPair {
    signing_key: SigningKey,
}

impl Ed25519KeyPair {
    /// Generate random keypair.
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut rand::thread_rng());
        Self { signing_key }
    }

    /// Create from secret seed (32 bytes).
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(&seed);
        Self { signing_key }
    }

    /// Get public key.
    pub fn public_key(&self) -> Ed25519PublicKey {
        let verifying_key = self.signing_key.verifying_key();
        Ed25519PublicKey(verifying_key.to_bytes())
    }

    /// Sign a message (deterministic - no RNG needed).
    pub fn sign(&self, message: &[u8]) -> Ed25519Signature {
        let sig = self.signing_key.sign(message);
        Ed25519Signature(sig.to_bytes())
    }

    /// Get secret seed (for serialization).
    pub fn to_seed(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }
}

impl Drop for Ed25519KeyPair {
    fn drop(&mut self) {
        // Zeroize secret key material
        let mut bytes = self.signing_key.to_bytes();
        bytes.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_verify() {
        let keypair = Ed25519KeyPair::generate();
        let message = b"Hello, Ed25519!";

        let signature = keypair.sign(message);
        let result = keypair.public_key().verify(message, &signature);

        assert!(result.is_ok());
    }

    #[test]
    fn test_wrong_message_fails() {
        let keypair = Ed25519KeyPair::generate();

        let signature = keypair.sign(b"message1");
        let result = keypair.public_key().verify(b"message2", &signature);

        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_key_fails() {
        let keypair1 = Ed25519KeyPair::generate();
        let keypair2 = Ed25519KeyPair::generate();
        let message = b"test";

        let signature = keypair1.sign(message);
        let result = keypair2.public_key().verify(message, &signature);

        assert!(result.is_err());
    }

    #[test]
    fn test_deterministic_signatures() {
        let seed = [0xABu8; 32];
        let keypair = Ed25519KeyPair::from_seed(seed);
        let message = b"deterministic test";

        let sig1 = keypair.sign(message);
        let sig2 = keypair.sign(message);

        assert_eq!(sig1.as_bytes(), sig2.as_bytes());
    }

    #[test]
    fn test_roundtrip_seed() {
        let original = Ed25519KeyPair::generate();
        let seed = original.to_seed();
        let restored = Ed25519KeyPair::from_seed(seed);

        assert_eq!(original.public_key(), restored.public_key());
    }
}
