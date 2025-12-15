//! # Ethereum Node Records (ENR)
//!
//! Implements EIP-778 inspired self-signed node identity records.
//!
//! ## Security Properties
//!
//! - Self-signed: Record is signed by the node's private key
//! - Sequence number: Prevents replay of old records
//! - Compact: Efficient wire format for gossip
//!
//! Reference: EIP-778 (Ethereum Node Records)

// Semantic submodules
mod cache;
mod capability;
mod config;
mod record;
mod security;

// Re-export public API
pub use cache::{CachedRecord, EnrCache};
pub use capability::{Capability, CapabilityData, CapabilityType};
pub use config::EnrConfig;
pub use record::{NodeRecord, NodeRecordConfig};
pub use security::{enr_hash, PublicKey, Signature};

#[cfg(test)]
mod tests;
