//! # Driven Ports (SPI - Outbound)
//!
//! These are the interfaces that the Smart Contract subsystem depends on.
//! External adapters implement these traits to provide:
//! - State access (Subsystem 4)
//! - Signature verification (Subsystem 10)
//!
//! ## Architecture Compliance (Architecture.md v2.3)
//!
//! - Dependencies point INWARD (adapters implement these traits)
//! - NO direct subsystem calls - communication via Event Bus
//! - Adapters translate between ports and IPC messages

use crate::domain::entities::AccountState;
use crate::domain::value_objects::{
    Address, Bytes, EcdsaSignature, Hash, StorageKey, StorageValue, U256,
};
use crate::errors::StateError;
use async_trait::async_trait;

// =============================================================================
// STATE ACCESS (Subsystem 4 Dependency)
// =============================================================================

/// Interface for accessing blockchain state.
///
/// ## IPC-MATRIX.md Compliance
///
/// This subsystem (11) is the ONLY one allowed to write state.
/// Reads are also performed during execution.
///
/// ## Implementation Notes
///
/// The adapter implementing this trait should:
/// 1. Translate calls to `StateReadRequest` / `StateWriteRequest` IPC messages
/// 2. Use correlation IDs for request/response matching
/// 3. Handle timeouts (30s default)
#[async_trait]
pub trait StateAccess: Send + Sync {
    /// Get account state.
    ///
    /// # Arguments
    ///
    /// * `address` - Account address to query
    ///
    /// # Returns
    ///
    /// * `Some(AccountState)` - If account exists
    /// * `None` - If account does not exist (never interacted with)
    async fn get_account(&self, address: Address) -> Result<Option<AccountState>, StateError>;

    /// Get storage value.
    ///
    /// # Arguments
    ///
    /// * `address` - Contract address
    /// * `key` - Storage slot key
    ///
    /// # Returns
    ///
    /// * `StorageValue` - Value at slot (zero if never written)
    async fn get_storage(
        &self,
        address: Address,
        key: StorageKey,
    ) -> Result<StorageValue, StateError>;

    /// Set storage value.
    ///
    /// Note: This queues the write. Actual application happens on commit.
    ///
    /// # Arguments
    ///
    /// * `address` - Contract address
    /// * `key` - Storage slot key
    /// * `value` - New value to store
    async fn set_storage(
        &self,
        address: Address,
        key: StorageKey,
        value: StorageValue,
    ) -> Result<(), StateError>;

    /// Get contract code.
    ///
    /// # Arguments
    ///
    /// * `address` - Contract address
    ///
    /// # Returns
    ///
    /// * `Bytes` - Contract bytecode (empty for EOA)
    async fn get_code(&self, address: Address) -> Result<Bytes, StateError>;

    /// Check if account exists.
    ///
    /// An account exists if it has non-zero balance, non-zero nonce,
    /// or non-empty code.
    async fn account_exists(&self, address: Address) -> Result<bool, StateError>;

    /// Get account balance.
    ///
    /// Convenience method that extracts balance from account state.
    async fn get_balance(&self, address: Address) -> Result<U256, StateError> {
        match self.get_account(address).await? {
            Some(account) => Ok(account.balance),
            None => Ok(U256::zero()),
        }
    }

    /// Get account nonce.
    ///
    /// Convenience method that extracts nonce from account state.
    async fn get_nonce(&self, address: Address) -> Result<u64, StateError> {
        match self.get_account(address).await? {
            Some(account) => Ok(account.nonce),
            None => Ok(0),
        }
    }

    /// Get code hash for an address.
    ///
    /// Returns the keccak256 hash of the code, or the empty code hash for EOAs.
    async fn get_code_hash(&self, address: Address) -> Result<Hash, StateError> {
        match self.get_account(address).await? {
            Some(account) => Ok(account.code_hash),
            None => Ok(AccountState::EMPTY_CODE_HASH),
        }
    }

    /// Get code size.
    async fn get_code_size(&self, address: Address) -> Result<usize, StateError> {
        let code = self.get_code(address).await?;
        Ok(code.len())
    }
}

// =============================================================================
// SIGNATURE VERIFIER (Subsystem 10 Dependency - ecrecover precompile)
// =============================================================================

