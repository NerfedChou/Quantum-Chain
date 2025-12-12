//! # EVM Gas Metering
//!
//! Gas costs for EVM opcodes per Berlin/London/Shanghai hard forks.
//! Implements EIP-2929 (access lists) and EIP-3529 (refund cap).

use crate::domain::value_objects::U256;

// =============================================================================
// BASE GAS COSTS
// =============================================================================

/// Gas costs for common operations.
pub mod costs {
    /// Zero gas.
    pub const ZERO: u64 = 0;
    /// Base cost (e.g., for `ADD`).
    pub const BASE: u64 = 2;
    /// Very low cost (e.g., for `MUL`).
    pub const VERY_LOW: u64 = 3;
    /// Low cost.
    pub const LOW: u64 = 5;
    /// Mid cost.
    pub const MID: u64 = 8;
    /// High cost.
    pub const HIGH: u64 = 10;
    /// Jump destination cost.
    pub const JUMPDEST: u64 = 1;

    // Transaction costs
    /// Base transaction gas.
    pub const TX_BASE: u64 = 21_000;
    /// Contract creation base gas.
    pub const TX_CREATE: u64 = 53_000;
    /// Gas per non-zero byte of calldata.
    pub const TX_DATA_NON_ZERO: u64 = 16;
    /// Gas per zero byte of calldata.
    pub const TX_DATA_ZERO: u64 = 4;

    // Memory costs
    /// Gas per word for memory copy.
    pub const COPY: u64 = 3;

    // Storage costs (EIP-2929)
    /// Cold storage read (first access).
    pub const COLD_SLOAD: u64 = 2100;
    /// Warm storage read (subsequent access).
    pub const WARM_SLOAD: u64 = 100;
    /// Cold account access.
    pub const COLD_ACCOUNT_ACCESS: u64 = 2600;
    /// Warm account access.
    pub const WARM_ACCOUNT_ACCESS: u64 = 100;

    // SSTORE costs (EIP-2200, EIP-3529)
    /// SSTORE when setting non-zero to zero (gives refund).
    pub const SSTORE_RESET: u64 = 2900;
    /// SSTORE when setting zero to non-zero.
    pub const SSTORE_SET: u64 = 20_000;
    /// SSTORE refund for clearing storage.
    pub const SSTORE_CLEAR_REFUND: u64 = 4800;

    // Call costs
    /// Base call cost.
    pub const CALL: u64 = 0; // Actual cost depends on context
    /// Cost for value transfer.
    pub const CALL_VALUE: u64 = 9000;
    /// Cost for creating new account.
    pub const CALL_NEW_ACCOUNT: u64 = 25_000;
    /// Stipend given to called contract when value > 0.
    pub const CALL_STIPEND: u64 = 2300;

    // Create costs
    /// CREATE opcode base cost.
    pub const CREATE: u64 = 32_000;
    /// CREATE2 hash cost per word.
    pub const KECCAK256_WORD: u64 = 6;

    // Log costs
    /// LOG base cost.
    pub const LOG: u64 = 375;
    /// LOG cost per topic.
    pub const LOG_TOPIC: u64 = 375;
    /// LOG cost per byte of data.
    pub const LOG_DATA: u64 = 8;

    // Other
    /// KECCAK256 (SHA3) base cost.
    pub const KECCAK256: u64 = 30;
    /// EXP base cost.
    pub const EXP: u64 = 10;
    /// EXP cost per byte of exponent.
    pub const EXP_BYTE: u64 = 50;
    /// SELFDESTRUCT base cost.
    pub const SELFDESTRUCT: u64 = 5000;
    /// SELFDESTRUCT to new account.
    pub const SELFDESTRUCT_NEW_ACCOUNT: u64 = 25_000;
    /// BALANCE opcode (cold).
    pub const BALANCE_COLD: u64 = 2600;
    /// BALANCE opcode (warm).
    pub const BALANCE_WARM: u64 = 100;
    /// EXTCODECOPY base cost.
    pub const EXTCODECOPY: u64 = 0; // Plus cold/warm access
    /// EXTCODESIZE base cost.
    pub const EXTCODESIZE: u64 = 0; // Plus cold/warm access
    /// EXTCODEHASH base cost.
    pub const EXTCODEHASH: u64 = 0; // Plus cold/warm access
    /// BLOCKHASH cost.
    pub const BLOCKHASH: u64 = 20;
}

// =============================================================================
// GAS CALCULATOR
// =============================================================================

