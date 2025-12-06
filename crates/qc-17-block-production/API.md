# QC-17 Block Production API Documentation

## Overview

The Block Production subsystem (QC-17) provides APIs for producing new blocks in the Quantum-Chain. It follows hexagonal architecture with well-defined ports for external communication.

## Core Components

### 1. Transaction Selector Service

**Purpose:** Selects and prioritizes transactions for block inclusion.

```rust
pub trait TransactionSelector {
    fn select_transactions(
        &self,
        candidates: Vec<TransactionCandidate>,
        gas_limit: u64,
    ) -> Result<Vec<TransactionCandidate>>;
}
```

**Algorithm:** Multi-Dimensional Knapsack with Priority Queue (SPEC-17 §3.1)
- **Input:** Transaction candidates, block gas limit
- **Output:** Optimally selected transactions maximizing fees
- **Complexity:** O(n log n) where n = number of candidates
- **Guarantees:** Deterministic, reproducible selection

### 2. Transaction Validation

**Purpose:** Validates transactions before block inclusion.

```rust
pub trait TransactionValidator {
    fn validate(&self, tx: &TransactionCandidate) -> Result<()>;
}
```

**Validators Available:**
- `GasValidator` - Gas limit and price validation
- `SignatureValidator` - Cryptographic signature verification
- `CompositeValidator` - Chains multiple validators

### 3. IPC Communication Ports

#### 3.1 Mempool Reader Port

**Purpose:** Fetch pending transactions from Subsystem 6 (Mempool).

```rust
#[async_trait]
pub trait MempoolReader: Send + Sync {
    async fn get_pending_transactions(
        &self,
        gas_limit: u64,
        max_count: u32,
        min_gas_price: U256,
    ) -> Result<Vec<TransactionCandidate>, IpcError>;
}
```

**IPC Message:** `qc6.mempool.pending` → `qc17.mempool.reply`
- **Timeout:** 2 seconds
- **Rate Limit:** As per IPC-MATRIX.md
- **Authentication:** Message signature validation

#### 3.2 State Reader Port

**Purpose:** Simulate transaction execution against state (Subsystem 4).

```rust
#[async_trait]
pub trait StateReader: Send + Sync {
    async fn simulate_transactions(
        &self,
        parent_state_root: H256,
        transactions: &[TransactionCandidate],
    ) -> Result<Vec<SimulationResult>, IpcError>;
}
```

**IPC Message:** `qc4.state.prefetch` → `qc17.state.reply`
- **Timeout:** 5 seconds
- **Batch Size:** Up to 5000 transactions
- **Returns:** Gas estimates, state changes, execution status

#### 3.3 Consensus Submitter Port

**Purpose:** Submit produced blocks to Subsystem 8 (Consensus).

```rust
#[async_trait]
pub trait ConsensusSubmitter: Send + Sync {
    async fn submit_block(
        &self,
        block: &BlockTemplate,
        nonce: Option<u64>,
        vrf_proof: Option<VRFProof>,
        validator_signature: Option<Vec<u8>>,
    ) -> Result<SubmissionReceipt, IpcError>;
}
```

**IPC Message:** `qc8.consensus.submit` → `qc17.consensus.reply`
- **Timeout:** 3 seconds
- **Modes:** PoW (with nonce), PoS (with VRF), PBFT (with signature)
- **Returns:** Block hash or rejection reason

## Domain Models

### BlockTemplate

```rust
pub struct BlockTemplate {
    pub header: BlockHeader,
    pub transactions: Vec<Vec<u8>>,
    pub total_fees: U256,
}
```

### BlockHeader

```rust
pub struct BlockHeader {
    pub parent_hash: H256,
    pub block_number: u64,
    pub timestamp: u64,
    pub beneficiary: [u8; 20],
    pub state_root: H256,
    pub transactions_root: H256,
    pub gas_used: u64,
    pub gas_limit: u64,
    pub difficulty: U256,
    pub nonce: Option<u64>,
}
```

### TransactionCandidate

```rust
pub struct TransactionCandidate {
    pub transaction: Vec<u8>,
    pub from: [u8; 20],
    pub nonce: u64,
    pub gas_price: U256,
    pub gas_limit: u64,
    pub signature_valid: bool,
}
```

## Security Features

### 1. Input Validation

- **Sender Whitelist:** Only subsystems 4, 6, 8, 9 (SPEC-17 Appendix B.1)
- **Rate Limiting:** Per-sender quotas (1000 req/min)
- **Gas Limits:** Max 30M gas per block
- **Signature Verification:** All IPC messages signed

### 2. DoS Protection

```rust
pub struct SecurityValidator {
    allowed_senders: HashSet<u8>,
    rate_limiter: Arc<RwLock<RateLimiter>>,
    max_block_gas_limit: u64,
    min_gas_price: U256,
}
```

**Features:**
- Request rate limiting per subsystem
- Transaction count limits
- Gas price floor enforcement
- Duplicate transaction detection

### 3. Invariant Enforcement

