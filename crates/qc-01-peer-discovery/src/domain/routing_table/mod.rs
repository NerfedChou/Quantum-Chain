//! Routing Table Implementation
//!
//! Reference: SPEC-01-PEER-DISCOVERY.md Section 2.2
//!
//! This module implements the Kademlia DHT routing table with all security
//! invariants from the specification.

// Semantic submodules
mod banned;
mod bucket;
mod config;
mod security;
mod table;

// Re-export public API
pub use banned::BannedPeers;
pub use bucket::KBucket;
pub use config::{MAX_TOTAL_PEERS, NUM_BUCKETS};
pub use security::{BanDetails, BannedEntry, PendingInsertion, PendingPeer, RoutingTableStats};
pub use table::RoutingTable;

#[cfg(test)]
mod tests;
