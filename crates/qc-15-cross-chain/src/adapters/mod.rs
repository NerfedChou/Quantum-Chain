//! # Adapters Layer (Hexagonal Architecture)
//!
//! Implements outbound port traits for cross-chain communication.
//!
//! Reference: SPEC-15-CROSS-CHAIN.md Section 7

mod chain_client;
mod finality_checker;
mod htlc_contract;

pub use chain_client::HttpChainClient;
pub use finality_checker::ConfigurableFinalityChecker;
pub use htlc_contract::InMemoryHTLCContract;
