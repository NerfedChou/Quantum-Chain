//! # IPC Module - Security Boundaries & Authorization
//!
//! Implements IPC-MATRIX.md security requirements for Subsystem 1 (Peer Discovery).
//!
//! ## Security Boundaries (IPC-MATRIX.md)
//!
//! **Allowed Senders:**
//! - Subsystem 5 (Block Propagation) - Request peer list
//! - Subsystem 7 (Bloom Filters) - Request full nodes
//! - Subsystem 13 (Light Clients) - Request full nodes
//! - External Bootstrap Nodes - Initial network entry
//!
//! **Allowed Recipients:**
//! - Subsystem 5, 7, 13 - PeerList responses
//! - Subsystem 10 - VerifyNodeIdentityRequest for DDoS defense
//!
//! ## Message Types
//!
//! All messages wrapped in `AuthenticatedMessage<T>` envelope per Architecture.md v2.2.

pub mod bootstrap;
pub mod handler;
pub mod payloads;
pub mod security;
pub mod verify_node_identity;

pub use bootstrap::*;
pub use handler::*;
pub use payloads::*;
pub use security::*;
pub use verify_node_identity::*;

