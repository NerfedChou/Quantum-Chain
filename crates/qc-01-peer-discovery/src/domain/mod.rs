//! Domain Layer - Pure business logic with no I/O
//!
//! This module contains the core Kademlia DHT logic including:
//! - Node identifiers and XOR distance calculation
//! - Routing table with k-buckets
//! - Peer lifecycle management
//! - Security invariants (Eclipse Attack Defense, Memory Bomb Defense)

pub mod entities;
pub mod errors;
pub mod routing_table;
pub mod services;
pub mod value_objects;

pub use entities::*;
pub use errors::*;
pub use routing_table::*;
pub use services::*;
pub use value_objects::*;
