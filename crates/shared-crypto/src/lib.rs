//! # Shared Crypto - Advanced Cryptographic Primitives
//!
//! **Status:** Phase 1 Implementation
//!
//! ## Components
//!
//! | Module | Algorithm | Use Case |
//! |--------|-----------|----------|
//! | `symmetric` | XChaCha20-Poly1305, AES-GCM | Encryption |
//! | `hashing` | BLAKE3 | Fast hashing |
//! | `signatures` | Ed25519 | Digital signatures (future P2P) |
//! | `ecdsa` | secp256k1 | Transaction/Node identity signing |
//!
//! ## Security Properties
//!
//! - **XChaCha20**: 192-bit nonce, constant-time, side-channel immune
//! - **Ed25519**: Deterministic nonces, no RNG dependency
//! - **secp256k1**: RFC 6979 deterministic, low-S normalization (EIP-2)
//! - **BLAKE3**: SIMD-accelerated, 5-10x faster than SHA-256

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod ecdsa;
pub mod errors;
pub mod hashing;
pub mod signatures;
pub mod symmetric;

// Re-exports
pub use ecdsa::{Secp256k1KeyPair, Secp256k1PublicKey, Secp256k1Signature};
pub use errors::CryptoError;
pub use hashing::{blake3_hash, Blake3Hasher};
pub use signatures::{Ed25519KeyPair, Ed25519PublicKey, Ed25519Signature};
pub use symmetric::{decrypt, encrypt, Cipher, Nonce, SecretKey};

/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn test_version() {
        assert!(!super::VERSION.is_empty());
    }
}
