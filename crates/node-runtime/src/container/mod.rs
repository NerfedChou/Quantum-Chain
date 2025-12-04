//! # Subsystem Container
//!
//! Central container holding all core subsystem instances with proper
//! lifetime management and dependency injection.
//!
//! ## Architecture Compliance (v2.3)
//!
//! - Subsystems initialized in dependency order (Level 0 → Level 4)
//! - All inter-subsystem communication via Event Bus (no direct calls)
//! - Adapters implement outbound ports for each subsystem

pub mod config;
pub mod subsystems;

pub use config::{ConfigError, NodeConfig};
pub use subsystems::SubsystemContainer;
