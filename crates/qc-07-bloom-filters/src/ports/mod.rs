//! Ports Layer
//!
//! Reference: Architecture.md - Hexagonal Architecture
//!
//! Defines the interfaces (traits) for:
//! - Driving Ports (inbound) - API for external callers
//! - Driven Ports (outbound) - Dependencies on other subsystems

pub mod inbound;
pub mod outbound;

pub use inbound::{BloomFilterApi, LogEntry, MatchResult, MatchedField, TransactionReceipt};
pub use outbound::{TransactionAddresses, TransactionDataProvider};
