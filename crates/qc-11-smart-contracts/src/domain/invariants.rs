//! # Domain Invariants
//!
//! Critical invariants that MUST hold during smart contract execution.
//! These are checked at runtime to prevent security vulnerabilities.
//!
//! ## Architecture Compliance (SPEC-11)
//!
//! Per SPEC-11, Section 2.2:
//! - INVARIANT-1: Gas Limit Enforcement
//! - INVARIANT-2: Deterministic Execution  
//! - INVARIANT-3: No State Change on Revert
//! - INVARIANT-4: Static Call Purity
//! - INVARIANT-5: Call Depth Limit

use crate::domain::entities::{ExecutionContext, ExecutionResult, StateChange, VmConfig};

// =============================================================================
// INVARIANT CHECKS
// =============================================================================

/// INVARIANT-1: Gas Limit Enforcement
///
/// Execution cannot use more gas than `gas_limit`.
/// This prevents infinite loops and `DoS` attacks.
#[must_use]
pub fn check_gas_limit_invariant(result: &ExecutionResult, ctx: &ExecutionContext) -> bool {
    result.gas_used <= ctx.gas_limit
}

/// INVARIANT-2: Deterministic Execution
///
/// Same inputs always produce same outputs.
/// This is ensured by the EVM specification:
/// - No random opcodes
/// - Block info is fixed at call time
/// - State reads are consistent within a transaction
///
/// This function checks that the execution result is well-formed.
#[must_use]
pub fn check_determinism_invariant(result: &ExecutionResult) -> bool {
    // A well-formed result has consistent state
    if result.success {
        // Success: should have no revert reason
        result.revert_reason.is_none()
    } else {
        // Failure: state changes should be empty (rolled back)
        result.state_changes.is_empty() && result.logs.is_empty()
    }
}

/// INVARIANT-3: No State Change on Revert
///
/// If execution reverts, state changes are NOT applied.
/// All modifications must be rolled back.
#[must_use]
pub fn check_revert_rollback_invariant(result: &ExecutionResult) -> bool {
    if result.success {
        true
    } else {
        // On failure, there should be no state changes to apply
        result.state_changes.is_empty() && result.logs.is_empty()
    }
}

/// INVARIANT-4: Static Call Purity
///
/// STATICCALL cannot modify state.
/// This includes: SSTORE, CREATE, CREATE2, SELFDESTRUCT, LOG*.
#[must_use]
pub fn check_static_purity_invariant(ctx: &ExecutionContext, result: &ExecutionResult) -> bool {
    if ctx.is_static {
        // In static context, no state-modifying changes allowed
        result.state_changes.iter().all(|change| {
            !matches!(change, StateChange::BalanceTransfer { .. })
                && !matches!(change, StateChange::StorageWrite { .. })
                && !matches!(change, StateChange::StorageDelete { .. })
                && !matches!(change, StateChange::ContractCreate { .. })
                && !matches!(change, StateChange::ContractDestroy { .. })
                && !matches!(change, StateChange::NonceIncrement { .. })
        })
    } else {
        true
    }
}

/// INVARIANT-5: Call Depth Limit
///
/// Execution cannot exceed max call depth.
/// Prevents stack overflow attacks.
#[must_use]
pub fn check_call_depth_invariant(ctx: &ExecutionContext, config: &VmConfig) -> bool {
    ctx.depth <= config.max_call_depth
}

/// Check all invariants at once.
#[must_use]
pub fn check_all_invariants(
    ctx: &ExecutionContext,
    result: &ExecutionResult,
    config: &VmConfig,
) -> InvariantCheckResult {
    let mut violations = Vec::new();

    if !check_gas_limit_invariant(result, ctx) {
        violations.push(InvariantViolation::GasLimitExceeded {
            used: result.gas_used,
            limit: ctx.gas_limit,
        });
    }

    if !check_determinism_invariant(result) {
        violations.push(InvariantViolation::NonDeterministic);
    }

    if !check_revert_rollback_invariant(result) {
        violations.push(InvariantViolation::StateNotRolledBack {
            changes: result.state_changes.len(),
            logs: result.logs.len(),
        });
    }

    if !check_static_purity_invariant(ctx, result) {
        violations.push(InvariantViolation::StaticCallViolation);
    }

    if !check_call_depth_invariant(ctx, config) {
        violations.push(InvariantViolation::CallDepthExceeded {
            depth: ctx.depth,
            max: config.max_call_depth,
        });
    }

    if violations.is_empty() {
        InvariantCheckResult::Valid
    } else {
        InvariantCheckResult::Invalid(violations)
    }
}

// =============================================================================
// INVARIANT TYPES
// =============================================================================

/// Result of checking all invariants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvariantCheckResult {
    /// All invariants hold.
    Valid,
    /// One or more invariants violated.
    Invalid(Vec<InvariantViolation>),
}

impl InvariantCheckResult {
    /// Returns true if all invariants hold.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }
}

/// Specific invariant violation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvariantViolation {
    /// Gas limit exceeded.
    GasLimitExceeded { used: u64, limit: u64 },
    /// Non-deterministic execution detected.
    NonDeterministic,
    /// State not properly rolled back on revert.
    StateNotRolledBack { changes: usize, logs: usize },
    /// Static call modified state.
    StaticCallViolation,
    /// Call depth exceeded.
    CallDepthExceeded { depth: u16, max: u16 },
}

