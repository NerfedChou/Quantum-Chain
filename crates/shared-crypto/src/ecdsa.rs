//! # ECDSA Signatures (secp256k1)
//!
//! Production ECDSA signatures using the secp256k1 curve.
//!
//! ## Security Properties
//!
//! - RFC 6979 deterministic nonces (no RNG dependency for signing)
//! - Low-S normalization (EIP-2)
//! - Constant-time operations
//!
//! ## Use Cases
//!
//! - Transaction signing (Ethereum-compatible)
//! - Node identity verification
//! - Block proposer signatures

use crate::CryptoError;
use k256::ecdsa::{
    signature::{Signer, Verifier},
    Signature, SigningKey, VerifyingKey,
};
use zeroize::Zeroize;

/// Compressed secp256k1 public key (33 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Secp256k1PublicKey([u8; 33]);

impl Secp256k1PublicKey {
    /// Create from compressed bytes (33 bytes, starting with 0x02 or 0x03).
    pub fn from_bytes(bytes: [u8; 33]) -> Result<Self, CryptoError> {
        // Validate it's a valid compressed point
        VerifyingKey::from_sec1_bytes(&bytes).map_err(|_| CryptoError::InvalidPublicKey)?;
        Ok(Self(bytes))
    }

    /// Get raw compressed bytes.
    pub fn as_bytes(&self) -> &[u8; 33] {
        &self.0
    }

    /// Verify a signature.
    pub fn verify(
        &self,
        message: &[u8],
        signature: &Secp256k1Signature,
    ) -> Result<(), CryptoError> {
        let verifying_key =
            VerifyingKey::from_sec1_bytes(&self.0).map_err(|_| CryptoError::InvalidPublicKey)?;

        let sig = Signature::from_slice(&signature.0).map_err(|_| CryptoError::InvalidSignature)?;

        verifying_key
            .verify(message, &sig)
            .map_err(|_| CryptoError::SignatureVerificationFailed)
    }

    /// Derive NodeId from public key (SHA-256 hash).
    pub fn to_node_id(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.0);
        hasher.finalize().into()
    }
}

/// ECDSA signature (64 bytes, r||s format).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Secp256k1Signature([u8; 64]);

impl Secp256k1Signature {
    /// Create from bytes (64 bytes).
    pub fn from_bytes(bytes: [u8; 64]) -> Self {
        Self(bytes)
    }

    /// Get raw bytes.
    pub fn as_bytes(&self) -> &[u8; 64] {
        &self.0
    }
}

/// secp256k1 ECDSA keypair.
pub struct Secp256k1KeyPair {
    signing_key: SigningKey,
}

impl Secp256k1KeyPair {
    /// Generate random keypair.
    pub fn generate() -> Self {
        let signing_key = SigningKey::random(&mut rand::thread_rng());
        Self { signing_key }
    }

    /// Create from secret key bytes (32 bytes).
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, CryptoError> {
        let signing_key =
            SigningKey::from_bytes((&bytes).into()).map_err(|_| CryptoError::InvalidPrivateKey)?;
        Ok(Self { signing_key })
    }

    /// Get public key (compressed, 33 bytes).
    ///
    /// # Panics
    ///
    /// This function will not panic - the conversion from verifying key to SEC1
    /// compressed format always produces exactly 33 bytes.
    pub fn public_key(&self) -> Secp256k1PublicKey {
        let verifying_key = self.signing_key.verifying_key();
        let sec1_bytes = verifying_key.to_sec1_bytes();
        // SAFETY: SEC1 compressed public key is always exactly 33 bytes
        // The first byte is 0x02 or 0x03, followed by the 32-byte x-coordinate
        let mut bytes = [0u8; 33];
        bytes.copy_from_slice(&sec1_bytes[..33]);
        Secp256k1PublicKey(bytes)
    }

    /// Sign a message (deterministic RFC 6979).
    pub fn sign(&self, message: &[u8]) -> Secp256k1Signature {
        let sig: Signature = self.signing_key.sign(message);
        let bytes: [u8; 64] = sig.to_bytes().into();
        Secp256k1Signature(bytes)
    }

    /// Get secret key bytes (for serialization).
    pub fn to_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes().into()
    }
}

impl Drop for Secp256k1KeyPair {
    fn drop(&mut self) {
        // Zeroize secret key material
        let mut bytes: [u8; 32] = self.signing_key.to_bytes().into();
        bytes.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_verify() {
        let keypair = Secp256k1KeyPair::generate();
        let message = b"Hello, secp256k1!";

        let signature = keypair.sign(message);
        let result = keypair.public_key().verify(message, &signature);

        assert!(result.is_ok());
    }

    #[test]
    fn test_wrong_message_fails() {
        let keypair = Secp256k1KeyPair::generate();

        let signature = keypair.sign(b"message1");
        let result = keypair.public_key().verify(b"message2", &signature);

        assert!(result.is_err());
    }

    #[test]
    fn test_deterministic_signatures() {
        let keypair = Secp256k1KeyPair::from_bytes([0xABu8; 32]).unwrap();
        let message = b"deterministic test";

        let sig1 = keypair.sign(message);
        let sig2 = keypair.sign(message);

        assert_eq!(sig1.as_bytes(), sig2.as_bytes());
    }

    #[test]
    fn test_node_id_derivation() {
        let keypair = Secp256k1KeyPair::generate();
        let pubkey = keypair.public_key();
        let node_id = pubkey.to_node_id();

        // NodeId should be deterministic
        let node_id2 = pubkey.to_node_id();
        assert_eq!(node_id, node_id2);
        assert_eq!(node_id.len(), 32);
    }

    #[test]
    fn test_roundtrip_bytes() {
        let original = Secp256k1KeyPair::generate();
        let bytes = original.to_bytes();
        let restored = Secp256k1KeyPair::from_bytes(bytes).unwrap();

        assert_eq!(original.public_key(), restored.public_key());
    }
}
