//! # Peer Scoring System (Gossip Scoring)
//!
//! Implements Libp2p-style peer scoring for spam protection.
//!
//! Reference: Libp2p GossipSub v1.1 Peer Scoring

// Semantic submodules
mod config;
mod manager;
mod security;

// Re-export public API
pub use config::PeerScoreConfig;
pub use manager::PeerScoreManager;
pub use security::PeerScore;

#[cfg(test)]
mod tests;
