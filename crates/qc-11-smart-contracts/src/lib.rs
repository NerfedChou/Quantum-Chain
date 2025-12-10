//! # QC-11 Smart Contracts - Programmable Execution Subsystem
//!
//! **Subsystem ID:** 11  
//! **Specification:** SPEC-11-SMART-CONTRACTS.md v2.3  
//! **Architecture:** Architecture.md v2.3, IPC-MATRIX.md v2.3  
//! **Status:** Production-Ready (Phase 3)
//!
//! ## Purpose
//!
//! Provides a sandboxed virtual machine (EVM) for executing deterministic smart
//! contract code. Manages gas metering, memory allocation, and state access while
//! ensuring isolation and security.
//!
//! ## Domain Invariants
//!
//! | ID | Invariant | Enforcement Location |
//! |----|-----------|---------------------|
//! | INVARIANT-1 | Gas Limit Enforcement | `domain/invariants.rs:26-28` - `check_gas_limit_invariant()` |
//! | INVARIANT-2 | Deterministic Execution | `domain/invariants.rs:39-49` - `check_determinism_invariant()` |
//! | INVARIANT-3 | No State Change on Revert | `domain/invariants.rs:55-63` - `check_revert_rollback_invariant()` |
//! | INVARIANT-4 | Static Call Purity | `domain/invariants.rs:69-84` - `check_static_purity_invariant()` |
//! | INVARIANT-5 | Call Depth Limit | `domain/invariants.rs:90-93` - `check_call_depth_invariant()` |
//!
//! ## Security (IPC-MATRIX.md)
//!
//! - **Centralized Security**: Uses `shared-types::security` for envelope validation
//! - **Envelope-Only Identity**: Identity derived solely from `sender_id`
//! - **Sandbox Execution**: Untrusted bytecode runs in isolated VM
//!
//! ### IPC Authorization Matrix
//!
//! | Message | Authorized Sender(s) | Enforcement |
//! |---------|---------------------|-------------|
//! | `ExecuteTransactionRequest` | Consensus (8), Tx Ordering (12) | `service.rs:130-143` |
//! | `ExecuteHTLCRequest` | Cross-Chain (15) ONLY | `service.rs:251-261` |
//!
//! ### Execution Safety Limits (MANDATORY per System.md)
//!
//! | Limit | Value | Purpose |
//! |-------|-------|---------|
//! | `max_call_depth` | 1024 | Prevent stack overflow |
//! | `max_code_size` | 24 KB (EIP-170) | Limit contract size |
//! | `max_init_code_size` | 48 KB (EIP-3860) | Limit deployment code |
//! | `max_stack_size` | 1024 | EVM stack limit |
//! | `max_memory_size` | 16 MB | Memory expansion limit |
//! | `execution_timeout` | 5 seconds | Hard timeout |
//!
//! ## Outbound Dependencies
//!
//! | Subsystem | Trait | Purpose |
//! |-----------|-------|---------|
//! | 4 (State Mgmt) | `StateAccess` | Read/write contract state |
//! | 10 (Sig Verify) | `SignatureVerifier` | ecrecover precompile |
//!
//! ## EVM Components
//!
//! | Component | Location | Purpose |
//! |-----------|----------|---------|
//! | Interpreter | `evm/interpreter.rs` | Main execution engine |
//! | Stack | `evm/stack.rs` | 1024-item stack |
//! | Memory | `evm/memory.rs` | Dynamic memory with gas |
//! | Gas | `evm/gas.rs` | Cost tables & calculations |
//! | Precompiles | `evm/precompiles/` | ecrecover, sha256, modexp |
//!
//! ## Usage Example
//!
//! ```ignore
//! use qc_11_smart_contracts::prelude::*;
//!
//! // Execute a transaction
//! let result = api.execute_transaction(&tx, &block_context).await?;
//!
//! // Check result
//! if result.success {
//!     println!("Gas used: {}", result.gas_used);
//!     println!("Output: {:?}", result.output);
//! }
//! ```

// Crate-level lints
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]

// =============================================================================
// MODULES
// =============================================================================

pub mod adapters;
pub mod domain;
pub mod errors;
pub mod events;
pub mod evm;
pub mod optimizer;
pub mod ports;
pub mod service;

// =============================================================================
// PRELUDE
// =============================================================================

/// Convenient re-exports for common usage.
pub mod prelude {
    // Domain entities
    pub use crate::domain::entities::{
        AccountState, BlockContext, ExecutionContext, ExecutionResult, EvmVersion, Log,
        StateChange, VmConfig,
    };

    // Value objects
    pub use crate::domain::value_objects::{
        Address, Bytes, EcdsaSignature, GasCounter, Hash, StorageKey, StorageValue, U256,
    };

    // Domain services
    pub use crate::domain::services::{
        compute_contract_address, compute_contract_address_create2, estimate_base_gas, keccak256,
        precompiles,
    };

    // Invariants
    pub use crate::domain::invariants::{
        check_all_invariants, limits, InvariantCheckResult, InvariantViolation,
    };

    // Ports
    pub use crate::ports::inbound::{
        BatchExecutor, HtlcExecutor, HtlcOperation, SignedTransaction, SmartContractApi,
        TransactionReceipt,
    };
    pub use crate::ports::outbound::{
        AccessList, AccessStatus, BlockHashOracle, SignatureVerifier, StateAccess,
        TransientStorage,
    };

    // Events
    pub use crate::events::{
        subsystem_ids, topics, ExecuteHTLCRequestPayload, ExecuteHTLCResponsePayload,
        ExecuteTransactionRequestPayload, ExecuteTransactionResponsePayload,
        GetCodeRequestPayload, GetCodeResponsePayload, HtlcOperationPayload,
        StateReadRequestPayload, StateReadResponsePayload, StateWriteRequestPayload,
        StateWriteResponsePayload,
    };

    // Errors
    pub use crate::errors::{IpcError, PrecompileError, StateError, VmError};

    // EVM components
    pub use crate::evm::{
        gas, memory::Memory, opcodes::Opcode, stack::Stack,
        transient::TransientStorage as EvmTransientStorage, Interpreter,
    };

    // Adapters
    pub use crate::adapters::{InMemoryAccessList, InMemoryState, SmartContractEventHandler};

    // Service
    pub use crate::service::{
        create_test_service, ServiceConfig, ServiceStats, SmartContractService,
    };
}

// =============================================================================
// CRATE INFO
// =============================================================================

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Subsystem ID for IPC (per IPC-MATRIX.md).
pub const SUBSYSTEM_ID: u8 = 11;

/// Subsystem name.
pub const SUBSYSTEM_NAME: &str = "Smart Contracts";

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subsystem_id() {
        assert_eq!(SUBSYSTEM_ID, 11);
    }

    #[test]
    fn test_prelude_exports() {
        // Verify prelude exports compile
        use prelude::*;
        let _ = VmConfig::default();
        let _ = Address::ZERO;
    }
}
