# SPECIFICATION: SMART CONTRACT EXECUTION

**Version:** 2.3  
**Subsystem ID:** 11  
**Bounded Context:** Programmable Execution  
**Crate Name:** `crates/smart-contracts`  
**Author:** Systems Architecture Team  
**Date:** 2024-12-01  
**Architecture Compliance:** Architecture.md v2.3, IPC-MATRIX.md v2.3, System.md v2.3

---

## 1. ABSTRACT

### 1.1 Purpose

The **Smart Contract Execution** subsystem provides a sandboxed virtual machine (EVM/WASM) for executing deterministic smart contract code. It manages gas metering, memory allocation, and state access while ensuring isolation and security.

### 1.2 Responsibility Boundaries

**In Scope:**
- Execute smart contract bytecode (EVM opcodes or WASM)
- Gas metering and limit enforcement
- Memory and stack management
- State read/write via State Management (Subsystem 4)
- Contract deployment and CREATE/CREATE2 operations
- Inter-contract calls (CALL, DELEGATECALL, STATICCALL)

**Out of Scope:**
- State storage (Subsystem 4)
- Transaction signature verification (Subsystem 10)
- Transaction ordering (Subsystem 12)
- Block validation (Subsystem 8)

### 1.3 Trust Model

```
┌─────────────────────────────────────────────────────────────────┐
│                    TRUST BOUNDARY (V2.3)                        │
├─────────────────────────────────────────────────────────────────┤
│  TRUSTED:                                                       │
│  ├─ State reads/writes via Subsystem 4 (State Management)       │
│  └─ Transaction sender verified by Subsystem 10                 │
│                                                                 │
│  UNTRUSTED:                                                     │
│  └─ Contract bytecode (may be malicious, sandboxed)             │
│                                                                 │
│  SECURITY (Envelope-Only Identity - V2.2 Amendment):            │
│  Identity from AuthenticatedMessage.sender_id only.             │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. DOMAIN MODEL

### 2.1 Core Entities

```rust
/// Execution context for a contract call
#[derive(Clone, Debug)]
pub struct ExecutionContext {
    /// Transaction sender (EOA)
    pub origin: Address,
    /// Current caller (may differ in nested calls)
    pub caller: Address,
    /// Contract being executed
    pub address: Address,
    /// Value transferred (wei)
    pub value: U256,
    /// Input data (calldata)
    pub data: Bytes,
    /// Gas limit for this call
    pub gas_limit: u64,
    /// Gas price
    pub gas_price: U256,
    /// Block context
    pub block: BlockContext,
    /// Call depth (for reentrancy limits)
    pub depth: u16,
    /// Is this a static call (no state changes)
    pub is_static: bool,
}

/// Block context for execution
#[derive(Clone, Debug)]
pub struct BlockContext {
    pub number: u64,
    pub timestamp: u64,
    pub coinbase: Address,
    pub difficulty: U256,
    pub gas_limit: u64,
    pub base_fee: U256,
    pub chain_id: u64,
}

/// Execution result
#[derive(Clone, Debug)]
pub struct ExecutionResult {
    /// Whether execution succeeded
    pub success: bool,
    /// Return data
    pub output: Bytes,
    /// Gas used
    pub gas_used: u64,
    /// Gas refund (for SSTORE clears)
    pub gas_refund: u64,
    /// State changes to apply
    pub state_changes: Vec<StateChange>,
    /// Logs emitted
    pub logs: Vec<Log>,
    /// Revert reason (if failed)
    pub revert_reason: Option<String>,
}

/// State change from execution
#[derive(Clone, Debug)]
pub enum StateChange {
    BalanceTransfer {
        from: Address,
        to: Address,
        amount: U256,
    },
    StorageWrite {
        address: Address,
        key: StorageKey,
        value: StorageValue,
    },
    StorageDelete {
        address: Address,
        key: StorageKey,
    },
    ContractCreate {
        address: Address,
        code: Bytes,
    },
    ContractDestroy {
        address: Address,
        beneficiary: Address,
    },
    NonceIncrement {
        address: Address,
    },
}

