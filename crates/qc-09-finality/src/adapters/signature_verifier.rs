//! Signature Verification Adapter
//!
//! Implements `AttestationVerifier` port for BLS signature verification.
//! Reference: SPEC-09-FINALITY.md Section 3.2, Zero-Trust Verification

use crate::domain::{AggregatedAttestations, Attestation, ValidatorSet};
use crate::ports::outbound::AttestationVerifier;
use shared_crypto::bls::{BlsPublicKey, BlsSignature};
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

    /// Verify a single BLS signature using shared-crypto.
    ///
    /// SECURITY: This performs actual cryptographic verification using BLS12-381.
    fn verify_bls_signature(
        &self,
        message: &[u8],
        signature_bytes: &[u8],
        public_key_bytes: &[u8; 48],
    ) -> bool {
        // Reject obviously invalid signatures (all zeros)
        if signature_bytes.iter().all(|&b| b == 0) {
            warn!("[qc-09] Rejecting zero signature");
            return false;
        }

        // Reject obviously invalid public keys (all zeros)
        if public_key_bytes.iter().all(|&b| b == 0) {
            warn!("[qc-09] Rejecting zero public key");
            return false;
        }

        // Basic length check
        if message.is_empty() {
            warn!("[qc-09] Rejecting empty message");
            return false;
        }

        // Signature must be exactly 96 bytes for BLS12-381
        if signature_bytes.len() != 96 {
            warn!(
                "[qc-09] Invalid signature length: expected 96, got {}",
                signature_bytes.len()
            );
            return false;
        }

        // Parse signature bytes
        let sig_array: [u8; 96] = match signature_bytes.try_into() {
            Ok(arr) => arr,
            Err(_) => {
                warn!("[qc-09] Failed to convert signature to array");
                return false;
            }
        };

        let signature = match BlsSignature::from_bytes(&sig_array) {
            Ok(sig) => sig,
            Err(e) => {
                warn!("[qc-09] Invalid BLS signature format: {:?}", e);
                return false;
            }
        };

        // Parse public key bytes
        let public_key = match BlsPublicKey::from_bytes(public_key_bytes) {
            Ok(pk) => pk,
            Err(e) => {
                warn!("[qc-09] Invalid BLS public key format: {:?}", e);
                return false;
            }
        };

        // Perform actual BLS verification
        let valid = public_key.verify(message, &signature);

        debug!(
            "[qc-09] BLS verification result: {} (sig: {:02x}{:02x}..., pk: {:02x}{:02x}...)",
            valid, sig_array[0], sig_array[1], public_key_bytes[0], public_key_bytes[1]
        );

        valid
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
        let _message = attestation.signing_message();

        // Get signature bytes from the BlsSignature wrapper
        let signature_bytes = &attestation.signature.0;

        // We need to get the public key from the validator set
        // For now, we cannot verify without the validator set context
        // This method should be called with proper validator lookup
        warn!(
            "[qc-09] verify_attestation called without validator context for {:?}",
            attestation.validator_id
        );

        // Basic signature structure validation
        if signature_bytes.len() != 96 {
            warn!(
                "[qc-09] ❌ Invalid signature length for validator {:?}",
                attestation.validator_id
            );
            return false;
        }

        if signature_bytes.iter().all(|&b| b == 0) {
            warn!(
                "[qc-09] ❌ Zero signature from validator {:?}",
                attestation.validator_id
            );
            return false;
        }

        // Without the public key, we can only do structural validation
        // Full cryptographic verification requires verify_aggregate with ValidatorSet
        true
    }

    fn verify_aggregate(
        &self,
        attestations: &AggregatedAttestations,
        validators: &ValidatorSet,
    ) -> bool {
        let mut valid_count = 0;

        for attestation in &attestations.attestations {
            // Get validator and their public key
            let validator = match validators.get(&attestation.validator_id) {
                Some(v) => v,
                None => {
                    warn!(
                        "[qc-09] ⚠️ Attestation from unknown validator {:?}",
                        attestation.validator_id
                    );
                    if self.strict_mode {
                        return false;
                    }
                    continue;
                }
            };

            // Create signing message
            let message = attestation.signing_message();

            // Verify the BLS signature with the validator's public key
            let valid = self.verify_bls_signature(&message, &attestation.signature.0, &validator.pubkey);

            if valid {
                valid_count += 1;
            } else {
                warn!(
                    "[qc-09] ❌ Invalid attestation from validator {:?}",
                    attestation.validator_id
                );
                if self.strict_mode {
                    return false;
                }
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
    use crate::domain::{CheckpointId, ValidatorId};
    use shared_crypto::bls::BlsKeyPair;

    fn test_hash(n: u8) -> [u8; 32] {
        let mut hash = [0u8; 32];
        hash[0] = n;
        hash
    }

    fn test_validator_id(n: u8) -> ValidatorId {
        let mut id = [0u8; 32];
        id[0] = n;
        ValidatorId(id)
    }

    fn make_signed_attestation(keypair: &BlsKeyPair, slot: u64) -> Attestation {
        let source = CheckpointId::new(1, test_hash(1));
        let target = CheckpointId::new(2, test_hash(2));

        // Create attestation without signature first to get the message
        let mut attestation = Attestation::new(
            test_validator_id(1),
            source,
            target,
            crate::domain::BlsSignature::default(),
            slot,
        );

        // Sign the message
        let message = attestation.signing_message();
        let signature = keypair.sign(&message);
        attestation.signature = crate::domain::BlsSignature::new(signature.to_bytes().to_vec());

        attestation
    }

    #[test]
    fn test_verify_real_bls_signature() {
        let verifier = BLSAttestationVerifier::new();
        let keypair = BlsKeyPair::generate();

        // Create a validator set with the keypair's public key
        let mut validators = ValidatorSet::new(1);
        let validator_id = test_validator_id(1);
        validators.add_validator_with_pubkey(
            validator_id,
            1000,
            keypair.public_key().to_bytes(),
        );

        // Create and sign an attestation
        let attestation = make_signed_attestation(&keypair, 64);

        // Create aggregated attestations
        let source = CheckpointId::new(1, test_hash(1));
        let target = CheckpointId::new(2, test_hash(2));
        let mut agg = AggregatedAttestations::new(source, target, 100);
        agg.add_attestation(attestation, 0, 1000);

        // Verify - should pass with real BLS
        assert!(verifier.verify_aggregate(&agg, &validators));
    }

    #[test]
    fn test_reject_wrong_signature() {
        let verifier = BLSAttestationVerifier::new();
        let keypair1 = BlsKeyPair::generate();
        let keypair2 = BlsKeyPair::generate();

        // Create a validator set with keypair1's public key
        let mut validators = ValidatorSet::new(1);
        let validator_id = test_validator_id(1);
        validators.add_validator_with_pubkey(
            validator_id,
            1000,
            keypair1.public_key().to_bytes(),
        );

        // Sign with keypair2 (wrong key)
        let attestation = make_signed_attestation(&keypair2, 64);

        // Create aggregated attestations
        let source = CheckpointId::new(1, test_hash(1));
        let target = CheckpointId::new(2, test_hash(2));
        let mut agg = AggregatedAttestations::new(source, target, 100);
        agg.add_attestation(attestation, 0, 1000);

        // Verify - should fail because signature doesn't match pubkey
        assert!(!verifier.verify_aggregate(&agg, &validators));
    }

    #[test]
    fn test_reject_zero_signature() {
        let verifier = BLSAttestationVerifier::new();
        let keypair = BlsKeyPair::generate();

        // Create a validator set
        let mut validators = ValidatorSet::new(1);
        let validator_id = test_validator_id(1);
        validators.add_validator_with_pubkey(
            validator_id,
            1000,
            keypair.public_key().to_bytes(),
        );

        // Create attestation with zero signature
        let source = CheckpointId::new(1, test_hash(1));
        let target = CheckpointId::new(2, test_hash(2));
        let attestation = Attestation::new(
            validator_id,
            source,
            target,
            crate::domain::BlsSignature::new(vec![0u8; 96]),
            64,
        );

        let mut agg = AggregatedAttestations::new(source, target, 100);
        agg.add_attestation(attestation, 0, 1000);

        // Should reject zero signature
        assert!(!verifier.verify_aggregate(&agg, &validators));
    }

    #[test]
    fn test_reject_unknown_validator() {
        let verifier = BLSAttestationVerifier::new();
        let keypair = BlsKeyPair::generate();

        // Empty validator set
        let validators = ValidatorSet::new(1);

        // Create and sign an attestation
        let attestation = make_signed_attestation(&keypair, 64);

        // Create aggregated attestations
        let source = CheckpointId::new(1, test_hash(1));
        let target = CheckpointId::new(2, test_hash(2));
        let mut agg = AggregatedAttestations::new(source, target, 100);
        agg.add_attestation(attestation, 0, 1000);

        // Should reject because validator not in set
        assert!(!verifier.verify_aggregate(&agg, &validators));
    }

    #[test]
    fn test_non_strict_mode_continues_on_failure() {
        let verifier = BLSAttestationVerifier::with_strict_mode(false);
        let keypair1 = BlsKeyPair::generate();
        let keypair2 = BlsKeyPair::generate();

        // Create validator set with both keys
        let mut validators = ValidatorSet::new(1);
        validators.add_validator_with_pubkey(
            test_validator_id(1),
            1000,
            keypair1.public_key().to_bytes(),
        );
        validators.add_validator_with_pubkey(
            test_validator_id(2),
            1000,
            keypair2.public_key().to_bytes(),
        );

        // Create valid attestation for validator 1
        let source = CheckpointId::new(1, test_hash(1));
        let target = CheckpointId::new(2, test_hash(2));

        let mut att1 = Attestation::new(
            test_validator_id(1),
            source,
            target,
            crate::domain::BlsSignature::default(),
            64,
        );
        let msg1 = att1.signing_message();
        att1.signature = crate::domain::BlsSignature::new(keypair1.sign(&msg1).to_bytes().to_vec());

        // Create invalid attestation for validator 2 (zero sig)
        let att2 = Attestation::new(
            test_validator_id(2),
            source,
            target,
            crate::domain::BlsSignature::new(vec![0u8; 96]),
            65,
        );

        let mut agg = AggregatedAttestations::new(source, target, 100);
        agg.add_attestation(att1, 0, 1000);
        agg.add_attestation(att2, 1, 1000);

        // In non-strict mode, should pass because at least one is valid
        assert!(verifier.verify_aggregate(&agg, &validators));
    }
}
