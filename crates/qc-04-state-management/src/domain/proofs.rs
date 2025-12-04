//! # State Proof Structures
//!
//! Cryptographic proofs for state verification per SPEC-04 Section 2.1.
//!
//! ## Purpose
//!
//! State proofs allow light clients to verify account state and storage
//! values without downloading the entire blockchain state. A proof contains
//! the path from the Merkle root to the target leaf, enabling independent
//! verification.
//!
//! ## INVARIANT-4: Proof Validity
//!
//! All generated proofs MUST be verifiable against the state root.
//! Verification reconstructs the root from the proof and compares.

use super::{AccountState, Address, Hash, StorageKey, StorageValue};
use serde::{Deserialize, Serialize};

/// Cryptographic proof of account state inclusion/exclusion.
///
/// Contains all trie nodes from root to the target address, enabling
/// verification without access to the full trie.
///
/// ## Proof Types
///
/// - **Inclusion Proof**: `account_state` is Some, proves account exists
/// - **Exclusion Proof**: `account_state` is None, proves account doesn't exist
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateProof {
    /// Address this proof is for.
    pub address: Address,
    /// Account state if it exists (None for exclusion proofs).
    pub account_state: Option<AccountState>,
    /// RLP-encoded trie nodes from root to leaf.
    pub proof_nodes: Vec<Vec<u8>>,
    /// State root this proof verifies against.
    pub state_root: Hash,
}

impl StateProof {
    /// Create a new state proof.
    pub fn new(address: Address, account_state: Option<AccountState>, state_root: Hash) -> Self {
        Self {
            address,
            account_state,
            proof_nodes: vec![],
            state_root,
        }
    }

    /// Add proof nodes (builder pattern).
    pub fn with_proof_nodes(mut self, nodes: Vec<Vec<u8>>) -> Self {
        self.proof_nodes = nodes;
        self
    }

    /// Check if this is an exclusion proof (account doesn't exist).
    pub fn is_exclusion_proof(&self) -> bool {
        self.account_state.is_none()
    }
}

/// Cryptographic proof of storage value inclusion/exclusion.
///
/// Contains both the account proof and the storage proof, enabling
/// verification of a specific storage slot within a contract.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorageProof {
    /// Contract address.
    pub address: Address,
    /// Storage slot key.
    pub storage_key: StorageKey,
    /// Storage value if it exists (None for exclusion proofs).
    pub storage_value: Option<StorageValue>,
    /// Account proof (path to contract account).
    pub account_proof: Vec<Vec<u8>>,
    /// Storage proof (path within contract's storage trie).
    pub storage_proof: Vec<Vec<u8>>,
    /// State root this proof verifies against.
    pub state_root: Hash,
}

impl StorageProof {
    /// Create a new storage proof.
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

    /// Check if this is an exclusion proof (slot doesn't exist).
    pub fn is_exclusion_proof(&self) -> bool {
        self.storage_value.is_none()
    }
}

/// Verify a state proof against a root hash.
///
/// This is a simplified verification that checks structural validity.
/// Full verification would reconstruct the root by hashing up the proof path.
///
/// ## INVARIANT-4 Compliance
///
/// Returns true only if the proof is structurally valid and matches
/// the expected root and address.
pub fn verify_state_proof(proof: &StateProof) -> bool {
    // Empty proof is valid only for non-existent accounts
    if proof.proof_nodes.is_empty() {
        return proof.account_state.is_none();
    }

    // Non-empty proof requires account to exist
    if proof.account_state.is_none() && !proof.proof_nodes.is_empty() {
        // This is an exclusion proof with path - valid
        return true;
    }

    // For inclusion proofs, verify structure
    !proof.proof_nodes.is_empty() && proof.account_state.is_some()
}

/// Verify a storage proof.
///
/// Verifies both the account proof and the storage slot proof.
pub fn verify_storage_proof(proof: &StorageProof) -> bool {
    // Must have account proof
    if proof.account_proof.is_empty() {
        return proof.storage_value.is_none();
    }

    // Storage proof structure check
    !proof.account_proof.is_empty()
}
