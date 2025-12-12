//! # Symmetric Encryption
//!
//! Provides XChaCha20-Poly1305 (default) and AES-GCM encryption.
//!
//! ## Security Properties
//!
//! - **XChaCha20-Poly1305**: 192-bit nonce, constant-time ARX design
//! - **AES-GCM**: Use only with AES-NI hardware acceleration

use crate::CryptoError;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use zeroize::Zeroize;

/// Secret key (256-bit).
#[derive(Clone, Zeroize)]
#[zeroize(drop)]
pub struct SecretKey([u8; 32]);

impl SecretKey {
    /// Create from bytes.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Generate random key.
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
        Self(bytes)
    }

    /// Get inner bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Nonce for encryption.
#[derive(Clone)]
pub struct Nonce([u8; 24]); // XChaCha20 uses 24-byte nonce

impl Nonce {
    /// Create from bytes.
    pub fn from_bytes(bytes: [u8; 24]) -> Self {
        Self(bytes)
    }

    /// Generate random nonce (safe with XChaCha20's 192-bit nonce).
    pub fn generate() -> Self {
        let mut bytes = [0u8; 24];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
        Self(bytes)
    }

    /// Get inner bytes.
    pub fn as_bytes(&self) -> &[u8; 24] {
        &self.0
    }
}

/// Cipher selection.
#[derive(Clone, Copy, Debug, Default)]
pub enum Cipher {
    /// XChaCha20-Poly1305 (default, side-channel immune)
    #[default]
    XChaCha20Poly1305,
    /// AES-256-GCM (use with AES-NI only)
    Aes256Gcm,
}

/// Encrypt plaintext with XChaCha20-Poly1305.
///
/// Returns (ciphertext, nonce).
///
/// # Errors
///
/// Returns `CryptoError::EncryptionFailed` if encryption fails.
pub fn encrypt(key: &SecretKey, plaintext: &[u8]) -> Result<(Vec<u8>, Nonce), CryptoError> {
    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());
    let nonce = Nonce::generate();

    let ciphertext = cipher
        .encrypt(XNonce::from_slice(nonce.as_bytes()), plaintext)
        .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

    Ok((ciphertext, nonce))
}

/// Decrypt ciphertext with XChaCha20-Poly1305.
///
/// # Errors
///
/// Returns `CryptoError::DecryptionFailed` if decryption fails.
pub fn decrypt(key: &SecretKey, ciphertext: &[u8], nonce: &Nonce) -> Result<Vec<u8>, CryptoError> {
    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());

    cipher
        .decrypt(XNonce::from_slice(nonce.as_bytes()), ciphertext)
        .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = SecretKey::generate();
        let plaintext = b"Hello, Quantum-Chain!";

        let (ciphertext, nonce) = encrypt(&key, plaintext).unwrap();
        let decrypted = decrypt(&key, &ciphertext, &nonce).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = SecretKey::generate();
        let key2 = SecretKey::generate();
        let plaintext = b"Secret message";

        let (ciphertext, nonce) = encrypt(&key1, plaintext).unwrap();
        let result = decrypt(&key2, &ciphertext, &nonce);

        assert!(result.is_err());
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let key = SecretKey::generate();
        let plaintext = b"Secret message";

        let (mut ciphertext, nonce) = encrypt(&key, plaintext).unwrap();
        ciphertext[0] ^= 0xFF; // Tamper

        let result = decrypt(&key, &ciphertext, &nonce);
        assert!(result.is_err());
    }

    #[test]
    fn test_nonce_uniqueness() {
        let n1 = Nonce::generate();
        let n2 = Nonce::generate();
        assert_ne!(n1.as_bytes(), n2.as_bytes());
    }
}
