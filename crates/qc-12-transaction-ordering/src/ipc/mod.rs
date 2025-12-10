//! IPC Module for Transaction Ordering
//!
//! Reference: IPC-MATRIX.md Subsystem 12
//!
//! ## Security Boundaries
//!
//! - Accept: OrderTransactionsRequest from Subsystem 8 (Consensus) ONLY
//! - Send: OrderedTransactions to Subsystem 11 (Smart Contracts)
//! - Query: Subsystem 4 (State Management) for conflict detection

pub mod handler;
pub mod payloads;

pub use handler::TransactionOrderingHandler;
pub use payloads::*;
