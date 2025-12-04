//! # Transaction Pool (Mempool) Subsystem
//!
//! **Subsystem ID:** 6  
//! **Specification:** SPEC-06-MEMPOOL.md v2.3  
//! **Architecture:** Architecture.md v2.3, IPC-MATRIX.md v2.3  
//! **Status:** Production-Ready
//!
//! ## Purpose
//!
//! Queues, validates, and prioritizes unconfirmed transactions awaiting block inclusion.
//! Implements a Two-Phase Commit protocol ensuring zero transaction loss during storage.
//!
//! ## Invariants (SPEC-06 Section 2.1)
//!
//! - **INVARIANT-1**: No duplicate transaction hashes in the pool
//! - **INVARIANT-2**: Transactions from same sender ordered by nonce
//! - **INVARIANT-3**: PENDING_INCLUSION transactions excluded from `get_for_block()`
//! - **INVARIANT-5**: Timed-out PENDING_INCLUSION transactions auto-rollback
//!
//! ## Two-Phase Commit Protocol
//!
//! Transactions are NEVER deleted when proposed. Deletion occurs ONLY upon
//! confirmation from Block Storage (Subsystem 2).
//!
//! ```text
//! [PENDING] ──propose──→ [PENDING_INCLUSION] ──confirm──→ [DELETED]
//!                               │
//!                               └── timeout/reject ──→ [PENDING]
//! ```
//!
//! ## IPC Authorization (IPC-MATRIX.md)
//!
//! | Message | Authorized Sender |
//! |---------|-------------------|
//! | AddTransactionRequest | Subsystem 10 (Signature Verification) |
//! | GetTransactionsRequest | Subsystem 8 (Consensus) |
//! | BlockStorageConfirmation | Subsystem 2 (Block Storage) |
//! | BlockRejectedNotification | Subsystems 2, 8 |
//!
//! ## Architecture (Hexagonal)
//!
//! - `domain/`: Pure business logic (TransactionPool, entities, services)
//! - `ports/`: Inbound API traits, outbound dependency traits
//! - `ipc/`: Security validation, message handlers, payloads
//! - `adapters/`: Event bus publisher/subscriber implementations

pub mod adapters;
pub mod domain;
pub mod ipc;
pub mod ports;
// pub mod service;

pub use adapters::*;
pub use domain::*;
pub use ipc::*;
