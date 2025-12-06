//! # Domain Layer - Mempool Subsystem
//!
//! Pure business logic implementing SPEC-06-MEMPOOL.md v2.3.
//!
//! ## Components
//!
//! - `entities`: Transaction state machine, MempoolTransaction, MempoolConfig
//! - `pool`: TransactionPool with priority queue and Two-Phase Commit
//! - `services`: Domain services (RBF calculation, nonce validation)
//! - `value_objects`: PricedTransaction, ShortTxId, MempoolStatus
//! - `errors`: MempoolError enumeration
//! - `typestate`: **NEW** Compile-time enforced state machine (Wormhole-safe)
//!
//! ## Data Types (IPC-MATRIX.md Compliance)
//!
//! - Address: `[u8; 20]` (20-byte account address)
//! - Hash: `[u8; 32]` (32-byte transaction/block hash)
//! - U256: Gas prices and values (from shared-types)
//!
//! ## Security Note
//!
//! The `typestate` module provides compile-time enforcement of the Two-Phase
//! Commit state machine. This prevents the "Wormhole Bypass" vulnerability
//! where direct field mutation could bypass coordinator validation.
//!
//! For new code, prefer `TypeStateTx` and `TypeStatePool` over `MempoolTransaction`
//! and `TransactionPool` when strict safety is required.

pub mod entities;
pub mod errors;
pub mod pool;
pub mod services;
pub mod typestate;
pub mod value_objects;

pub use entities::*;
pub use errors::*;
pub use pool::*;
pub use services::*;
pub use typestate::{Confirmed, Pending, Proposed, TypeStatePool, TypeStateTx};
pub use value_objects::*;
