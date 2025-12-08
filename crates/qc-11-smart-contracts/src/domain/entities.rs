//! # Core Domain Entities
//!
//! Main business entities for smart contract execution.
//! These represent the core concepts in the EVM execution domain.

use crate::domain::value_objects::{Address, Bytes, Hash, StorageKey, StorageValue, U256};
use serde::{Deserialize, Serialize};

// =============================================================================
// EXECUTION CONTEXT
// =============================================================================

/// Execution context for a contract call.
///
/// Contains all information needed to execute a contract:
/// - Caller/origin information
/// - Value transfer
/// - Gas limits
/// - Block context
#[derive(Clone, Debug)]
pub struct ExecutionContext {
    /// Transaction sender (EOA that initiated the transaction).
    pub origin: Address,
    /// Current caller (may differ in nested calls).
    pub caller: Address,
    /// Contract being executed.
    pub address: Address,
    /// Value transferred (wei).
    pub value: U256,
    /// Input data (calldata).
    pub data: Bytes,
    /// Gas limit for this call.
    pub gas_limit: u64,
    /// Gas price.
    pub gas_price: U256,
    /// Block context.
    pub block: BlockContext,
    /// Call depth (for reentrancy limits).
    pub depth: u16,
    /// Is this a static call (no state changes allowed).
    pub is_static: bool,
}

impl ExecutionContext {
    /// Creates a new execution context for a top-level transaction.
    #[must_use]
    pub fn new_transaction(
        origin: Address,
        to: Address,
        value: U256,
        data: Bytes,
        gas_limit: u64,
        gas_price: U256,
        block: BlockContext,
    ) -> Self {
        Self {
            origin,
            caller: origin,
            address: to,
            value,
            data,
            gas_limit,
            gas_price,
            block,
            depth: 0,
            is_static: false,
        }
    }

    /// Creates a child context for a nested CALL.
    #[must_use]
    pub fn child_call(
        &self,
        caller: Address,
        address: Address,
        value: U256,
        data: Bytes,
        gas: u64,
    ) -> Self {
        Self {
            origin: self.origin,
            caller,
            address,
            value,
            data,
            gas_limit: gas,
            gas_price: self.gas_price,
            block: self.block.clone(),
            depth: self.depth.saturating_add(1),
            is_static: self.is_static,
        }
    }

    /// Creates a child context for DELEGATECALL.
    #[must_use]
    pub fn child_delegatecall(&self, _code_address: Address, data: Bytes, gas: u64) -> Self {
        Self {
            origin: self.origin,
            caller: self.caller, // Preserves caller
            address: self.address, // Preserves address
            value: self.value, // Preserves value
            data,
            gas_limit: gas,
            gas_price: self.gas_price,
            block: self.block.clone(),
            depth: self.depth.saturating_add(1),
            is_static: self.is_static,
        }
    }

    /// Creates a child context for STATICCALL.
    #[must_use]
    pub fn child_staticcall(&self, address: Address, data: Bytes, gas: u64) -> Self {
        Self {
            origin: self.origin,
            caller: self.address,
            address,
            value: U256::zero(),
            data,
            gas_limit: gas,
            gas_price: self.gas_price,
            block: self.block.clone(),
            depth: self.depth.saturating_add(1),
            is_static: true, // Static call enforced
        }
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            origin: Address::ZERO,
            caller: Address::ZERO,
            address: Address::ZERO,
            value: U256::zero(),
            data: Bytes::new(),
            gas_limit: 0,
            gas_price: U256::zero(),
            block: BlockContext::default(),
            depth: 0,
            is_static: false,
        }
    }
}

// =============================================================================
// BLOCK CONTEXT
// =============================================================================

/// Block context for execution.
///
/// Provides access to block-level information during EVM execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockContext {
    /// Block number.
    pub number: u64,
    /// Block timestamp (unix seconds).
    pub timestamp: u64,
    /// Coinbase address (block proposer).
    pub coinbase: Address,
    /// Block difficulty (legacy, use prevrandao in PoS).
    pub difficulty: U256,
    /// Block gas limit.
    pub gas_limit: u64,
    /// Base fee (EIP-1559).
    pub base_fee: U256,
    /// Chain ID (EIP-155).
    pub chain_id: u64,
}

impl Default for BlockContext {
    fn default() -> Self {
        Self {
            number: 0,
            timestamp: 0,
            coinbase: Address::ZERO,
            difficulty: U256::zero(),
            gas_limit: 30_000_000,
            base_fee: U256::zero(),
            chain_id: 1,
        }
    }
}

// =============================================================================
// EXECUTION RESULT
// =============================================================================

/// Result of smart contract execution.
#[derive(Clone, Debug, Default)]
pub struct ExecutionResult {
    /// Whether execution succeeded.
    pub success: bool,
    /// Return data.
    pub output: Bytes,
    /// Gas used.
    pub gas_used: u64,
    /// Gas refund (for SSTORE clears).
    pub gas_refund: u64,
    /// State changes to apply.
    pub state_changes: Vec<StateChange>,
    /// Logs emitted.
    pub logs: Vec<Log>,
    /// Revert reason (if failed).
    pub revert_reason: Option<String>,
}

