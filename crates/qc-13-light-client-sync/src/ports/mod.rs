//! # Ports Module
//!
//! Hexagonal architecture ports (inbound API, outbound dependencies).
//!
//! Reference: SPEC-13 Section 3

pub mod inbound;
pub mod outbound;

pub use inbound::*;
pub use outbound::*;
