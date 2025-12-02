//! # Transaction Pool (Mempool) Subsystem
//!
//! **Subsystem ID:** 6  
//! **Specification:** `SPECS/SPEC-06-MEMPOOL.md` v2.3  
//! **Bounded Context:** Transaction Management
//!
//! ## Purpose
//!
//! The Mempool subsystem queues, validates, and prioritizes unconfirmed transactions
//! awaiting inclusion in blocks. It implements a Two-Phase Commit protocol to prevent
//! transaction loss during block storage.
//!
//! ## Architecture
//!
//! This crate follows Hexagonal Architecture:
//!
//! - **Domain Layer** (`domain/`): Pure business logic for transaction pool management
//! - **Ports Layer** (`ports/`): Traits for inbound API and outbound dependencies
//! - **Service Layer** (`service.rs`): Application service implementing the API
//! - **IPC Layer** (`ipc/`): Security boundaries and message handling
//! - **Adapters Layer** (`adapters/`): Event bus integration
//!
//! ## Two-Phase Commit Protocol
//!
//! Transactions are NEVER deleted when proposed for a block. They are only deleted
//! upon receiving confirmation from Block Storage (Subsystem 2).
//!
//! ```text
//! [PENDING] ──propose──→ [PENDING_INCLUSION] ──confirm──→ [DELETED]
//!                               │
//!                               └── timeout/reject ──→ [PENDING] (rollback)
//! ```
//!
//! ## Security
//!
//! Per IPC-MATRIX.md:
//! - `AddTransactionRequest`: Subsystem 10 ONLY (pre-verified signatures)
//! - `GetTransactionsRequest`: Subsystem 8 ONLY (Consensus)
//! - `BlockStorageConfirmation`: Subsystem 2 ONLY (Block Storage)
//!
//! ## Example
//!
//! ```rust,ignore
//! use qc_06_mempool::domain::{TransactionPool, MempoolConfig, MempoolTransaction};
//!
//! let mut pool = TransactionPool::new(MempoolConfig::default());
//!
//! // Add a transaction
//! let tx = MempoolTransaction::new(
//!     [1u8; 32],  // hash
//!     [0xAA; 32], // sender
//!     0,          // nonce
//!     1_000_000_000, // gas_price (1 gwei)
//!     21000,      // gas_limit
//!     0,          // value
//!     vec![],     // data
//!     1000,       // added_at (timestamp)
//! );
//! pool.add(tx).unwrap();
//!
//! // Get transactions for block
//! let batch = pool.get_for_block(100, 30_000_000);
//!
//! // Propose transactions (Phase 1)
//! let hashes: Vec<_> = batch.iter().map(|t| t.hash).collect();
//! pool.propose(&hashes, 1, 2000);
//!
//! // Confirm inclusion (Phase 2a) - transactions are deleted
//! pool.confirm(&hashes);
//! ```

pub mod adapters;
pub mod domain;
pub mod ipc;
pub mod ports;
// pub mod service;

pub use adapters::*;
pub use domain::*;
pub use ipc::*;
