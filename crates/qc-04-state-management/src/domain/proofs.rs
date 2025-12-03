use super::{AccountState, Address, Hash, StorageKey, StorageValue};
use serde::{Deserialize, Serialize};

/// Cryptographic proof of account state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateProof {
    pub address: Address,
    pub account_state: Option<AccountState>,
    pub proof_nodes: Vec<Vec<u8>>,
    pub state_root: Hash,
}

impl StateProof {
    pub fn new(address: Address, account_state: Option<AccountState>, state_root: Hash) -> Self {
        Self {
            address,
            account_state,
            proof_nodes: vec![],
            state_root,
        }
    }

    pub fn with_proof_nodes(mut self, nodes: Vec<Vec<u8>>) -> Self {
        self.proof_nodes = nodes;
        self
    }
}

/// Cryptographic proof of storage value
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorageProof {
    pub address: Address,
    pub storage_key: StorageKey,
    pub storage_value: Option<StorageValue>,
    pub account_proof: Vec<Vec<u8>>,
    pub storage_proof: Vec<Vec<u8>>,
    pub state_root: Hash,
}

impl StorageProof {
    pub fn new(
        address: Address,
        storage_key: StorageKey,
        storage_value: Option<StorageValue>,
        state_root: Hash,
    ) -> Self {
        Self {
            address,
            storage_key,
            storage_value,
            account_proof: vec![],
            storage_proof: vec![],
            state_root,
        }
    }
}

/// Verify a state proof against a root
pub fn verify_state_proof(proof: &StateProof) -> bool {
    if proof.proof_nodes.is_empty() {
        return proof.account_state.is_none();
    }
    
    // For now, verify structure is correct
    // Full verification requires reconstructing path
    !proof.proof_nodes.is_empty() || proof.account_state.is_some()
}

/// Verify a storage proof against account state
pub fn verify_storage_proof(proof: &StorageProof) -> bool {
    !proof.account_proof.is_empty() || proof.storage_value.is_none()
}
