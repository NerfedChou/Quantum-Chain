//! # Driving Ports (API - Inbound)
//!
//! These are the interfaces exposed by the Smart Contract subsystem.
//! External systems (Consensus, Transaction Ordering, Cross-Chain) use these
//! to request contract execution.
//!
//! ## Architecture Compliance (Architecture.md v2.3)
//!
//! - These traits define the public API of Subsystem 11
//! - Adapters implement these traits to handle incoming requests
//! - NO direct subsystem calls - all via Event Bus (EDA pattern)

use crate::domain::entities::{BlockContext, ExecutionContext, ExecutionResult};
use crate::domain::value_objects::{Address, Bytes, Hash, U256};
use crate::errors::VmError;
use async_trait::async_trait;

// =============================================================================
// SIGNED TRANSACTION (Re-exported for API use)
// =============================================================================

/// Signed transaction for execution.
///
/// This mirrors the shared-types SignedTransaction but is defined here
/// to avoid tight coupling to shared-types internals.
#[derive(Clone, Debug)]
pub struct SignedTransaction {
    /// Sender address (20 bytes).
    pub from: Address,
    /// Recipient address (None for contract creation).
    pub to: Option<Address>,
    /// Transaction value in wei.
    pub value: U256,
    /// Sender's nonce.
    pub nonce: u64,
    /// Gas price in wei.
    pub gas_price: U256,
    /// Gas limit.
    pub gas_limit: u64,
    /// Transaction data (calldata or init code).
    pub data: Bytes,
    /// Transaction hash (computed from signed data).
    pub hash: Hash,
}

impl SignedTransaction {
    /// Returns true if this is a contract creation transaction.
    #[must_use]
    pub fn is_contract_creation(&self) -> bool {
        self.to.is_none()
    }

    /// Returns the transaction hash.
    #[must_use]
    pub fn hash(&self) -> Hash {
        self.hash
    }

    /// Returns the sender address.
    #[must_use]
    pub fn sender(&self) -> Address {
        self.from
    }
}

impl Default for SignedTransaction {
    fn default() -> Self {
        Self {
            from: Address::ZERO,
            to: None,
            value: U256::zero(),
            nonce: 0,
            gas_price: U256::from(1_000_000_000u64), // 1 gwei
            gas_limit: 21000,                        // Basic transfer gas
            data: Bytes::new(),
            hash: Hash::ZERO,
        }
    }
}

// =============================================================================
// SMART CONTRACT API (Primary Driving Port)
// =============================================================================

/// Primary API for smart contract execution.
///
/// ## IPC-MATRIX.md Compliance
///
/// Authorized senders:
/// - Subsystem 8 (Consensus): Execute transactions in validated blocks
/// - Subsystem 12 (Transaction Ordering): Execute ordered transactions
///
/// ## Usage
///
/// ```ignore
/// let result = api.execute_transaction(&tx, &block_context).await?;
/// ```
#[async_trait]
pub trait SmartContractApi: Send + Sync {
    /// Execute a contract call with the given context and code.
    ///
    /// This is the low-level execution primitive. Most callers should use
    /// `execute_transaction` instead.
    ///
    /// # Arguments
    ///
    /// * `context` - Execution context (caller, value, gas, etc.)
    /// * `code` - Contract bytecode to execute
    ///
    /// # Returns
    ///
    /// * `ExecutionResult` - Contains success/failure, output, gas used, state changes
    async fn execute(
        &self,
        context: ExecutionContext,
        code: &[u8],
    ) -> Result<ExecutionResult, VmError>;

    /// Execute a signed transaction.
    ///
    /// This handles:
    /// - Contract creation (if `to` is None)
    /// - Contract call (if `to` is Some)
    /// - Balance transfer
    /// - Gas deduction
    ///
    /// # Arguments
    ///
    /// * `tx` - The signed transaction to execute
    /// * `block` - Block context for execution
    ///
    /// # Returns
    ///
    /// * `ExecutionResult` - Contains success/failure, output, gas used, state changes
    async fn execute_transaction(
        &self,
        tx: &SignedTransaction,
        block: &BlockContext,
    ) -> Result<ExecutionResult, VmError>;

    /// Estimate gas for a call.
    ///
    /// Runs the execution and returns the gas used. Does NOT apply state changes.
    ///
    /// # Arguments
    ///
    /// * `context` - Execution context
    /// * `code` - Contract bytecode
    ///
    /// # Returns
    ///
    /// * `u64` - Estimated gas required
    async fn estimate_gas(&self, context: ExecutionContext, code: &[u8]) -> Result<u64, VmError>;

