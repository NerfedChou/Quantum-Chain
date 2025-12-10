//! Ports module for Transaction Ordering
//!
//! Defines inbound (API) and outbound (SPI) port traits.

pub mod inbound;
pub mod outbound;

pub use inbound::TransactionOrderingApi;
pub use outbound::{AccessPatternAnalyzer, ConflictDetector};
