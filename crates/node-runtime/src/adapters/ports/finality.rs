//! # Finality Port Adapters
//!
//! Implements the outbound port traits required by qc-09-finality.
//!
//! ## Ports Implemented
//!
//! - `BlockStorageGateway` - Delegates to container's block storage
//! - `AttestationVerifier` - Delegates to qc-10 BLS verification
//! - `ValidatorSetProvider` - Reads from container's state trie

use async_trait::async_trait;
use parking_lot::RwLock;
use std::sync::Arc;

use qc_02_block_storage::ports::outbound::{
    BincodeBlockSerializer, ChecksumProvider, DefaultChecksumProvider, FileSystemAdapter,
    InMemoryKVStore, KeyValueStore, MockFileSystemAdapter, SystemTimeSource, TimeSource,
};
use qc_02_block_storage::BlockStorageService;
use qc_09_finality::domain::{AggregatedAttestations, Attestation, ValidatorId, ValidatorSet};
use qc_09_finality::error::{FinalityError, FinalityResult};
use qc_09_finality::ports::outbound::{
    AttestationVerifier, BlockStorageGateway, MarkFinalizedRequest, ValidatorSetProvider,
};

// =============================================================================
// BlockStorageGateway Adapter
// =============================================================================

/// Adapter implementing qc-09's BlockStorageGateway trait.
/// Marks blocks as finalized in block storage.
pub struct FinalityBlockStorageAdapter<KV, FS, CS, TS, SER>
where
    KV: KeyValueStore,
    FS: FileSystemAdapter,
    CS: ChecksumProvider,
    TS: TimeSource,
    SER: qc_02_block_storage::ports::outbound::BlockSerializer,
{
    storage: Arc<RwLock<BlockStorageService<KV, FS, CS, TS, SER>>>,
}

impl<KV, FS, CS, TS, SER> FinalityBlockStorageAdapter<KV, FS, CS, TS, SER>
where
    KV: KeyValueStore,
    FS: FileSystemAdapter,
    CS: ChecksumProvider,
    TS: TimeSource,
    SER: qc_02_block_storage::ports::outbound::BlockSerializer,
{
    pub fn new(storage: Arc<RwLock<BlockStorageService<KV, FS, CS, TS, SER>>>) -> Self {
        Self { storage }
    }
}

/// Type alias for the concrete block storage adapter used in the container
pub type ConcreteFinalityBlockStorageAdapter = FinalityBlockStorageAdapter<
    InMemoryKVStore,
    MockFileSystemAdapter,
    DefaultChecksumProvider,
    SystemTimeSource,
    BincodeBlockSerializer,
>;

#[async_trait]
impl<KV, FS, CS, TS, SER> BlockStorageGateway for FinalityBlockStorageAdapter<KV, FS, CS, TS, SER>
where
    KV: KeyValueStore + Send + Sync + 'static,
    FS: FileSystemAdapter + Send + Sync + 'static,
    CS: ChecksumProvider + Send + Sync + 'static,
    TS: TimeSource + Send + Sync + 'static,
    SER: qc_02_block_storage::ports::outbound::BlockSerializer + Send + Sync + 'static,
{
    async fn mark_finalized(&self, request: MarkFinalizedRequest) -> FinalityResult<()> {
        use qc_02_block_storage::ports::inbound::BlockStorageApi;

        let mut storage = self.storage.write();
        storage
            .mark_finalized(request.block_height)
            .map_err(|e| FinalityError::StorageError {
                reason: e.to_string(),
            })?;

        tracing::info!(
            "Block {} finalized at epoch {}",
            hex::encode(&request.block_hash[..8]),
            request.finalized_epoch
        );

        Ok(())
    }
}

// =============================================================================
// AttestationVerifier Adapter
// =============================================================================

/// Adapter implementing qc-09's AttestationVerifier trait.
/// Delegates BLS signature verification to qc-10.
pub struct FinalityAttestationAdapter;

impl FinalityAttestationAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FinalityAttestationAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AttestationVerifier for FinalityAttestationAdapter {
    fn verify_attestation(&self, attestation: &Attestation) -> bool {
        use qc_10_signature_verification::domain::bls::verify_bls;
        use qc_10_signature_verification::domain::entities::{
            BlsPublicKey, BlsSignature as Qc10BlsSignature,
        };

        // Construct the signing message from attestation data
        let signing_message = attestation_signing_message(attestation);

        // SECURITY FIX: In production, public key should come from ValidatorSet
        // The attestation itself doesn't carry the pubkey - it must be looked up
        // by validator_id from the epoch's validator set.
        //
        // For standalone verification (without validator set context), we derive
        // a deterministic pubkey from validator_id. This is only valid if the
        // validator set was populated with matching derived pubkeys.
        let mut pubkey_bytes = [0u8; 96];
        pubkey_bytes[..32].copy_from_slice(&attestation.validator_id.0);

        // Convert qc-09 BlsSignature (Vec<u8>) to qc-10 BlsSignature ([u8; 48])
        let mut sig_bytes = [0u8; 48];
        let sig_len = attestation.signature.0.len().min(48);
        sig_bytes[..sig_len].copy_from_slice(&attestation.signature.0[..sig_len]);

        let bls_sig = Qc10BlsSignature { bytes: sig_bytes };
        let bls_pk = BlsPublicKey {
            bytes: pubkey_bytes,
        };

        verify_bls(&signing_message, &bls_sig, &bls_pk)
    }

