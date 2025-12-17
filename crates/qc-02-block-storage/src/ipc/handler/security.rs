//! # Handler Security
//!
//! Core security constants and utilities for IPC handlers.
//!
//! ## Security Invariants
//!
//! - **Sender Validation**: Each handler validates sender per IPC-MATRIX
//! - **Read Authorization**: Most reads are permissive (any connected subsystem)
//! - **Write Authorization**: Writes are restricted to specific senders

use crate::ipc::envelope::subsystem_ids;

// =============================================================================
// ALLOWED SENDERS PER OPERATION
// =============================================================================

/// Allowed senders for BlockValidated events (Subsystem 8: Consensus)
pub const BLOCK_VALIDATED_SENDERS: &[u8] = &[subsystem_ids::CONSENSUS];

/// Allowed senders for MerkleRootComputed events (Subsystem 3: Transaction Indexing)
pub const MERKLE_ROOT_SENDERS: &[u8] = &[subsystem_ids::TRANSACTION_INDEXING];

/// Allowed senders for StateRootComputed events (Subsystem 4: State Management)
pub const STATE_ROOT_SENDERS: &[u8] = &[subsystem_ids::STATE_MANAGEMENT];

/// Allowed senders for MarkFinalized requests (Subsystem 9: Finality)
pub const MARK_FINALIZED_SENDERS: &[u8] = &[subsystem_ids::FINALITY];

/// Allowed senders for GetChainInfo requests (Subsystem 17: Block Production)
pub const CHAIN_INFO_SENDERS: &[u8] = &[subsystem_ids::BLOCK_PRODUCTION];

/// Allowed senders for GetTransactionLocation/Hashes requests
pub const TX_SENDERS: &[u8] = &[subsystem_ids::TRANSACTION_INDEXING];

// =============================================================================
