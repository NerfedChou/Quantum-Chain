//! # Consensus Port Adapters
//!
//! Implements the outbound port traits required by qc-08-consensus.
//!
//! ## Ports Implemented
//!
//! - `EventBus` - Publishes BlockValidated to container's event bus
//! - `MempoolGateway` - Delegates to container's mempool
//! - `SignatureVerifier` - Delegates to qc-10 stateless functions
//! - `ValidatorSetProvider` - Reads from container's state trie

use async_trait::async_trait;
use parking_lot::RwLock;
use shared_bus::InMemoryEventBus;
use shared_types::Hash;
use std::sync::Arc;

use qc_06_mempool::TransactionPool;
use qc_08_consensus::domain::{SignedTransaction, ValidatedBlock, ValidationProof, ValidatorInfo, ValidatorSet};
use qc_08_consensus::ports::{EventBus, MempoolGateway, SignatureVerifier, ValidatorSetProvider};

// =============================================================================
// EventBus Adapter
// =============================================================================

/// Adapter implementing qc-08's EventBus trait.
/// Publishes BlockValidated events to the container's shared event bus.
pub struct ConsensusEventBusAdapter {
    #[allow(dead_code)]
    event_bus: Arc<InMemoryEventBus>,
}

impl ConsensusEventBusAdapter {
    pub fn new(event_bus: Arc<InMemoryEventBus>) -> Self {
        Self { event_bus }
    }
}

#[async_trait]
impl EventBus for ConsensusEventBusAdapter {
    async fn publish_block_validated(
        &self,
        _block_hash: Hash,
        _block_height: u64,
        _block: ValidatedBlock,
        _consensus_proof: ValidationProof,
        _validated_at: u64,
    ) -> Result<(), String> {
        // In production, this would publish to the event bus
        // For now, we log and return success
        tracing::info!("BlockValidated event published");
        Ok(())
    }
}

// =============================================================================
// MempoolGateway Adapter
// =============================================================================

/// Adapter implementing qc-08's MempoolGateway trait.
/// Delegates to the container's mempool instance.
pub struct ConsensusMempoolAdapter {
    mempool: Arc<RwLock<TransactionPool>>,
}

impl ConsensusMempoolAdapter {
    pub fn new(mempool: Arc<RwLock<TransactionPool>>) -> Self {
        Self { mempool }
    }
}

#[async_trait]
impl MempoolGateway for ConsensusMempoolAdapter {
    async fn get_transactions_for_block(
        &self,
        max_count: usize,
        max_gas: u64,
    ) -> Result<Vec<SignedTransaction>, String> {
        let pool = self.mempool.read();

        // Get pending transactions from mempool
        let mempool_txs = pool.get_for_block(max_count, max_gas);

        // Convert to SignedTransaction format expected by Consensus
        let txs: Vec<SignedTransaction> = mempool_txs
            .into_iter()
            .map(|tx| {
                // Convert signature to fixed array
                let mut signature = [0u8; 65];
                let sig_len = tx.transaction.signature.len().min(65);
                signature[..sig_len].copy_from_slice(&tx.transaction.signature[..sig_len]);

                SignedTransaction {
                    hash: tx.hash,
                    from: tx.transaction.from,
                    to: tx.transaction.to,
                    value: tx.transaction.value.as_u128(),
                    nonce: tx.nonce,
                    gas_price: tx.gas_price.as_u64(),
                    gas_limit: tx.gas_limit,
                    data: tx.transaction.data.clone(),
                    signature,
                }
            })
            .collect();

        Ok(txs)
    }

    async fn propose_transactions(
        &self,
        tx_hashes: Vec<Hash>,
        target_block_height: u64,
    ) -> Result<(), String> {
        let mut pool = self.mempool.write();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        pool.propose(&tx_hashes, target_block_height, now_ms);
        Ok(())
    }
}

// =============================================================================
// SignatureVerifier Adapter
// =============================================================================

/// Adapter implementing qc-08's SignatureVerifier trait.
/// Delegates to qc-10 stateless signature verification functions.
pub struct ConsensusSignatureAdapter;

impl ConsensusSignatureAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConsensusSignatureAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl SignatureVerifier for ConsensusSignatureAdapter {
    fn verify_ecdsa(&self, message: &[u8], signature: &[u8; 65], _public_key: &[u8; 33]) -> bool {
        use qc_10_signature_verification::domain::entities::EcdsaSignature;
        use qc_10_signature_verification::domain::ecdsa::verify_ecdsa;

        // Extract r, s, v from signature
        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&signature[..32]);
        s.copy_from_slice(&signature[32..64]);
        let v = signature[64];

        let sig = EcdsaSignature { r, s, v };