impl ExecutionResult {
    /// Creates a successful execution result.
    #[must_use]
    pub fn success(output: Bytes, gas_used: u64) -> Self {
        Self {
            success: true,
            output,
            gas_used,
            gas_refund: 0,
            state_changes: Vec::new(),
            logs: Vec::new(),
            revert_reason: None,
        }
    }

    /// Creates a failed execution result.
    #[must_use]
    pub fn failure(reason: impl Into<String>, gas_used: u64) -> Self {
        Self {
            success: false,
            output: Bytes::new(),
            gas_used,
            gas_refund: 0,
            state_changes: Vec::new(),
            logs: Vec::new(),
            revert_reason: Some(reason.into()),
        }
    }

    /// Creates an out-of-gas result.
    #[must_use]
    pub fn out_of_gas(gas_limit: u64) -> Self {
        Self::failure("out of gas", gas_limit)
    }

    /// Creates a revert result with data.
    #[must_use]
    pub fn revert(data: Bytes, gas_used: u64) -> Self {
        // Try to decode revert reason from data
        let reason = decode_revert_reason(&data);
        Self {
            success: false,
            output: data,
            gas_used,
            gas_refund: 0,
            state_changes: Vec::new(),
            logs: Vec::new(),
            revert_reason: reason,
        }
    }
}

/// Attempts to decode a revert reason from output data.
fn decode_revert_reason(data: &Bytes) -> Option<String> {
    // Error(string) selector: 0x08c379a0
    if data.len() < 68 {
        return None;
    }

    let selector = &data.as_slice()[0..4];
    if selector != [0x08, 0xc3, 0x79, 0xa0] {
        return None;
    }

    // Decode string from ABI encoding
    // Skip selector (4) + offset (32) + length position
    let offset = 4 + 32;
    if data.len() < offset + 32 {
        return None;
    }

    // Read string length
    let len_bytes = &data.as_slice()[offset..offset + 32];
    let len = U256::from_big_endian(len_bytes).as_usize();

    if data.len() < offset + 32 + len {
        return None;
    }

    let string_bytes = &data.as_slice()[offset + 32..offset + 32 + len];
    String::from_utf8(string_bytes.to_vec()).ok()
}

// =============================================================================
// STATE CHANGE
// =============================================================================

/// State change from execution.
///
/// These changes are collected during execution and applied atomically
/// on success. On revert, all changes are discarded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StateChange {
    /// Transfer balance between accounts.
    BalanceTransfer {
        from: Address,
        to: Address,
        amount: U256,
    },
    /// Write to contract storage.
    StorageWrite {
        address: Address,
        key: StorageKey,
        value: StorageValue,
    },
    /// Delete storage slot (set to zero).
    StorageDelete {
        address: Address,
        key: StorageKey,
    },
    /// Create a new contract.
    ContractCreate {
        address: Address,
        code: Bytes,
    },
    /// Self-destruct a contract.
    ContractDestroy {
        address: Address,
        beneficiary: Address,
    },
    /// Increment account nonce.
    NonceIncrement { address: Address },
}

// =============================================================================
// LOG (EVENT)
// =============================================================================

/// Emitted log (event) from contract execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Log {
    /// Contract address that emitted the log.
    pub address: Address,
    /// Indexed topics (up to 4).
    pub topics: Vec<Hash>,
    /// Non-indexed data.
    pub data: Bytes,
}

impl Log {
    /// Creates a new log.
    #[must_use]
    pub fn new(address: Address, topics: Vec<Hash>, data: Bytes) -> Self {
        Self {
            address,
            topics,
            data,
        }
    }
}

// =============================================================================
// VM CONFIGURATION
// =============================================================================

/// Virtual Machine configuration.
///
/// Per System.md and SPEC-11: Execution limits to prevent DoS attacks.
#[derive(Clone, Debug)]
pub struct VmConfig {
    /// Maximum call depth (default: 1024).
    pub max_call_depth: u16,
    /// Maximum code size in bytes (EIP-170: 24KB).
    pub max_code_size: usize,
    /// Maximum init code size in bytes (EIP-3860: 48KB).
    pub max_init_code_size: usize,
    /// Maximum stack size (default: 1024).
    pub max_stack_size: usize,
    /// Maximum memory size in bytes (default: 16MB).
    pub max_memory_size: usize,
    /// EVM version/fork.
    pub evm_version: EvmVersion,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            max_call_depth: 1024,
            max_code_size: 24_576,         // 24 KB (EIP-170)
            max_init_code_size: 49_152,    // 48 KB (EIP-3860)
            max_stack_size: 1024,
            max_memory_size: 16 * 1024 * 1024, // 16 MB
            evm_version: EvmVersion::Shanghai,
        }
    }
}

