//! # Secret Generation and Verification
//!
//! Cryptographic operations for HTLC secrets.
//!
//! Reference: System.md Lines 736, 739

use sha2::{Sha256, Digest};
use rand::RngCore;
use crate::domain::{Hash, Secret, CrossChainError};

/// Generate a cryptographically secure random secret.
///
/// Reference: System.md Line 736
pub fn generate_random_secret() -> Secret {
    let mut secret = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut secret);
    secret
}

/// Create a hashlock from a secret using SHA-256.
///
/// Reference: System.md Line 753 - "Use SHA-256, avoid weak hashes"
pub fn create_hash_lock(secret: &Secret) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(secret);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Verify that a secret matches a hashlock.
///
/// Reference: System.md Line 739
pub fn verify_secret(secret: &Secret, hash_lock: &Hash) -> bool {
    let computed_hash = create_hash_lock(secret);
    computed_hash == *hash_lock
}

/// Verify claim is valid.
pub fn verify_claim(
    secret: &Secret,
    hash_lock: &Hash,
    claimer: &[u8; 20],
    authorized_recipient: &[u8; 20],
    current_time: u64,
    time_lock: u64,
) -> Result<(), CrossChainError> {
    // 1. Check not expired
    if current_time > time_lock {
        return Err(CrossChainError::HTLCExpired);
    }

    // 2. Check secret matches hashlock
    if !verify_secret(secret, hash_lock) {
        return Err(CrossChainError::InvalidSecret);
    }

    // 3. Check claimer is authorized
    if claimer != authorized_recipient {
        return Err(CrossChainError::UnauthorizedClaimer);
    }

    Ok(())
}

/// Verify refund is valid.
pub fn verify_refund(current_time: u64, time_lock: u64) -> Result<(), CrossChainError> {
    if current_time <= time_lock {
        return Err(CrossChainError::HTLCNotExpired);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_random_secret() {
        let s1 = generate_random_secret();
        let s2 = generate_random_secret();
        assert_ne!(s1, s2); // Should be different
    }

    #[test]
    fn test_create_hash_lock_deterministic() {
        let secret = [0xABu8; 32];
        let h1 = create_hash_lock(&secret);
        let h2 = create_hash_lock(&secret);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_create_hash_lock_different_secrets() {
        let s1 = [0xABu8; 32];
        let s2 = [0xCDu8; 32];
        assert_ne!(create_hash_lock(&s1), create_hash_lock(&s2));
    }

    #[test]
    fn test_verify_secret_valid() {
        let secret = generate_random_secret();
        let hash_lock = create_hash_lock(&secret);
        assert!(verify_secret(&secret, &hash_lock));
    }

    #[test]
    fn test_verify_secret_invalid() {
        let secret = [0xABu8; 32];
        let wrong_hash = [0xCDu8; 32];
        assert!(!verify_secret(&secret, &wrong_hash));
    }

    #[test]
    fn test_verify_claim_success() {
        let secret = [0xABu8; 32];
        let hash_lock = create_hash_lock(&secret);
        let claimer = [0x11u8; 20];
        let authorized = [0x11u8; 20];

        assert!(verify_claim(&secret, &hash_lock, &claimer, &authorized, 1000, 2000).is_ok());
    }

    #[test]
    fn test_verify_claim_expired() {
        let secret = [0xABu8; 32];
        let hash_lock = create_hash_lock(&secret);
        let claimer = [0x11u8; 20];

        let result = verify_claim(&secret, &hash_lock, &claimer, &claimer, 3000, 2000);
        assert!(matches!(result, Err(CrossChainError::HTLCExpired)));
    }

    #[test]
    fn test_verify_claim_invalid_secret() {
        let secret = [0xABu8; 32];
        let wrong_secret = [0xCDu8; 32];
        let hash_lock = create_hash_lock(&secret);
        let claimer = [0x11u8; 20];

        let result = verify_claim(&wrong_secret, &hash_lock, &claimer, &claimer, 1000, 2000);
        assert!(matches!(result, Err(CrossChainError::InvalidSecret)));
    }

    #[test]
    fn test_verify_claim_unauthorized() {
        let secret = [0xABu8; 32];
        let hash_lock = create_hash_lock(&secret);
        let claimer = [0x11u8; 20];
        let authorized = [0x22u8; 20];

        let result = verify_claim(&secret, &hash_lock, &claimer, &authorized, 1000, 2000);
        assert!(matches!(result, Err(CrossChainError::UnauthorizedClaimer)));
    }

    #[test]
    fn test_verify_refund_expired() {
        assert!(verify_refund(3000, 2000).is_ok());
    }

    #[test]
    fn test_verify_refund_not_expired() {
        assert!(verify_refund(1000, 2000).is_err());
    }
}