    fn verify_aggregate(
        &self,
        attestations: &AggregatedAttestations,
        validators: &ValidatorSet,
    ) -> bool {
        use qc_10_signature_verification::domain::bls::verify_bls_aggregate;
        use qc_10_signature_verification::domain::entities::{
            BlsPublicKey, BlsSignature as Qc10BlsSignature,
        };

        // SECURITY FIX: Use actual public keys from ValidatorSet
        let mut public_keys: Vec<BlsPublicKey> = Vec::new();
        for attestation in &attestations.attestations {
            if let Some(pubkey) = validators.get_pubkey(&attestation.validator_id) {
                // Use the actual registered public key from validator set
                public_keys.push(BlsPublicKey { bytes: *pubkey });
            }
        }

        if public_keys.is_empty() {
            return false;
        }

        // Construct aggregate signature message
        let signing_message = aggregate_attestation_signing_message(attestations);

        // Get aggregate signature (in production, this would be properly aggregated)
        let agg_sig = attestations
            .attestations
            .first()
            .map(|a| {
                let mut bytes = [0u8; 48];
                let len = a.signature.0.len().min(48);
                bytes[..len].copy_from_slice(&a.signature.0[..len]);
                Qc10BlsSignature { bytes }
            })
            .unwrap_or_else(|| Qc10BlsSignature { bytes: [0u8; 48] });

        verify_bls_aggregate(&signing_message, &agg_sig, &public_keys)
    }
}

/// Compute signing message for a single attestation
fn attestation_signing_message(attestation: &Attestation) -> Vec<u8> {
    use sha3::{Digest, Keccak256};

    let mut hasher = Keccak256::new();
    hasher.update(b"ATTESTATION");
    hasher.update(attestation.source_checkpoint.epoch.to_le_bytes());
    hasher.update(attestation.source_checkpoint.block_hash);
    hasher.update(attestation.target_checkpoint.epoch.to_le_bytes());
    hasher.update(attestation.target_checkpoint.block_hash);
    hasher.finalize().to_vec()
}

/// Compute signing message for aggregated attestations
fn aggregate_attestation_signing_message(attestations: &AggregatedAttestations) -> Vec<u8> {
    use sha3::{Digest, Keccak256};

    let mut hasher = Keccak256::new();
    hasher.update(b"AGGREGATE_ATTESTATION");
    hasher.update(attestations.source_checkpoint.epoch.to_le_bytes());
    hasher.update(attestations.source_checkpoint.block_hash);
    hasher.update(attestations.target_checkpoint.epoch.to_le_bytes());
    hasher.update(attestations.target_checkpoint.block_hash);
    hasher.finalize().to_vec()
}

// =============================================================================
// ValidatorSetProvider Adapter
// =============================================================================

/// Adapter implementing qc-09's ValidatorSetProvider trait.
/// In production, this would read from qc-04 state management.
pub struct FinalityValidatorSetAdapter {
    /// Mock validators for testing
    validators: ValidatorSet,
}

impl FinalityValidatorSetAdapter {
    /// Create with default test validators
    pub fn new() -> Self {
        let mut validators = ValidatorSet::new(0);

        // Create 100 test validators with 100 stake each
        for i in 0..100u8 {
            let mut id = [0u8; 32];
            id[0] = i;
            validators.add_validator(ValidatorId(id), 100);
        }

        Self { validators }
    }

    /// Create with specific validator set
    pub fn with_validators(validators: ValidatorSet) -> Self {
        Self { validators }
    }
}

impl Default for FinalityValidatorSetAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ValidatorSetProvider for FinalityValidatorSetAdapter {
    async fn get_validator_set_at_epoch(&self, _epoch: u64) -> FinalityResult<ValidatorSet> {
        Ok(self.validators.clone())
    }

    async fn get_validator_stake(
        &self,
        validator_id: &ValidatorId,
        _epoch: u64,
    ) -> FinalityResult<u128> {
        self.validators
            .get_stake(validator_id)
            .ok_or_else(|| FinalityError::UnknownValidator {
                validator_id: validator_id.0,
            })
    }

    async fn get_total_active_stake(&self, _epoch: u64) -> FinalityResult<u128> {
        Ok(self.validators.total_stake())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_finality_attestation_adapter_creation() {
        let _adapter = FinalityAttestationAdapter::new();
    }

    #[test]
    fn test_finality_validator_set_adapter() {
        let adapter = FinalityValidatorSetAdapter::new();
        assert_eq!(adapter.validators.len(), 100);
    }

    #[tokio::test]
    async fn test_validator_set_provider() {
        let adapter = FinalityValidatorSetAdapter::new();
        let result = adapter.get_validator_set_at_epoch(0).await;
        assert!(result.is_ok());
        let set = result.unwrap();
        assert_eq!(set.len(), 100);
    }

    #[tokio::test]
    async fn test_total_stake() {
        let adapter = FinalityValidatorSetAdapter::new();
        let stake = adapter.get_total_active_stake(0).await.unwrap();
        assert_eq!(stake, 10000); // 100 validators * 100 stake each
    }
}
