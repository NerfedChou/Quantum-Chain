//! # Inbound Ports (Driving Ports / API)
//!
//! Traits that define the public API of this subsystem.
//!
//! Reference: SPEC-10 Section 3.1 (Driving Ports)

use crate::domain::entities::{
    Address, BatchVerificationRequest, BatchVerificationResult, BlsPublicKey, BlsSignature,
    EcdsaSignature, VerificationResult, VerifiedTransaction,
};
use crate::domain::errors::SignatureError;
use shared_types::{Hash, Transaction};

/// Primary Signature Verification API.
///
/// Reference: SPEC-10 Section 3.1
///
/// This is the main entry point for signature verification operations.
/// Implementations must be thread-safe (`Send + Sync`).
pub trait SignatureVerificationApi: Send + Sync {
    // =========================================================================
    // ECDSA Operations
    // =========================================================================

    /// Verify an ECDSA signature and recover the signer address.
    ///
    /// Reference: SPEC-10 Section 3.1 `verify_ecdsa`
    ///
    /// # Security
    /// - Rejects signatures with high S values (EIP-2 malleability protection)
    fn verify_ecdsa(&self, message_hash: &Hash, signature: &EcdsaSignature) -> VerificationResult;

    /// Verify an ECDSA signature and check that recovered signer matches expected.
    ///
    /// Reference: SPEC-10 Section 3.1 `verify_ecdsa_signer`
    fn verify_ecdsa_signer(
        &self,
        message_hash: &Hash,
        signature: &EcdsaSignature,
        expected: Address,
    ) -> VerificationResult;

    /// Recover the signer's Ethereum address from a signature.
    ///
    /// Reference: SPEC-10 Section 3.1 `recover_address`
    fn recover_address(
        &self,
        message_hash: &Hash,
        signature: &EcdsaSignature,
    ) -> Result<Address, SignatureError>;

    /// Batch verify multiple ECDSA signatures in parallel.
    ///
    /// Reference: SPEC-10 Section 3.1 `batch_verify_ecdsa`
    ///
    /// # Performance
    /// Uses parallel processing for improved throughput.
    fn batch_verify_ecdsa(&self, request: &BatchVerificationRequest) -> BatchVerificationResult;

    // =========================================================================
    // BLS Operations
    // =========================================================================

    /// Verify a single BLS signature.
    ///
    /// Reference: SPEC-10 Section 3.1 `verify_bls`
    fn verify_bls(
        &self,
        message: &[u8],
        signature: &BlsSignature,
        public_key: &BlsPublicKey,
    ) -> bool;

    /// Verify an aggregated BLS signature against multiple public keys.
    ///
    /// Reference: SPEC-10 Section 3.1 `verify_bls_aggregate`
    fn verify_bls_aggregate(
        &self,
        message: &[u8],
        aggregate_signature: &BlsSignature,
        public_keys: &[BlsPublicKey],
    ) -> bool;

    /// Aggregate multiple BLS signatures into one.
    ///
    /// Reference: SPEC-10 Section 3.1 `aggregate_bls_signatures`
    fn aggregate_bls_signatures(
        &self,
        signatures: &[BlsSignature],
    ) -> Result<BlsSignature, SignatureError>;

    // =========================================================================
    // Transaction Verification
    // =========================================================================

    /// Verify a signed transaction and prepare it for Mempool submission.
    ///
    /// Reference: SPEC-10 Section 3.1 `verify_transaction`
    ///
    /// This is the primary entry point for transaction verification from the network.
    fn verify_transaction(
        &self,
        transaction: Transaction,
    ) -> Result<VerifiedTransaction, SignatureError>;
}