/// Calculate gas cost for EXP opcode.
#[must_use]
pub fn exp_gas_cost(exponent: U256) -> u64 {
    if exponent.is_zero() {
        return costs::EXP;
    }

    // Count bytes in exponent
    let byte_size = (256 - u64::from(exponent.leading_zeros())).div_ceil(8);
    costs::EXP + costs::EXP_BYTE * byte_size
}

/// Calculate gas cost for KECCAK256.
#[must_use]
pub fn keccak256_gas_cost(data_size: usize) -> u64 {
    let word_size = data_size.div_ceil(32);
    costs::KECCAK256 + costs::KECCAK256_WORD * word_size as u64
}

/// Calculate gas cost for LOG opcode.
#[must_use]
pub fn log_gas_cost(data_size: usize, topic_count: usize) -> u64 {
    costs::LOG + costs::LOG_TOPIC * topic_count as u64 + costs::LOG_DATA * data_size as u64
}

/// Calculate gas cost for COPY operations (CALLDATACOPY, CODECOPY, etc.).
#[must_use]
pub fn copy_gas_cost(size: usize) -> u64 {
    let word_size = size.div_ceil(32);
    costs::COPY * word_size as u64
}

/// Calculate gas cost for CREATE/CREATE2.
#[must_use]
pub fn create_gas_cost(init_code_size: usize) -> u64 {
    let word_size = init_code_size.div_ceil(32);
    costs::CREATE + costs::KECCAK256_WORD * word_size as u64
}

/// Calculate gas for CALL-like opcodes.
#[derive(Clone, Debug)]
pub struct CallGasParams {
    /// Is this a cold account access?
    pub is_cold: bool,
    /// Is value being transferred?
    pub has_value: bool,
    /// Is the target account empty (doesn't exist)?
    pub is_empty: bool,
}

/// Calculate gas cost for CALL.
#[must_use]
pub fn call_gas_cost(params: &CallGasParams) -> u64 {
    let mut gas = if params.is_cold {
        costs::COLD_ACCOUNT_ACCESS
    } else {
        costs::WARM_ACCOUNT_ACCESS
    };

    if params.has_value {
        gas += costs::CALL_VALUE;
        if params.is_empty {
            gas += costs::CALL_NEW_ACCOUNT;
        }
    }

    gas
}

/// Calculate gas to pass to a subcall (63/64 rule per EIP-150).
#[must_use]
pub fn calculate_call_gas(available_gas: u64, requested_gas: u64, has_value: bool) -> u64 {
    // Max gas to pass is 63/64 of available
    let max_gas = available_gas - (available_gas / 64);
    let mut gas = requested_gas.min(max_gas);

    // Add stipend if value transfer
    if has_value {
        gas = gas.saturating_add(costs::CALL_STIPEND);
    }

    gas
}

// =============================================================================
// GAS REFUND
// =============================================================================

/// Maximum refund as percentage of gas used (50% per EIP-3529).
pub const MAX_REFUND_PERCENT: u64 = 50;

/// Calculate effective refund (capped at 50% of gas used).
#[must_use]
pub fn calculate_refund(gas_used: u64, refund: u64) -> u64 {
    let max_refund = gas_used / 2;
    refund.min(max_refund)
}

// =============================================================================
// OPCODE GAS COSTS TABLE
// =============================================================================

