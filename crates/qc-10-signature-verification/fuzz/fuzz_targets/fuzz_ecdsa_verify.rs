//! Fuzz target for ECDSA signature verification.
//!
//! This fuzz target tests the robustness of the ECDSA verification logic
//! against malformed and adversarial inputs.
//!
//! ## Running
//!
//! ```bash
//! cd crates/qc-10-signature-verification
//! cargo +nightly fuzz run fuzz_ecdsa_verify
//! ```

#![no_main]

use libfuzzer_sys::fuzz_target;
use qc_10_signature_verification::{EcdsaSignature, EcdsaVerifier};
use qc_10_signature_verification::ports::inbound::SignatureVerificationApi;

/// Fuzz input structure for ECDSA verification.
#[derive(Debug, arbitrary::Arbitrary)]
struct FuzzInput {
    /// Message hash to verify against (32 bytes)
    message_hash: [u8; 32],
    /// R component of signature
    r: [u8; 32],
    /// S component of signature
    s: [u8; 32],
    /// Recovery ID
    v: u8,
}

fuzz_target!(|input: FuzzInput| {
    // Create signature from fuzzed input
    let signature = EcdsaSignature {
        r: input.r,
        s: input.s,
        v: input.v,
    };

    // Create verifier
    let verifier = EcdsaVerifier;

    // Verify - this should NEVER panic, regardless of input
    let result = verifier.verify_ecdsa(&input.message_hash, &signature);

    // Basic sanity checks that should always hold
    // 1. Result should be deterministic
    let result2 = verifier.verify_ecdsa(&input.message_hash, &signature);
    assert_eq!(result.valid, result2.valid);

    // 2. If valid, recovered address should exist
    if result.valid {
        assert!(result.recovered_address.is_some());
    }

    // 3. Should not have both valid=true and error=Some
    if result.valid {
        assert!(result.error.is_none());
    }
});
