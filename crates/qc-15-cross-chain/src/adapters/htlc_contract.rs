//! HTLC Contract Adapter
//!
//! Implements `HTLCContract` port for HTLC operations.
//! Reference: SPEC-15 Section 3.2

use crate::domain::{ChainId, CrossChainError, CrossChainProof, Hash, Secret};
use crate::ports::outbound::{HTLCContract, HTLCDeployParams};
use async_trait::async_trait;
use parking_lot::RwLock;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use tracing::{debug, info};

/// In-memory HTLC contract for testing.
///
/// In production, this would deploy actual smart contracts.
pub struct InMemoryHTLCContract {
    /// Deployed HTLCs: (chain, htlc_id) -> HTLCState.
    htlcs: RwLock<HashMap<(ChainId, Hash), HTLCData>>,
    /// Block heights per chain (for timestamp simulation).
    current_time: RwLock<u64>,
}

/// Internal HTLC state.
#[derive(Clone, Debug)]
struct HTLCData {
    params: HTLCDeployParams,
    state: HTLCInternalState,
    claim_tx: Option<Hash>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HTLCInternalState {
    Locked,
    Claimed,
    Refunded,
}

impl InMemoryHTLCContract {
    /// Create a new contract.
    pub fn new() -> Self {
        Self {
            htlcs: RwLock::new(HashMap::new()),
            current_time: RwLock::new(1_700_000_000),
        }
    }

    /// Set current time for testing.
    pub fn set_time(&self, time: u64) {
        *self.current_time.write() = time;
    }

    /// Advance time for testing.
    pub fn advance_time(&self, secs: u64) {
        *self.current_time.write() += secs;
    }
}

impl Default for InMemoryHTLCContract {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate HTLC ID from params.
fn generate_htlc_id(params: &HTLCDeployParams) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(&(params.chain as u8).to_le_bytes());
    hasher.update(&params.hash_lock);
    hasher.update(&params.time_lock.to_le_bytes());
    hasher.update(&params.amount.to_le_bytes());
    hasher.update(&params.sender);
    hasher.update(&params.recipient);

    let result = hasher.finalize();
    let mut id = [0u8; 32];
    id.copy_from_slice(&result);
    id
}

#[async_trait]
impl HTLCContract for InMemoryHTLCContract {
    async fn deploy(&self, params: HTLCDeployParams) -> Result<Hash, CrossChainError> {
        let htlc_id = generate_htlc_id(&params);

        info!(
            "[qc-15] Deploying HTLC {:02x}{:02x}... on {:?}",
            htlc_id[0], htlc_id[1], params.chain
        );

        let data = HTLCData {
            params: params.clone(),
            state: HTLCInternalState::Locked,
            claim_tx: None,
        };

        self.htlcs.write().insert((params.chain, htlc_id), data);

        Ok(htlc_id)
    }

    async fn claim(
        &self,
        chain: ChainId,
        htlc_id: Hash,
        secret: Secret,
    ) -> Result<(), CrossChainError> {
        debug!(
            "[qc-15] Claiming HTLC {:02x}{:02x}... on {:?}",
            htlc_id[0], htlc_id[1], chain
        );

        let mut htlcs = self.htlcs.write();
        let data = htlcs
            .get_mut(&(chain, htlc_id))
            .ok_or(CrossChainError::HTLCNotFound(htlc_id))?;

        // Check state
        if data.state != HTLCInternalState::Locked {
            return Err(CrossChainError::InvalidHTLCTransition {
                from: format!("{:?}", data.state),
                to: "Claimed".to_string(),
            });
        }

        // Verify secret
        let mut hasher = Sha256::new();
        hasher.update(&secret);
        let hash_result = hasher.finalize();
        let mut computed_hash = [0u8; 32];
        computed_hash.copy_from_slice(&hash_result);

        if computed_hash != data.params.hash_lock {
            return Err(CrossChainError::InvalidSecret);
        }

        // Check timelock
        let current = *self.current_time.read();
        if current > data.params.time_lock {
            return Err(CrossChainError::HTLCExpired);
        }

        // Update state
        data.state = HTLCInternalState::Claimed;
        data.claim_tx = Some({
            let mut tx = [0u8; 32];
            tx[..8].copy_from_slice(&current.to_le_bytes());
            tx[8..].copy_from_slice(&htlc_id[..24]);
            tx
        });

        Ok(())
    }

