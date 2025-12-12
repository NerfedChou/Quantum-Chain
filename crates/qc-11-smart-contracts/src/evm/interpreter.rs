//! # EVM Interpreter
//!
//! The main execution engine for EVM bytecode.
//! Implements all opcodes and execution flow.

use crate::domain::entities::{ExecutionContext, ExecutionResult, Log, StateChange};
use crate::domain::services::keccak256;
use crate::domain::value_objects::{Address, Bytes, Hash, StorageKey, StorageValue, U256};
use crate::errors::VmError;
use crate::evm::gas::{self, costs, OPCODE_GAS};
use crate::evm::memory::{memory_expansion_cost, Memory};
use crate::evm::opcodes::Opcode;
use crate::evm::stack::Stack;
use crate::ports::outbound::{AccessList, AccessStatus, StateAccess};
use std::collections::HashSet;

/// Maximum execution steps to prevent infinite loops (safety limit).
const MAX_EXECUTION_STEPS: u64 = 10_000_000;

/// EVM Interpreter state.
pub struct Interpreter<'a, S, A>
where
    S: StateAccess,
    A: AccessList,
{
    /// Execution context.
    pub context: ExecutionContext,
    /// Contract bytecode.
    pub code: &'a [u8],
    /// Program counter.
    pub pc: usize,
    /// EVM stack.
    pub stack: Stack,
    /// EVM memory.
    pub memory: Memory,
    /// Return data from last call.
    pub return_data: Bytes,
    /// State changes accumulated.
    pub state_changes: Vec<StateChange>,
    /// Logs emitted.
    pub logs: Vec<Log>,
    /// Gas remaining.
    pub gas_remaining: u64,
    /// Gas refund accumulated.
    pub gas_refund: u64,
    /// State access interface.
    pub state: &'a S,
    /// Access list for warm/cold tracking.
    pub access_list: &'a mut A,
    /// Valid jump destinations (cached).
    pub jump_dests: HashSet<usize>,
    /// Execution stopped flag.
    pub stopped: bool,
    /// Execution reverted flag.
    pub reverted: bool,
}

