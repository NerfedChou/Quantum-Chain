//! # Adapters Layer (Hexagonal Architecture)
//!
//! Implements outbound port traits for light client functionality.
//!
//! Reference: SPEC-13-LIGHT-CLIENT.md Section 7

mod full_node;
mod peer_discovery;

pub use full_node::HttpFullNodeConnection;
pub use peer_discovery::PeerDiscoveryAdapter;
