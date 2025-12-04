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
//!
//! ## Data Types (IPC-MATRIX.md Compliance)
//!
//! - Address: `[u8; 20]` (20-byte account address)
//! - Hash: `[u8; 32]` (32-byte transaction/block hash)
//! - U256: Gas prices and values (from shared-types)

pub mod entities;
pub mod errors;
pub mod pool;
pub mod services;
pub mod value_objects;

pub use entities::*;
pub use errors::*;
pub use pool::*;
pub use services::*;
pub use value_objects::*;
