//! Cryptographic security for ENR.
//!
//! SECURITY-CRITICAL: This file contains all signing and verification logic.
//! Isolate for security audits.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Compressed secp256k1 public key (33 bytes)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicKey(pub [u8; 33]);

impl PublicKey {
    /// Create from bytes
    pub fn new(bytes: [u8; 33]) -> Self {
        Self(bytes)
    }

    /// Create an empty public key
    pub fn empty() -> Self {
        Self([0u8; 33])
    }

    /// Get as bytes
    pub fn as_bytes(&self) -> &[u8; 33] {
        &self.0
    }
}

/// ECDSA signature (64 bytes: r + s)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature(pub [u8; 64]);

impl Signature {
    /// Create from bytes
    pub fn new(bytes: [u8; 64]) -> Self {
        Self(bytes)
    }

    /// Create an empty signature
    pub fn empty() -> Self {
        Self([0u8; 64])
    }

    /// Get as bytes
    pub fn as_bytes(&self) -> &[u8; 64] {
        &self.0
    }
}

/// Hash function for ENR NodeId derivation.
///
/// # Security Note
/// This is a PLACEHOLDER implementation.
///
/// ## Production Requirements
/// Replace with Keccak256 before mainnet:
/// ```ignore
/// use sha3::{Keccak256, Digest};
/// let hash = Keccak256::digest(data);
/// ```
///
/// ## Why Not Yet
/// Using simplified hash during development to avoid
/// crypto crate dependencies until we finalize the implementation.
pub fn enr_hash(data: &[u8]) -> u32 {
    // TODO(security): Replace with Keccak256 before production
    // This placeholder uses SipHash for reasonable security during development
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish() as u32
}
