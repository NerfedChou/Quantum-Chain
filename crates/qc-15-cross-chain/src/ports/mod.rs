//! # Ports Module
//!
//! Hexagonal architecture ports (inbound API, outbound dependencies).
//!
//! Reference: SPEC-15 Section 3

pub mod inbound;
pub mod outbound;

pub use inbound::*;
pub use outbound::*;