/// Emitted log (event)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Log {
    pub address: Address,
    pub topics: Vec<Hash>,
    pub data: Bytes,
}

/// VM configuration
#[derive(Clone, Debug)]
pub struct VmConfig {
    /// Maximum call depth
    pub max_call_depth: u16,
    /// Maximum code size (EIP-170)
    pub max_code_size: usize,
    /// Maximum init code size (EIP-3860)
    pub max_init_code_size: usize,
    /// Stack size limit
    pub max_stack_size: usize,
    /// Memory expansion limit
    pub max_memory_size: usize,
    /// EVM version/fork
    pub evm_version: EvmVersion,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            max_call_depth: 1024,
            max_code_size: 24_576,        // 24 KB (EIP-170)
            max_init_code_size: 49_152,   // 48 KB (EIP-3860)
            max_stack_size: 1024,
            max_memory_size: 1 << 24,     // 16 MB
            evm_version: EvmVersion::Shanghai,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum EvmVersion {
    Istanbul,
    Berlin,
    London,
    Paris,
    Shanghai,
}
```

### 2.2 Invariants

```rust
/// INVARIANT-1: Gas Limit
/// Execution cannot use more gas than gas_limit.
fn invariant_gas_limit(result: &ExecutionResult, ctx: &ExecutionContext) -> bool {
    result.gas_used <= ctx.gas_limit
}

/// INVARIANT-2: Deterministic Execution
/// Same inputs always produce same outputs.
fn invariant_deterministic(
    code: &[u8],
    ctx: &ExecutionContext,
    state: &dyn StateReader,
) -> bool {
    let result1 = execute(code, ctx, state);
    let result2 = execute(code, ctx, state);
    result1 == result2
}

/// INVARIANT-3: No State Change on Revert
/// If execution reverts, state changes are NOT applied.
fn invariant_revert_rollback(result: &ExecutionResult) -> bool {
    if !result.success {
        result.state_changes.is_empty()
    } else {
        true
    }
}

/// INVARIANT-4: Static Call Purity
/// STATICCALL cannot modify state.
fn invariant_static_purity(ctx: &ExecutionContext, result: &ExecutionResult) -> bool {
    if ctx.is_static {
        result.state_changes.iter().all(|c| matches!(c, StateChange::BalanceTransfer { .. } | StateChange::StorageWrite { .. }) == false)
    } else {
        true
    }
}

/// INVARIANT-5: Call Depth Limit
/// Execution cannot exceed max call depth.
fn invariant_call_depth(ctx: &ExecutionContext, config: &VmConfig) -> bool {
    ctx.depth <= config.max_call_depth
}
```

---

## 3. PORTS (HEXAGONAL ARCHITECTURE)

### 3.1 Driving Ports (API - Inbound)

```rust
/// Primary Smart Contract API
#[async_trait]
pub trait SmartContractApi: Send + Sync {
    /// Execute a contract call
    async fn execute(
        &self,
        context: ExecutionContext,
        code: &[u8],
    ) -> Result<ExecutionResult, VmError>;
    
    /// Execute a transaction (may deploy or call)
    async fn execute_transaction(
        &self,
        tx: &SignedTransaction,
        block: &BlockContext,
    ) -> Result<ExecutionResult, VmError>;
    
    /// Estimate gas for a call
    async fn estimate_gas(
        &self,
        context: ExecutionContext,
        code: &[u8],
    ) -> Result<u64, VmError>;
    
    /// Dry-run a call (no state changes)
    async fn call(
        &self,
        context: ExecutionContext,
        code: &[u8],
    ) -> Result<Bytes, VmError>;
}
```

### 3.2 Driven Ports (SPI - Outbound Dependencies)

```rust
/// State access interface
#[async_trait]
pub trait StateAccess: Send + Sync {
    /// Read account state
    async fn get_account(&self, address: Address) -> Result<Option<AccountState>, StateError>;
    
    /// Read storage value
    async fn get_storage(&self, address: Address, key: StorageKey) -> Result<StorageValue, StateError>;
    
