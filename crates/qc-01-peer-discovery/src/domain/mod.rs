//! Domain Layer - Pure business logic with no I/O
//!
//! This module contains the core Kademlia DHT logic including:
//! - Node identifiers and XOR distance calculation
//! - Routing table with k-buckets
//! - Peer lifecycle management
//! - Security invariants (Eclipse Attack Defense, Memory Bomb Defense)
//! - Address Manager (New/Tried bucket system - Bitcoin addrman)
//! - Peer Scoring (Gossip scoring for spam protection)
//! - Connection Slots (Score-Based Eviction)
//! - Feeler Connections (Poisson-Process Probing)
//! - Chain-Aware Handshakes (Fork-ID Convergence)
//! - ENR (Ethereum Node Records - EIP-778)

pub mod address_manager;
pub mod connection_slots;
pub mod enr;
pub mod feeler;
pub mod handshake;
pub mod peer_score;
pub mod routing_table;
pub mod services;
/// Core domain types (entities, values, errors)
pub mod types;

pub use address_manager::*;
pub use connection_slots::*;
pub use enr::*;
pub use feeler::*;
pub use handshake::*;
pub use peer_score::*;
pub use routing_table::*;
pub use services::*;
pub use types::*;