    async fn refund(&self, chain: ChainId, htlc_id: Hash) -> Result<(), CrossChainError> {
        debug!(
            "[qc-15] Refunding HTLC {:02x}{:02x}... on {:?}",
            htlc_id[0], htlc_id[1], chain
        );

        let mut htlcs = self.htlcs.write();
        let data = htlcs
            .get_mut(&(chain, htlc_id))
            .ok_or(CrossChainError::HTLCNotFound(htlc_id))?;

        // Check state
        if data.state != HTLCInternalState::Locked {
            return Err(CrossChainError::InvalidHTLCTransition {
                from: format!("{:?}", data.state),
                to: "Refunded".to_string(),
            });
        }

        // Check timelock expired
        let current = *self.current_time.read();
        if current <= data.params.time_lock {
            return Err(CrossChainError::HTLCNotExpired);
        }

        data.state = HTLCInternalState::Refunded;
        Ok(())
    }

    async fn get_proof(
        &self,
        chain: ChainId,
        htlc_id: Hash,
    ) -> Result<CrossChainProof, CrossChainError> {
        let htlcs = self.htlcs.read();
        let data = htlcs
            .get(&(chain, htlc_id))
            .ok_or(CrossChainError::HTLCNotFound(htlc_id))?;

        // Generate mock proof
        let proof = CrossChainProof {
            chain,
            block_hash: {
                let mut h = [0u8; 32];
                h[..8].copy_from_slice(&1000u64.to_le_bytes());
                h
            },
            block_height: 1000,
            tx_hash: data.claim_tx.unwrap_or(htlc_id),
            merkle_proof: vec![[1u8; 32], [2u8; 32]],
            confirmations: 10,
        };

        Ok(proof)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_params() -> HTLCDeployParams {
        // Secret: [1u8; 32], hash it for hash_lock
        let secret = [1u8; 32];
        let mut hasher = Sha256::new();
        hasher.update(&secret);
        let mut hash_lock = [0u8; 32];
        hash_lock.copy_from_slice(&hasher.finalize());

        HTLCDeployParams {
            chain: ChainId::QuantumChain,
            hash_lock,
            time_lock: 1_700_100_000,
            amount: 1000,
            sender: [1u8; 20],
            recipient: [2u8; 20],
        }
    }

    #[tokio::test]
    async fn test_deploy_htlc() {
        let contract = InMemoryHTLCContract::new();
        let params = create_test_params();

        let htlc_id = contract.deploy(params).await.unwrap();
        assert_ne!(htlc_id, [0u8; 32]);
    }

    #[tokio::test]
    async fn test_claim_with_valid_secret() {
        let contract = InMemoryHTLCContract::new();
        let params = create_test_params();
        let htlc_id = contract.deploy(params.clone()).await.unwrap();

        let secret = [1u8; 32];
        let result = contract.claim(params.chain, htlc_id, secret).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_claim_with_invalid_secret_fails() {
        let contract = InMemoryHTLCContract::new();
        let params = create_test_params();
        let htlc_id = contract.deploy(params.clone()).await.unwrap();

        let bad_secret = [99u8; 32];
        let result = contract.claim(params.chain, htlc_id, bad_secret).await;
        assert!(matches!(result, Err(CrossChainError::InvalidSecret)));
    }

    #[tokio::test]
    async fn test_claim_after_expiry_fails() {
        let contract = InMemoryHTLCContract::new();
        let params = create_test_params();
        let htlc_id = contract.deploy(params.clone()).await.unwrap();

        // Advance past timelock
        contract.set_time(1_700_200_000);

        let secret = [1u8; 32];
        let result = contract.claim(params.chain, htlc_id, secret).await;
        assert!(matches!(result, Err(CrossChainError::HTLCExpired)));
    }

    #[tokio::test]
    async fn test_refund_after_expiry() {
        let contract = InMemoryHTLCContract::new();
        let params = create_test_params();
        let htlc_id = contract.deploy(params.clone()).await.unwrap();

        // Advance past timelock
        contract.set_time(1_700_200_000);

        let result = contract.refund(params.chain, htlc_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_refund_before_expiry_fails() {
        let contract = InMemoryHTLCContract::new();
        let params = create_test_params();
        let htlc_id = contract.deploy(params.clone()).await.unwrap();

        let result = contract.refund(params.chain, htlc_id).await;
        assert!(matches!(result, Err(CrossChainError::HTLCNotExpired)));
    }

    #[tokio::test]
    async fn test_get_proof() {
        let contract = InMemoryHTLCContract::new();
        let params = create_test_params();
        let htlc_id = contract.deploy(params.clone()).await.unwrap();

        let proof = contract.get_proof(params.chain, htlc_id).await.unwrap();
        assert_eq!(proof.chain, ChainId::QuantumChain);
        assert!(!proof.merkle_proof.is_empty());
    }
}
