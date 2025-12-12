//! # Event Schema (EDA Compliance)
//!
//! Defines all IPC message payloads for Smart Contract Execution.
//! These messages are wrapped in `AuthenticatedMessage<T>` for transport.
//!
//! ## Architecture Compliance (Architecture.md v2.3, IPC-MATRIX.md)
//!
//! - **Envelope-Only Identity (v2.2):** NO `requester_id` fields in payloads
//! - **Correlation IDs:** All request/response pairs use `correlation_id`
//! - **Security Boundaries:** Validated via `envelope.sender_id`
//!
//! ## Authorized Senders (per IPC-MATRIX.md Subsystem 11)
//!
//! | Message Type | Authorized Sender(s) |
//! |--------------|---------------------|
//! | `ExecuteTransactionRequest` | Subsystems 8, 12 ONLY |
//! | `ExecuteHTLCRequest` | Subsystem 15 ONLY |

use crate::domain::entities::{BlockContext, Log, StateChange};
use crate::domain::value_objects::{Address, Bytes, Hash, StorageKey, StorageValue, U256};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// =============================================================================
// INBOUND EVENTS (From Other Subsystems)
// =============================================================================

/// Request to execute a transaction in a block.
///
/// ## IPC-MATRIX.md Security
///
/// - Authorized senders: Subsystem 8 (Consensus), Subsystem 12 (Transaction Ordering)
/// - Envelope validation: `envelope.sender_id` MUST be 8 or 12
///
/// ## Envelope-Only Identity (v2.2)
///
/// NO `requester_id` in payload. Identity from `AuthenticatedMessage.sender_id`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecuteTransactionRequestPayload {
    // NO requester_id - per Envelope-Only Identity v2.2
    /// Transaction sender address.
    pub from: Address,
    /// Transaction recipient (None for contract creation).
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
    /// Transaction hash.
    pub tx_hash: Hash,
    /// Block context for execution.
    pub block_context: BlockContext,
}

/// Response to transaction execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecuteTransactionResponsePayload {
    /// Whether execution succeeded.
    pub success: bool,
    /// Gas used by this transaction.
    pub gas_used: u64,
    /// Return data.
    pub output: Bytes,
    /// Logs emitted during execution.
    pub logs: Vec<Log>,
    /// State changes to apply.
    pub state_changes: Vec<StateChange>,
    /// Contract address (if this was a contract creation).
    pub contract_address: Option<Address>,
    /// Revert reason (if execution failed).
    pub revert_reason: Option<String>,
}

/// HTLC operation type.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HtlcOperationPayload {
    /// Claim funds by revealing the preimage.
    Claim {
        /// The preimage that hashes to the hashlock.
        secret: Hash,
    },
    /// Refund after timelock expires.
    Refund,
}

/// Request to execute an HTLC operation.
///
/// ## IPC-MATRIX.md Security
///
/// - Authorized sender: Subsystem 15 (Cross-Chain) ONLY
/// - Envelope validation: `envelope.sender_id` MUST be 15
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecuteHTLCRequestPayload {
    // NO requester_id - per Envelope-Only Identity v2.2
    /// Address of the HTLC contract.
    pub htlc_contract: Address,
    /// Operation to perform.
    pub operation: HtlcOperationPayload,
    /// Block context for execution.
    pub block_context: BlockContext,
}

/// Response to HTLC execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecuteHTLCResponsePayload {
    /// Whether the operation succeeded.
    pub success: bool,
    /// Gas used.
    pub gas_used: u64,
    /// Revert reason (if failed).
    pub revert_reason: Option<String>,
}

// =============================================================================
// OUTBOUND EVENTS (To Other Subsystems)
// =============================================================================

/// Request to read state from Subsystem 4 (State Management).
///
/// ## IPC-MATRIX.md Security
///
/// - Recipient: Subsystem 4 ONLY
/// - This subsystem (11) is allowed to send `StateReadRequest`
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateReadRequestPayload {
    // NO requester_id - per Envelope-Only Identity v2.2
    /// Account address to query.
    pub address: Address,
    /// Storage key (None for account balance/nonce query).
    pub storage_key: Option<StorageKey>,
}

/// Response from State Management with state data.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateReadResponsePayload {
    /// Account exists.
    pub exists: bool,
    /// Account balance.
    pub balance: U256,
    /// Account nonce.
    pub nonce: u64,
    /// Code hash.
    pub code_hash: Hash,
    /// Storage value (if `storage_key` was provided).
    pub storage_value: Option<StorageValue>,
}

/// Request to write state to Subsystem 4 (State Management).
///
/// ## IPC-MATRIX.md Security
///
/// - Recipient: Subsystem 4 ONLY
/// - ONLY Subsystem 11 is allowed to write state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateWriteRequestPayload {
    // NO requester_id - per Envelope-Only Identity v2.2
    /// Contract address.
    pub address: Address,
    /// Storage key.
    pub storage_key: StorageKey,
    /// New value.
    pub value: StorageValue,
    /// Execution context ID (for atomicity).
    pub execution_id: Uuid,
}

/// Response from State Management confirming write.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateWriteResponsePayload {
    /// Whether the write was accepted.
    pub success: bool,
    /// Error message (if failed).
    pub error: Option<String>,
}

