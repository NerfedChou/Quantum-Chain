use std::collections::HashSet;


use crate::domain::{
    Address, Bytes, ExecutionContext, ExecutionResult, Hash, Log,
    StateChange, StorageKey, StorageValue,
};
use crate::evm::gas::{self, costs};
use crate::evm::opcodes::Opcode;
use crate::errors::VmError;
use crate::evm::memory::Memory;
use crate::evm::stack::Stack;
use crate::ports::outbound::{AccessList, AccessStatus, StateAccess};
use primitive_types::U256;

/// EVM Interpreter
pub struct Interpreter<'a, S: StateAccess, A: AccessList> {
    state: &'a S,
    context: ExecutionContext,
    code: Bytes,
    stack: Stack,
    memory: Memory,
    pc: usize,
    gas_remaining: u64,
    stopped: bool,
    reverted: bool,
    return_data: Bytes,
    logs: Vec<Log>,
    state_changes: Vec<StateChange>,
    access_list: &'a mut A,
    jump_dests: HashSet<usize>,
    gas_refund: u64,
}

impl<'a, S: StateAccess, A: AccessList> Interpreter<'a, S, A> {
    pub fn new(
        context: ExecutionContext,
        code: impl Into<Bytes>,
        state: &'a S,
        access_list: &'a mut A,
    ) -> Self {
        let code = code.into();
        let jump_dests = analyze_jump_dests(code.as_slice());
        Self {
            state,
            context: context.clone(), // Use gas limit from context
            gas_remaining: context.gas_limit,
            code,
            stack: Stack::new(),
            memory: Memory::new(),
            pc: 0,
            stopped: false,
            reverted: false,
            return_data: Bytes::new(),
            logs: Vec::new(),
            state_changes: Vec::new(),
            access_list,
            jump_dests,
            gas_refund: 0,
        }
    }

    pub async fn execute(&mut self) -> Result<ExecutionResult, VmError> {
        while !self.stopped {
            if self.pc >= self.code.len() {
                self.stopped = true;
                break;
            }

            let byte = self.code.as_slice()[self.pc];
            let opcode = Opcode::from_byte(byte).unwrap_or(Opcode::Invalid);

            let base_cost = gas::OPCODE_GAS[opcode as u8 as usize];
            if !self.consume_gas(base_cost) {
                return Err(VmError::OutOfGas);
            }

            // Execute the opcode
            self.execute_opcode(opcode).await?;
        }

        let gas_used = self.context.gas_limit - self.gas_remaining;
        let effective_refund = gas::calculate_refund(gas_used, self.gas_refund);
        let final_gas_used = gas_used.saturating_sub(effective_refund);

        Ok(ExecutionResult {
            success: !self.reverted,
            gas_used: final_gas_used,
            output: self.return_data.clone(),
            logs: self.logs.clone(),
            state_changes: if self.reverted { Vec::new() } else { self.state_changes.clone() },
            gas_refund: effective_refund,
            revert_reason: None, // Simplified
        })
    }

