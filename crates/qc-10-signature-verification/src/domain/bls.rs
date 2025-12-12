//! # BLS Verification (BLS12-381)
//!
//! Pure domain logic for BLS signature verification.
//!
//! Reference: SPEC-10 Section 2.1, 3.1
//!
//! ## Notes
//!
//! BLS signatures are used for:
//! - PoS attestation aggregation (Subsystem 8, 9)
//! - Efficient batch verification
//!
//! ## Implementation Details
//!
//! Per SPEC-10:
//! - Signatures are on G1 (48 bytes compressed)
//! - Public keys are on G2 (96 bytes compressed)
//!
//! This uses blst's `min_sig` variant for smaller signatures.

use super::entities::{BlsPublicKey, BlsSignature};
use super::errors::SignatureError;
use blst::min_sig::{AggregateSignature, PublicKey, Signature};
use blst::BLST_ERROR;

/// Domain Separation Tag for BLS signatures (Ethereum 2.0 style)
const DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_";

/// Verify a single BLS signature.
///
/// Reference: SPEC-10 Section 3.1 `verify_bls`
///
/// # Arguments
/// * `message` - The raw message bytes
/// * `signature` - The BLS signature (G1 point, 48 bytes)
/// * `public_key` - The BLS public key (G2 point, 96 bytes)
///
/// # Returns
/// * `true` if signature is valid, `false` otherwise
pub fn verify_bls(message: &[u8], signature: &BlsSignature, public_key: &BlsPublicKey) -> bool {
    // Parse signature (G1 point, 48 bytes per SPEC-10)
    let Ok(sig) = Signature::from_bytes(&signature.bytes) else {
        return false;
    };

    // Parse public key (G2 point, 96 bytes per SPEC-10)
    let Ok(pk) = PublicKey::from_bytes(&public_key.bytes) else {
        return false;
    };

    // Verify using pairing check
    let result = sig.verify(true, message, DST, &[], &pk, true);
    result == BLST_ERROR::BLST_SUCCESS
}

/// Verify an aggregated BLS signature against multiple public keys.
///
/// Reference: SPEC-10 Section 3.1 `verify_bls_aggregate`
///
/// All signers must have signed the same message.
pub fn verify_bls_aggregate(
    message: &[u8],
    aggregate_signature: &BlsSignature,
    public_keys: &[BlsPublicKey],
) -> bool {
    if public_keys.is_empty() {
        return false;
    }

    // Parse aggregate signature
    let sig = match Signature::from_bytes(&aggregate_signature.bytes) {
        Ok(s) => s,
        Err(_) => return false,
    };

    // Parse all public keys
    let pks: Vec<PublicKey> = public_keys
        .iter()
        .filter_map(|pk| PublicKey::from_bytes(&pk.bytes).ok())
        .collect();

    if pks.len() != public_keys.len() {
        return false; // Some public keys failed to parse
    }

    // Create references for verification
    let pk_refs: Vec<&PublicKey> = pks.iter().collect();

    // For aggregate_verify, we need to provide messages as slices
    let msgs: Vec<&[u8]> = vec![message; pk_refs.len()];

    // Verify aggregate signature
    let result = sig.aggregate_verify(true, &msgs, DST, &pk_refs, true);
    result == BLST_ERROR::BLST_SUCCESS
}

/// Aggregate multiple BLS signatures into one.
///
/// Reference: SPEC-10 Section 3.1 `aggregate_bls_signatures`
///
/// # Errors
/// * `EmptyAggregation` if the input list is empty
pub fn aggregate_bls_signatures(
    signatures: &[BlsSignature],
) -> Result<BlsSignature, SignatureError> {
    if signatures.is_empty() {
        return Err(SignatureError::EmptyAggregation);
    }

    // Parse first signature to initialize aggregate
    let first_sig =
        Signature::from_bytes(&signatures[0].bytes).map_err(|_| SignatureError::InvalidFormat)?;

    let mut aggregate = AggregateSignature::from_signature(&first_sig);

    // Add remaining signatures
    for sig in &signatures[1..] {
        let parsed =
            Signature::from_bytes(&sig.bytes).map_err(|_| SignatureError::InvalidFormat)?;
        aggregate
            .add_signature(&parsed, true)
            .map_err(|_| SignatureError::BlsPairingFailed)?;
    }

    // Convert to bytes (48 bytes for G1 point per SPEC-10)
    let result_bytes = aggregate.to_signature().to_bytes();

    Ok(BlsSignature {
        bytes: result_bytes,
    })
}

