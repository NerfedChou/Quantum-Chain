//! BLS12-381 Signature Implementation
//!
//! Provides BLS signature primitives for:
//! - Key generation
//! - Sign/verify operations
//! - Signature and public key aggregation
//!
//! Used by qc-09-finality for attestation verification.

use blst::min_pk::{AggregatePublicKey, AggregateSignature, PublicKey, SecretKey, Signature};
use blst::BLST_ERROR;
use rand::RngCore;
use zeroize::Zeroize;

use crate::CryptoError;

/// Domain separation tag for BLS signatures (Ethereum 2.0 compatible)
const DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_";

/// BLS secret key wrapper (32 bytes)
#[derive(Clone, Zeroize)]
#[zeroize(drop)]
pub struct BlsSecretKey([u8; 32]);

impl BlsSecretKey {
    /// Create from raw bytes
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self(*bytes)
    }

    /// Get raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// BLS public key (48 bytes compressed)
#[derive(Clone, Debug)]
pub struct BlsPublicKey(PublicKey);

impl PartialEq for BlsPublicKey {
    fn eq(&self, other: &Self) -> bool {
        self.to_bytes() == other.to_bytes()
    }
}

impl Eq for BlsPublicKey {}

/// BLS signature (96 bytes)
#[derive(Clone, Debug)]
pub struct BlsSignature(Signature);

impl PartialEq for BlsSignature {
    fn eq(&self, other: &Self) -> bool {
        self.to_bytes() == other.to_bytes()
    }
}

impl Eq for BlsSignature {}

/// BLS key pair for signing operations
pub struct BlsKeyPair {
    secret: SecretKey,
    public: BlsPublicKey,
}

impl BlsKeyPair {
    /// Generate a new random key pair
    pub fn generate() -> Self {
        let mut ikm = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut ikm);
        let secret = SecretKey::key_gen(&ikm, &[]).expect("valid IKM");
        let public = BlsPublicKey(secret.sk_to_pk());
        Self { secret, public }
    }

    /// Create from existing secret key bytes
    pub fn from_secret_bytes(bytes: &[u8; 32]) -> Result<Self, CryptoError> {
        let secret =
            SecretKey::from_bytes(bytes).map_err(|_| CryptoError::InvalidPrivateKey)?;
        let public = BlsPublicKey(secret.sk_to_pk());
        Ok(Self { secret, public })
    }

    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> BlsSignature {
        BlsSignature(self.secret.sign(message, DST, &[]))
    }

    /// Get the public key
    pub fn public_key(&self) -> BlsPublicKey {
        self.public.clone()
    }

    /// Get the secret key bytes (be careful with this!)
    pub fn secret_bytes(&self) -> [u8; 32] {
        self.secret.to_bytes()
    }
}

impl BlsPublicKey {
    /// Verify a signature against this public key
    pub fn verify(&self, message: &[u8], signature: &BlsSignature) -> bool {
        signature.0.verify(true, message, DST, &[], &self.0, true) == BLST_ERROR::BLST_SUCCESS
    }

    /// Create from 48-byte compressed representation
    pub fn from_bytes(bytes: &[u8; 48]) -> Result<Self, CryptoError> {
        PublicKey::from_bytes(bytes)
            .map(BlsPublicKey)
            .map_err(|_| CryptoError::InvalidPublicKey)
    }

    /// Serialize to 48-byte compressed form
    pub fn to_bytes(&self) -> [u8; 48] {
        self.0.to_bytes()
    }

    /// Aggregate multiple public keys into one
    ///
    /// The aggregated key can verify aggregated signatures.
    pub fn aggregate(keys: &[BlsPublicKey]) -> Result<Self, CryptoError> {
        if keys.is_empty() {
            return Err(CryptoError::InvalidInput("empty key list".into()));
        }
        let refs: Vec<&PublicKey> = keys.iter().map(|k| &k.0).collect();
        AggregatePublicKey::aggregate(&refs, true)
            .map(|apk| BlsPublicKey(apk.to_public_key()))
            .map_err(|_| CryptoError::AggregationFailed)
    }
}

