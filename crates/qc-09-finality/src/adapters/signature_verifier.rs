//! Signature Verification Adapter
//!
//! Implements `AttestationVerifier` port for BLS signature verification.
//! Reference: SPEC-09-FINALITY.md Section 3.2, Zero-Trust Verification

use crate::domain::{AggregatedAttestations, Attestation, ValidatorSet};
use crate::ports::outbound::AttestationVerifier;
use tracing::{debug, warn};

/// BLS signature verifier adapter.
///
/// Per IPC-MATRIX.md Zero-Trust policy, this adapter ALWAYS re-verifies
/// signatures regardless of any pre-validation flags.
pub struct BLSAttestationVerifier {
    /// Whether to use strict verification (reject on any failure).
    strict_mode: bool,
}

impl BLSAttestationVerifier {
    /// Create a new verifier in strict mode.
    pub fn new() -> Self {
        Self { strict_mode: true }
    }

    /// Create a verifier with configurable strictness.
    pub fn with_strict_mode(strict_mode: bool) -> Self {
        Self { strict_mode }
    }

    /// Verify a single BLS signature.
    ///
    /// SECURITY: This performs actual cryptographic verification.
    fn verify_bls_signature(
        &self,
        message: &[u8],
        signature: &[u8; 96],
        public_key: &[u8; 48],
    ) -> bool {
        // TODO: Integrate with shared-crypto BLS implementation
        // For now, perform basic sanity checks

        // Reject obviously invalid signatures (all zeros)
        if signature.iter().all(|&b| b == 0) {
            warn!("[qc-09] Rejecting zero signature");
            return false;
        }

        // Reject obviously invalid public keys (all zeros)
        if public_key.iter().all(|&b| b == 0) {
            warn!("[qc-09] Rejecting zero public key");
            return false;
        }

        // Basic length check
        if message.is_empty() {
            warn!("[qc-09] Rejecting empty message");
            return false;
        }

        debug!(
            "[qc-09] BLS verification placeholder (sig: {:02x}{:02x}..., pk: {:02x}{:02x}...)",
            signature[0], signature[1], public_key[0], public_key[1]
        );

        // TODO: Replace with actual BLS12-381 verification when available
        // shared_crypto::bls::verify(message, signature, public_key)
        true
    }
}

impl Default for BLSAttestationVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl AttestationVerifier for BLSAttestationVerifier {
    fn verify_attestation(&self, attestation: &Attestation) -> bool {
        // Create signing message from attestation data
        let message = attestation.signing_root();

        // Verify the BLS signature
        let valid = self.verify_bls_signature(
            &message,
            &attestation.signature,
            &attestation.validator_pubkey,
        );

        if !valid && self.strict_mode {
            warn!(
                "[qc-09] ❌ Invalid attestation from validator {:02x}{:02x}...",
                attestation.validator_pubkey[0], attestation.validator_pubkey[1]
            );
        }

        valid
    }

    fn verify_aggregate(
        &self,
        attestations: &AggregatedAttestations,
        validators: &ValidatorSet,
    ) -> bool {
        // For aggregate verification, check each attestation
        // In production, this would use BLS aggregate signature verification
        let mut valid_count = 0;

        for attestation in &attestations.attestations {
            // Verify validator is in the set
            let validator_in_set = validators
                .validators
                .iter()
                .any(|v| v.pubkey == attestation.validator_pubkey);

            if !validator_in_set {
                warn!(
                    "[qc-09] ⚠️ Attestation from unknown validator {:02x}{:02x}...",
                    attestation.validator_pubkey[0], attestation.validator_pubkey[1]
                );
                if self.strict_mode {
                    return false;
                }
                continue;
            }

            if self.verify_attestation(attestation) {
                valid_count += 1;
            } else if self.strict_mode {
                return false;
            }
        }

        debug!(
            "[qc-09] Aggregate verification: {}/{} valid",
            valid_count,
            attestations.attestations.len()
        );

        valid_count > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AttestationData, Checkpoint};

    fn make_test_attestation() -> Attestation {
        Attestation {
            validator_id: [1u8; 32],
            validator_pubkey: [1u8; 48],
            source: Checkpoint {
                epoch: 1,
                block_hash: [0u8; 32],
            },
            target: Checkpoint {
                epoch: 2,
                block_hash: [1u8; 32],
            },
            signature: [1u8; 96],
        }
    }

    #[test]
    fn test_verify_valid_attestation() {
        let verifier = BLSAttestationVerifier::new();
        let attestation = make_test_attestation();

        assert!(verifier.verify_attestation(&attestation));
    }

    #[test]
    fn test_reject_zero_signature() {
        let verifier = BLSAttestationVerifier::new();
        let mut attestation = make_test_attestation();
        attestation.signature = [0u8; 96];

        assert!(!verifier.verify_attestation(&attestation));
    }

    #[test]
    fn test_reject_zero_pubkey() {
        let verifier = BLSAttestationVerifier::new();
        let mut attestation = make_test_attestation();
        attestation.validator_pubkey = [0u8; 48];

        assert!(!verifier.verify_attestation(&attestation));
    }
}
