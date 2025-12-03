//! # Block Propagation Subsystem (qc-05)
//!
//! Distributes validated blocks across the P2P network using epidemic gossip protocol.
//! Implements BIP152-style compact block relay for bandwidth efficiency.
//!
//! ## Architecture Role
//!
//! ```text
//! [Consensus (8)] ──PropagateBlockRequest──→ [Block Propagation (5)]
//!                                                    │
//!                                                    ↓ gossip (fanout=8)
//!                                            ┌───────┴───────┐
//!                                            ↓               ↓
//!                                       [Peer A]        [Peer B] ...
//! ```
//!
//! ## Security (IPC-MATRIX.md)
//!
//! - Only Consensus (8) can request block propagation
//! - Network blocks require signature verification via Subsystem 10
//! - Invalid signatures → SILENT DROP (IP spoofing defense)

pub mod domain;
pub mod events;
pub mod ipc;
pub mod ports;
pub mod service;

pub use domain::*;
pub use events::PropagationError;
pub use ports::inbound::BlockPropagationApi;
pub use service::BlockPropagationService;