impl BlsSignature {
    /// Create from 96-byte representation
    pub fn from_bytes(bytes: &[u8; 96]) -> Result<Self, CryptoError> {
        Signature::from_bytes(bytes)
            .map(BlsSignature)
            .map_err(|_| CryptoError::InvalidSignature)
    }

    /// Serialize to 96-byte form
    pub fn to_bytes(&self) -> [u8; 96] {
        self.0.to_bytes()
    }

    /// Aggregate multiple signatures into one
    ///
    /// The aggregated signature can be verified against the aggregated public key.
    pub fn aggregate(sigs: &[BlsSignature]) -> Result<Self, CryptoError> {
        if sigs.is_empty() {
            return Err(CryptoError::InvalidInput("empty signature list".into()));
        }
        let refs: Vec<&Signature> = sigs.iter().map(|s| &s.0).collect();
        AggregateSignature::aggregate(&refs, true)
            .map(|asig| BlsSignature(asig.to_signature()))
            .map_err(|_| CryptoError::AggregationFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bls_sign_verify_roundtrip() {
        let keypair = BlsKeyPair::generate();
        let message = b"test message";
        let signature = keypair.sign(message);
        assert!(keypair.public_key().verify(message, &signature));
    }

    #[test]
    fn test_bls_invalid_signature_rejected() {
        let keypair = BlsKeyPair::generate();
        let message = b"test message";
        // Zero bytes may or may not parse as valid depending on curve point
        // So we test with a different message instead
        let signature = keypair.sign(message);
        let wrong_message = b"wrong message";
        assert!(!keypair.public_key().verify(wrong_message, &signature));
    }

    #[test]
    fn test_bls_different_key_rejected() {
        let keypair1 = BlsKeyPair::generate();
        let keypair2 = BlsKeyPair::generate();
        let message = b"test message";
        let signature = keypair1.sign(message);
        // Signature from keypair1 should not verify with keypair2's public key
        assert!(!keypair2.public_key().verify(message, &signature));
    }

    #[test]
    fn test_bls_aggregate_signatures() {
        let kp1 = BlsKeyPair::generate();
        let kp2 = BlsKeyPair::generate();
        let message = b"same message";

        let sig1 = kp1.sign(message);
        let sig2 = kp2.sign(message);

        let agg_sig = BlsSignature::aggregate(&[sig1, sig2]).unwrap();
        let agg_pk =
            BlsPublicKey::aggregate(&[kp1.public_key(), kp2.public_key()]).unwrap();

        assert!(agg_pk.verify(message, &agg_sig));
    }

    #[test]
    fn test_bls_aggregate_empty_fails() {
        let result = BlsSignature::aggregate(&[]);
        assert!(result.is_err());

        let result = BlsPublicKey::aggregate(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_bls_serialization_roundtrip() {
        let keypair = BlsKeyPair::generate();
        let message = b"test message";
        let signature = keypair.sign(message);

        // Serialize and deserialize public key
        let pk_bytes = keypair.public_key().to_bytes();
        let pk_restored = BlsPublicKey::from_bytes(&pk_bytes).unwrap();
        assert_eq!(keypair.public_key(), pk_restored);

        // Serialize and deserialize signature
        let sig_bytes = signature.to_bytes();
        let sig_restored = BlsSignature::from_bytes(&sig_bytes).unwrap();
        assert_eq!(signature, sig_restored);

        // Verify with restored values
        assert!(pk_restored.verify(message, &sig_restored));
    }

    #[test]
    fn test_bls_from_secret_bytes() {
        let keypair1 = BlsKeyPair::generate();
        let secret_bytes = keypair1.secret_bytes();

        let keypair2 = BlsKeyPair::from_secret_bytes(&secret_bytes).unwrap();

        // Both should have the same public key
        assert_eq!(keypair1.public_key(), keypair2.public_key());

        // Signatures should be identical
        let message = b"test";
        let sig1 = keypair1.sign(message);
        let sig2 = keypair2.sign(message);
        assert_eq!(sig1, sig2);
    }
}
