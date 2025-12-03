use crate::domain::{Address, ConflictInfo, Hash, StorageKey, TransactionAccessPattern};
use serde::{Deserialize, Serialize};

/// V2.3: Subscribed from Event Bus (published by Consensus, Subsystem 8)
/// This triggers state root computation as part of the choreography
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockValidatedPayload {
    pub block_hash: Hash,
    pub block_height: u64,
    pub transactions: Vec<TransactionData>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionData {
    pub hash: Hash,
    pub from: Address,
    pub to: Option<Address>,
    pub value: u128,
    pub nonce: u64,
}

/// V2.3: Published to Event Bus after computing state root
/// Block Storage (Subsystem 2) subscribes as part of Stateful Assembler
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateRootComputedPayload {
    pub block_hash: Hash,
    pub block_height: u64,
    pub state_root: Hash,
    pub previous_state_root: Hash,
    pub accounts_modified: u32,
    pub storage_slots_modified: u32,
    pub computation_time_ms: u64,
}

/// State read request from Subsystems 6, 11, 12, 14
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateReadRequestPayload {
    pub address: Address,
    pub storage_key: Option<StorageKey>,
    pub block_number: Option<u64>,
}

/// State read response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateReadResponsePayload {
    pub address: Address,
    pub storage_key: Option<StorageKey>,
    pub value: Option<Vec<u8>>,
    pub block_number: u64,
}

/// State write request from Subsystem 11 ONLY
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateWriteRequestPayload {
    pub address: Address,
    pub storage_key: StorageKey,
    pub value: [u8; 32],
    pub block_height: u64,
    pub tx_hash: Hash,
}

/// Balance check request from Subsystem 6 (Mempool)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BalanceCheckRequestPayload {
    pub address: Address,
    pub required_balance: u128,
}

/// Balance check response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BalanceCheckResponsePayload {
    pub address: Address,
    pub has_sufficient_balance: bool,
    pub current_balance: u128,
    pub required_balance: u128,
}

/// Conflict detection request from Subsystem 12
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConflictDetectionRequestPayload {
    pub transactions: Vec<TransactionAccessPattern>,
}

/// Conflict detection response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConflictDetectionResponsePayload {
    pub conflicts: Vec<ConflictInfo>,
    pub total_transactions: usize,
    pub conflicting_pairs: usize,
}
