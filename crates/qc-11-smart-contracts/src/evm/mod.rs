//! # EVM Implementation
//!
//! Ethereum Virtual Machine implementation for smart contract execution.
//!
//! ## Architecture Compliance (Architecture.md v2.3)
//!
//! This is the **outer layer** (adapter) of the hexagonal architecture.
//! It implements the domain ports to provide actual EVM execution.
//!
//! ## Components
//!
//! - `interpreter.rs` - Opcode execution engine
//! - `gas.rs` - Gas metering and costs
//! - `memory.rs` - Memory management
//! - `stack.rs` - Stack operations
//! - `opcodes.rs` - Opcode definitions
//! - `precompiles/` - Precompiled contracts
//! - `transient.rs` - Transient storage (EIP-1153)

pub mod gas;
pub mod interpreter;
pub mod memory;
pub mod opcodes;
pub mod precompiles;
pub mod stack;
pub mod transient;

pub use gas::*;
pub use interpreter::*;
pub use memory::*;
pub use opcodes::*;
pub use stack::*;
pub use transient::*;
