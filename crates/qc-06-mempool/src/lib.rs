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
//! ## Domain Invariants
//!
//! | ID | Invariant | Enforcement Location |
//! |----|-----------|---------------------|
//! | INVARIANT-1 | No Duplicate Transactions | `domain/pool.rs:131-133` - `add()` check |
//! | INVARIANT-2 | Nonce Ordering Per Sender | `domain/pool.rs:364-379` - BTreeMap keys |
//! | INVARIANT-3 | PENDING_INCLUSION Exclusion | `domain/pool.rs:48-49` - by_price only has PENDING |
//! | INVARIANT-5 | Auto-Rollback on Timeout | `domain/pool.rs:489-502` - `cleanup_timeouts()` |
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
//! | Stage | Method | Effect |
//! |-------|--------|--------|
//! | Propose | `pool.propose()` | Move to PENDING_INCLUSION, NOT deleted |
//! | Confirm | `pool.confirm()` | Permanently delete transactions |
//! | Rollback | `pool.rollback()` | Return to PENDING state |
//! | Timeout | `cleanup_timeouts()` | Auto-rollback after 30s |
//!
//! ## Security (IPC-MATRIX.md)
//!
//! - **Centralized Security**: Uses `shared-types::security` for HMAC, nonce, timestamp
//! - **Envelope-Only Identity**: Identity derived solely from `AuthenticatedMessage.sender_id`
//! - **Replay Prevention**: Centralized `NonceCache` tracks used nonces
//!
//! ### IPC Authorization Matrix
//!
//! | Message | Authorized Sender(s) | Enforcement |
//! |---------|---------------------|-------------|
//! | `AddTransactionRequest` | Signature Verification (10) | `ipc/security.rs:55-63` |
//! | `GetTransactionsRequest` | Consensus (8) | `ipc/security.rs:68-76` |
//! | `RemoveTransactionsRequest` | Consensus (8) | `ipc/security.rs:81-89` |
//! | `BlockStorageConfirmation` | Block Storage (2) | `ipc/security.rs:94-102` |
//! | `BlockRejectedNotification` | Block Storage (2), Consensus (8) | `ipc/security.rs:107-115` |
//!
//! ## Outbound Dependencies
//!
//! | Subsystem | Trait | Purpose |
//! |-----------|-------|---------|
//! | 4 (State Management) | `StateProvider` | Balance/nonce validation |
//!
//! ## Module Structure (Hexagonal Architecture)
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      OUTER LAYER                                │
//! │  adapters/ - Event bus publisher/subscriber implementations     │
//! └─────────────────────────────────────────────────────────────────┘
//!                          ↑ implements ↑
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      MIDDLE LAYER                               │
//! │  ports/inbound.rs  - MempoolApi trait                          │
//! │  ports/outbound.rs - StateProvider, TimeSource traits          │
//! └─────────────────────────────────────────────────────────────────┘
//!                          ↑ uses ↑
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      INNER LAYER                                │
//! │  domain/entities.rs    - MempoolTransaction, TransactionState  │
//! │  domain/pool.rs        - TransactionPool with priority queue   │
//! │  domain/typestate.rs   - TypeStateTx (compile-time safety)     │
//! │  domain/services.rs    - RBF calculation, nonce validation     │
//! │  domain/value_objects.rs - PricedTransaction, MempoolStatus    │
//! │  domain/errors.rs      - MempoolError enum                     │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## TypeState Pattern (Compile-Time Safety)
//!
//! The `typestate` module provides compile-time enforcement of the Two-Phase
//! Commit state machine, preventing "Wormhole Bypass" vulnerabilities:
//!
//! ```rust,ignore
//! // Only valid transitions compile:
//! let tx: TypeStateTx<Pending> = pool.add(tx)?;
//! let tx: TypeStateTx<Proposed> = tx.propose(block_height, now);
//! let tx: TypeStateTx<Confirmed> = tx.confirm(); // OR
//! let tx: TypeStateTx<Pending> = tx.rollback();
//! ```

pub mod adapters;
pub mod domain;
pub mod ipc;
pub mod ports;
// pub mod service;

pub use adapters::*;
pub use domain::*;
pub use ipc::*;
