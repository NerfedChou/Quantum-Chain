//! # Signature Verification Service
//!
//! Application service layer that implements the `SignatureVerificationApi` trait.
//!
//! Reference: SPEC-10 Section 3.1
//!
//! ## Architecture
//!
//! This is the hexagonal "application service" that:
//! - Implements the inbound port (`SignatureVerificationApi`)
//! - Uses the outbound port (`MempoolGateway`) for forwarding verified transactions
//! - Delegates cryptographic operations to domain layer

use crate::domain::bls;
use crate::domain::ecdsa;
use crate::domain::entities::{
    Address, BatchVerificationRequest, BatchVerificationResult, BlsPublicKey, BlsSignature,
    EcdsaSignature, VerificationResult, VerifiedTransaction,
};
use crate::domain::errors::SignatureError;
use crate::ports::inbound::SignatureVerificationApi;
use crate::ports::outbound::MempoolGateway;
use shared_types::{Hash, Transaction};

/// Signature Verification Service.
///
/// Reference: SPEC-10 Section 3.1
///
/// This service implements `SignatureVerificationApi` and delegates
/// cryptographic operations to the domain layer.
///
/// The mempool gateway is used for the async `verify_and_submit` flow
/// per SPEC-10 Section 4.1 (AddTransactionRequest to Mempool).
pub struct SignatureVerificationService<M: MempoolGateway> {
    mempool: M,
}

impl<M: MempoolGateway> SignatureVerificationService<M> {
    /// Create a new signature verification service.
    ///
    /// # Arguments
    /// * `mempool` - The mempool gateway for forwarding verified transactions
    pub fn new(mempool: M) -> Self {
        Self { mempool }
    }

    /// Verify a transaction and submit to mempool if valid.
    ///
    /// Reference: SPEC-10 Section 4.1 - AddTransactionRequest flow
    ///
    /// This is the async entry point that combines verification with
    /// submission to the mempool subsystem.
    pub async fn verify_and_submit(&self, transaction: Transaction) -> Result<(), SignatureError> {
        // First verify the transaction
        let verified = self.verify_transaction(transaction)?;

        // Then submit to mempool
        self.mempool
            .submit_verified_transaction(verified)
            .await
            .map_err(|e| SignatureError::SubmissionFailed(e.to_string()))?;

        Ok(())
    }
}

impl<M: MempoolGateway> SignatureVerificationApi for SignatureVerificationService<M> {
    fn verify_ecdsa(&self, message_hash: &Hash, signature: &EcdsaSignature) -> VerificationResult {
        ecdsa::verify_ecdsa(message_hash, signature)
    }

    fn verify_ecdsa_signer(
        &self,
        message_hash: &Hash,
        signature: &EcdsaSignature,
        expected: Address,
    ) -> VerificationResult {
        ecdsa::verify_ecdsa_signer(message_hash, signature, expected)
    }

    fn recover_address(
        &self,
        message_hash: &Hash,
        signature: &EcdsaSignature,
    ) -> Result<Address, SignatureError> {
        ecdsa::recover_address(message_hash, signature)
    }

    fn batch_verify_ecdsa(&self, request: &BatchVerificationRequest) -> BatchVerificationResult {
        ecdsa::batch_verify_ecdsa(&request.requests)
    }

    fn verify_bls(
        &self,
        message: &[u8],
        signature: &BlsSignature,
        public_key: &BlsPublicKey,
    ) -> bool {
        bls::verify_bls(message, signature, public_key)
    }

    fn verify_bls_aggregate(
        &self,
        message: &[u8],
        aggregate_signature: &BlsSignature,
        public_keys: &[BlsPublicKey],
    ) -> bool {
        bls::verify_bls_aggregate(message, aggregate_signature, public_keys)
    }

    fn aggregate_bls_signatures(
        &self,
        signatures: &[BlsSignature],
    ) -> Result<BlsSignature, SignatureError> {
        bls::aggregate_bls_signatures(signatures)
    }