/// Aggregate multiple BLS public keys into one.
///
/// This is useful when verifying an aggregate signature where all signers
/// signed different messages (multi-message aggregation).
///
/// # Arguments
/// * `public_keys` - The BLS public keys to aggregate
///
/// # Errors
/// * `EmptyAggregation` if the input list is empty
/// * `InvalidFormat` if any public key cannot be parsed
pub fn aggregate_bls_public_keys(
    public_keys: &[BlsPublicKey],
) -> Result<BlsPublicKey, SignatureError> {
    use blst::min_sig::AggregatePublicKey;

    if public_keys.is_empty() {
        return Err(SignatureError::EmptyAggregation);
    }

    // Parse all public keys
    let pks: Result<Vec<PublicKey>, SignatureError> = public_keys
        .iter()
        .map(|pk| PublicKey::from_bytes(&pk.bytes).map_err(|_| SignatureError::InvalidFormat))
        .collect();

    let pks = pks?;
    let pk_refs: Vec<&PublicKey> = pks.iter().collect();

    // Aggregate public keys
    let aggregate = AggregatePublicKey::aggregate(&pk_refs, true)
        .map_err(|_| SignatureError::BlsPairingFailed)?;

    Ok(BlsPublicKey {
        bytes: aggregate.to_public_key().to_bytes(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use blst::min_sig::SecretKey;

    fn generate_keypair() -> (SecretKey, BlsPublicKey) {
        let mut ikm = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut ikm);
        let sk = SecretKey::key_gen(&ikm, &[]).unwrap();
        let pk = sk.sk_to_pk();
        (
            sk,
            BlsPublicKey {
                bytes: pk.to_bytes(),
            },
        )
    }

    fn sign_message(sk: &SecretKey, message: &[u8]) -> BlsSignature {
        let sig = sk.sign(message, DST, &[]);
        BlsSignature {
            bytes: sig.to_bytes(),
        }
    }

    #[test]
    fn test_bls_verify_valid() {
        let (sk, pk) = generate_keypair();
        let message = b"test message";
        let signature = sign_message(&sk, message);

        assert!(verify_bls(message, &signature, &pk));
    }

    #[test]
    fn test_bls_verify_invalid_wrong_message() {
        let (sk, pk) = generate_keypair();
        let signature = sign_message(&sk, b"message 1");

        assert!(!verify_bls(b"message 2", &signature, &pk));
    }

    #[test]
    fn test_bls_verify_invalid_wrong_key() {
        let (sk1, _pk1) = generate_keypair();
        let (_sk2, pk2) = generate_keypair();
        let message = b"test";
        let signature = sign_message(&sk1, message);

        assert!(!verify_bls(message, &signature, &pk2));
    }

    #[test]
    fn test_bls_aggregate_empty_fails() {
        let result = aggregate_bls_signatures(&[]);
        assert!(matches!(result, Err(SignatureError::EmptyAggregation)));
    }

    #[test]
    fn test_bls_aggregate_and_verify() {
        let message = b"aggregate test";
        let mut signatures = Vec::new();
        let mut public_keys = Vec::new();

        for _ in 0..5 {
            let (sk, pk) = generate_keypair();
            signatures.push(sign_message(&sk, message));
            public_keys.push(pk);
        }

        let aggregate = aggregate_bls_signatures(&signatures).unwrap();
        assert!(verify_bls_aggregate(message, &aggregate, &public_keys));
    }

    #[test]
    fn test_bls_aggregate_public_keys_empty_fails() {
        let result = aggregate_bls_public_keys(&[]);
        assert!(matches!(result, Err(SignatureError::EmptyAggregation)));
    }

    #[test]
    fn test_bls_aggregate_public_keys() {
        let mut public_keys = Vec::new();

        for _ in 0..5 {
            let (_, pk) = generate_keypair();
            public_keys.push(pk);
        }

        let result = aggregate_bls_public_keys(&public_keys);
        assert!(result.is_ok());

        // Verify the aggregated key is 96 bytes
        let agg_pk = result.unwrap();
        assert_eq!(agg_pk.bytes.len(), 96);
    }

    #[test]
    fn test_bls_aggregate_public_keys_single() {
        let (_, pk) = generate_keypair();
        let result = aggregate_bls_public_keys(std::slice::from_ref(&pk));
        assert!(result.is_ok());
    }
}
