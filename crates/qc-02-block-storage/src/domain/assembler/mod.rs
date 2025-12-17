//! # Stateful Assembler
//!
//! Implements the V2.3 Choreography Pattern for block assembly.
//!
//! ## Architecture (Architecture.md v2.3)
//!
//! Block Storage is a **Stateful Assembler** that:
//! 1. Subscribes to THREE independent events (no orchestrator)
//! 2. Buffers incoming components by `block_hash` until all three arrive
//! 3. Performs atomic write when all components are present
//! 4. Implements assembly timeout for resource exhaustion defense
//!
//! ## SPEC-02 Reference
//!
//! - Section 2.4: Stateful Assembler Structures
//! - Section 2.6: INVARIANT-7 (Assembly Timeout), INVARIANT-8 (Bounded Buffer)
//!
//! ## Module Structure
//!
//! - `buffer` - BlockAssemblyBuffer implementation
//! - `config` - AssemblyConfig configuration
//! - `pending` - PendingBlockAssembly struct
//! - `security` - Security validation and limits

mod buffer;
mod config;
mod pending;
pub mod security;

#[cfg(test)]
mod tests;

// Re-export public API
pub use buffer::BlockAssemblyBuffer;
pub use config::AssemblyConfig;
pub use pending::PendingBlockAssembly;