    /// Verify a signed transaction and prepare it for Mempool submission.
    ///
    /// Reference: SPEC-10 Section 3.1 `verify_transaction`
    ///
    /// # Security Warning: Replay Protection
    ///
    /// **This function does NOT prevent replay attacks.**
    ///
    /// Callers MUST validate transaction nonces separately to prevent replay attacks.
    /// The Mempool subsystem (Subsystem 6) is responsible for:
    /// - Checking nonce against current account state
    /// - Rejecting transactions with already-used nonces
    /// - Ensuring sequential nonce ordering
    ///
    /// Additionally, block proposers should ensure block height is included in
    /// signed data to prevent cross-block replay.
    fn verify_transaction(
        &self,
        transaction: Transaction,
    ) -> Result<VerifiedTransaction, SignatureError> {
        // 1. Compute transaction hash (the message that was signed)
        let tx_hash = compute_transaction_hash(&transaction);

        // 2. Extract r and s from signature
        //    shared_types::Signature is [u8; 64] = r || s (no v)
        let (r, s) = extract_rs_from_signature(&transaction)?;

        // 3. Derive expected sender address from public key
        let expected_sender = derive_address_from_pubkey(&transaction.from);

        // 4. Try both recovery IDs (27 and 28 in Ethereum convention)
        //    Since we don't have 'v' in the signature, we must try both
        for v in [27u8, 28u8] {
            let signature = EcdsaSignature { r, s, v };
            let result = self.verify_ecdsa_signer(&tx_hash, &signature, expected_sender);

            if result.valid {
                return Ok(VerifiedTransaction {
                    transaction,
                    sender: expected_sender,
                    signature_valid: true,
                });
            }
        }

        // Neither recovery ID worked
        Err(SignatureError::VerificationFailed)
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Compute the hash of a transaction for signature verification.
///
/// This creates the message that was signed by the sender.
fn compute_transaction_hash(tx: &Transaction) -> Hash {
    use sha3::{Digest, Keccak256};

    let mut hasher = Keccak256::new();

    // Hash the transaction fields (excluding signature)
    hasher.update(tx.from);
    if let Some(ref to) = tx.to {
        hasher.update(to);
    }
    hasher.update(tx.value.to_le_bytes());
    hasher.update(tx.nonce.to_le_bytes());
    hasher.update(&tx.data);

    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Extract r and s components from transaction signature.
///
/// shared_types::Signature is [u8; 64] = r || s
/// Returns (r, s) tuple for use with multiple recovery IDs.
fn extract_rs_from_signature(tx: &Transaction) -> Result<([u8; 32], [u8; 32]), SignatureError> {
    if tx.signature.len() != 64 {
        return Err(SignatureError::InvalidFormat);
    }

    let mut r = [0u8; 32];
    let mut s = [0u8; 32];
    r.copy_from_slice(&tx.signature[..32]);
    s.copy_from_slice(&tx.signature[32..]);

    Ok((r, s))
}

/// Derive Ethereum-style address from a 32-byte public key.
///
/// ## Key Format Issue (ARCHITECTURAL NOTE)
///
/// There is a design mismatch in shared-types:
/// - `PublicKey` is defined as `[u8; 32]` with comment "Ed25519 public key"
/// - But ECDSA uses secp256k1 which has 33-byte compressed or 65-byte uncompressed keys
/// - Ed25519 keys CANNOT be used for secp256k1 ECDSA verification
///
/// ## Current Behavior
///
/// This function treats the 32-byte key as a **33-byte compressed secp256k1 key
/// with implied even y-parity (0x02 prefix)**. This allows address derivation
/// to work consistently, but requires that:
/// 1. Transaction creators use secp256k1 keys (not Ed25519)
/// 2. The 32-byte storage is the x-coordinate of a secp256k1 point
///
/// ## Proper Fix Required
///
/// When shared-types key format is finalized, this should be updated to either:
/// - A) Accept 33-byte compressed secp256k1 keys in `Transaction.from`
/// - B) Store 20-byte addresses directly in `Transaction.from`
/// - C) Use a tagged union for different key types
fn derive_address_from_pubkey(pubkey: &[u8; 32]) -> Address {
    use k256::elliptic_curve::sec1::FromEncodedPoint;
    use k256::{AffinePoint, EncodedPoint};
    use sha3::{Digest, Keccak256};

    // Attempt to interpret as secp256k1 x-coordinate with even y-parity
    let mut compressed = [0u8; 33];
    compressed[0] = 0x02; // Even y-parity prefix
    compressed[1..].copy_from_slice(pubkey);

    // Try to decompress the point
    if let Ok(encoded) = EncodedPoint::from_bytes(compressed) {
        let ct_option = AffinePoint::from_encoded_point(&encoded);
        if ct_option.is_some().into() {
            let point: AffinePoint = ct_option.unwrap();
            // Successfully decompressed - derive address properly
            let uncompressed = EncodedPoint::from(point);
            let pubkey_bytes = uncompressed.as_bytes();

            // Keccak256 hash of uncompressed public key (skip 0x04 prefix)
            let mut hasher = Keccak256::new();
            hasher.update(&pubkey_bytes[1..]);
            let result = hasher.finalize();

            let mut address = [0u8; 20];
            address.copy_from_slice(&result[12..]);
            return address;
        }
    }

    // Fallback: If not a valid secp256k1 x-coordinate, hash raw bytes
    // This ensures deterministic behavior even for invalid keys
    let mut hasher = Keccak256::new();
    hasher.update(pubkey);
    let result = hasher.finalize();

    let mut address = [0u8; 20];
    address.copy_from_slice(&result[12..]);
    address
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ecdsa::test_helpers::{generate_keypair, sign};
    use crate::domain::ecdsa::{address_from_pubkey, keccak256};
    use crate::ports::outbound::MempoolError;
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    // =========================================================================
    // Mock MempoolGateway for testing
    // =========================================================================

    /// Mock mempool gateway that records submitted transactions.
    pub struct MockMempoolGateway {
        pub submitted: Arc<Mutex<Vec<VerifiedTransaction>>>,
    }

    impl MockMempoolGateway {
        pub fn new() -> Self {
            Self {
                submitted: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    #[async_trait]
    impl MempoolGateway for MockMempoolGateway {
        async fn submit_verified_transaction(
            &self,
            tx: VerifiedTransaction,
        ) -> Result<(), MempoolError> {
            self.submitted.lock().unwrap().push(tx);
            Ok(())
        }
    }

    // =========================================================================
    // Service Layer Tests (SPEC-10 Section 5.1 style)
    // =========================================================================

    /// Test: Service can be instantiated with mock mempool
    #[test]
    fn test_service_creation() {
        let mempool = MockMempoolGateway::new();
        let _service = SignatureVerificationService::new(mempool);
    }

    /// Test: Service delegates verify_ecdsa to domain
    #[test]
    fn test_service_verify_ecdsa_delegates() {
        let mempool = MockMempoolGateway::new();
        let service = SignatureVerificationService::new(mempool);

        let (private_key, public_key) = generate_keypair();
        let message_hash = keccak256(b"test message");
        let signature = sign(&message_hash, &private_key);

        let result = service.verify_ecdsa(&message_hash, &signature);

        assert!(result.valid);
        assert_eq!(
            result.recovered_address,
            Some(address_from_pubkey(&public_key))
        );
    }

    /// Test: Service delegates verify_ecdsa_signer to domain
    #[test]
    fn test_service_verify_ecdsa_signer_delegates() {
        let mempool = MockMempoolGateway::new();
        let service = SignatureVerificationService::new(mempool);

        let (private_key, public_key) = generate_keypair();
        let expected_address = address_from_pubkey(&public_key);
        let message_hash = keccak256(b"test message");
        let signature = sign(&message_hash, &private_key);

        let result = service.verify_ecdsa_signer(&message_hash, &signature, expected_address);

        assert!(result.valid);
    }

    /// Test: Service delegates recover_address to domain
    #[test]
    fn test_service_recover_address_delegates() {
        let mempool = MockMempoolGateway::new();
        let service = SignatureVerificationService::new(mempool);

        let (private_key, public_key) = generate_keypair();
        let expected_address = address_from_pubkey(&public_key);
        let message_hash = keccak256(b"test message");
        let signature = sign(&message_hash, &private_key);

        let recovered = service.recover_address(&message_hash, &signature).unwrap();

        assert_eq!(recovered, expected_address);
    }

    /// Test: Service delegates batch_verify_ecdsa to domain
    #[test]
    fn test_service_batch_verify_delegates() {
        use crate::domain::ecdsa::test_helpers::create_valid_verification_request;

        let mempool = MockMempoolGateway::new();
        let service = SignatureVerificationService::new(mempool);

        let requests: Vec<_> = (0..10)
            .map(|_| create_valid_verification_request())
            .collect();
        let batch_request = BatchVerificationRequest { requests };

        let result = service.batch_verify_ecdsa(&batch_request);

        assert!(result.all_valid);
        assert_eq!(result.valid_count, 10);
    }

    /// Test: Service delegates verify_bls to domain
    #[test]
    fn test_service_verify_bls_delegates() {
        use blst::min_sig::SecretKey;

        let mempool = MockMempoolGateway::new();
        let service = SignatureVerificationService::new(mempool);

        // Generate BLS keypair
        let mut ikm = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut ikm);
        let sk = SecretKey::key_gen(&ikm, &[]).unwrap();
        let pk = sk.sk_to_pk();

        let message = b"test message";
        let sig = sk.sign(message, b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_", &[]);

        let bls_sig = BlsSignature {
            bytes: sig.to_bytes(),
        };
        let bls_pk = BlsPublicKey {
            bytes: pk.to_bytes(),
        };

        let result = service.verify_bls(message, &bls_sig, &bls_pk);

        assert!(result);
    }

    /// Test: Service delegates aggregate_bls_signatures to domain
    #[test]
    fn test_service_aggregate_bls_delegates() {
        use blst::min_sig::SecretKey;

        let mempool = MockMempoolGateway::new();
        let service = SignatureVerificationService::new(mempool);

        let message = b"aggregate test";
        let mut signatures = Vec::new();

        for _ in 0..3 {
            let mut ikm = [0u8; 32];
            rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut ikm);
            let sk = SecretKey::key_gen(&ikm, &[]).unwrap();
            let sig = sk.sign(message, b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_", &[]);
            signatures.push(BlsSignature {
                bytes: sig.to_bytes(),
            });
        }

        let result = service.aggregate_bls_signatures(&signatures);

        assert!(result.is_ok());
    }

    /// Test: compute_transaction_hash is deterministic
    #[test]
    fn test_compute_transaction_hash_deterministic() {
        let tx = Transaction {
            from: [1u8; 32],
            to: Some([2u8; 32]),
            value: 1000,
            nonce: 1,
            data: vec![0xde, 0xad, 0xbe, 0xef],
            signature: [0u8; 64],
        };

        let hash1 = compute_transaction_hash(&tx);
        let hash2 = compute_transaction_hash(&tx);

        assert_eq!(hash1, hash2);
    }

    /// Test: extract_rs_from_signature extracts r and s correctly
    #[test]
    fn test_extract_rs_from_signature() {
        let mut sig = [0u8; 64];
        sig[..32].copy_from_slice(&[0xAA; 32]); // r
        sig[32..].copy_from_slice(&[0xBB; 32]); // s

        let tx = Transaction {
            from: [1u8; 32],
            to: None,
            value: 0,
            nonce: 0,
            data: vec![],
            signature: sig,
        };

        let (r, s) = extract_rs_from_signature(&tx).unwrap();

        assert_eq!(r, [0xAA; 32]);
        assert_eq!(s, [0xBB; 32]);
    }
}
