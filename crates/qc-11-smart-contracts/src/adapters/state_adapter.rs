//! # State Adapter
//!
//! In-memory state access implementation for testing.
//! Production implementation would communicate with Subsystem 4 via Event Bus.

use crate::domain::entities::AccountState;
use crate::domain::value_objects::{Address, Bytes, StorageKey, StorageValue, U256};
use crate::errors::StateError;
use crate::ports::outbound::StateAccess;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

/// In-memory state for testing.
#[derive(Debug, Default)]
pub struct InMemoryState {
    /// Account states.
    accounts: RwLock<HashMap<Address, AccountState>>,
    /// Contract code.
    code: RwLock<HashMap<Address, Bytes>>,
    /// Storage.
    storage: RwLock<HashMap<(Address, StorageKey), StorageValue>>,
}

impl InMemoryState {
    /// Create a new empty state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set account state.
    pub fn set_account(&self, address: Address, state: AccountState) {
        self.accounts.write().unwrap().insert(address, state);
    }

    /// Set balance for an address.
    pub fn set_balance(&self, address: Address, balance: U256) {
        let mut accounts = self.accounts.write().unwrap();
        let account = accounts
            .entry(address)
            .or_insert_with(|| AccountState::new_eoa(U256::zero(), 0));
        account.balance = balance;
    }

    /// Set code for a contract.
    pub fn set_code(&self, address: Address, code: Bytes) {
        // Update code hash
        let code_hash = if code.is_empty() {
            AccountState::EMPTY_CODE_HASH
        } else {
            crate::domain::services::keccak256(code.as_slice())
        };

        // Update account
        let mut accounts = self.accounts.write().unwrap();
        let account = accounts
            .entry(address)
            .or_insert_with(|| AccountState::new_eoa(U256::zero(), 0));
        account.code_hash = code_hash;

        // Store code
        self.code.write().unwrap().insert(address, code);
    }

    /// Set storage value.
    pub fn set_storage_value(&self, address: Address, key: StorageKey, value: StorageValue) {
        self.storage.write().unwrap().insert((address, key), value);
    }
}

#[async_trait]
impl StateAccess for InMemoryState {
    async fn get_account(&self, address: Address) -> Result<Option<AccountState>, StateError> {
        Ok(self.accounts.read().unwrap().get(&address).cloned())
    }

    async fn get_storage(
        &self,
        address: Address,
        key: StorageKey,
    ) -> Result<StorageValue, StateError> {
        Ok(self
            .storage
            .read()
            .unwrap()
            .get(&(address, key))
            .copied()
            .unwrap_or(StorageValue::ZERO))
    }

    async fn set_storage(
        &self,
        address: Address,
        key: StorageKey,
        value: StorageValue,
    ) -> Result<(), StateError> {
        self.storage.write().unwrap().insert((address, key), value);
        Ok(())
    }

    async fn get_code(&self, address: Address) -> Result<Bytes, StateError> {
        Ok(self
            .code
            .read()
            .unwrap()
            .get(&address)
            .cloned()
            .unwrap_or_default())
    }

    async fn account_exists(&self, address: Address) -> Result<bool, StateError> {
        Ok(self.accounts.read().unwrap().contains_key(&address))
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_set_balance() {
        let state = InMemoryState::new();
        let addr = Address::new([1u8; 20]);

        // Initially no account
        let account = state.get_account(addr).await.unwrap();
        assert!(account.is_none());

        // Set balance
        state.set_balance(addr, U256::from(1000));

        let balance = state.get_balance(addr).await.unwrap();
        assert_eq!(balance, U256::from(1000));
    }

    #[tokio::test]
    async fn test_get_set_storage() {
        let state = InMemoryState::new();
        let addr = Address::new([1u8; 20]);
        let key = StorageKey::new([0u8; 32]);

        // Initially zero
        let value = state.get_storage(addr, key).await.unwrap();
        assert!(value.is_zero());

        // Set value
        let new_value = StorageValue::from_u256(U256::from(42));
        state.set_storage(addr, key, new_value).await.unwrap();

        let value = state.get_storage(addr, key).await.unwrap();
        assert_eq!(value.to_u256(), U256::from(42));
    }

    #[tokio::test]
    async fn test_get_set_code() {
        let state = InMemoryState::new();
        let addr = Address::new([1u8; 20]);
        let code = Bytes::from_slice(&[0x60, 0x00, 0x60, 0x00, 0xF3]); // PUSH0 PUSH0 RETURN

        state.set_code(addr, code.clone());

        let retrieved = state.get_code(addr).await.unwrap();
        assert_eq!(retrieved.as_slice(), code.as_slice());

        // Code hash should be updated
        let account = state.get_account(addr).await.unwrap().unwrap();
        assert_ne!(account.code_hash, AccountState::EMPTY_CODE_HASH);
    }

    #[tokio::test]
    async fn test_account_exists() {
        let state = InMemoryState::new();
        let addr = Address::new([1u8; 20]);

        assert!(!state.account_exists(addr).await.unwrap());

        state.set_balance(addr, U256::from(1));
        assert!(state.account_exists(addr).await.unwrap());
    }
}