    /// Write storage value (queued, applied on commit)
    async fn set_storage(&self, address: Address, key: StorageKey, value: StorageValue) -> Result<(), StateError>;
    
    /// Get code for contract
    async fn get_code(&self, address: Address) -> Result<Bytes, StateError>;
    
    /// Check if account exists
    async fn account_exists(&self, address: Address) -> Result<bool, StateError>;
}

/// Signature verification for ecrecover precompile
pub trait SignatureVerifier: Send + Sync {
    fn ecrecover(&self, hash: &Hash, signature: &EcdsaSignature) -> Option<Address>;
}
```

---

## 4. EVENT SCHEMA

### 4.1 Messages

```rust
/// Request to execute a transaction
/// SECURITY: Envelope sender_id MUST be 8 (Consensus) or 12 (Ordering)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecuteTransactionRequest {
    pub correlation_id: CorrelationId,
    pub transaction: SignedTransaction,
    pub block_context: BlockContext,
}

/// Execution result response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecuteTransactionResponse {
    pub correlation_id: CorrelationId,
    pub success: bool,
    pub gas_used: u64,
    pub output: Bytes,
    pub logs: Vec<Log>,
    pub state_changes: Vec<StateChange>,
}

/// State read request (to Subsystem 4)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateReadRequest {
    pub correlation_id: CorrelationId,
    pub reply_to: Topic,
    pub address: Address,
    pub storage_key: Option<StorageKey>,
}

/// State write request (to Subsystem 4)
/// SECURITY: Only this subsystem can write state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateWriteRequest {
    pub correlation_id: CorrelationId,
    pub address: Address,
    pub storage_key: StorageKey,
    pub value: StorageValue,
    pub execution_context: ExecutionContextId,
}
```

---

## 5. TEST-DRIVEN DEVELOPMENT STRATEGY

### 5.1 Unit Tests

```rust
#[cfg(test)]
mod unit_tests {
    use super::*;
    
    // === Gas Tests ===
    
    #[test]
    fn test_gas_limit_enforced() {
        let vm = Evm::new(VmConfig::default());
        let mut ctx = create_test_context();
        ctx.gas_limit = 1000;
        
        // Infinite loop contract
        let code = compile("while(true) {}");
        
        let result = vm.execute_sync(&ctx, &code);
        
        assert!(!result.success);
        assert!(result.gas_used <= ctx.gas_limit);
    }
    
    #[test]
    fn test_gas_refund_capped() {
        let vm = Evm::new(VmConfig::default());
        let ctx = create_test_context();
        
        // Contract that clears many storage slots (triggers refund)
        let code = compile_storage_clear(100);
        
        let result = vm.execute_sync(&ctx, &code);
        
        // Refund capped at 50% of gas used (EIP-3529)
        assert!(result.gas_refund <= result.gas_used / 2);
    }
    
    // === Reentrancy Tests ===
    
    #[test]
    fn test_reentrancy_guard() {
        let vm = Evm::new(VmConfig::default());
        
        // Contract with reentrancy vulnerability
        let vulnerable = deploy_vulnerable_contract(&vm);
        let attacker = deploy_attacker_contract(&vm);
        
        // Checks-Effects-Interactions prevents exploit
        let result = vm.execute_call(attacker, vulnerable, "attack()");
        
        // Should revert due to reentrancy guard
        assert!(!result.success);
    }
    
    // === Call Depth Tests ===
    
    #[test]
    fn test_call_depth_limit() {
        let config = VmConfig {
            max_call_depth: 100,
            ..Default::default()
        };
        let vm = Evm::new(config);
        
        // Contract that calls itself recursively
        let recursive = deploy_recursive_contract(&vm);
        
        let result = vm.execute_call(Address::zero(), recursive, "recurse(200)");
        
        assert!(!result.success);
        assert!(result.revert_reason.unwrap().contains("call depth"));
    }
    
    // === Static Call Tests ===
    
