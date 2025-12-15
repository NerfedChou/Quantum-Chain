//! # Address Manager - New/Tried Bucket System
//!
//! Implements Bitcoin's `addrman` pattern for Eclipse Attack resistance.
//!
//! ## Design (Bitcoin-Inspired)
//!
//! - **New Table**: Addresses heard about but never successfully connected to
//! - **Tried Table**: Addresses we've successfully connected to
//!
//! ## Anti-Eclipse Properties
//!
//! 1. Per-subnet bucketing prevents IP flooding attacks
//! 2. Segregation prevents poisoning Tried with unverified addresses
//! 3. Source-based bucketing distributes gossip across buckets
//!
//! Reference: Bitcoin Core's `addrman.h`

// Semantic submodules
mod bucket;
mod config;
mod manager;
mod security;
mod table;
mod types;

// Re-export public API
pub use bucket::AddressBucket;
pub use config::AddressManagerConfig;
pub use manager::AddressManager;
pub use security::{secure_bucket_hash, AddressManagerError, SubnetKey};
pub use table::AddressTable;
pub use types::{AddressEntry, AddressManagerStats};

#[cfg(test)]
mod tests;