impl<'a, S, A> Interpreter<'a, S, A>
where
    S: StateAccess,
    A: AccessList,
{
    /// Create a new interpreter.
    pub fn new(
        context: ExecutionContext,
        code: &'a [u8],
        state: &'a S,
        access_list: &'a mut A,
    ) -> Self {
        let gas_remaining = context.gas_limit;
        let jump_dests = analyze_jump_dests(code);

        Self {
            context,
            code,
            pc: 0,
            stack: Stack::new(),
            memory: Memory::new(),
            return_data: Bytes::new(),
            state_changes: Vec::new(),
            logs: Vec::new(),
            gas_remaining,
            gas_refund: 0,
            state,
            access_list,
            jump_dests,
            stopped: false,
            reverted: false,
        }
    }

    /// Execute the bytecode and return the result.
    pub async fn execute(&mut self) -> Result<ExecutionResult, VmError> {
        let mut steps = 0u64;

        while !self.stopped && self.pc < self.code.len() {
            steps += 1;
            if steps > MAX_EXECUTION_STEPS {
                return Err(VmError::Timeout {
                    elapsed_ms: 0,
                    max_ms: 5000,
                });
            }

            let opcode_byte = self.code[self.pc];
            let opcode = Opcode::from_byte(opcode_byte);

            // Check for invalid opcode
            let opcode = match opcode {
                Some(op) => op,
                None => return Err(VmError::InvalidOpcode(opcode_byte)),
            };

            // Consume base gas
            let base_gas = OPCODE_GAS[opcode_byte as usize];
            if !self.consume_gas(base_gas) {
                return Err(VmError::OutOfGas);
            }

            // Execute the opcode
            self.execute_opcode(opcode).await?;
        }

        // Build result
        let gas_used = self.context.gas_limit - self.gas_remaining;

        if self.reverted {
            Ok(ExecutionResult {
                success: false,
                output: self.return_data.clone(),
                gas_used,
                gas_refund: 0,
                state_changes: Vec::new(), // Rolled back
                logs: Vec::new(),          // Rolled back
                revert_reason: None,
            })
        } else {
            Ok(ExecutionResult {
                success: true,
                output: self.return_data.clone(),
                gas_used,
                gas_refund: self.gas_refund,
                state_changes: std::mem::take(&mut self.state_changes),
                logs: std::mem::take(&mut self.logs),
                revert_reason: None,
            })
        }
    }

    /// Consume gas, returning false if insufficient.
    fn consume_gas(&mut self, amount: u64) -> bool {
        if amount > self.gas_remaining {
            self.gas_remaining = 0;
            false
        } else {
            self.gas_remaining -= amount;
            true
        }
    }

    /// Execute a single opcode.
    async fn execute_opcode(&mut self, opcode: Opcode) -> Result<(), VmError> {
        self.pc += 1;

        match opcode {
            // =================================================================
            // STOP & ARITHMETIC
            // =================================================================
            Opcode::Stop => {
                self.stopped = true;
            }

            Opcode::Add => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                self.stack.push(a.overflowing_add(b).0)?;
            }

            Opcode::Mul => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                self.stack.push(a.overflowing_mul(b).0)?;
            }

            Opcode::Sub => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                self.stack.push(a.overflowing_sub(b).0)?;
            }

            Opcode::Div => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                let result = if b.is_zero() { U256::zero() } else { a / b };
                self.stack.push(result)?;
            }

            Opcode::SDiv => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                let result = if b.is_zero() {
                    U256::zero()
                } else {
                    signed_div(a, b)
                };
                self.stack.push(result)?;
            }

            Opcode::Mod => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                let result = if b.is_zero() { U256::zero() } else { a % b };
                self.stack.push(result)?;
            }

            Opcode::SMod => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                let result = if b.is_zero() {
                    U256::zero()
                } else {
                    signed_mod(a, b)
                };
                self.stack.push(result)?;
            }

            Opcode::AddMod => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                let n = self.stack.pop()?;
                let result = if n.is_zero() {
                    U256::zero()
                } else {
                    // Use 512-bit arithmetic to prevent overflow
                    let sum = u256_to_u512(a) + u256_to_u512(b);
                    let result = sum % u256_to_u512(n);
                    u512_to_u256(result)
                };
                self.stack.push(result)?;
            }

            Opcode::MulMod => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                let n = self.stack.pop()?;
                let result = if n.is_zero() {
                    U256::zero()
                } else {
                    // Use 512-bit arithmetic
                    let prod = u256_to_u512(a) * u256_to_u512(b);
                    let result = prod % u256_to_u512(n);
                    u512_to_u256(result)
                };
                self.stack.push(result)?;
            }

            Opcode::Exp => {
                let base = self.stack.pop()?;
                let exp = self.stack.pop()?;

                // Dynamic gas cost
                let exp_gas = gas::exp_gas_cost(exp) - costs::EXP;
                if !self.consume_gas(exp_gas) {
                    return Err(VmError::OutOfGas);
                }

                let result = exp_by_squaring(base, exp);
                self.stack.push(result)?;
            }

            Opcode::SignExtend => {
                let k = self.stack.pop()?;
                let x = self.stack.pop()?;

                let result = if k < U256::from(32) {
                    let k = k.as_usize();
                    let bit_index = 8 * k + 7;
                    let bit = x.bit(bit_index);
                    let mask = (U256::one() << (bit_index + 1)) - 1;
                    if bit {
                        x | !mask
                    } else {
                        x & mask
                    }
                } else {
                    x
                };
                self.stack.push(result)?;
            }

            // =================================================================
            // COMPARISON & BITWISE
            // =================================================================
            Opcode::Lt => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                self.stack
                    .push(if a < b { U256::one() } else { U256::zero() })?;
            }

            Opcode::Gt => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                self.stack
                    .push(if a > b { U256::one() } else { U256::zero() })?;
            }

            Opcode::SLt => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                let result = if signed_lt(a, b) {
                    U256::one()
                } else {
                    U256::zero()
                };
                self.stack.push(result)?;
            }

            Opcode::SGt => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                let result = if signed_lt(b, a) {
                    U256::one()
                } else {
                    U256::zero()
                };
                self.stack.push(result)?;
            }

            Opcode::Eq => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                self.stack
                    .push(if a == b { U256::one() } else { U256::zero() })?;
            }

            Opcode::IsZero => {
                let a = self.stack.pop()?;
                self.stack.push(if a.is_zero() {
                    U256::one()
                } else {
                    U256::zero()
                })?;
            }

            Opcode::And => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                self.stack.push(a & b)?;
            }

            Opcode::Or => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                self.stack.push(a | b)?;
            }

            Opcode::Xor => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                self.stack.push(a ^ b)?;
            }

            Opcode::Not => {
                let a = self.stack.pop()?;
                self.stack.push(!a)?;
            }

            Opcode::Byte => {
                let i = self.stack.pop()?;
                let x = self.stack.pop()?;
                let result = if i < U256::from(32) {
                    let byte_index = 31 - i.as_usize();
                    let mut bytes = [0u8; 32];
                    x.to_big_endian(&mut bytes);
                    U256::from(bytes[31 - byte_index])
                } else {
                    U256::zero()
                };
                self.stack.push(result)?;
            }

            Opcode::Shl => {
                let shift = self.stack.pop()?;
                let value = self.stack.pop()?;
                let result = if shift >= U256::from(256) {
                    U256::zero()
                } else {
                    value << shift.as_usize()
                };
                self.stack.push(result)?;
            }

            Opcode::Shr => {
                let shift = self.stack.pop()?;
                let value = self.stack.pop()?;
                let result = if shift >= U256::from(256) {
                    U256::zero()
                } else {
                    value >> shift.as_usize()
                };
                self.stack.push(result)?;
            }

            Opcode::Sar => {
                let shift = self.stack.pop()?;
                let value = self.stack.pop()?;
                let result = sar(value, shift);
                self.stack.push(result)?;
            }

            // =================================================================
            // KECCAK256
            // =================================================================
            Opcode::Keccak256 => {
                let offset = self.stack.pop()?.as_usize();
                let size = self.stack.pop()?.as_usize();

                // Memory expansion gas
                let words_added = self.memory.expand(offset + size)?;
                let mem_gas = memory_expansion_cost(
                    self.memory.word_size() - words_added,
                    self.memory.word_size(),
                );
                if !self.consume_gas(mem_gas) {
                    return Err(VmError::OutOfGas);
                }

                // Hash cost
                let hash_gas = gas::keccak256_gas_cost(size) - costs::KECCAK256;
                if !self.consume_gas(hash_gas) {
                    return Err(VmError::OutOfGas);
                }

                let data = self.memory.read_bytes(offset, size);
                let hash = keccak256(&data);
                self.stack.push(U256::from_big_endian(hash.as_bytes()))?;
            }

            // =================================================================
            // ENVIRONMENTAL INFORMATION
            // =================================================================
            Opcode::Address => {
                let mut bytes = [0u8; 32];
                bytes[12..].copy_from_slice(self.context.address.as_bytes());
                self.stack.push(U256::from_big_endian(&bytes))?;
            }

            Opcode::Balance => {
                let addr_val = self.stack.pop()?;
                let addr = u256_to_address(addr_val);

                // Check warm/cold
                let is_cold = self.access_list.touch_account(addr) == AccessStatus::Cold;
                let gas = if is_cold {
                    costs::COLD_ACCOUNT_ACCESS
                } else {
                    costs::WARM_ACCOUNT_ACCESS
                };
                if !self.consume_gas(gas) {
                    return Err(VmError::OutOfGas);
                }

                let balance = self.state.get_balance(addr).await?;
                self.stack.push(balance)?;
            }

            Opcode::Origin => {
                let mut bytes = [0u8; 32];
                bytes[12..].copy_from_slice(self.context.origin.as_bytes());
                self.stack.push(U256::from_big_endian(&bytes))?;
            }

            Opcode::Caller => {
                let mut bytes = [0u8; 32];
                bytes[12..].copy_from_slice(self.context.caller.as_bytes());
                self.stack.push(U256::from_big_endian(&bytes))?;
            }

            Opcode::CallValue => {
                self.stack.push(self.context.value)?;
            }

            Opcode::CallDataLoad => {
                let offset = self.stack.pop()?.as_usize();
                let data = &self.context.data;
                let mut result = [0u8; 32];

                for (i, byte) in result.iter_mut().enumerate() {
                    let pos = offset.saturating_add(i);
                    if pos < data.len() {
                        *byte = data.as_slice()[pos];
                    }
                }

                self.stack.push(U256::from_big_endian(&result))?;
            }

            Opcode::CallDataSize => {
                self.stack.push(U256::from(self.context.data.len()))?;
            }

            Opcode::CallDataCopy => {
                let dest_offset = self.stack.pop()?.as_usize();
                let data_offset = self.stack.pop()?.as_usize();
                let size = self.stack.pop()?.as_usize();

                // Memory expansion
                let words_added = self.memory.expand(dest_offset + size)?;
                let mem_gas = memory_expansion_cost(
                    self.memory.word_size() - words_added,
                    self.memory.word_size(),
                );
                if !self.consume_gas(mem_gas) {
                    return Err(VmError::OutOfGas);
                }

                // Copy gas
                let copy_gas = gas::copy_gas_cost(size);
                if !self.consume_gas(copy_gas) {
                    return Err(VmError::OutOfGas);
                }

                // Copy data with zero padding
                let data = &self.context.data;
                for i in 0..size {
                    let byte = if data_offset + i < data.len() {
                        data.as_slice()[data_offset + i]
                    } else {
                        0
                    };
                    self.memory.write_byte(dest_offset + i, byte)?;
                }
            }

            Opcode::CodeSize => {
                self.stack.push(U256::from(self.code.len()))?;
            }

            Opcode::CodeCopy => {
                let dest_offset = self.stack.pop()?.as_usize();
                let code_offset = self.stack.pop()?.as_usize();
                let size = self.stack.pop()?.as_usize();

                // Memory expansion
                let words_added = self.memory.expand(dest_offset + size)?;
                let mem_gas = memory_expansion_cost(
                    self.memory.word_size() - words_added,
                    self.memory.word_size(),
                );
                if !self.consume_gas(mem_gas) {
                    return Err(VmError::OutOfGas);
                }

                // Copy gas
                let copy_gas = gas::copy_gas_cost(size);
                if !self.consume_gas(copy_gas) {
                    return Err(VmError::OutOfGas);
                }

                // Copy code with zero padding
                for i in 0..size {
                    let byte = if code_offset + i < self.code.len() {
                        self.code[code_offset + i]
                    } else {
                        0
                    };
                    self.memory.write_byte(dest_offset + i, byte)?;
                }
            }

            Opcode::GasPrice => {
                self.stack.push(self.context.gas_price)?;
            }

            Opcode::ReturnDataSize => {
                self.stack.push(U256::from(self.return_data.len()))?;
            }

            Opcode::ReturnDataCopy => {
                let dest_offset = self.stack.pop()?.as_usize();
                let data_offset = self.stack.pop()?.as_usize();
                let size = self.stack.pop()?.as_usize();

                // Check bounds
                if data_offset.saturating_add(size) > self.return_data.len() {
                    return Err(VmError::ReturnDataOutOfBounds {
                        offset: data_offset,
                        size,
                        available: self.return_data.len(),
                    });
                }

                // Memory expansion
                let words_added = self.memory.expand(dest_offset + size)?;
                let mem_gas = memory_expansion_cost(
                    self.memory.word_size() - words_added,
                    self.memory.word_size(),
                );
                if !self.consume_gas(mem_gas) {
                    return Err(VmError::OutOfGas);
                }

                // Copy gas
                let copy_gas = gas::copy_gas_cost(size);
                if !self.consume_gas(copy_gas) {
                    return Err(VmError::OutOfGas);
                }

                // Copy return data
                let data = &self.return_data.as_slice()[data_offset..data_offset + size];
                self.memory.write_bytes(dest_offset, data)?;
            }

            // =================================================================
            // BLOCK INFORMATION
            // =================================================================
            Opcode::BlockHash => {
                let number = self.stack.pop()?;
                // Only last 256 blocks available
                let current = self.context.block.number;
                let result = if number >= U256::from(current)
                    || number < U256::from(current.saturating_sub(256))
                {
                    U256::zero()
                } else {
                    // Would need block hash oracle
                    U256::zero() // Simplified
                };
                self.stack.push(result)?;
            }

            Opcode::Coinbase => {
                let mut bytes = [0u8; 32];
                bytes[12..].copy_from_slice(self.context.block.coinbase.as_bytes());
                self.stack.push(U256::from_big_endian(&bytes))?;
            }

            Opcode::Timestamp => {
                self.stack.push(U256::from(self.context.block.timestamp))?;
            }

            Opcode::Number => {
                self.stack.push(U256::from(self.context.block.number))?;
            }

            Opcode::PrevRandao => {
                self.stack.push(self.context.block.difficulty)?;
            }

            Opcode::GasLimit => {
                self.stack.push(U256::from(self.context.block.gas_limit))?;
            }

            Opcode::ChainId => {
                self.stack.push(U256::from(self.context.block.chain_id))?;
            }

            Opcode::SelfBalance => {
                let balance = self.state.get_balance(self.context.address).await?;
                self.stack.push(balance)?;
            }

            Opcode::BaseFee => {
                self.stack.push(self.context.block.base_fee)?;
            }

            // =================================================================
            // STACK, MEMORY, STORAGE
            // =================================================================
            Opcode::Pop => {
                self.stack.pop()?;
            }

            Opcode::MLoad => {
                let offset = self.stack.pop()?.as_usize();

                // Memory expansion
                let words_added = self.memory.expand(offset + 32)?;
                let mem_gas = memory_expansion_cost(
                    self.memory.word_size() - words_added,
                    self.memory.word_size(),
                );
                if !self.consume_gas(mem_gas) {
                    return Err(VmError::OutOfGas);
                }

                let value = self.memory.read_word(offset);
                self.stack.push(U256::from_big_endian(&value))?;
            }

            Opcode::MStore => {
                let offset = self.stack.pop()?.as_usize();
                let value = self.stack.pop()?;

                // Memory expansion
                let words_added = self.memory.expand(offset + 32)?;
                let mem_gas = memory_expansion_cost(
                    self.memory.word_size() - words_added,
                    self.memory.word_size(),
                );
                if !self.consume_gas(mem_gas) {
                    return Err(VmError::OutOfGas);
                }

                let mut bytes = [0u8; 32];
                value.to_big_endian(&mut bytes);
                self.memory.write_word(offset, &bytes)?;
            }

            Opcode::MStore8 => {
                let offset = self.stack.pop()?.as_usize();
                let value = self.stack.pop()?;

                // Memory expansion
                let words_added = self.memory.expand(offset + 1)?;
                let mem_gas = memory_expansion_cost(
                    self.memory.word_size() - words_added,
                    self.memory.word_size(),
                );
                if !self.consume_gas(mem_gas) {
                    return Err(VmError::OutOfGas);
                }

                self.memory.write_byte(offset, value.byte(0))?;
            }

            Opcode::SLoad => {
                let key = self.stack.pop()?;
                let storage_key = StorageKey::from_u256(key);

                // Check warm/cold
                let is_cold = self
                    .access_list
                    .touch_storage(self.context.address, storage_key)
                    == AccessStatus::Cold;
                let gas = if is_cold {
                    costs::COLD_SLOAD
                } else {
                    costs::WARM_SLOAD
                };
                if !self.consume_gas(gas) {
                    return Err(VmError::OutOfGas);
                }

                let value = self
                    .state
                    .get_storage(self.context.address, storage_key)
                    .await?;
                self.stack.push(value.to_u256())?;
            }

            Opcode::SStore => {
                if self.context.is_static {
                    return Err(VmError::WriteInStaticContext);
                }

                let key = self.stack.pop()?;
                let value = self.stack.pop()?;
                let storage_key = StorageKey::from_u256(key);
                let storage_value = StorageValue::from_u256(value);

                // Check warm/cold (SSTORE has complex gas rules)
                let is_cold = self
                    .access_list
                    .touch_storage(self.context.address, storage_key)
                    == AccessStatus::Cold;
                if is_cold
                    && !self.consume_gas(costs::COLD_SLOAD) {
                    return Err(VmError::OutOfGas);
                }

                // Simplified SSTORE gas (full implementation needs original value)
                let gas = if value.is_zero() {
                    costs::SSTORE_RESET
                } else {
                    costs::SSTORE_SET
                };
                if !self.consume_gas(gas) {
                    return Err(VmError::OutOfGas);
                }

                self.state_changes.push(StateChange::StorageWrite {
                    address: self.context.address,
                    key: storage_key,
                    value: storage_value,
                });
            }

            Opcode::Jump => {
                let dest = self.stack.pop()?.as_usize();
                if !self.jump_dests.contains(&dest) {
                    return Err(VmError::InvalidJump(dest));
                }
                self.pc = dest;
            }

            Opcode::JumpI => {
                let dest = self.stack.pop()?.as_usize();
                let condition = self.stack.pop()?;
                if !condition.is_zero() {
                    if !self.jump_dests.contains(&dest) {
                        return Err(VmError::InvalidJump(dest));
                    }
                    self.pc = dest;
                }
            }

            Opcode::Pc => {
                self.stack.push(U256::from(self.pc - 1))?;
            }

            Opcode::MSize => {
                self.stack.push(U256::from(self.memory.len()))?;
            }

            Opcode::Gas => {
                self.stack.push(U256::from(self.gas_remaining))?;
            }

            Opcode::JumpDest => {
                // No-op, just a marker
            }

            // =================================================================
            // PUSH OPERATIONS
            // =================================================================
            Opcode::Push0 => {
                self.stack.push(U256::zero())?;
            }

            Opcode::Push1
            | Opcode::Push2
            | Opcode::Push3
            | Opcode::Push4
            | Opcode::Push5
            | Opcode::Push6
            | Opcode::Push7
            | Opcode::Push8
            | Opcode::Push9
            | Opcode::Push10
            | Opcode::Push11
            | Opcode::Push12
            | Opcode::Push13
            | Opcode::Push14
            | Opcode::Push15
            | Opcode::Push16
            | Opcode::Push17
            | Opcode::Push18
            | Opcode::Push19
            | Opcode::Push20
            | Opcode::Push21
            | Opcode::Push22
            | Opcode::Push23
            | Opcode::Push24
            | Opcode::Push25
            | Opcode::Push26
            | Opcode::Push27
            | Opcode::Push28
            | Opcode::Push29
            | Opcode::Push30
            | Opcode::Push31
            | Opcode::Push32 => {
                let size = opcode.push_size().unwrap_or(0);
                let mut bytes = [0u8; 32];
                let end = (self.pc + size).min(self.code.len());
                let data_len = end - self.pc;
                if data_len > 0 {
                    bytes[32 - size..32 - size + data_len]
                        .copy_from_slice(&self.code[self.pc..end]);
                }
                self.stack.push(U256::from_big_endian(&bytes))?;
                self.pc += size;
            }

            // =================================================================
            // DUP OPERATIONS
            // =================================================================
            Opcode::Dup1 => self.stack.dup(0)?,
            Opcode::Dup2 => self.stack.dup(1)?,
            Opcode::Dup3 => self.stack.dup(2)?,
            Opcode::Dup4 => self.stack.dup(3)?,
            Opcode::Dup5 => self.stack.dup(4)?,
            Opcode::Dup6 => self.stack.dup(5)?,
            Opcode::Dup7 => self.stack.dup(6)?,
            Opcode::Dup8 => self.stack.dup(7)?,
            Opcode::Dup9 => self.stack.dup(8)?,
            Opcode::Dup10 => self.stack.dup(9)?,
            Opcode::Dup11 => self.stack.dup(10)?,
            Opcode::Dup12 => self.stack.dup(11)?,
            Opcode::Dup13 => self.stack.dup(12)?,
            Opcode::Dup14 => self.stack.dup(13)?,
            Opcode::Dup15 => self.stack.dup(14)?,
            Opcode::Dup16 => self.stack.dup(15)?,

            // =================================================================
            // SWAP OPERATIONS
            // =================================================================
            Opcode::Swap1 => self.stack.swap(1)?,
            Opcode::Swap2 => self.stack.swap(2)?,
            Opcode::Swap3 => self.stack.swap(3)?,
            Opcode::Swap4 => self.stack.swap(4)?,
            Opcode::Swap5 => self.stack.swap(5)?,
            Opcode::Swap6 => self.stack.swap(6)?,
            Opcode::Swap7 => self.stack.swap(7)?,
            Opcode::Swap8 => self.stack.swap(8)?,
            Opcode::Swap9 => self.stack.swap(9)?,
            Opcode::Swap10 => self.stack.swap(10)?,
            Opcode::Swap11 => self.stack.swap(11)?,
            Opcode::Swap12 => self.stack.swap(12)?,
            Opcode::Swap13 => self.stack.swap(13)?,
            Opcode::Swap14 => self.stack.swap(14)?,
            Opcode::Swap15 => self.stack.swap(15)?,
            Opcode::Swap16 => self.stack.swap(16)?,

            // =================================================================
            // LOG OPERATIONS
            // =================================================================
            Opcode::Log0 | Opcode::Log1 | Opcode::Log2 | Opcode::Log3 | Opcode::Log4 => {
                if self.context.is_static {
                    return Err(VmError::WriteInStaticContext);
                }

                let topic_count = match opcode {
                    Opcode::Log0 => 0,
                    Opcode::Log1 => 1,
                    Opcode::Log2 => 2,
                    Opcode::Log3 => 3,
                    Opcode::Log4 => 4,
                    _ => unreachable!(),
                };

                let offset = self.stack.pop()?.as_usize();
                let size = self.stack.pop()?.as_usize();

                let mut topics = Vec::with_capacity(topic_count);
                for _ in 0..topic_count {
                    let topic = self.stack.pop()?;
                    let mut bytes = [0u8; 32];
                    topic.to_big_endian(&mut bytes);
                    topics.push(Hash::new(bytes));
                }

                // Memory expansion
                let words_added = self.memory.expand(offset + size)?;
                let mem_gas = memory_expansion_cost(
                    self.memory.word_size() - words_added,
                    self.memory.word_size(),
                );
                if !self.consume_gas(mem_gas) {
                    return Err(VmError::OutOfGas);
                }

                // Log gas
                let log_gas = gas::log_gas_cost(size, topic_count) - costs::LOG;
                if !self.consume_gas(log_gas) {
                    return Err(VmError::OutOfGas);
                }

                let data = self.memory.read_bytes(offset, size);
                self.logs.push(Log::new(
                    self.context.address,
                    topics,
                    Bytes::from_vec(data),
                ));
            }

            // =================================================================
            // SYSTEM OPERATIONS
            // =================================================================
            Opcode::Return => {
                let offset = self.stack.pop()?.as_usize();
                let size = self.stack.pop()?.as_usize();

                // Memory expansion
                let words_added = self.memory.expand(offset + size)?;
                let mem_gas = memory_expansion_cost(
                    self.memory.word_size() - words_added,
                    self.memory.word_size(),
                );
                if !self.consume_gas(mem_gas) {
                    return Err(VmError::OutOfGas);
                }

                self.return_data = Bytes::from_vec(self.memory.read_bytes(offset, size));
                self.stopped = true;
            }

            Opcode::Revert => {
                let offset = self.stack.pop()?.as_usize();
                let size = self.stack.pop()?.as_usize();

                // Memory expansion
                let words_added = self.memory.expand(offset + size)?;
                let mem_gas = memory_expansion_cost(
                    self.memory.word_size() - words_added,
                    self.memory.word_size(),
                );
                if !self.consume_gas(mem_gas) {
                    return Err(VmError::OutOfGas);
                }

                self.return_data = Bytes::from_vec(self.memory.read_bytes(offset, size));
                self.stopped = true;
                self.reverted = true;
            }

            Opcode::Invalid => {
                return Err(VmError::InvalidOpcode(0xFE));
            }

            // Simplified: These require subcall handling
            Opcode::Create
            | Opcode::Create2
            | Opcode::Call
            | Opcode::CallCode
            | Opcode::DelegateCall
            | Opcode::StaticCall
            | Opcode::SelfDestruct
            | Opcode::ExtCodeSize
            | Opcode::ExtCodeCopy
            | Opcode::ExtCodeHash
            | Opcode::TLoad
            | Opcode::TStore
            | Opcode::MCopy => {
                // These need more complex implementation
                return Err(VmError::Internal(format!(
                    "Opcode {opcode:?} not yet implemented"
                )));
            }
        }

        Ok(())
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Analyze bytecode to find valid JUMPDEST locations.
fn analyze_jump_dests(code: &[u8]) -> HashSet<usize> {
    let mut dests = HashSet::new();
    let mut i = 0;

    while i < code.len() {
        let op = code[i];
        if op == 0x5B {
            // JUMPDEST
            dests.insert(i);
        }
        // Skip PUSH data bytes
        if (0x60..=0x7F).contains(&op) {
            let size = (op - 0x5F) as usize;
            i += size;
        }
        i += 1;
    }

    dests
}

