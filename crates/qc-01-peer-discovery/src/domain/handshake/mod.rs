//! # Chain-Aware Handshakes
//!
//! Implements Fork-ID Convergence for fast-fail chain verification.
//!
//! Reference: Ethereum's Fork-ID (EIP-2124), Go-Ethereum's handshake

// Semantic submodules
mod config;
mod fork_id;
mod security;
mod types;

// Re-export public API
pub use config::HandshakeConfig;
pub use fork_id::ForkId;
pub use security::verify_handshake;
pub use types::{
    ChainInfo, HandshakeData, HandshakeResult, HeadState, PeerClassification, RejectReason,
};

#[cfg(test)]
mod tests;
