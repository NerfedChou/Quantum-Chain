//! Domain Services - Pure functions for Kademlia operations
//!
//! Reference: SPEC-01-PEER-DISCOVERY.md Section 2
//!
//! All functions in this module are pure (no I/O, no state mutation)
//! and deterministic (same inputs → same outputs).

// Semantic submodules
mod distance;
mod security;
mod sorting;

// Re-export public API
pub use distance::{bucket_for_peer, calculate_bucket_index, xor_distance};
pub use security::is_same_subnet;
pub use sorting::{find_k_closest, sort_peers_by_distance};

#[cfg(test)]
mod tests;
