//! # Error Types
//!
//! All error types for smart contract execution.

use crate::domain::value_objects::{Address, U256};
use thiserror::Error;

// =============================================================================
// VM ERRORS
// =============================================================================

/// Errors that can occur during EVM execution.
#[derive(Debug, Error, Clone)]
pub enum VmError {
    /// Execution ran out of gas.
    #[error("out of gas")]
    OutOfGas,

    /// Stack overflow (>1024 items).
    #[error("stack overflow")]
    StackOverflow,

    /// Stack underflow (pop from empty stack).
    #[error("stack underflow")]
    StackUnderflow,

    /// Invalid opcode encountered.
    #[error("invalid opcode: 0x{0:02X}")]
    InvalidOpcode(u8),

    /// Invalid jump destination.
    #[error("invalid jump destination: {0}")]
    InvalidJump(usize),

    /// Call depth exceeded maximum.
    #[error("call depth exceeded: {depth} > {max}")]
    CallDepthExceeded { depth: u16, max: u16 },

    /// Contract code size exceeded limit.
    #[error("code size exceeded: {size} > {max} bytes")]
    CodeSizeExceeded { size: usize, max: usize },

    /// Init code size exceeded limit (EIP-3860).
    #[error("init code size exceeded: {size} > {max} bytes")]
    InitCodeSizeExceeded { size: usize, max: usize },

    /// Attempted to modify state in static context.
    #[error("write operation in static context")]
    WriteInStaticContext,

    /// Insufficient balance for transfer.
    #[error("insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: U256, available: U256 },

    /// State access error.
    #[error("state error: {0}")]
    StateError(#[from] StateError),

    /// Execution reverted.
    #[error("revert: {0}")]
    Revert(String),

    /// Memory access out of bounds.
    #[error("memory access out of bounds: offset {offset}, size {size}")]
    MemoryOutOfBounds { offset: usize, size: usize },

    /// Memory expansion would exceed limit.
    #[error("memory limit exceeded: {requested} > {max} bytes")]
    MemoryLimitExceeded { requested: usize, max: usize },

    /// Return data out of bounds (RETURNDATACOPY).
    #[error("return data out of bounds: offset {offset}, size {size}, available {available}")]
    ReturnDataOutOfBounds {
        offset: usize,
        size: usize,
        available: usize,
    },

    /// Contract already exists at CREATE address.
    #[error("contract already exists at address: {0:?}")]
    ContractAlreadyExists(Address),

    /// Invalid contract creation (empty code after init).
    #[error("invalid contract creation: code is empty")]
    InvalidContractCreation,

    /// Code starts with 0xEF (reserved for EOF).
    #[error("code starts with 0xEF byte (reserved for EOF)")]
    InvalidCodePrefix,

    /// Execution timeout exceeded.
    #[error("execution timeout: {elapsed_ms}ms > {max_ms}ms")]
    Timeout { elapsed_ms: u64, max_ms: u64 },

    /// Internal error (should not happen in production).
    #[error("internal error: {0}")]
    Internal(String),
}

impl VmError {
    /// Returns true if this error is recoverable (can continue execution).
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        matches!(self, Self::Revert(_))
    }

    /// Returns true if this error consumes all gas.
    #[must_use]
    pub fn consumes_all_gas(&self) -> bool {
        !matches!(self, Self::Revert(_))
    }
}

// =============================================================================
// STATE ERRORS
// =============================================================================

/// Errors from state access operations.
#[derive(Debug, Error, Clone)]
pub enum StateError {
    /// State not found (shouldn't happen for valid addresses).
    #[error("state not found for address: {0:?}")]
    NotFound(Address),

    /// State database is corrupted.
    #[error("state corruption detected")]
    Corrupted,

    /// State access timed out.
    #[error("state access timeout")]
    Timeout,

    /// State access was rejected (permission denied).
    #[error("state access denied")]
    AccessDenied,

    /// Connection to state subsystem lost.
    #[error("state subsystem unavailable")]
    Unavailable,