    /// Execute a read-only call (eth_call).
    ///
    /// Executes the call but does NOT apply state changes.
    /// Always runs in static mode.
    ///
    /// # Arguments
    ///
    /// * `context` - Execution context (is_static forced to true)
    /// * `code` - Contract bytecode
    ///
    /// # Returns
    ///
    /// * `Bytes` - Return data from the call
    async fn call(&self, context: ExecutionContext, code: &[u8]) -> Result<Bytes, VmError>;
}

// =============================================================================
// HTLC EXECUTOR (For Cross-Chain Subsystem)
// =============================================================================

/// HTLC operation types.
#[derive(Clone, Debug)]
pub enum HtlcOperation {
    /// Claim funds by revealing the secret.
    Claim {
        /// The preimage that hashes to the hashlock.
        secret: Hash,
    },
    /// Refund after timelock expires.
    Refund,
}

/// HTLC (Hash Time-Locked Contract) execution interface.
///
/// ## IPC-MATRIX.md Compliance
///
/// Authorized sender: Subsystem 15 (Cross-Chain) ONLY
///
/// ## Usage
///
/// ```ignore
/// let result = executor.execute_htlc(htlc_address, HtlcOperation::Claim { secret }).await?;
/// ```
#[async_trait]
pub trait HtlcExecutor: Send + Sync {
    /// Execute an HTLC operation (claim or refund).
    ///
    /// # Arguments
    ///
    /// * `htlc_contract` - Address of the HTLC contract
    /// * `operation` - The operation to perform
    /// * `block` - Current block context
    ///
    /// # Returns
    ///
    /// * `ExecutionResult` - Result of the HTLC operation
    async fn execute_htlc(
        &self,
        htlc_contract: Address,
        operation: HtlcOperation,
        block: &BlockContext,
    ) -> Result<ExecutionResult, VmError>;
}

// =============================================================================
// BATCH EXECUTOR (For Block Processing)
// =============================================================================

/// Result of a single transaction in a batch.
#[derive(Clone, Debug)]
pub struct TransactionReceipt {
    /// Transaction hash.
    pub tx_hash: Hash,
    /// Whether the transaction succeeded.
    pub success: bool,
    /// Gas used by this transaction.
    pub gas_used: u64,
    /// Cumulative gas used in the block so far.
    pub cumulative_gas_used: u64,
    /// Return data (for contract calls).
    pub output: Bytes,
    /// Logs emitted.
    pub logs: Vec<crate::domain::entities::Log>,
    /// Contract address (if this was a contract creation).
    pub contract_address: Option<Address>,
}

/// Batch transaction executor for block processing.
///
/// ## IPC-MATRIX.md Compliance
///
/// Authorized sender: Subsystem 8 (Consensus) ONLY
#[async_trait]
pub trait BatchExecutor: Send + Sync {
    /// Execute a batch of transactions in a block.
    ///
    /// Transactions are executed in order. If one fails, execution continues
    /// with the next transaction (failed tx still consumes gas).
    ///
    /// # Arguments
    ///
    /// * `transactions` - Transactions to execute in order
    /// * `block` - Block context
    ///
    /// # Returns
    ///
    /// * `Vec<TransactionReceipt>` - Receipt for each transaction
    async fn execute_batch(
        &self,
        transactions: &[SignedTransaction],
        block: &BlockContext,
    ) -> Result<Vec<TransactionReceipt>, VmError>;
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signed_transaction_is_contract_creation() {
        let tx = SignedTransaction {
            from: Address::ZERO,
            to: None,
            value: U256::zero(),
            nonce: 0,
            gas_price: U256::from(1),
            gas_limit: 21000,
            data: Bytes::new(),
            hash: Hash::ZERO,
        };

        assert!(tx.is_contract_creation());

        let tx_call = SignedTransaction {
            to: Some(Address::new([1u8; 20])),
            ..tx
        };

        assert!(!tx_call.is_contract_creation());
    }

    #[test]
    fn test_htlc_operation() {
        let claim = HtlcOperation::Claim {
            secret: Hash::new([42u8; 32]),
        };

        match claim {
            HtlcOperation::Claim { secret } => {
                assert_eq!(secret.as_bytes()[0], 42);
            }
            _ => panic!("Expected Claim"),
        }
    }
}
