# QC-06 Mempool (Transaction Pool)

**Subsystem ID:** 6  
**Specification:** `SPECS/SPEC-06-MEMPOOL.md` v2.3  
**Status:** ✅ Phase 1-5 Complete (76 tests)

## Overview

The Mempool subsystem manages unconfirmed transactions awaiting inclusion in blocks. It implements a **Two-Phase Commit** protocol to prevent transaction loss during block storage failures.

## Key Features

- **Priority Queue**: Transactions ordered by gas price (highest first)
- **Nonce Ordering**: Per-account transaction ordering by nonce
- **Two-Phase Commit**: Safe transaction removal with rollback capability
- **Replace-by-Fee (RBF)**: Transaction replacement with fee bump
- **Eviction**: LRU eviction when pool is full
- **Security**: IPC authorization per IPC-MATRIX.md

## Two-Phase Commit Protocol

```
[PENDING] ──propose──→ [PENDING_INCLUSION] ──confirm──→ [DELETED]
                              │
                              └── timeout/reject ──→ [PENDING] (rollback)
```

### Why Two-Phase Commit?

- Prevents **Transaction Loss**: Transactions are never deleted until block storage confirms
- Enables **Atomicity**: Block building and storage are atomic operations
- Supports **Rollback**: Failed blocks return transactions to the pool

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        qc-06-mempool                             │
├─────────────────────────────────────────────────────────────────┤
│  domain/                  │ Pure business logic                  │
│  ├── entities.rs         │ MempoolTransaction, TransactionState │
│  ├── pool.rs             │ TransactionPool (priority queue)     │
│  ├── value_objects.rs    │ PricedTransaction, ShortTxId         │
│  ├── services.rs         │ RBF calculation, nonce validation    │
│  └── errors.rs           │ MempoolError enum                    │
├───────────────────────────┼──────────────────────────────────────┤
│  ports/                   │ Hexagonal architecture boundaries    │
│  ├── inbound.rs          │ MempoolApi trait (driving port)      │
│  └── outbound.rs         │ StateProvider, TimeSource (driven)   │
├───────────────────────────┼──────────────────────────────────────┤
│  ipc/                     │ Inter-subsystem communication        │
│  ├── payloads.rs         │ Request/Response message types       │
│  ├── security.rs         │ Authorization rules per IPC-MATRIX   │
│  └── handler.rs          │ IPC message handler                  │
└─────────────────────────────────────────────────────────────────┘
```

## Security (IPC-MATRIX.md)

| Message Type | Authorized Sender |
|--------------|-------------------|
| `AddTransactionRequest` | Subsystem 10 (Signature Verification) ONLY |
| `GetTransactionsRequest` | Subsystem 8 (Consensus) ONLY |
| `BlockStorageConfirmation` | Subsystem 2 (Block Storage) ONLY |
| `BlockRejectedNotification` | Subsystems 2, 8 |

## Usage

```rust
use qc_06_mempool::domain::{TransactionPool, MempoolConfig, MempoolTransaction};

// Create pool
let mut pool = TransactionPool::new(MempoolConfig::default());

// Add transaction
let tx = MempoolTransaction::new(
    [0xAA; 32],     // hash
    [0xBB; 32],     // sender
    0,              // nonce
    1_000_000_000,  // gas_price (1 gwei)
    21000,          // gas_limit
    0,              // value
    vec![],         // data
    1000,           // added_at
);
pool.add(tx).unwrap();

// Get transactions for block (Phase 1)
let batch = pool.get_for_block(100, 30_000_000);
let hashes: Vec<_> = batch.iter().map(|t| t.hash).collect();
pool.propose(&hashes, 1, 2000);

// Confirm inclusion (Phase 2a)
pool.confirm(&hashes);
```

## Configuration

```rust
MempoolConfig {
    max_transactions: 5000,           // Max pool size
    max_per_account: 16,              // Max per sender
    min_gas_price: 1_000_000_000,     // 1 gwei minimum
    max_gas_per_tx: 30_000_000,       // 30M gas limit
    pending_inclusion_timeout_ms: 30_000,  // 30s timeout
    nonce_gap_timeout_ms: 600_000,    // 10 min nonce gap
    enable_rbf: true,                 // RBF enabled
    rbf_min_bump_percent: 10,         // 10% minimum bump
}
```

## Dependencies

| Subsystem | Type | Purpose |
|-----------|------|---------|
| 10 (Signature Verification) | Receives from | Pre-verified transactions |
| 8 (Consensus) | Receives from | GetTransactionsRequest |
| 2 (Block Storage) | Receives from | BlockStorageConfirmation |
| 4 (State Management) | Queries | Balance/nonce validation |
| 8 (Consensus) | Sends to | ProposeTransactionBatch |

## Test Coverage

- **Domain Tests**: 51 tests for core logic
- **IPC Tests**: 25 tests for security and message handling
- **Total**: 76 tests passing

```bash
cargo test -p qc-06-mempool
```

## License

MIT