impl VmConfig {
    /// Block gas limit (30 million per System.md).
    pub const BLOCK_GAS_LIMIT: u64 = 30_000_000;

    /// Get maximum gas limit for estimation.
    #[must_use]
    pub fn max_gas_limit(&self) -> u64 {
        Self::BLOCK_GAS_LIMIT
    }
}

/// EVM hard fork version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EvmVersion {
    /// Istanbul hard fork.
    Istanbul,
    /// Berlin hard fork (EIP-2929 access lists).
    Berlin,
    /// London hard fork (EIP-1559 base fee).
    London,
    /// Paris hard fork (The Merge).
    Paris,
    /// Shanghai hard fork (withdrawals).
    #[default]
    Shanghai,
}

// =============================================================================
// ACCOUNT STATE (for StateAccess port)
// =============================================================================

/// Account state in the state trie.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AccountState {
    /// Account balance.
    pub balance: U256,
    /// Account nonce.
    pub nonce: u64,
    /// Code hash (keccak256 of code, or empty hash for EOA).
    pub code_hash: Hash,
    /// Storage root (merkle root of storage trie).
    pub storage_root: Hash,
}

impl AccountState {
    /// Empty code hash (keccak256 of empty bytes).
    pub const EMPTY_CODE_HASH: Hash = Hash([
        0xc5, 0xd2, 0x46, 0x01, 0x86, 0xf7, 0x23, 0x3c,
        0x92, 0x7e, 0x7d, 0xb2, 0xdc, 0xc7, 0x03, 0xc0,
        0xe5, 0x00, 0xb6, 0x53, 0xca, 0x82, 0x27, 0x3b,
        0x7b, 0xfa, 0xd8, 0x04, 0x5d, 0x85, 0xa4, 0x70,
    ]);

    /// Creates a new empty EOA.
    #[must_use]
    pub fn new_eoa(balance: U256, nonce: u64) -> Self {
        Self {
            balance,
            nonce,
            code_hash: Self::EMPTY_CODE_HASH,
            storage_root: Hash::ZERO,
        }
    }

    /// Returns true if this is an EOA (externally owned account).
    #[must_use]
    pub fn is_eoa(&self) -> bool {
        self.code_hash == Self::EMPTY_CODE_HASH
    }

    /// Returns true if this is a contract.
    #[must_use]
    pub fn is_contract(&self) -> bool {
        !self.is_eoa()
    }

    /// Returns true if this account is empty (can be pruned).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.balance.is_zero()
            && self.nonce == 0
            && (self.code_hash == Self::EMPTY_CODE_HASH || self.code_hash == Hash::ZERO)
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_context_child_call() {
        let parent = ExecutionContext {
            origin: Address::new([1u8; 20]),
            caller: Address::new([1u8; 20]),
            address: Address::new([2u8; 20]),
            value: U256::from(100),
            data: Bytes::from_slice(&[0x01, 0x02]),
            gas_limit: 1000,
            gas_price: U256::from(1),
            block: BlockContext::default(),
            depth: 0,
            is_static: false,
        };

        let child = parent.child_call(
            Address::new([2u8; 20]),
            Address::new([3u8; 20]),
            U256::from(50),
            Bytes::from_slice(&[0x03]),
            500,
        );

        assert_eq!(child.origin, parent.origin); // Origin preserved
        assert_eq!(child.caller, Address::new([2u8; 20]));
        assert_eq!(child.address, Address::new([3u8; 20]));
        assert_eq!(child.depth, 1);
        assert!(!child.is_static);
    }

    #[test]
    fn test_execution_context_staticcall() {
        let parent = ExecutionContext::default();
        let child = parent.child_staticcall(
            Address::new([1u8; 20]),
            Bytes::new(),
            100,
        );

        assert!(child.is_static);
        assert!(child.value.is_zero());
    }

    #[test]
    fn test_execution_result_success() {
        let result = ExecutionResult::success(
            Bytes::from_slice(&[0x01, 0x02]),
            21000,
        );

        assert!(result.success);
        assert_eq!(result.gas_used, 21000);
        assert!(result.revert_reason.is_none());
    }

    #[test]
    fn test_execution_result_failure() {
        let result = ExecutionResult::failure("test error", 10000);

        assert!(!result.success);
        assert_eq!(result.revert_reason, Some("test error".to_string()));
    }

    #[test]
    fn test_account_state_eoa() {
        let eoa = AccountState::new_eoa(U256::from(100), 5);
        assert!(eoa.is_eoa());
        assert!(!eoa.is_contract());
        assert!(!eoa.is_empty());
    }

    #[test]
    fn test_account_state_empty() {
        let empty = AccountState::default();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_vm_config_defaults() {
        let config = VmConfig::default();
        assert_eq!(config.max_call_depth, 1024);
        assert_eq!(config.max_code_size, 24_576);
        assert_eq!(config.max_init_code_size, 49_152);
        assert_eq!(config.max_stack_size, 1024);
        assert_eq!(config.max_memory_size, 16 * 1024 * 1024);
    }
}