**Pre-conditions:**
- All transactions have valid signatures
- Total gas ≤ block gas limit
- No duplicate transactions
- Monotonic nonce sequence

**Post-conditions:**
- Block hash matches PoW difficulty (if PoW mode)
- Transactions ordered by priority
- Total fees calculated correctly
- State root integrity

## Performance Characteristics

### Transaction Selection

- **Throughput:** 50,000 tx/sec evaluation
- **Latency:** <100ms for 10,000 transactions
- **Memory:** O(n) where n = transaction count
- **CPU:** O(n log n) sorting complexity

### IPC Communication

- **Concurrent Requests:** Up to 100 parallel
- **Retry Strategy:** Exponential backoff (1s, 2s, 4s)
- **Circuit Breaker:** Opens after 3 consecutive failures
- **Timeout Handling:** Graceful degradation

## Error Handling

### Error Types

```rust
pub enum BlockProductionError {
    // Configuration errors
    InvalidBlockGasLimit { limit: u64, max: u64 },
    InvalidDifficulty(String),
    
    // Transaction errors
    GasLimitExceeded { limit: u64, used: u64 },
    InvalidTransaction(String),
    
    // IPC errors
    MempoolUnavailable(String),
    StateSimulationFailed(String),
    ConsensusRejected(String),
    
    // Internal errors
    InternalError(String),
}
```

### Recovery Strategies

- **Transient Failures:** Automatic retry with backoff
- **Permanent Failures:** Fallback to cached data
- **Critical Failures:** Alert operators, halt block production

## Usage Examples

### Basic Block Production

```rust
use qc_17_block_production::*;

// Initialize service
let selector = BasicTransactionSelector::new();
let validator = CompositeValidator::new()
    .add_validator(GasValidator::new(30_000_000, U256::from(1_000_000_000)))
    .add_validator(SignatureValidator::new());

// Get transactions from mempool
let mempool = IpcMempoolReader::new();
let candidates = mempool
    .get_pending_transactions(30_000_000, 5000, U256::from(1_000_000_000))
    .await?;

// Validate and select
validator.validate_batch(&candidates)?;
let selected = selector.select_transactions(candidates, 30_000_000)?;

// Build block template
let template = BlockTemplate {
    header: BlockHeader { /* ... */ },
    transactions: selected.iter().map(|tx| tx.transaction.clone()).collect(),
    total_fees: selected.iter().map(|tx| tx.gas_price * tx.gas_limit).sum(),
};

// Submit to consensus
let submitter = IpcConsensusSubmitter::new();
let receipt = submitter.submit_block(&template, None, None, None).await?;
```

### Custom Transaction Selector

```rust
struct CustomSelector;

impl TransactionSelector for CustomSelector {
    fn select_transactions(
        &self,
        candidates: Vec<TransactionCandidate>,
        gas_limit: u64,
    ) -> Result<Vec<TransactionCandidate>> {
        // Custom selection logic
        let mut selected = Vec::new();
        let mut total_gas = 0u64;
        
        for tx in candidates {
            if total_gas + tx.gas_limit <= gas_limit {
                total_gas += tx.gas_limit;
                selected.push(tx);
            }
        }
        
        Ok(selected)
    }
}
```

## Configuration

### Environment Variables

```bash
# Gas limits
QC17_MAX_BLOCK_GAS_LIMIT=30000000
QC17_MIN_GAS_PRICE=1000000000

# IPC timeouts (milliseconds)
QC17_MEMPOOL_TIMEOUT=2000
QC17_STATE_TIMEOUT=5000
QC17_CONSENSUS_TIMEOUT=3000

# Rate limits
QC17_MAX_REQUESTS_PER_MIN=1000

# Logging
QC17_LOG_LEVEL=info
```

### Cargo Features

```toml
[features]
default = []
mock-ipc = []      # Use mock IPC adapters for testing
metrics = []       # Enable Prometheus metrics
tracing = []       # Detailed execution tracing
```

## Monitoring & Metrics

### Key Metrics

- `qc17_blocks_produced_total` - Total blocks produced
- `qc17_transactions_selected` - Transactions included in blocks
- `qc17_selection_duration_seconds` - Time to select transactions
- `qc17_ipc_errors_total` - IPC communication failures
- `qc17_validation_failures_total` - Transaction validation failures

### Health Checks

```rust
pub struct HealthStatus {
    pub mempool_healthy: bool,
    pub state_healthy: bool,
    pub consensus_healthy: bool,
    pub last_block_time: SystemTime,
}
```

## Testing

### Unit Tests

```bash
cargo test --package qc-17-block-production
```

### Integration Tests

```bash
cargo test --package qc-17-block-production --test integration
```

### Benchmarks

```bash
cargo bench --package qc-17-block-production
```

## References

- [SPEC-17: Block Production Specification](../SPECS/SPEC-17.md)
- [IPC-MATRIX: Inter-Process Communication](../IPC-MATRIX.md)
- [Architecture: System Design](../Architecture.md)
