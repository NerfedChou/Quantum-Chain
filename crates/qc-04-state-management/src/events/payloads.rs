//! # IPC Payloads for State Management
//!
//! Serializable message payloads for inter-process communication.
//! Per IPC-MATRIX.md authorization rules.

use crate::domain::{Address, ConflictInfo, Hash, StorageKey, TransactionAccessPattern};
use serde::{Deserialize, Serialize};

// =============================================================================
// CHOREOGRAPHY EVENTS (V2.3)
// =============================================================================

/// BlockValidated event payload from Consensus (Subsystem 8).
///
/// Triggers state root computation as part of the V2.3 choreography flow.
/// State Management computes new state root and publishes StateRootComputed.
///
/// ## IPC-MATRIX Authorization
///
/// - Source: Subsystem 8 (Consensus) only
/// - Verified via MessageVerifier
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockValidatedPayload {
    /// Hash of the validated block.
    pub block_hash: Hash,
    /// Height of the validated block.
    pub block_height: u64,
    /// Transactions to apply to state.
    pub transactions: Vec<TransactionData>,
}

/// Transaction data within a BlockValidated payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionData {
    /// Transaction hash.
    pub hash: Hash,
    /// Sender address.
    pub from: Address,
    /// Recipient address (None for contract creation).
    pub to: Option<Address>,
    /// Transfer value in base units.
    pub value: u128,
    /// Sender nonce.
    pub nonce: u64,
}

/// StateRootComputed event payload.
///
/// Published after computing state root from BlockValidated transactions.
/// Block Storage (Subsystem 2) subscribes as Stateful Assembler.
///
/// ## IPC-MATRIX Authorization
///
/// - Source: Subsystem 4 (State Management) only
/// - Destinations: Subsystem 2 (Block Storage)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateRootComputedPayload {
    /// Block hash this state root is for.
    pub block_hash: Hash,
    /// Block height.
    pub block_height: u64,
    /// Computed state root (Merkle trie root hash).
    pub state_root: Hash,
    /// State root before this block.
    pub previous_state_root: Hash,
    /// Number of accounts modified.
    pub accounts_modified: u32,
    /// Number of storage slots modified.
    pub storage_slots_modified: u32,
    /// Time to compute state root in milliseconds.
    pub computation_time_ms: u64,
}

// =============================================================================
// REQUEST/RESPONSE PAYLOADS
// =============================================================================

/// State read request payload.
///
/// ## IPC-MATRIX Authorization
///
/// - Allowed sources: Subsystems 6, 11, 12, 14
/// - Mempool (6): Balance checks
/// - Smart Contracts (11): Contract state reads
/// - Tx Ordering (12): Conflict detection
/// - Sharding (14): Cross-shard state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateReadRequestPayload {
    /// Account address to read.
    pub address: Address,
    /// Storage key (None for account state, Some for storage slot).
    pub storage_key: Option<StorageKey>,
    /// Block number (None for latest).
    pub block_number: Option<u64>,
}

/// State read response payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateReadResponsePayload {
    /// Queried address.
    pub address: Address,
    /// Queried storage key (if any).
    pub storage_key: Option<StorageKey>,
    /// Value (None if not found).
    pub value: Option<Vec<u8>>,
    /// Block number of the state.
    pub block_number: u64,
}

/// State write request payload.
///
/// ## IPC-MATRIX Authorization
///
/// - Allowed source: Subsystem 11 (Smart Contracts) ONLY
/// - All other sources are rejected
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateWriteRequestPayload {
    /// Contract address.
    pub address: Address,
    /// Storage slot key.
    pub storage_key: StorageKey,
    /// New value (32 bytes).
    pub value: [u8; 32],
    /// Block height context.
    pub block_height: u64,
    /// Transaction hash for audit.
    pub tx_hash: Hash,
}

/// Balance check request payload.
///
/// ## IPC-MATRIX Authorization
///
/// - Allowed source: Subsystem 6 (Mempool) ONLY
/// - Used for transaction validation before pool admission
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BalanceCheckRequestPayload {
    /// Account to check.
    pub address: Address,
    /// Minimum required balance.
    pub required_balance: u128,
}

/// Balance check response payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BalanceCheckResponsePayload {
    /// Queried address.
    pub address: Address,
    /// Whether balance is sufficient.
    pub has_sufficient_balance: bool,
    /// Current balance.
    pub current_balance: u128,
    /// Required balance from request.
    pub required_balance: u128,
}

/// Conflict detection request payload.
///
/// ## IPC-MATRIX Authorization
///
/// - Allowed source: Subsystem 12 (Transaction Ordering) ONLY
/// - Used for parallel execution dependency analysis
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConflictDetectionRequestPayload {
    /// Transactions to check for conflicts.
    pub transactions: Vec<TransactionAccessPattern>,
}

/// Conflict detection response payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConflictDetectionResponsePayload {
    /// Detected conflicts.
    pub conflicts: Vec<ConflictInfo>,
    /// Total transactions analyzed.
    pub total_transactions: usize,
    /// Number of conflicting pairs.
    pub conflicting_pairs: usize,
}