    #[test]
    fn test_static_call_no_state_change() {
        let vm = Evm::new(VmConfig::default());
        
        let contract = deploy_test_contract(&vm);
        let mut ctx = create_test_context();
        ctx.is_static = true;
        
        // Contract tries to write storage in STATICCALL
        let result = vm.execute_with_context(&ctx, &contract, "writeStorage()");
        
        assert!(!result.success);
        assert!(result.state_changes.is_empty());
    }
    
    // === Precompile Tests ===
    
    #[test]
    fn test_ecrecover_precompile() {
        let vm = Evm::new(VmConfig::default());
        
        let (private_key, address) = generate_keypair();
        let message_hash = keccak256(b"test");
        let signature = sign(&message_hash, &private_key);
        
        let input = encode_ecrecover_input(&message_hash, &signature);
        let result = vm.call_precompile(ECRECOVER_ADDRESS, &input);
        
        assert!(result.success);
        let recovered = Address::from_slice(&result.output[12..32]);
        assert_eq!(recovered, address);
    }
    
    // === Determinism Tests ===
    
    #[test]
    fn test_execution_determinism() {
        let vm = Evm::new(VmConfig::default());
        let code = compile_complex_contract();
        let ctx = create_test_context();
        
        let result1 = vm.execute_sync(&ctx, &code);
        let result2 = vm.execute_sync(&ctx, &code);
        
        assert_eq!(result1.success, result2.success);
        assert_eq!(result1.output, result2.output);
        assert_eq!(result1.gas_used, result2.gas_used);
        assert_eq!(result1.state_changes, result2.state_changes);
    }
}
```

### 5.2 Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_contract_deployment() {
        let (state, _) = create_mock_state();
        let vm = SmartContractService::new(state);
        
        let deploy_tx = create_deployment_transaction(ERC20_BYTECODE);
        let block = create_test_block_context();
        
        let result = vm.execute_transaction(&deploy_tx, &block).await.unwrap();
        
        assert!(result.success);
        assert!(result.state_changes.iter().any(|c| matches!(c, StateChange::ContractCreate { .. })));
    }
    
    #[tokio::test]
    async fn test_token_transfer() {
        let (state, _) = create_mock_state();
        let vm = SmartContractService::new(state);
        
        // Deploy ERC20
        let token = deploy_erc20(&vm).await;
        
        // Transfer tokens
        let transfer_tx = create_call_transaction(
            token,
            "transfer(address,uint256)",
            (BOB, U256::from(100)),
        );
        
        let result = vm.execute_transaction(&transfer_tx, &create_test_block_context()).await.unwrap();
        
        assert!(result.success);
        assert!(result.logs.iter().any(|log| log.topics[0] == TRANSFER_EVENT_SIGNATURE));
    }
    
    #[tokio::test]
    async fn test_state_writes_sent_to_state_management() {
        let (state, state_rx) = create_mock_state_with_receiver();
        let vm = SmartContractService::new(state);
        
        let contract = deploy_storage_contract(&vm).await;
        let write_tx = create_call_transaction(contract, "store(uint256)", U256::from(42));
        
        vm.execute_transaction(&write_tx, &create_test_block_context()).await.unwrap();
        
        // Verify write request sent
        let write_req = state_rx.recv().await.unwrap();
        assert_eq!(write_req.address, contract);
    }
}
```

---

## 6. ERROR HANDLING

```rust
#[derive(Debug, thiserror::Error)]
pub enum VmError {
    #[error("Out of gas")]
    OutOfGas,
    
    #[error("Stack overflow")]
    StackOverflow,
    
    #[error("Stack underflow")]
    StackUnderflow,
    
    #[error("Invalid opcode: 0x{0:02X}")]
    InvalidOpcode(u8),
    
    #[error("Invalid jump destination")]
    InvalidJump,
    
    #[error("Call depth exceeded")]
    CallDepthExceeded,
    
    #[error("Contract code too large: {size} > {max}")]
    CodeSizeExceeded { size: usize, max: usize },
    
    #[error("Write in static context")]
    WriteInStaticContext,
    
    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: U256, available: U256 },
    
    #[error("State access error: {0}")]
    StateError(#[from] StateError),
    
    #[error("Revert: {0}")]
    Revert(String),
}
```