    /// Invalid state root.
    #[error("invalid state root")]
    InvalidStateRoot,

    /// Other state error.
    #[error("state error: {0}")]
    Other(String),
}

// =============================================================================
// PRECOMPILE ERRORS
// =============================================================================

/// Errors from precompiled contract execution.
#[derive(Debug, Error, Clone)]
pub enum PrecompileError {
    /// Invalid input length.
    #[error("invalid input length: expected {expected}, got {actual}")]
    InvalidInputLength { expected: usize, actual: usize },

    /// Invalid input data.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// Out of gas during precompile execution.
    #[error("precompile out of gas")]
    OutOfGas,

    /// Cryptographic operation failed.
    #[error("crypto error: {0}")]
    CryptoError(String),

    /// Precompile not implemented.
    #[error("precompile not implemented: {0:?}")]
    NotImplemented(Address),
}

impl From<PrecompileError> for VmError {
    fn from(err: PrecompileError) -> Self {
        match err {
            PrecompileError::OutOfGas => VmError::OutOfGas,
            _ => VmError::Revert(err.to_string()),
        }
    }
}

// =============================================================================
// IPC ERRORS
// =============================================================================

/// Errors related to IPC communication.
#[derive(Debug, Error, Clone)]
pub enum IpcError {
    /// Message validation failed.
    #[error("message validation failed: {0}")]
    ValidationFailed(String),

    /// Unauthorized sender.
    #[error("unauthorized sender: {sender_id} not in allowed list {allowed:?}")]
    UnauthorizedSender { sender_id: u8, allowed: Vec<u8> },

    /// Correlation ID mismatch.
    #[error("correlation ID mismatch")]
    CorrelationMismatch,

    /// Response timeout.
    #[error("IPC response timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    /// Serialization error.
    #[error("serialization error: {0}")]
    SerializationError(String),

    /// Event bus error.
    #[error("event bus error: {0}")]
    EventBusError(String),

    /// Reply-to validation failed (forwarding attack prevention).
    #[error("reply-to mismatch: reply_to.subsystem_id={reply_to} != sender_id={sender}")]
    ReplyToMismatch { reply_to: u8, sender: u8 },
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_error_display() {
        let err = VmError::OutOfGas;
        assert_eq!(err.to_string(), "out of gas");

        let err = VmError::InvalidOpcode(0xFE);
        assert_eq!(err.to_string(), "invalid opcode: 0xFE");

        let err = VmError::CallDepthExceeded {
            depth: 1025,
            max: 1024,
        };
        assert_eq!(err.to_string(), "call depth exceeded: 1025 > 1024");
    }

    #[test]
    fn test_vm_error_recoverable() {
        assert!(!VmError::OutOfGas.is_recoverable());
        assert!(!VmError::StackOverflow.is_recoverable());
        assert!(VmError::Revert("test".to_string()).is_recoverable());
    }

    #[test]
    fn test_vm_error_consumes_gas() {
        assert!(VmError::OutOfGas.consumes_all_gas());
        assert!(VmError::InvalidOpcode(0xFF).consumes_all_gas());
        assert!(!VmError::Revert("test".to_string()).consumes_all_gas());
    }

    #[test]
    fn test_state_error_conversion() {
        let state_err = StateError::Timeout;
        let vm_err: VmError = state_err.into();
        assert!(matches!(vm_err, VmError::StateError(_)));
    }

    #[test]
    fn test_precompile_error_conversion() {
        let pre_err = PrecompileError::OutOfGas;
        let vm_err: VmError = pre_err.into();
        assert!(matches!(vm_err, VmError::OutOfGas));

        let pre_err = PrecompileError::InvalidInput("bad".to_string());
        let vm_err: VmError = pre_err.into();
        assert!(matches!(vm_err, VmError::Revert(_)));
    }

    #[test]
    fn test_ipc_error_display() {
        let err = IpcError::UnauthorizedSender {
            sender_id: 5,
            allowed: vec![8, 12],
        };
        assert!(err.to_string().contains("unauthorized"));
        assert!(err.to_string().contains('5'));
    }
}