impl std::fmt::Display for InvariantViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GasLimitExceeded { used, limit } => {
                write!(f, "gas limit exceeded: used {used} > limit {limit}")
            }
            Self::NonDeterministic => {
                write!(f, "non-deterministic execution detected")
            }
            Self::StateNotRolledBack { changes, logs } => {
                write!(
                    f,
                    "state not rolled back on revert: {changes} changes, {logs} logs"
                )
            }
            Self::StaticCallViolation => {
                write!(f, "static call attempted state modification")
            }
            Self::CallDepthExceeded { depth, max } => {
                write!(f, "call depth exceeded: {depth} > {max}")
            }
        }
    }
}

// =============================================================================
// EXECUTION LIMIT CONSTANTS (System.md Compliance)
// =============================================================================

/// Execution limits per System.md and SPEC-11.
pub mod limits {
    /// Maximum call depth (prevents stack overflow).
    pub const MAX_CALL_DEPTH: u16 = 1024;

    /// Maximum code size in bytes (EIP-170).
    pub const MAX_CODE_SIZE: usize = 24_576; // 24 KB

    /// Maximum init code size in bytes (EIP-3860).
    pub const MAX_INIT_CODE_SIZE: usize = 49_152; // 48 KB

    /// Maximum stack size (EVM specification).
    pub const MAX_STACK_SIZE: usize = 1024;

    /// Maximum memory size in bytes.
    pub const MAX_MEMORY_SIZE: usize = 16 * 1024 * 1024; // 16 MB

    /// Execution timeout in seconds.
    pub const EXECUTION_TIMEOUT_SECS: u64 = 5;

    /// Block gas limit.
    pub const BLOCK_GAS_LIMIT: u64 = 30_000_000;

    /// Gas refund cap (50% of gas used per EIP-3529).
    pub const GAS_REFUND_CAP_PERCENT: u64 = 50;
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::*;
    use crate::domain::value_objects::*;

    fn create_test_context() -> ExecutionContext {
        ExecutionContext {
            origin: Address::new([1u8; 20]),
            caller: Address::new([1u8; 20]),
            address: Address::new([2u8; 20]),
            value: U256::zero(),
            data: Bytes::new(),
            gas_limit: 1000,
            gas_price: U256::from(1),
            block: BlockContext::default(),
            depth: 0,
            is_static: false,
        }
    }

    #[test]
    fn test_gas_limit_invariant_valid() {
        let ctx = create_test_context();
        let result = ExecutionResult::success(Bytes::new(), 500);
        assert!(check_gas_limit_invariant(&result, &ctx));
    }

    #[test]
    fn test_gas_limit_invariant_exceeded() {
        let ctx = create_test_context();
        let result = ExecutionResult::success(Bytes::new(), 1500);
        assert!(!check_gas_limit_invariant(&result, &ctx));
    }

    #[test]
    fn test_revert_rollback_invariant() {
        // Success with state changes: OK
        let mut result = ExecutionResult::success(Bytes::new(), 100);
        result.state_changes.push(StateChange::NonceIncrement {
            address: Address::ZERO,
        });
        assert!(check_revert_rollback_invariant(&result));

        // Failure with state changes: VIOLATION
        let mut result = ExecutionResult::failure("error", 100);
        result.state_changes.push(StateChange::NonceIncrement {
            address: Address::ZERO,
        });
        assert!(!check_revert_rollback_invariant(&result));
    }

    #[test]
    fn test_static_purity_invariant() {
        let mut ctx = create_test_context();
        ctx.is_static = true;

        // Static call with no changes: OK
        let result = ExecutionResult::success(Bytes::new(), 100);
        assert!(check_static_purity_invariant(&ctx, &result));

        // Static call with state change: VIOLATION
        let mut result = ExecutionResult::success(Bytes::new(), 100);
        result.state_changes.push(StateChange::StorageWrite {
            address: Address::ZERO,
            key: StorageKey::ZERO,
            value: StorageValue::from_u256(U256::from(1)),
        });
        assert!(!check_static_purity_invariant(&ctx, &result));
    }

    #[test]
    fn test_call_depth_invariant() {
        let config = VmConfig::default();
        let mut ctx = create_test_context();

        ctx.depth = 100;
        assert!(check_call_depth_invariant(&ctx, &config));

        ctx.depth = 1024;
        assert!(check_call_depth_invariant(&ctx, &config));

        ctx.depth = 1025;
        assert!(!check_call_depth_invariant(&ctx, &config));
    }

    #[test]
    fn test_check_all_invariants_valid() {
        let ctx = create_test_context();
        let result = ExecutionResult::success(Bytes::new(), 500);
        let config = VmConfig::default();

        let check = check_all_invariants(&ctx, &result, &config);
        assert!(check.is_valid());
    }

    #[test]
    fn test_check_all_invariants_multiple_violations() {
        let mut ctx = create_test_context();
        ctx.depth = 2000;
        ctx.is_static = true;

        let mut result = ExecutionResult::failure("error", 2000);
        result.state_changes.push(StateChange::NonceIncrement {
            address: Address::ZERO,
        });

        let config = VmConfig::default();
        let check = check_all_invariants(&ctx, &result, &config);

        match check {
            InvariantCheckResult::Invalid(violations) => {
                assert!(violations.len() >= 2);
            }
            InvariantCheckResult::Valid => panic!("Expected violations"),
        }
    }
}