/// Static gas costs for opcodes (excludes dynamic costs).
#[rustfmt::skip]
pub const OPCODE_GAS: [u64; 256] = {
    let mut table = [0u64; 256];
    
    // Stop and arithmetic
    table[0x00] = 0;                    // STOP
    table[0x01] = costs::VERY_LOW;      // ADD
    table[0x02] = costs::LOW;           // MUL
    table[0x03] = costs::VERY_LOW;      // SUB
    table[0x04] = costs::LOW;           // DIV
    table[0x05] = costs::LOW;           // SDIV
    table[0x06] = costs::LOW;           // MOD
    table[0x07] = costs::LOW;           // SMOD
    table[0x08] = costs::MID;           // ADDMOD
    table[0x09] = costs::MID;           // MULMOD
    table[0x0A] = costs::EXP;           // EXP (base, dynamic added)
    table[0x0B] = costs::LOW;           // SIGNEXTEND
    
    // Comparison
    table[0x10] = costs::VERY_LOW;      // LT
    table[0x11] = costs::VERY_LOW;      // GT
    table[0x12] = costs::VERY_LOW;      // SLT
    table[0x13] = costs::VERY_LOW;      // SGT
    table[0x14] = costs::VERY_LOW;      // EQ
    table[0x15] = costs::VERY_LOW;      // ISZERO
    table[0x16] = costs::VERY_LOW;      // AND
    table[0x17] = costs::VERY_LOW;      // OR
    table[0x18] = costs::VERY_LOW;      // XOR
    table[0x19] = costs::VERY_LOW;      // NOT
    table[0x1A] = costs::VERY_LOW;      // BYTE
    table[0x1B] = costs::VERY_LOW;      // SHL
    table[0x1C] = costs::VERY_LOW;      // SHR
    table[0x1D] = costs::VERY_LOW;      // SAR
    
    // Keccak256
    table[0x20] = costs::KECCAK256;     // KECCAK256 (base, dynamic added)
    
    // Environment
    table[0x30] = costs::BASE;          // ADDRESS
    table[0x31] = 0;                    // BALANCE (dynamic: cold/warm)
    table[0x32] = costs::BASE;          // ORIGIN
    table[0x33] = costs::BASE;          // CALLER
    table[0x34] = costs::BASE;          // CALLVALUE
    table[0x35] = costs::VERY_LOW;      // CALLDATALOAD
    table[0x36] = costs::BASE;          // CALLDATASIZE
    table[0x37] = costs::VERY_LOW;      // CALLDATACOPY (base, dynamic added)
    table[0x38] = costs::BASE;          // CODESIZE
    table[0x39] = costs::VERY_LOW;      // CODECOPY (base, dynamic added)
    table[0x3A] = costs::BASE;          // GASPRICE
    table[0x3B] = 0;                    // EXTCODESIZE (dynamic: cold/warm)
    table[0x3C] = 0;                    // EXTCODECOPY (dynamic: cold/warm + copy)
    table[0x3D] = costs::BASE;          // RETURNDATASIZE
    table[0x3E] = costs::VERY_LOW;      // RETURNDATACOPY (base, dynamic added)
    table[0x3F] = 0;                    // EXTCODEHASH (dynamic: cold/warm)
    
    // Block info
    table[0x40] = costs::BLOCKHASH;     // BLOCKHASH
    table[0x41] = costs::BASE;          // COINBASE
    table[0x42] = costs::BASE;          // TIMESTAMP
    table[0x43] = costs::BASE;          // NUMBER
    table[0x44] = costs::BASE;          // PREVRANDAO (was DIFFICULTY)
    table[0x45] = costs::BASE;          // GASLIMIT
    table[0x46] = costs::BASE;          // CHAINID
    table[0x47] = costs::LOW;           // SELFBALANCE
    table[0x48] = costs::BASE;          // BASEFEE
    
    // Stack operations
    table[0x50] = costs::BASE;          // POP
    table[0x51] = costs::VERY_LOW;      // MLOAD
    table[0x52] = costs::VERY_LOW;      // MSTORE
    table[0x53] = costs::VERY_LOW;      // MSTORE8
    table[0x54] = 0;                    // SLOAD (dynamic: cold/warm)
    table[0x55] = 0;                    // SSTORE (dynamic)
    table[0x56] = costs::MID;           // JUMP
    table[0x57] = costs::HIGH;          // JUMPI
    table[0x58] = costs::BASE;          // PC
    table[0x59] = costs::BASE;          // MSIZE
    table[0x5A] = costs::BASE;          // GAS
    table[0x5B] = costs::JUMPDEST;      // JUMPDEST
    
    // Transient storage (EIP-1153)
    table[0x5C] = costs::WARM_SLOAD;    // TLOAD
    table[0x5D] = costs::WARM_SLOAD;    // TSTORE
    
    // Memory copy (EIP-5656)
    table[0x5E] = costs::VERY_LOW;      // MCOPY (base, dynamic added)
    
    // Push operations (0x5F-0x7F)
    table[0x5F] = costs::BASE;          // PUSH0
    // PUSH1-PUSH32 (0x60-0x7F)
    let mut i = 0x60;
    while i <= 0x7F {
        table[i] = costs::VERY_LOW;
        i += 1;
    }
    
    // DUP operations (0x80-0x8F)
    i = 0x80;
    while i <= 0x8F {
        table[i] = costs::VERY_LOW;
        i += 1;
    }
    
    // SWAP operations (0x90-0x9F)
    i = 0x90;
    while i <= 0x9F {
        table[i] = costs::VERY_LOW;
        i += 1;
    }
    
    // LOG operations (0xA0-0xA4)
    table[0xA0] = costs::LOG;           // LOG0
    table[0xA1] = costs::LOG;           // LOG1
    table[0xA2] = costs::LOG;           // LOG2
    table[0xA3] = costs::LOG;           // LOG3
    table[0xA4] = costs::LOG;           // LOG4
    
    // System operations
    table[0xF0] = costs::CREATE;        // CREATE
    table[0xF1] = 0;                    // CALL (dynamic)
    table[0xF2] = 0;                    // CALLCODE (dynamic)
    table[0xF3] = 0;                    // RETURN
    table[0xF4] = 0;                    // DELEGATECALL (dynamic)
    table[0xF5] = costs::CREATE;        // CREATE2 (base, dynamic added)
    table[0xFA] = 0;                    // STATICCALL (dynamic)
    table[0xFD] = 0;                    // REVERT
    table[0xFE] = 0;                    // INVALID (consumes all gas)
    table[0xFF] = costs::SELFDESTRUCT;  // SELFDESTRUCT (base, dynamic added)
    
    table
};

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exp_gas_cost() {
        assert_eq!(exp_gas_cost(U256::zero()), costs::EXP);
        assert_eq!(exp_gas_cost(U256::from(1)), costs::EXP + costs::EXP_BYTE);
        assert_eq!(exp_gas_cost(U256::from(255)), costs::EXP + costs::EXP_BYTE);
        assert_eq!(
            exp_gas_cost(U256::from(256)),
            costs::EXP + costs::EXP_BYTE * 2
        );
    }

    #[test]
    fn test_keccak256_gas_cost() {
        assert_eq!(keccak256_gas_cost(0), costs::KECCAK256);
        assert_eq!(
            keccak256_gas_cost(32),
            costs::KECCAK256 + costs::KECCAK256_WORD
        );
        assert_eq!(
            keccak256_gas_cost(64),
            costs::KECCAK256 + costs::KECCAK256_WORD * 2
        );
    }

    #[test]
    fn test_log_gas_cost() {
        // LOG0 with 32 bytes data
        let cost = log_gas_cost(32, 0);
        assert_eq!(cost, costs::LOG + costs::LOG_DATA * 32);

        // LOG2 with 64 bytes data
        let cost = log_gas_cost(64, 2);
        assert_eq!(
            cost,
            costs::LOG + costs::LOG_TOPIC * 2 + costs::LOG_DATA * 64
        );
    }

    #[test]
    fn test_copy_gas_cost() {
        assert_eq!(copy_gas_cost(0), 0);
        assert_eq!(copy_gas_cost(32), costs::COPY);
        assert_eq!(copy_gas_cost(64), costs::COPY * 2);
        assert_eq!(copy_gas_cost(33), costs::COPY * 2); // Rounded up
    }

    #[test]
    fn test_call_gas_cost() {
        let params = CallGasParams {
            is_cold: true,
            has_value: false,
            is_empty: false,
        };
        assert_eq!(call_gas_cost(&params), costs::COLD_ACCOUNT_ACCESS);

        let params = CallGasParams {
            is_cold: false,
            has_value: true,
            is_empty: false,
        };
        assert_eq!(
            call_gas_cost(&params),
            costs::WARM_ACCOUNT_ACCESS + costs::CALL_VALUE
        );

        let params = CallGasParams {
            is_cold: true,
            has_value: true,
            is_empty: true,
        };
        assert_eq!(
            call_gas_cost(&params),
            costs::COLD_ACCOUNT_ACCESS + costs::CALL_VALUE + costs::CALL_NEW_ACCOUNT
        );
    }

    #[test]
    fn test_calculate_call_gas() {
        // Without value, 63/64 rule
        let gas = calculate_call_gas(64000, 50000, false);
        assert!(gas <= 63000); // 63/64 of 64000

        // With value, add stipend
        let gas = calculate_call_gas(64000, 50000, true);
        assert!(gas > 50000); // Should include stipend
    }

    #[test]
    fn test_calculate_refund() {
        // Refund capped at 50%
        assert_eq!(calculate_refund(1000, 600), 500);
        assert_eq!(calculate_refund(1000, 400), 400);
        assert_eq!(calculate_refund(1000, 500), 500);
    }

    #[test]
    fn test_opcode_gas_table() {
        assert_eq!(OPCODE_GAS[0x01], costs::VERY_LOW); // ADD
        assert_eq!(OPCODE_GAS[0x02], costs::LOW); // MUL
        assert_eq!(OPCODE_GAS[0x60], costs::VERY_LOW); // PUSH1
        assert_eq!(OPCODE_GAS[0x80], costs::VERY_LOW); // DUP1
    }
}
