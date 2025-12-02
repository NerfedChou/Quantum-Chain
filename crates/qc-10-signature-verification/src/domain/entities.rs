//! # Domain Entities
//!
//! Core data structures for signature verification.
//!
//! Reference: SPEC-10 Section 2.1 (Core Entities)

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, Bytes};
use shared_types::Hash;

/// Ethereum-style address derived from public key (last 20 bytes of keccak256(pubkey))
pub type Address = [u8; 20];

// =============================================================================
// ECDSA Types (secp256k1)
// =============================================================================

/// ECDSA signature on the secp256k1 curve.
///
/// Reference: SPEC-10 Section 2.1
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EcdsaSignature {
    /// R component (32 bytes)
    pub r: [u8; 32],
    /// S component (32 bytes)
    pub s: [u8; 32],
    /// Recovery ID (0, 1, 27, or 28)
    pub v: u8,
}

/// ECDSA public key (uncompressed format).
///
/// Format: 0x04 || x (32 bytes) || y (32 bytes) = 65 bytes total
#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EcdsaPublicKey {
    /// Uncompressed public key bytes
    #[serde_as(as = "Bytes")]
    pub bytes: [u8; 65],
}

// =============================================================================
// BLS Types (BLS12-381)
// =============================================================================

/// BLS signature (G1 point, compressed).
///
/// Reference: SPEC-10 Section 2.1
/// BLS signatures are on G1 curve (48 bytes compressed)
#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlsSignature {
    /// G1 point (48 bytes compressed)
    #[serde_as(as = "Bytes")]
    pub bytes: [u8; 48],
}

/// BLS public key (G2 point, compressed).
///
/// Reference: SPEC-10 Section 2.1
/// BLS public keys are on G2 curve (96 bytes compressed)
#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlsPublicKey {
    /// G2 point (96 bytes compressed)
    #[serde_as(as = "Bytes")]
    pub bytes: [u8; 96],
}

// =============================================================================
// Verification Request/Result Types
// =============================================================================

/// Request to verify an ECDSA signature.
///
/// Reference: SPEC-10 Section 2.1
#[derive(Clone, Debug)]
pub struct VerificationRequest {
    /// The hash of the message that was signed
    pub message_hash: Hash,
    /// The signature to verify
    pub signature: EcdsaSignature,
    /// Optional expected signer address (if provided, verification checks recovered address)
    pub expected_signer: Option<Address>,
}

/// Result of signature verification.
///
/// Reference: SPEC-10 Section 2.1
#[derive(Clone, Debug)]
pub struct VerificationResult {
    /// Whether the signature is valid
    pub valid: bool,
    /// The recovered address (if verification succeeded)
    pub recovered_address: Option<Address>,
    /// Error details (if verification failed)
    pub error: Option<super::errors::SignatureError>,
}

impl VerificationResult {
    /// Create a successful verification result.
    pub fn valid(recovered_address: Address) -> Self {
        Self {
            valid: true,
            recovered_address: Some(recovered_address),
            error: None,
        }
    }

    /// Create a failed verification result.
    pub fn invalid(error: super::errors::SignatureError) -> Self {
        Self {
            valid: false,
            recovered_address: None,
            error: Some(error),
        }
    }
}

/// Request for batch ECDSA verification.
///
/// Reference: SPEC-10 Section 2.1
#[derive(Clone, Debug)]
pub struct BatchVerificationRequest {
    /// The verification requests to process
    pub requests: Vec<VerificationRequest>,
}

/// Result of batch verification.
///
/// Reference: SPEC-10 Section 2.1
#[derive(Clone, Debug)]
pub struct BatchVerificationResult {
    /// Individual results for each request
    pub results: Vec<VerificationResult>,
    /// Whether all verifications passed
    pub all_valid: bool,
    /// Count of valid signatures
    pub valid_count: usize,
    /// Count of invalid signatures
    pub invalid_count: usize,
}

impl BatchVerificationResult {
    /// Create a batch result from individual results.
    pub fn from_results(results: Vec<VerificationResult>) -> Self {
        let valid_count = results.iter().filter(|r| r.valid).count();
        let invalid_count = results.len() - valid_count;
        let all_valid = invalid_count == 0;

        Self {
            results,
            all_valid,
            valid_count,
            invalid_count,
        }
    }
}

/// A verified transaction ready for forwarding to Mempool.
///
/// Reference: SPEC-10 Section 3.1
#[derive(Clone, Debug)]
pub struct VerifiedTransaction {
    /// The original signed transaction
    pub transaction: shared_types::Transaction,
    /// The recovered sender address
    pub sender: Address,
    /// Whether the signature was valid
    pub signature_valid: bool,
}