/// Convert U256 to address (take lower 20 bytes).
fn u256_to_address(value: U256) -> Address {
    let mut bytes = [0u8; 32];
    value.to_big_endian(&mut bytes);
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&bytes[12..32]);
    Address::new(addr)
}

/// Signed less than comparison.
fn signed_lt(a: U256, b: U256) -> bool {
    let a_neg = a.bit(255);
    let b_neg = b.bit(255);
    match (a_neg, b_neg) {
        (true, false) => true,
        (false, true) => false,
        _ => a < b,
    }
}

/// Signed division.
fn signed_div(a: U256, b: U256) -> U256 {
    let a_neg = a.bit(255);
    let b_neg = b.bit(255);
    let a_abs = if a_neg {
        (!a).overflowing_add(U256::one()).0
    } else {
        a
    };
    let b_abs = if b_neg {
        (!b).overflowing_add(U256::one()).0
    } else {
        b
    };
    let result = a_abs / b_abs;
    if a_neg == b_neg {
        result
    } else {
        (!result).overflowing_add(U256::one()).0
    }
}

/// Signed modulo.
fn signed_mod(a: U256, b: U256) -> U256 {
    let a_neg = a.bit(255);
    let a_abs = if a_neg {
        (!a).overflowing_add(U256::one()).0
    } else {
        a
    };
    let b_abs = if b.bit(255) {
        (!b).overflowing_add(U256::one()).0
    } else {
        b
    };
    let result = a_abs % b_abs;
    if a_neg {
        (!result).overflowing_add(U256::one()).0
    } else {
        result
    }
}