/// Interface for ECDSA signature verification.
///
/// ## IPC-MATRIX.md Compliance
///
/// Used by the ecrecover precompile (0x01).
/// Communication with Subsystem 10 should be via IPC, NOT direct calls.
///
/// ## Implementation Notes
///
/// The adapter should:
/// 1. Send `VerifySignatureRequest` to Subsystem 10
/// 2. Wait for response (with timeout)
/// 3. Return recovered address or None
pub trait SignatureVerifier: Send + Sync {
    /// Recover signer address from ECDSA signature.
    ///
    /// This is used by the ecrecover precompile.
    ///
    /// # Arguments
    ///
    /// * `hash` - 32-byte message hash
    /// * `signature` - ECDSA signature (r, s, v)
    ///
    /// # Returns
    ///
    /// * `Some(Address)` - Recovered signer address
    /// * `None` - If signature is invalid
    fn ecrecover(&self, hash: &Hash, signature: &EcdsaSignature) -> Option<Address>;
}

// =============================================================================
// BLOCK HASH ORACLE (For BLOCKHASH opcode)
// =============================================================================

/// Interface for querying historical block hashes.
///
/// Used by the BLOCKHASH opcode which can access the last 256 block hashes.
#[async_trait]
pub trait BlockHashOracle: Send + Sync {
    /// Get block hash for a given block number.
    ///
    /// # Arguments
    ///
    /// * `number` - Block number to query
    /// * `current_number` - Current block number (for range validation)
    ///
    /// # Returns
    ///
    /// * `Some(Hash)` - Block hash if within valid range (last 256 blocks)
    /// * `None` - If block is too old or doesn't exist
    async fn get_block_hash(&self, number: u64, current_number: u64) -> Option<Hash>;
}

// =============================================================================
// TRANSIENT STORAGE (EIP-1153)
// =============================================================================

/// Interface for transient storage (EIP-1153).
///
/// Transient storage is cleared at the end of each transaction.
/// Used by TLOAD and TSTORE opcodes.
pub trait TransientStorage: Send + Sync {
    /// Load from transient storage.
    fn tload(&self, address: Address, key: StorageKey) -> StorageValue;

    /// Store to transient storage.
    fn tstore(&mut self, address: Address, key: StorageKey, value: StorageValue);

    /// Clear all transient storage (called at end of transaction).
    fn clear(&mut self);
}

// =============================================================================
// ACCESS LIST (EIP-2929/2930)
// =============================================================================

/// Access status for storage/accounts (EIP-2929).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessStatus {
    /// First access in this transaction (cold).
    Cold,
    /// Already accessed in this transaction (warm).
    Warm,
}

/// Interface for tracking warm/cold access status.
///
/// Per EIP-2929, first access to an account or storage slot costs more gas.
pub trait AccessList: Send + Sync {
    /// Check and mark account as accessed.
    ///
    /// Returns the previous access status.
    fn touch_account(&mut self, address: Address) -> AccessStatus;

    /// Check and mark storage slot as accessed.
    ///
    /// Returns the previous access status.
    fn touch_storage(&mut self, address: Address, key: StorageKey) -> AccessStatus;

    /// Check if account is warm.
    fn is_account_warm(&self, address: Address) -> bool;

    /// Check if storage slot is warm.
    fn is_storage_warm(&self, address: Address, key: StorageKey) -> bool;

    /// Pre-warm an account (make it warm without returning status).
    fn warm_account(&mut self, address: Address) {
        let _ = self.touch_account(address);
    }

    /// Pre-warm a storage slot (make it warm without returning status).
    fn warm_storage(&mut self, address: Address, key: StorageKey) {
        let _ = self.touch_storage(address, key);
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_access_status() {
        assert_eq!(AccessStatus::Cold, AccessStatus::Cold);
        assert_ne!(AccessStatus::Cold, AccessStatus::Warm);
    }

    // Mock implementation for testing
    struct MockStateAccess;

    #[async_trait]
    impl StateAccess for MockStateAccess {
        async fn get_account(&self, _address: Address) -> Result<Option<AccountState>, StateError> {
            Ok(Some(AccountState::new_eoa(U256::from(1000), 5)))
        }

        async fn get_storage(
            &self,
            _address: Address,
            _key: StorageKey,
        ) -> Result<StorageValue, StateError> {
            Ok(StorageValue::ZERO)
        }

        async fn set_storage(
            &self,
            _address: Address,
            _key: StorageKey,
            _value: StorageValue,
        ) -> Result<(), StateError> {
            Ok(())
        }

        async fn get_code(&self, _address: Address) -> Result<Bytes, StateError> {
            Ok(Bytes::new())
        }

        async fn account_exists(&self, _address: Address) -> Result<bool, StateError> {
            Ok(true)
        }
    }

    #[tokio::test]
    async fn test_mock_state_access() {
        let state = MockStateAccess;
        let addr = Address::new([1u8; 20]);

        let balance = state.get_balance(addr).await.unwrap();
        assert_eq!(balance, U256::from(1000));

        let nonce = state.get_nonce(addr).await.unwrap();
        assert_eq!(nonce, 5);
    }
}