    async fn execute_opcode(&mut self, opcode: Opcode) -> Result<(), VmError> {
        self.pc += 1; // Increment PC before execution (except for jumps, which might overwrite it)

        match opcode {
            // Refactored into helper methods
            Opcode::Add | Opcode::Mul | Opcode::Sub | Opcode::Div | Opcode::SDiv | Opcode::Mod |
            Opcode::SMod | Opcode::AddMod | Opcode::MulMod | Opcode::Exp | Opcode::SignExtend => {
                self.exec_arithmetic(opcode)
            }
            Opcode::Lt | Opcode::Gt | Opcode::SLt | Opcode::SGt | Opcode::Eq | Opcode::IsZero => {
                self.exec_comparison(opcode)
            }
            Opcode::And | Opcode::Or | Opcode::Xor | Opcode::Not | Opcode::Byte |
            Opcode::Shl | Opcode::Shr | Opcode::Sar => {
                self.exec_bitwise(opcode)
            }
            Opcode::Keccak256 => self.exec_keccak256(),
            Opcode::Address | Opcode::Balance | Opcode::Origin | Opcode::Caller |
            Opcode::CallValue | Opcode::CallDataLoad | Opcode::CallDataSize | Opcode::CallDataCopy |
            Opcode::CodeSize | Opcode::CodeCopy | Opcode::GasPrice | Opcode::ExtCodeSize |
            Opcode::ExtCodeCopy | Opcode::ReturnDataSize | Opcode::ReturnDataCopy |
            Opcode::ExtCodeHash => {
                self.exec_environmental(opcode).await
            }
            Opcode::BlockHash | Opcode::Coinbase | Opcode::Timestamp | Opcode::Number |
            Opcode::PrevRandao | Opcode::GasLimit | Opcode::ChainId | Opcode::SelfBalance |
            Opcode::BaseFee => {
                self.exec_block_info(opcode)
            }
            Opcode::MLoad | Opcode::MStore | Opcode::MStore8 | Opcode::MSize | Opcode::MCopy => {
                self.exec_memory_ops(opcode)
            }
            Opcode::SLoad | Opcode::SStore | Opcode::TLoad | Opcode::TStore => {
                 self.exec_storage_ops(opcode).await
            }
            Opcode::Jump | Opcode::JumpI | Opcode::Pc | Opcode::JumpDest => {
                self.exec_flow_control(opcode)
            }
            Opcode::Pop | Opcode::Gas |
            Opcode::Push0 | Opcode::Push1 | Opcode::Push2 | Opcode::Push3 | Opcode::Push4 |
            Opcode::Push5 | Opcode::Push6 | Opcode::Push7 | Opcode::Push8 | Opcode::Push9 |
            Opcode::Push10 | Opcode::Push11 | Opcode::Push12 | Opcode::Push13 | Opcode::Push14 |
            Opcode::Push15 | Opcode::Push16 | Opcode::Push17 | Opcode::Push18 | Opcode::Push19 |
            Opcode::Push20 | Opcode::Push21 | Opcode::Push22 | Opcode::Push23 | Opcode::Push24 |
            Opcode::Push25 | Opcode::Push26 | Opcode::Push27 | Opcode::Push28 | Opcode::Push29 |
            Opcode::Push30 | Opcode::Push31 | Opcode::Push32 |
            Opcode::Dup1 | Opcode::Dup2 | Opcode::Dup3 | Opcode::Dup4 | Opcode::Dup5 |
            Opcode::Dup6 | Opcode::Dup7 | Opcode::Dup8 | Opcode::Dup9 | Opcode::Dup10 |
            Opcode::Dup11 | Opcode::Dup12 | Opcode::Dup13 | Opcode::Dup14 | Opcode::Dup15 |
            Opcode::Dup16 |
            Opcode::Swap1 | Opcode::Swap2 | Opcode::Swap3 | Opcode::Swap4 | Opcode::Swap5 |
            Opcode::Swap6 | Opcode::Swap7 | Opcode::Swap8 | Opcode::Swap9 | Opcode::Swap10 |
            Opcode::Swap11 | Opcode::Swap12 | Opcode::Swap13 | Opcode::Swap14 | Opcode::Swap15 |
            Opcode::Swap16 => {
                self.exec_stack_ops(opcode)
            }
            Opcode::Log0 | Opcode::Log1 | Opcode::Log2 | Opcode::Log3 | Opcode::Log4 => {
                self.exec_log(opcode).await
            }
            Opcode::Create | Opcode::Call | Opcode::CallCode | Opcode::Return |
            Opcode::DelegateCall | Opcode::Create2 | Opcode::StaticCall | Opcode::Revert |
            Opcode::Invalid | Opcode::SelfDestruct | Opcode::Stop => {
                self.exec_system(opcode)
            }
        }
    }

    // Breakdown helpers below