/// Request to get contract code from Subsystem 4.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetCodeRequestPayload {
    // NO requester_id - per Envelope-Only Identity v2.2
    /// Contract address.
    pub address: Address,
}

/// Response with contract code.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetCodeResponsePayload {
    /// Contract bytecode.
    pub code: Bytes,
}

// =============================================================================
// EVENT BUS TOPICS
// =============================================================================

/// Event topics for Smart Contract Execution subsystem.
pub mod topics {
    /// Topic for receiving transaction execution requests.
    pub const EXECUTE_TRANSACTION_REQUEST: &str = "smart_contracts.execute.request";

    /// Topic for publishing transaction execution responses.
    pub const EXECUTE_TRANSACTION_RESPONSE: &str = "smart_contracts.execute.response";

    /// Topic for receiving HTLC execution requests.
    pub const EXECUTE_HTLC_REQUEST: &str = "smart_contracts.htlc.request";

    /// Topic for publishing HTLC execution responses.
    pub const EXECUTE_HTLC_RESPONSE: &str = "smart_contracts.htlc.response";

    /// Topic for state read requests (to Subsystem 4).
    pub const STATE_READ_REQUEST: &str = "state_management.read.request";

    /// Topic for state write requests (to Subsystem 4).
    pub const STATE_WRITE_REQUEST: &str = "state_management.write.request";

    /// Topic for code retrieval requests (to Subsystem 4).
    pub const GET_CODE_REQUEST: &str = "state_management.code.request";

    /// Topic for signature verification (to Subsystem 10).
    pub const ECRECOVER_REQUEST: &str = "signature_verification.ecrecover.request";

    /// Dead letter queue for failed executions.
    pub const DLQ: &str = "dlq.smart_contracts";
}

// =============================================================================
// SUBSYSTEM ID VALIDATION
// =============================================================================

/// Subsystem IDs for validation.
pub mod subsystem_ids {
    /// Consensus (validates blocks, sends execution requests).
    pub const CONSENSUS: u8 = 8;

    /// Transaction Ordering (orders transactions, sends execution requests).
    pub const TRANSACTION_ORDERING: u8 = 12;

    /// Cross-Chain (HTLC operations).
    pub const CROSS_CHAIN: u8 = 15;

    /// State Management (state read/write).
    pub const STATE_MANAGEMENT: u8 = 4;

    /// Signature Verification (ecrecover).
    pub const SIGNATURE_VERIFICATION: u8 = 10;

    /// Smart Contracts (this subsystem).
    pub const SMART_CONTRACTS: u8 = 11;

    /// Validates that sender is authorized for `ExecuteTransactionRequest`.
    #[must_use]
    pub fn is_authorized_execution_sender(sender_id: u8) -> bool {
        sender_id == CONSENSUS || sender_id == TRANSACTION_ORDERING
    }

    /// Validates that sender is authorized for `ExecuteHTLCRequest`.
    #[must_use]
    pub fn is_authorized_htlc_sender(sender_id: u8) -> bool {
        sender_id == CROSS_CHAIN
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subsystem_id_validation() {
        // Consensus (8) can send execution requests
        assert!(subsystem_ids::is_authorized_execution_sender(8));

        // Transaction Ordering (12) can send execution requests
        assert!(subsystem_ids::is_authorized_execution_sender(12));

        // Cross-Chain (15) cannot send execution requests
        assert!(!subsystem_ids::is_authorized_execution_sender(15));

        // Cross-Chain (15) can send HTLC requests
        assert!(subsystem_ids::is_authorized_htlc_sender(15));

        // Consensus (8) cannot send HTLC requests
        assert!(!subsystem_ids::is_authorized_htlc_sender(8));
    }

    #[test]
    fn test_execute_transaction_request_serialization() {
        let payload = ExecuteTransactionRequestPayload {
            from: Address::new([1u8; 20]),
            to: Some(Address::new([2u8; 20])),
            value: U256::from(1000),
            nonce: 5,
            gas_price: U256::from(20),
            gas_limit: 21000,
            data: Bytes::new(),
            tx_hash: Hash::new([0u8; 32]),
            block_context: BlockContext::default(),
        };

        // Test serialization roundtrip
        let serialized = serde_json::to_string(&payload).unwrap();
        let deserialized: ExecuteTransactionRequestPayload =
            serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.nonce, 5);
        assert_eq!(deserialized.gas_limit, 21000);
    }

    #[test]
    fn test_htlc_operation_serialization() {
        let claim = HtlcOperationPayload::Claim {
            secret: Hash::new([42u8; 32]),
        };

        let serialized = serde_json::to_string(&claim).unwrap();
        assert!(serialized.contains("Claim"));

        let refund = HtlcOperationPayload::Refund;
        let serialized = serde_json::to_string(&refund).unwrap();
        assert!(serialized.contains("Refund"));
    }

    #[test]
    fn test_state_change_in_response() {
        let response = ExecuteTransactionResponsePayload {
            success: true,
            gas_used: 21000,
            output: Bytes::new(),
            logs: vec![],
            state_changes: vec![StateChange::NonceIncrement {
                address: Address::new([1u8; 20]),
            }],
            contract_address: None,
            revert_reason: None,
        };

        assert!(response.success);
        assert_eq!(response.state_changes.len(), 1);
    }
}
