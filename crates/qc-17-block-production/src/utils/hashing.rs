//! Hashing utilities for block production
//!
//! Provides consistent hashing implementations used across the subsystem.
//!
//! ## Performance (BLAKE3 vs SHA-256)
//!
//! | Algorithm | Speed | Use Case |
//! |-----------|-------|----------|
//! | SHA-256 | ~500 MB/s | Ethereum compatibility |
//! | BLAKE3 | ~3000 MB/s | Internal fast hashing |

use primitive_types::{H256, U256};
use sha2::{Digest, Sha256};
use shared_crypto::blake3_hash;

/// Compute SHA-256 hash of data
#[inline]
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut output = [0u8; 32];
    output.copy_from_slice(&result);
    output
}

/// Compute double SHA-256 hash (Bitcoin-style)
///
/// Used for PoW mining to prevent length extension attacks
#[inline]
pub fn sha256d(data: &[u8]) -> [u8; 32] {
    let first_hash = sha256(data);
    sha256(&first_hash)
}

/// Compute BLAKE3 hash (5-10x faster than SHA-256)
///
/// Use for internal hashing where Ethereum compatibility is not required.
#[inline]
pub fn blake3(data: &[u8]) -> [u8; 32] {
    blake3_hash(data)
}

/// Compute double BLAKE3 hash (for PoW when BLAKE3 algorithm is selected)
#[inline]
pub fn blake3d(data: &[u8]) -> [u8; 32] {
    let first_hash = blake3(data);
    blake3(&first_hash)
}

/// Convert hash bytes to H256
#[inline]
pub fn bytes_to_h256(bytes: &[u8; 32]) -> H256 {
    H256::from(*bytes)
}

/// Convert hash bytes to U256 (big-endian)
#[inline]
pub fn bytes_to_u256(bytes: &[u8; 32]) -> U256 {
    U256::from_big_endian(bytes)
}

/// Compute transaction hash
///
/// Returns H256 hash of transaction data
pub fn transaction_hash(tx: &[u8]) -> H256 {
    let hash_bytes = sha256(tx);
    bytes_to_h256(&hash_bytes)
}

/// Serialize block header for hashing
///
/// Creates canonical byte representation for PoW/PoS
pub fn serialize_block_header(
    parent_hash: &H256,
    block_number: u64,
    timestamp: u64,
    beneficiary: &[u8; 20],
    gas_used: u64,
    nonce_opt: Option<u64>,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(128);

    bytes.extend_from_slice(parent_hash.as_bytes());
    bytes.extend_from_slice(&block_number.to_le_bytes());
    bytes.extend_from_slice(&timestamp.to_le_bytes());
    bytes.extend_from_slice(beneficiary);
    bytes.extend_from_slice(&gas_used.to_le_bytes());

    if let Some(nonce) = nonce_opt {
        bytes.extend_from_slice(&nonce.to_le_bytes());
    }

    bytes
}

/// Check if hash meets difficulty target
///
/// Returns true if hash <= target (more leading zeros = harder)
#[inline]
pub fn meets_difficulty(hash: &[u8; 32], target: U256) -> bool {
    bytes_to_u256(hash) <= target
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_deterministic() {
        let data = b"hello world";
        let hash1 = sha256(data);
        let hash2 = sha256(data);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_sha256d_double_hash() {
        let data = b"test";
        let hash1 = sha256(data);
        let hash2 = sha256(&hash1);
        let double = sha256d(data);
        assert_eq!(hash2, double);
    }

    #[test]
    fn test_transaction_hash() {
        let tx = vec![1, 2, 3, 4];
        let hash = transaction_hash(&tx);
        assert_ne!(hash, H256::zero());
    }

    #[test]
    fn test_serialize_block_header() {
        let parent = H256::zero();
        let serialized = serialize_block_header(&parent, 1, 1000, &[0u8; 20], 21000, Some(12345));

        // Should include all fields
        assert!(serialized.len() > 32); // At least parent hash
    }

    #[test]
    fn test_meets_difficulty() {
        // Hash with all zeros meets any difficulty
        let easy_hash = [0u8; 32];
        assert!(meets_difficulty(&easy_hash, U256::MAX));

        // Hash with all ones only meets MAX difficulty
        let hard_hash = [0xFFu8; 32];
        assert!(meets_difficulty(&hard_hash, U256::MAX));
        assert!(!meets_difficulty(&hard_hash, U256::from(1)));
    }

    #[test]
    fn test_blake3_deterministic() {
        let data = b"hello world";
        let hash1 = blake3(data);
        let hash2 = blake3(data);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_blake3d_double_hash() {
        let data = b"test";
        let hash1 = blake3(data);
        let hash2 = blake3(&hash1);
        let double = blake3d(data);
        assert_eq!(hash2, double);
    }

    #[test]
    fn test_blake3_differs_from_sha256() {
        let data = b"test";
        let b3_hash = blake3(data);
        let sha_hash = sha256(data);
        // BLAKE3 and SHA-256 should produce different outputs
        assert_ne!(b3_hash, sha_hash);
    }
}