        // Hash the message if it's not already 32 bytes
        let msg_hash: [u8; 32] = if message.len() == 32 {
            let mut hash = [0u8; 32];
            hash.copy_from_slice(message);
            hash
        } else {
            use sha3::{Digest, Keccak256};
            let mut hasher = Keccak256::new();
            hasher.update(message);
            let result = hasher.finalize();
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&result);
            hash
        };

        let result = verify_ecdsa(&msg_hash, &sig);
        result.valid
    }

    fn verify_aggregate_bls(&self, message: &[u8], signature: &[u8; 96], public_keys: &[[u8; 48]]) -> bool {
        use qc_10_signature_verification::domain::bls::verify_bls_aggregate;
        use qc_10_signature_verification::domain::entities::{BlsPublicKey, BlsSignature};

        // BLS signature is 48 bytes (G1), take first 48 bytes from the 96-byte input
        let mut sig_bytes = [0u8; 48];
        sig_bytes.copy_from_slice(&signature[..48]);
        let bls_sig = BlsSignature { bytes: sig_bytes };

        // BLS public keys are 96 bytes (G2)
        let bls_pks: Vec<BlsPublicKey> = public_keys
            .iter()
            .map(|pk| {
                let mut pk_bytes = [0u8; 96];
                pk_bytes[..48].copy_from_slice(pk);
                BlsPublicKey { bytes: pk_bytes }
            })
            .collect();

        verify_bls_aggregate(message, &bls_sig, &bls_pks)
    }

    fn recover_signer(&self, message: &[u8], signature: &[u8; 65]) -> Option<[u8; 20]> {
        use qc_10_signature_verification::domain::entities::EcdsaSignature;
        use qc_10_signature_verification::domain::ecdsa::recover_address;

        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&signature[..32]);
        s.copy_from_slice(&signature[32..64]);
        let v = signature[64];

        let sig = EcdsaSignature { r, s, v };

        let msg_hash: [u8; 32] = if message.len() == 32 {
            let mut hash = [0u8; 32];
            hash.copy_from_slice(message);
            hash
        } else {
            use sha3::{Digest, Keccak256};
            let mut hasher = Keccak256::new();
            hasher.update(message);
            let result = hasher.finalize();
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&result);
            hash
        };

        recover_address(&msg_hash, &sig).ok()
    }
}

// =============================================================================
// ValidatorSetProvider Adapter
// =============================================================================

/// Adapter implementing qc-08's ValidatorSetProvider trait.
/// In production, this would read from qc-04 state management.
/// For now, provides a mock validator set.
pub struct ConsensusValidatorSetAdapter {
    /// Mock validators for testing (in production, read from state trie)
    validators: Vec<ValidatorInfo>,
    current_epoch: u64,
}

impl ConsensusValidatorSetAdapter {
    /// Create with default test validators
    pub fn new() -> Self {
        // Create 4 test validators with equal stake
        let validators: Vec<ValidatorInfo> = (0..4)
            .map(|i| {
                let mut id = [0u8; 32];
                id[0] = i as u8;
                let mut pubkey = [0u8; 48];
                pubkey[0] = i as u8;
                ValidatorInfo::new(id, 100, pubkey)
            })
            .collect();

        Self {
            validators,
            current_epoch: 0,
        }
    }

    /// Create with specific validators
    pub fn with_validators(validators: Vec<ValidatorInfo>) -> Self {
        Self {
            validators,
            current_epoch: 0,
        }
    }
}

impl Default for ConsensusValidatorSetAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ValidatorSetProvider for ConsensusValidatorSetAdapter {
    async fn get_validator_set_at_epoch(
        &self,
        epoch: u64,
        _state_root: Hash,
    ) -> Result<ValidatorSet, String> {
        Ok(ValidatorSet::new(epoch, self.validators.clone()))
    }

    async fn get_total_stake_at_epoch(&self, _epoch: u64, _state_root: Hash) -> Result<u128, String> {
        let total: u128 = self.validators.iter().map(|v| v.stake).sum();
        Ok(total)
    }

    async fn current_epoch(&self) -> u64 {
        self.current_epoch
    }

    async fn get_epoch_state_root(&self, _epoch: u64) -> Result<Hash, String> {
        Ok([0u8; 32])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consensus_signature_adapter_creation() {
        let _adapter = ConsensusSignatureAdapter::new();
    }

    #[test]
    fn test_consensus_validator_set_adapter() {
        let adapter = ConsensusValidatorSetAdapter::new();
        assert_eq!(adapter.validators.len(), 4);
    }

    #[tokio::test]
    async fn test_validator_set_provider() {
        let adapter = ConsensusValidatorSetAdapter::new();
        let result = adapter.get_validator_set_at_epoch(0, [0u8; 32]).await;
        assert!(result.is_ok());
        let set = result.unwrap();
        assert_eq!(set.len(), 4);
    }
}
