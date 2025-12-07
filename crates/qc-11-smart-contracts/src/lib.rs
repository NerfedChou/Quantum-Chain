//! # Smart Contract Execution (Subsystem 11)
//!
//! Provides a sandboxed virtual machine (EVM) for executing deterministic
//! smart contract code. Manages gas metering, memory allocation, and state access.
//!
//! ## Architecture Compliance (Architecture.md v2.3)
//!
//! - **Bounded Context:** Programmable Execution
//! - **Hexagonal Architecture:** Domain (pure logic) → Ports (traits) → Adapters
//! - **EDA Pattern:** Subscribes to events from Event Bus, publishes results
//! - **Envelope-Only Identity (v2.2):** No `requester_id` in payloads
//!
//! ## Dependencies (System.md v2.3)
//!
//! | Dependency | Subsystem | Purpose |
//! |------------|-----------|---------|
//! | State Management | 4 | Read/write contract state |
//! | Signature Verification | 10 | ecrecover precompile |
//!
//! ## Authorized Senders (IPC-MATRIX.md)
//!
//! | Message | Authorized Senders |
//! |---------|-------------------|
//! | `ExecuteTransactionRequest` | Consensus (8), Transaction Ordering (12) |
//! | `ExecuteHTLCRequest` | Cross-Chain (15) |
//!
//! ## Phase
//!
//! This subsystem is in **Phase 3 (Advanced - Weeks 9-12)** per System.md.
//!
//! ## Example
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
