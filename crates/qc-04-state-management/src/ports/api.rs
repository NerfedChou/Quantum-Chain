use crate::domain::{
    AccountState, Address, ConflictInfo, Hash, StateError, StateProof,
    StorageKey, StorageProof, StorageValue, TransactionAccessPattern,
};

/// Primary API for state operations
pub trait StateManagementApi: Send + Sync {
    // === State Reads ===
    
    fn get_account_state(
        &self,
        address: Address,
        block_number: Option<u64>,
    ) -> Result<Option<AccountState>, StateError>;
    
    fn get_storage(
        &self,
        address: Address,
        key: StorageKey,
        block_number: Option<u64>,
    ) -> Result<Option<StorageValue>, StateError>;
    
    fn get_balance(
        &self,
        address: Address,
        block_number: Option<u64>,
    ) -> Result<u128, StateError>;
    
    fn get_nonce(
        &self,
        address: Address,
        block_number: Option<u64>,
    ) -> Result<u64, StateError>;
    
    // === Proofs ===
    
    fn get_state_proof(
        &self,
        address: Address,
        block_number: Option<u64>,
    ) -> Result<StateProof, StateError>;
    
    fn get_storage_proof(
        &self,
        address: Address,
        keys: Vec<StorageKey>,
        block_number: Option<u64>,
    ) -> Result<StorageProof, StateError>;
    
    // === Validation ===
    
    fn check_balance(
        &self,
        address: Address,
        required: u128,
    ) -> Result<bool, StateError>;
    
    fn get_expected_nonce(
        &self,
        address: Address,
    ) -> Result<u64, StateError>;
    
    // === Conflict Detection ===
    
    fn detect_conflicts(
        &self,
        access_patterns: Vec<TransactionAccessPattern>,
    ) -> Result<Vec<ConflictInfo>, StateError>;
    
    // === State Root ===
    
    fn get_state_root(&self, block_number: u64) -> Result<Hash, StateError>;
    
    fn get_current_state_root(&self) -> Result<Hash, StateError>;
}
