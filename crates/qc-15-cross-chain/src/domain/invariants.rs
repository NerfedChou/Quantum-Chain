//! # Domain Invariants
//!
//! Business rules for Cross-Chain Communication.
//!
//! Reference: SPEC-15 Section 2.2 (Lines 177-211)

use super::entities::HTLC;
use super::errors::{CrossChainError, Hash};

/// Minimum timelock margin (6 hours).
/// Reference: System.md Line 752
pub const MIN_TIMELOCK_MARGIN_SECS: u64 = 6 * 3600;

/// Invariant: Timelock ordering.
/// Reference: SPEC-15 Lines 194-198, System.md Line 752
///
/// Source HTLC MUST timeout AFTER target HTLC + margin.
/// This gives the initiator time to claim target HTLC after secret reveal.
pub fn invariant_timelock_ordering(
    source_timelock: u64,
    target_timelock: u64,
    min_margin_secs: u64,
) -> Result<(), CrossChainError> {
    if source_timelock <= target_timelock + min_margin_secs {
        return Err(CrossChainError::InvalidTimelockMargin {
            source_timelock,
            target_timelock,
            required_margin: min_margin_secs,
        });
    }
    Ok(())
}

/// Invariant: Hashlock match.
/// Reference: SPEC-15 Lines 200-204
///
/// Both HTLCs must use the same hashlock.
pub fn invariant_hashlock_match(source_hashlock: &Hash, target_hashlock: &Hash) -> bool {
    source_hashlock == target_hashlock
}

/// Invariant: Secret matches hashlock.
/// Reference: SPEC-15 Lines 206-210
///
/// SHA-256(secret) must equal hashlock for valid claim.
pub fn invariant_secret_matches(secret: &[u8; 32], hashlock: &Hash) -> bool {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(secret);
    let result = hasher.finalize();
    result.as_slice() == hashlock
}

/// Invariant: HTLC claim authorization.
/// Reference: SPEC-15 Lines 578-610
///
/// Only the designated recipient can claim.
pub fn invariant_authorized_claimer(htlc: &HTLC, claimer: &[u8; 20]) -> bool {
    htlc.recipient.address == *claimer
}

/// Invariant: Sufficient confirmations for finality.
/// Reference: SPEC-15 Lines 650-654
pub fn invariant_sufficient_confirmations(
    confirmations: u64,
    required: u64,
) -> Result<(), CrossChainError> {
    if confirmations < required {
        return Err(CrossChainError::NotFinalized {
            got: confirmations,
            required,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timelock_ordering_valid() {
        // Source: 30000, Target: 20000, Margin: 6 hours (21600)
        // 30000 > 20000 + 21600 = 41600? No, fails
        // Let's use: Source: 50000, Target: 20000
        // 50000 > 20000 + 21600 = 41600? Yes!
        assert!(invariant_timelock_ordering(50000, 20000, 21600).is_ok());
    }

    #[test]
    fn test_timelock_ordering_invalid() {
        // Source: 30000, Target: 20000, Margin: 21600
        // 30000 > 20000 + 21600 = 41600? No!
        assert!(invariant_timelock_ordering(30000, 20000, 21600).is_err());
    }

    #[test]
    fn test_timelock_ordering_equal_fails() {
        assert!(invariant_timelock_ordering(10000, 10000, 0).is_err());
    }

    #[test]
    fn test_hashlock_match() {
        let hash = [0xABu8; 32];
        assert!(invariant_hashlock_match(&hash, &hash));
    }

    #[test]
    fn test_hashlock_mismatch() {
        let hash1 = [0xABu8; 32];
        let hash2 = [0xCDu8; 32];
        assert!(!invariant_hashlock_match(&hash1, &hash2));
    }

    #[test]
    fn test_secret_matches() {
        // Compute expected hash
        use sha2::{Digest, Sha256};
        let secret = [0xABu8; 32];
        let mut hasher = Sha256::new();
        hasher.update(secret);
        let hash: [u8; 32] = hasher.finalize().into();

        assert!(invariant_secret_matches(&secret, &hash));
    }

    #[test]
    fn test_secret_not_matches() {
        let secret = [0xABu8; 32];
        let wrong_hash = [0xCDu8; 32];
        assert!(!invariant_secret_matches(&secret, &wrong_hash));
    }

    #[test]
    fn test_sufficient_confirmations_pass() {
        assert!(invariant_sufficient_confirmations(6, 6).is_ok());
        assert!(invariant_sufficient_confirmations(12, 6).is_ok());
    }

    #[test]
    fn test_insufficient_confirmations_fail() {
        assert!(invariant_sufficient_confirmations(3, 6).is_err());
    }
}