    fn exec_arithmetic(&mut self, opcode: Opcode) -> Result<(), VmError> {
        match opcode {
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
                self.stack.push(if b.is_zero() { U256::zero() } else { a / b })?;
            }
            Opcode::SDiv => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                self.stack.push(sdiv(a, b))?;
            }
            Opcode::Mod => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                self.stack.push(if b.is_zero() { U256::zero() } else { a % b })?;
            }
            Opcode::SMod => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                self.stack.push(smod(a, b))?;
            }
            Opcode::AddMod => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                let c = self.stack.pop()?;
                self.stack.push(addmod(a, b, c))?;
            }
            Opcode::MulMod => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                let c = self.stack.pop()?;
                self.stack.push(mulmod(a, b, c))?;
            }
            Opcode::Exp => {
                let base = self.stack.pop()?;
                let exponent = self.stack.pop()?;
                let dynamic_gas = gas::exp_gas_cost(exponent);
                if !self.consume_gas(dynamic_gas - costs::EXP) { // Deduct base cost already paid
                    return Err(VmError::OutOfGas);
                }
                self.stack.push(base.overflowing_pow(exponent).0)?;
            }
            Opcode::SignExtend => {
                let k = self.stack.pop()?;
                let x = self.stack.pop()?;
                let result = sign_extend(k, x);
                self.stack.push(result)?;
            }
             _ => unreachable!(),
        }
        Ok(())
    }

    fn exec_comparison(&mut self, opcode: Opcode) -> Result<(), VmError> {
        let res = match opcode {
            Opcode::Lt => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                if a < b { U256::one() } else { U256::zero() }
            }
            Opcode::Gt => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                if a > b { U256::one() } else { U256::zero() }
            }
            Opcode::SLt => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                if slt(a, b) { U256::one() } else { U256::zero() }
            }
            Opcode::SGt => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                if sgt(a, b) { U256::one() } else { U256::zero() }
            }
            Opcode::Eq => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                if a == b { U256::one() } else { U256::zero() }
            }
            Opcode::IsZero => {
                let a = self.stack.pop()?;
                if a.is_zero() { U256::one() } else { U256::zero() }
            }
            _ => unreachable!(),
        };
        self.stack.push(res)
    }

    fn exec_bitwise(&mut self, opcode: Opcode) -> Result<(), VmError> {
        match opcode {
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
                 if i > U256::from(31) {
                    self.stack.push(U256::zero())?;
                } else {
                    let byte = x.byte(31 - i.as_usize());
                    self.stack.push(U256::from(byte))?;
                }
            }
            Opcode::Shl => {
                let shift = self.stack.pop()?;
                let val = self.stack.pop()?;
                self.stack.push(val << shift)?;
            }
            Opcode::Shr => {
                let shift = self.stack.pop()?;
                let val = self.stack.pop()?;
                self.stack.push(val >> shift)?;
            }
            Opcode::Sar => {
                let shift = self.stack.pop()?;
                let val = self.stack.pop()?;
                self.stack.push(sar(val, shift))?;
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    fn exec_keccak256(&mut self) -> Result<(), VmError> {
        let offset = self.stack.pop()?;
        let size = self.stack.pop()?;
        
        // Memory expansion check
        let offset_usize = offset.as_usize();
        let size_usize = size.as_usize();
        
        let dynamic_gas = gas::keccak256_gas_cost(size_usize);
        let mem_cost = crate::evm::memory::memory_expansion_cost(
            self.memory.word_size(),
            (offset_usize + size_usize).div_ceil(32),
        );
        if !self.consume_gas(dynamic_gas + mem_cost) {
             return Err(VmError::OutOfGas);
        }
        
        self.memory.expand(offset_usize + size_usize)?;
        let data = self.memory.read_bytes(offset_usize, size_usize);
        let hash = keccak256(&data);
        self.stack.push(U256::from_big_endian(hash.as_bytes()))?;
        Ok(())
    }

    async fn exec_environmental(&mut self, opcode: Opcode) -> Result<(), VmError> {
        match opcode {
            Opcode::Address => self.stack.push(U256::from_big_endian(self.context.address.as_bytes()))?,
            Opcode::Balance => {
                 let addr_u256 = self.stack.pop()?;
                 let addr = address_from_u256(addr_u256);
                 
                 let is_cold = self.access_list.touch_account(addr) == AccessStatus::Cold;
                 let cost = if is_cold {
                     costs::BALANCE_COLD
                 } else {
                     costs::BALANCE_WARM
                 };
                 
                 if !self.consume_gas(cost) {
                     return Err(VmError::OutOfGas);
                 }

                 let balance = self.state.get_balance(addr).await.map_err(VmError::StateError)?;
                 self.stack.push(balance)?;
            }
            Opcode::Origin => self.stack.push(U256::from_big_endian(self.context.origin.as_bytes()))?,
            Opcode::Caller => self.stack.push(U256::from_big_endian(self.context.caller.as_bytes()))?,
            Opcode::CallValue => self.stack.push(self.context.value)?,
            Opcode::CallDataLoad => {
                let offset = self.stack.pop()?;
                let val = if offset > U256::from(self.context.data.len()) {
                    U256::zero()
                } else {
                    let start = offset.as_usize();
                    let mut bytes = [0u8; 32];
                    let data = self.context.data.as_slice();
                    let len = (data.len() - start).min(32);
                    bytes[..len].copy_from_slice(&data[start..start+len]);
                    U256::from_big_endian(&bytes)
                };
                self.stack.push(val)?;
            }
            Opcode::CallDataSize => self.stack.push(U256::from(self.context.data.len()))?,
            Opcode::CallDataCopy => {
                let dest_offset = self.stack.pop()?.as_usize();
                let params_offset = self.stack.pop()?.as_usize();
                let size = self.stack.pop()?.as_usize();
                
                // Gas and memory expansion
                self.memory.expand(dest_offset + size)?;
                // Copy logic

                 for i in 0..size {
                    let byte = self.context.data.as_slice().get(params_offset + i).copied().unwrap_or(0);
                    self.memory.write_byte(dest_offset + i, byte)?;
                }
            }
            Opcode::CodeSize => self.stack.push(U256::from(self.code.len()))?,
            Opcode::CodeCopy => {
                let dest_offset = self.stack.pop()?.as_usize();
                let code_offset = self.stack.pop()?.as_usize();
                let size = self.stack.pop()?.as_usize();
                 self.memory.expand(dest_offset + size)?;
                
                for i in 0..size {
                    let byte = self.code.as_slice().get(code_offset + i).copied().unwrap_or(0);
                    self.memory.write_byte(dest_offset + i, byte)?;
                }
            }
            Opcode::GasPrice => self.stack.push(self.context.gas_price)?,
            _ => return Err(VmError::Internal("Not implemented".to_string())),
        }
        Ok(())
    }

    fn exec_block_info(&mut self, opcode: Opcode) -> Result<(), VmError> {
        match opcode {
             Opcode::Coinbase => self.stack.push(U256::from_big_endian(self.context.block.coinbase.as_bytes()))?,
             Opcode::Timestamp => self.stack.push(U256::from(self.context.block.timestamp))?,
             Opcode::Number => self.stack.push(U256::from(self.context.block.number))?,
             Opcode::GasLimit => self.stack.push(U256::from(self.context.block.gas_limit))?,
             Opcode::ChainId => self.stack.push(U256::from(self.context.block.chain_id))?,
             Opcode::BaseFee => self.stack.push(self.context.block.base_fee)?,
             Opcode::BlockHash => {
                  // Simplified: return 0 for now as we don't have blockhash oracle in context yet
                  let _number = self.stack.pop()?;
                  self.stack.push(U256::zero())?;
             }
             _ => return Err(VmError::Internal("Not implemented".to_string())),
        }
        Ok(())
    }

    fn exec_memory_ops(&mut self, opcode: Opcode) -> Result<(), VmError> {
         match opcode {
            Opcode::MLoad => {
                let offset = self.stack.pop()?.as_usize();
                self.memory.expand(offset + 32)?;
                let val = U256::from_big_endian(&self.memory.read_word(offset));
                self.stack.push(val)?;
            }
            Opcode::MStore => {
                 let offset = self.stack.pop()?.as_usize();
                 let val = self.stack.pop()?;
                 self.memory.expand(offset + 32)?;
                 let mut bytes = [0u8; 32];
                 val.to_big_endian(&mut bytes);
                 self.memory.write_word(offset, &bytes)?;
            }
            Opcode::MStore8 => {
                 let offset = self.stack.pop()?.as_usize();
                 let val = self.stack.pop()?;
                 self.memory.expand(offset + 1)?;
                 self.memory.write_byte(offset, (val.low_u32() & 0xFF) as u8)?;
            }
            Opcode::MSize => {
                 self.stack.push(U256::from(self.memory.len()))?;
            }
            Opcode::MCopy => {
                 // Simplified placeholder for MCOPY
                 // Not fully implemented in memory.rs yet?
                 return Err(VmError::Internal("MCOPY not implemented".to_string()));
            }
             _ => return Err(VmError::Internal("Not implemented".to_string())),
        }
        Ok(())
    }

    async fn exec_storage_ops(&mut self, opcode: Opcode) -> Result<(), VmError> {
        match opcode {
            Opcode::SLoad => {
                 let key = self.stack.pop()?;
                 let storage_key = StorageKey::from(key);

                 let is_cold = self.access_list.touch_storage(self.context.address, storage_key) == AccessStatus::Cold;
                 let cost = if is_cold {
                     costs::COLD_SLOAD
                 } else {
                     costs::WARM_SLOAD
                 };

                 if !self.consume_gas(cost) {
                     return Err(VmError::OutOfGas);
                 }

                 let val = self.state.get_storage(self.context.address, storage_key).await.map_err(VmError::StateError)?;
                 self.stack.push(val.to_u256())?;
            }
            Opcode::SStore => {
                if self.context.is_static {
                    return Err(VmError::WriteInStaticContext);
                }
                let key = self.stack.pop()?;
                let val = self.stack.pop()?;
                let storage_key = StorageKey::from(key);

                // EIP-2929: Cold SLOAD cost is paid for SSTORE too if cold
                let is_cold = self.access_list.touch_storage(self.context.address, storage_key) == AccessStatus::Cold;
                let access_cost = if is_cold {
                    costs::COLD_SLOAD
                } else {
                    costs::WARM_SLOAD
                };
                
                if !self.consume_gas(access_cost + costs::SSTORE_SET) {
                     return Err(VmError::OutOfGas);
                }

                self.state.set_storage(self.context.address, storage_key, StorageValue::from(val)).await.map_err(VmError::StateError)?;
                let storage_val = StorageValue::from(val);
                let change = if storage_val.is_zero() {
                    StateChange::StorageDelete {
                        address: self.context.address,
                        key: StorageKey::from(key),
                    }
                } else {
                    StateChange::StorageWrite { 
                        address: self.context.address, 
                        key: StorageKey::from(key), 
                        value: storage_val,
                    }
                };
                self.state_changes.push(change);
            }
            Opcode::TLoad | Opcode::TStore => {
                 return Err(VmError::Internal("Transient storage not implemented".to_string()));
            }
             _ => return Err(VmError::Internal("Not implemented".to_string())),
        }
        Ok(())
    }

    fn jump(&mut self, dest: usize) -> Result<(), VmError> {
        if !self.jump_dests.contains(&dest) {
            return Err(VmError::InvalidJump(dest));
        }
        self.pc = dest;
        Ok(())
    }

    fn exec_flow_control(&mut self, opcode: Opcode) -> Result<(), VmError> {
        match opcode {
            Opcode::Jump => {
                let dest = self.stack.pop()?.as_usize();
                self.jump(dest)?;
            }
             Opcode::JumpI => {
                let dest = self.stack.pop()?.as_usize();
                let cond = self.stack.pop()?;
                if !cond.is_zero() {
                    self.jump(dest)?;
                }
            }
            Opcode::Pc => {
                self.stack.push(U256::from(self.pc - 1))?;
            }
            Opcode::JumpDest => {
                // No-op
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    fn exec_stack_ops(&mut self, opcode: Opcode) -> Result<(), VmError> {
        match opcode {
             Opcode::Pop => self.stack.pop().map(|_| ()),
             Opcode::Gas => self.stack.push(U256::from(self.gas_remaining)),
             Opcode::Push0 | Opcode::Push1 | Opcode::Push2 | Opcode::Push3 | Opcode::Push4 |
             Opcode::Push5 | Opcode::Push6 | Opcode::Push7 | Opcode::Push8 | Opcode::Push9 |
             Opcode::Push10 | Opcode::Push11 | Opcode::Push12 | Opcode::Push13 | Opcode::Push14 |
             Opcode::Push15 | Opcode::Push16 | Opcode::Push17 | Opcode::Push18 | Opcode::Push19 |
             Opcode::Push20 | Opcode::Push21 | Opcode::Push22 | Opcode::Push23 | Opcode::Push24 |
             Opcode::Push25 | Opcode::Push26 | Opcode::Push27 | Opcode::Push28 | Opcode::Push29 |
             Opcode::Push30 | Opcode::Push31 | Opcode::Push32 => self.exec_push(opcode),
             Opcode::Dup1 | Opcode::Dup2 | Opcode::Dup3 | Opcode::Dup4 | Opcode::Dup5 |
             Opcode::Dup6 | Opcode::Dup7 | Opcode::Dup8 | Opcode::Dup9 | Opcode::Dup10 |
             Opcode::Dup11 | Opcode::Dup12 | Opcode::Dup13 | Opcode::Dup14 | Opcode::Dup15 |
             Opcode::Dup16 => self.exec_dup(opcode),
             Opcode::Swap1 | Opcode::Swap2 | Opcode::Swap3 | Opcode::Swap4 | Opcode::Swap5 |
             Opcode::Swap6 | Opcode::Swap7 | Opcode::Swap8 | Opcode::Swap9 | Opcode::Swap10 |
             Opcode::Swap11 | Opcode::Swap12 | Opcode::Swap13 | Opcode::Swap14 | Opcode::Swap15 |
             Opcode::Swap16 => self.exec_swap(opcode),
             _ => unreachable!(),
        }
    }

    fn exec_push(&mut self, opcode: Opcode) -> Result<(), VmError> {
        let size = opcode.push_size().unwrap_or(0);
        let mut bytes = [0u8; 32];
        let end = (self.pc + size).min(self.code.len());
        let data_len = end - self.pc;
        if data_len > 0 {
             bytes[32 - size..32 - size + data_len].copy_from_slice(&self.code.as_slice()[self.pc..end]);
        }
        self.stack.push(U256::from_big_endian(&bytes))?;
        self.pc += size;
        Ok(())
    }

    fn exec_dup(&mut self, opcode: Opcode) -> Result<(), VmError> {
        let idx = (opcode as u8 - Opcode::Dup1 as u8) as usize;
        self.stack.dup(idx)
    }

    fn exec_swap(&mut self, opcode: Opcode) -> Result<(), VmError> {
        let idx = (opcode as u8 - Opcode::Swap1 as u8 + 1) as usize;
        self.stack.swap(idx)
    }

    async fn exec_log(&mut self, opcode: Opcode) -> Result<(), VmError> {
        if self.context.is_static {
            return Err(VmError::WriteInStaticContext);
        }

        let topic_count = (opcode as u8 - Opcode::Log0 as u8) as usize;
        let offset = self.stack.pop()?.as_usize();
        let size = self.stack.pop()?.as_usize();
        
        // Gas dynamic
        let cost = gas::log_gas_cost(size, topic_count);
        if !self.consume_gas(cost - costs::LOG) { // Base LOG cost already paid
              return Err(VmError::OutOfGas);
        }
        
        self.memory.expand(offset + size)?;
        
        let mut topics = Vec::with_capacity(topic_count);
        for _ in 0..topic_count {
            let val = self.stack.pop()?;
            let mut bytes = [0u8; 32];
            val.to_big_endian(&mut bytes);
            topics.push(Hash::from(bytes));
        }
        
        let data = self.memory.read_bytes(offset, size);
        
        self.logs.push(Log {
            address: self.context.address,
            topics,
            data: Bytes::from(data),
        });
        
        Ok(())
    }

    fn exec_system(&mut self, opcode: Opcode) -> Result<(), VmError> {
         match opcode {
            Opcode::Return => {
                let offset = self.stack.pop()?.as_usize();
                let size = self.stack.pop()?.as_usize();
                self.memory.expand(offset + size)?;
                self.return_data = Bytes::from(self.memory.read_bytes(offset, size));
                self.stopped = true;
            }
            Opcode::Revert => {
                let offset = self.stack.pop()?.as_usize();
                let size = self.stack.pop()?.as_usize();
                self.memory.expand(offset + size)?;
                self.return_data = Bytes::from(self.memory.read_bytes(offset, size));
                self.stopped = true;
                self.reverted = true;
                return Err(VmError::Revert("Revert opcode".to_string()));
            }
            Opcode::Stop => {
                self.stopped = true;
            }
            Opcode::Invalid => {
                return Err(VmError::InvalidOpcode(0xFE));
            }
             _ => return Err(VmError::Internal("Not implemented".to_string())),
        }
        Ok(())
    }

    fn consume_gas(&mut self, amount: u64) -> bool {
        if self.gas_remaining >= amount {
            self.gas_remaining -= amount;
            true
        } else {
            false
        }
    }
}

// Helper functions (outside impl)

fn analyze_jump_dests(code: &[u8]) -> HashSet<usize> {
    let mut dests = HashSet::new();
    let mut i = 0;
    while i < code.len() {
        let op = Opcode::from_byte(code[i]).unwrap_or(Opcode::Invalid);
        if op == Opcode::JumpDest {
            dests.insert(i);
        }
        if let Some(size) = op.push_size() {
            i += size;
        }
        i += 1;
    }
    dests
}

fn address_from_u256(value: U256) -> Address {
    let mut bytes = [0u8; 32];
    value.to_big_endian(&mut bytes);
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&bytes[12..]);
    Address::new(addr)
}

fn sdiv(a: U256, b: U256) -> U256 {
    // Basic signed division placeholder
    if b.is_zero() { U256::zero() } else { a / b }
}

fn smod(a: U256, b: U256) -> U256 {
    if b.is_zero() { U256::zero() } else { a % b }
}

fn addmod(a: U256, b: U256, c: U256) -> U256 {
     if c.is_zero() { U256::zero() } else { (a + b) % c }
}

fn mulmod(a: U256, b: U256, c: U256) -> U256 {
    if c.is_zero() { U256::zero() } else { (a * b) % c }
}

fn sign_extend(k: U256, x: U256) -> U256 {
    let k = k.as_usize();
    let bit = k * 8 + 7;
    if bit >= 256 {
        return x;
    }

    let mask = (U256::one() << (bit + 1)) - U256::one();
    let sign_bit = x.bit(bit);

    if sign_bit {
        x | !mask
    } else {
        x & mask
    }
}

fn keccak256(data: &[u8]) -> Hash {
    crate::domain::services::keccak256(data)
}

fn slt(a: U256, b: U256) -> bool {
    let a_neg = a.bit(255);
    let b_neg = b.bit(255);
    if a_neg && !b_neg {
        return true;
    } 
    if !a_neg && b_neg {
        return false;
    }
    a < b
}

fn sgt(a: U256, b: U256) -> bool {
    slt(b, a)
}

fn sar(value: U256, shift: U256) -> U256 {
    let shift_usize = shift.low_u64() as usize; // safe enough for shift check
    if shift >= U256::from(256) {
        if value.bit(255) {
            return !U256::zero(); // All ones
        }
        return U256::zero();
    }
    
    let result = value >> shift_usize;
    if value.bit(255) {
         // Sign extend
         let mask = !U256::zero() << (256 - shift_usize);
         result | mask
    } else {
        result
    }
}
