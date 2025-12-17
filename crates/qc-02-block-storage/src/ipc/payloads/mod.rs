//! # Payloads Module
//!
//! IPC Payload definitions per SPEC-02 Section 4.
//!
//! ## Modules
//!
//! - `events`: Event payloads (BlockValidated, MerkleRootComputed, etc.)
//! - `requests`: Request payloads (ReadBlock, MarkFinalized, etc.)
//! - `responses`: Response payloads (BlockStored, BlockFinalized, etc.)
//! - `security`: Payload validation and sanitization

mod events;
mod requests;
mod responses;
pub mod security;
#[cfg(test)]
mod tests;

pub use events::*;
pub use requests::*;
pub use responses::*;
