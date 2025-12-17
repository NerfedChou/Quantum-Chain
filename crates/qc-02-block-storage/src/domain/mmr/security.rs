//! # MMR Security
//!
//! Security controls for Merkle Mountain Range operations.
//!
//! ## Security Invariants
//!
//! - Proof verification must be deterministic
//! - Inclusion proofs must be validated against known root

use shared_types::Hash;

/// Maximum proof depth to prevent DoS attacks.
pub const MAX_PROOF_DEPTH: usize = 64;

/// Validate proof path length.
pub fn validate_proof_depth(depth: usize) -> Result<(), &'static str> {
    if depth > MAX_PROOF_DEPTH {
        return Err("Proof depth exceeds maximum");
    }
    Ok(())
}

/// Verify a hash is non-zero (valid).
pub fn validate_hash(hash: &Hash) -> bool {
    hash.iter().any(|b| *b != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_proof_depth() {
        assert!(validate_proof_depth(32).is_ok());
        assert!(validate_proof_depth(64).is_ok());
        assert!(validate_proof_depth(65).is_err());
    }

    #[test]
    fn test_validate_hash() {
        assert!(validate_hash(&[1u8; 32]));
        assert!(!validate_hash(&[0u8; 32]));
    }
}
