//! Fuzz target for BLS signature verification.
//!
//! This fuzz target tests the robustness of the BLS verification logic
//! against malformed and adversarial inputs.

#![no_main]

use libfuzzer_sys::fuzz_target;
use qc_10_signature_verification::{verify_bls, BlsSignature, BlsPublicKey};

/// Fuzz input structure for BLS verification.
#[derive(Debug, arbitrary::Arbitrary)]
struct BlsFuzzInput {
    /// Message to verify
    message: Vec<u8>,
    /// Signature bytes (48 bytes for BLS12-381 G1)
    signature_bytes: [u8; 48],
    /// Public key bytes (96 bytes for BLS12-381 G2)
    pubkey_bytes: [u8; 96],
}

fuzz_target!(|input: BlsFuzzInput| {
    // Create signature from fuzzed input
    let signature = BlsSignature {
        bytes: input.signature_bytes,
    };

    let pubkey = BlsPublicKey {
        bytes: input.pubkey_bytes,
    };

    // Verify - this should NEVER panic, regardless of input
    let result = verify_bls(&input.message, &signature, &pubkey);

    // Basic sanity: result should be deterministic
    let result2 = verify_bls(&input.message, &signature, &pubkey);
    assert_eq!(result, result2);
});
