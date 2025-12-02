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
pub struct SignatureVerificationService<M: MempoolGateway> {
    #[allow(dead_code)]
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

    fn verify_transaction(
        &self,
        transaction: Transaction,
    ) -> Result<VerifiedTransaction, SignatureError> {
        // 1. Compute transaction hash (the message that was signed)
        let tx_hash = compute_transaction_hash(&transaction);

        // 2. Extract ECDSA signature from transaction
        //    shared_types::Signature is [u8; 64] = r || s (no v)
        //    We need to try both recovery IDs (0 and 1)
        let signature = extract_ecdsa_signature(&transaction)?;

        // 3. Verify signature recovers to the claimed sender
        //    The sender's public key is in transaction.from
        let expected_sender = derive_address_from_pubkey(&transaction.from);
        let result = self.verify_ecdsa_signer(&tx_hash, &signature, expected_sender);

        if !result.valid {
            return Err(result.error.unwrap_or(SignatureError::VerificationFailed));
        }

        // 4. Return verified transaction
        Ok(VerifiedTransaction {
            transaction,
            sender: expected_sender,
            signature_valid: true,
        })
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

/// Extract ECDSA signature from transaction.
///
/// shared_types::Signature is [u8; 64] = r || s
/// We need to determine the recovery ID by trying both values.
fn extract_ecdsa_signature(tx: &Transaction) -> Result<EcdsaSignature, SignatureError> {
    if tx.signature.len() != 64 {
        return Err(SignatureError::InvalidFormat);
    }

    let mut r = [0u8; 32];
    let mut s = [0u8; 32];
    r.copy_from_slice(&tx.signature[..32]);
    s.copy_from_slice(&tx.signature[32..]);

    // Try recovery ID 27 first (Ethereum convention)
    // If verification fails, the caller will try 28
    // For now, we'll use 27 as default and let verify_ecdsa_signer handle recovery
    Ok(EcdsaSignature { r, s, v: 27 })
}

/// Derive Ethereum-style address from a 32-byte public key.
///
/// Note: shared_types uses 32-byte compressed public keys.
/// For ECDSA address derivation, we need the uncompressed form.
/// This is a placeholder - actual implementation depends on key format.
fn derive_address_from_pubkey(pubkey: &[u8; 32]) -> Address {
    use sha3::{Digest, Keccak256};

    // For compressed secp256k1 public keys, we'd need to decompress first.
    // For now, we hash the raw bytes and take last 20 bytes.
    // TODO: Verify this matches the actual key format used in shared_types.
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

    /// Test: extract_ecdsa_signature extracts r and s correctly
    #[test]
    fn test_extract_ecdsa_signature() {
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

        let ecdsa_sig = extract_ecdsa_signature(&tx).unwrap();

        assert_eq!(ecdsa_sig.r, [0xAA; 32]);
        assert_eq!(ecdsa_sig.s, [0xBB; 32]);
        assert_eq!(ecdsa_sig.v, 27);
    }
}
