//! # Secure Secret Type
//!
//! Wrapper for HTLC secrets that zeroizes memory on drop.
//!
//! ## Security
//!
//! Secrets contain sensitive cryptographic material that should not
//! linger in memory after use. This wrapper ensures the secret is
//! zeroed when dropped, preventing:
//!
//! - Memory dumps from revealing secrets
//! - Cold boot attacks
//! - Core dump exposure

use zeroize::{Zeroize, ZeroizeOnDrop};
use serde::{Deserialize, Serialize};

/// A secure secret that zeroizes on drop.
///
/// # Security
///
/// This type implements `Zeroize` and `ZeroizeOnDrop` to ensure
/// the secret bytes are zeroed when the value is dropped.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecureSecret {
    /// The secret bytes (32 bytes for HTLC).
    inner: [u8; 32],
}

impl SecureSecret {
    /// Create a new secure secret from bytes.
    pub fn new(bytes: [u8; 32]) -> Self {
        Self { inner: bytes }
    }

    /// Create from a slice (copies into fixed array).
    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        if slice.len() != 32 {
            return None;
        }
        let mut inner = [0u8; 32];
        inner.copy_from_slice(slice);
        Some(Self { inner })
    }

    /// Get the secret bytes (use carefully!).
    ///
    /// # Security
    ///
    /// Avoid keeping references to the returned slice.
    /// Use immediately and let go.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.inner
    }

    /// Expose as array for compatibility with existing functions.
    pub fn expose(&self) -> [u8; 32] {
        self.inner
    }
}

impl std::fmt::Debug for SecureSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the actual secret
        f.write_str("SecureSecret(***)")
    }
}

// Serialization that doesn't expose raw bytes in logs
impl Serialize for SecureSecret {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize as base64 or hex, not raw bytes
        serializer.serialize_str(&hex::encode(&self.inner))
    }
}

impl<'de> Deserialize<'de> for SecureSecret {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let bytes = hex::decode(&s).map_err(serde::de::Error::custom)?;
        Self::from_slice(&bytes).ok_or_else(|| serde::de::Error::custom("invalid secret length"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_secret_creation() {
        let secret = SecureSecret::new([0xABu8; 32]);
        assert_eq!(secret.as_bytes()[0], 0xAB);
    }

    #[test]
    fn test_secure_secret_debug_hides_value() {
        let secret = SecureSecret::new([0xABu8; 32]);
        let debug_str = format!("{:?}", secret);
        assert!(!debug_str.contains("AB"));
        assert!(debug_str.contains("***"));
    }

    #[test]
    fn test_secure_secret_from_slice() {
        let bytes = [0xCDu8; 32];
        let secret = SecureSecret::from_slice(&bytes).unwrap();
        assert_eq!(secret.expose(), bytes);
    }

    #[test]
    fn test_secure_secret_from_slice_wrong_length() {
        let bytes = [0xCDu8; 16]; // Wrong size
        assert!(SecureSecret::from_slice(&bytes).is_none());
    }
}