/// Arithmetic shift right.
fn sar(value: U256, shift: U256) -> U256 {
    if shift >= U256::from(256) {
        if value.bit(255) {
            U256::MAX
        } else {
            U256::zero()
        }
    } else {
        let shift = shift.as_usize();
        let is_negative = value.bit(255);
        let shifted = value >> shift;
        if is_negative {
            // Fill with 1s
            let mask = U256::MAX << (256 - shift);
            shifted | mask
        } else {
            shifted
        }
    }
}

/// Exponentiation by squaring.
fn exp_by_squaring(base: U256, mut exp: U256) -> U256 {
    if exp.is_zero() {
        return U256::one();
    }

    let mut result = U256::one();
    let mut base = base;

    while !exp.is_zero() {
        if exp.bit(0) {
            result = result.overflowing_mul(base).0;
        }
        exp >>= 1;
        base = base.overflowing_mul(base).0;
    }

    result
}

/// Convert U256 to U512 for addmod/mulmod.
fn u256_to_u512(value: U256) -> primitive_types::U512 {
    let mut bytes = [0u8; 64];
    value.to_big_endian(&mut bytes[32..]);
    primitive_types::U512::from_big_endian(&bytes)
}

/// Convert U512 back to U256.
fn u512_to_u256(value: primitive_types::U512) -> U256 {
    let mut bytes = [0u8; 64];
    value.to_big_endian(&mut bytes);
    U256::from_big_endian(&bytes[32..])
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_jump_dests() {
        // Code: PUSH1 0x04 JUMP JUMPDEST STOP
        let code = vec![0x60, 0x04, 0x56, 0x5B, 0x00];
        let dests = analyze_jump_dests(&code);
        assert!(dests.contains(&3)); // JUMPDEST at position 3
        assert!(!dests.contains(&0));
    }

    #[test]
    fn test_u256_to_address() {
        let value = U256::from(0x1234u64);
        let addr = u256_to_address(value);
        assert_eq!(addr.as_bytes()[19], 0x34);
        assert_eq!(addr.as_bytes()[18], 0x12);
    }

    #[test]
    fn test_exp_by_squaring() {
        assert_eq!(exp_by_squaring(U256::from(2), U256::from(0)), U256::one());
        assert_eq!(exp_by_squaring(U256::from(2), U256::from(1)), U256::from(2));
        assert_eq!(
            exp_by_squaring(U256::from(2), U256::from(10)),
            U256::from(1024)
        );
        assert_eq!(
            exp_by_squaring(U256::from(3), U256::from(3)),
            U256::from(27)
        );
    }

    #[test]
    fn test_signed_lt() {
        let neg_one = !U256::zero(); // -1 in two's complement
        let one = U256::one();

        assert!(signed_lt(neg_one, one)); // -1 < 1
        assert!(!signed_lt(one, neg_one)); // 1 > -1
        assert!(!signed_lt(one, one)); // 1 == 1
    }
}