---

## 7. CONFIGURATION

```toml
[smart_contracts]
evm_version = "shanghai"
max_call_depth = 1024
max_code_size = 24576
max_init_code_size = 49152
max_stack_size = 1024
max_memory_size = 16777216

# Gas settings
gas_refund_cap_percent = 50
```

---

## APPENDIX A: DEPENDENCY MATRIX

**Reference:** System.md V2.3 Unified Dependency Graph, IPC-MATRIX.md Subsystem 11

| This Subsystem | Depends On | Dependency Type | Purpose | Reference |
|----------------|------------|-----------------|---------|-----------|
| Smart Contracts (11) | Subsystem 4 (State Mgmt) | Read/Write | State access | System.md Subsystem 11 |
| Smart Contracts (11) | Subsystem 10 (Sig Verify) | Uses | ecrecover precompile | System.md Subsystem 11 |
| Smart Contracts (11) | Subsystem 8, 12 | Accepts from | Transaction execution requests | IPC-MATRIX.md Subsystem 11 |

---

## APPENDIX B: SECURITY BOUNDARIES (IPC-MATRIX.md Compliance)

**Reference:** IPC-MATRIX.md, Subsystem 11 Section

### B.1 Authorized Message Senders

| Message Type | Authorized Sender(s) | Reference |
|--------------|---------------------|-----------|
| `ExecuteTransactionRequest` | Subsystems 8, 12 ONLY | IPC-MATRIX.md Security Boundaries |
| `ExecuteHTLCRequest` | Subsystem 15 (Cross-Chain) ONLY | IPC-MATRIX.md Security Boundaries |

### B.2 Execution Safety Limits

**Reference:** System.md, Subsystem 11 Security Defenses

```rust
/// MANDATORY limits per System.md
const EXECUTION_LIMITS: ExecutionLimits = ExecutionLimits {
    max_call_depth: 1024,          // Prevent stack overflow
    max_code_size: 24576,          // 24KB max contract code
    max_init_code_size: 49152,     // 48KB max deployment code
    max_stack_size: 1024,          // EVM stack limit
    max_memory_size: 16 * 1024 * 1024, // 16MB memory
    execution_timeout_secs: 5,     // Hard timeout
    block_gas_limit: 30_000_000,   // Block gas cap
};
```

---

## APPENDIX C: CROSS-REFERENCES

### C.1 Master Document References

| Document | Section | Relevance to This Spec |
|----------|---------|------------------------|
| Architecture.md | Section 3.2.1 | Envelope-Only Identity mandate |
| IPC-MATRIX.md | Subsystem 11 | Security boundaries, execution requests |
| System.md | Subsystem 11 | EVM/WASM execution, gas metering |

### C.2 Related Specifications

| Specification | Relationship | Notes |
|--------------|--------------|-------|
| SPEC-04-STATE-MANAGEMENT.md | Dependency | State read/write during execution |
| SPEC-08-CONSENSUS.md | Producer | Sends ExecuteTransactionRequest for block txs |
| SPEC-10-SIGNATURE-VERIFICATION.md | Dependency | ecrecover precompile |
| SPEC-12-TRANSACTION-ORDERING.md | Producer | Sends ordered transactions for execution |
| SPEC-15-CROSS-CHAIN.md | Consumer | HTLC contract execution |

### C.3 Implementation Priority

**Reference:** System.md, Implementation Priority Section

This subsystem is in **Phase 3 (Advanced - Weeks 9-12)** because:
- Depends on Subsystems 4 (State) and 10 (Signatures)
- Complex EVM implementation
- Can be implemented after core block processing is complete

---

**END OF SPECIFICATION**

**Next Steps:**
1. Review this specification for completeness
2. Approve for TDD Phase (write tests)
3. Implement domain logic (pass tests)
4. Implement adapters (wire to runtime)
