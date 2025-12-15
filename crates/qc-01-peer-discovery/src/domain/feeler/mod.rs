//! # Feeler Connection Service
//!
//! Implements stochastic probing to promote New Table addresses to Tried Table.
//!
//! Reference: Bitcoin Core's `-feeler` connection logic

// Semantic submodules
mod bucket;
mod config;
mod service;
mod types;

// Re-export public API
pub use bucket::BucketFreshness;
pub use config::FeelerConfig;
pub use service::FeelerState;
pub use types::{FeelerProbe, FeelerResult};

#[cfg(test)]
mod tests;
