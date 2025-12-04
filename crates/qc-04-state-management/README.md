# qc-04-state-management

State Management subsystem for Quantum-Chain.

## Overview

This subsystem maintains the authoritative current state of all accounts and smart contract storage on the blockchain. It provides:

- **Patricia Merkle Trie**: Efficient state lookups with O(log n) complexity
- **Cryptographic Proofs**: Verifiable state proofs for light clients
- **State Root Computation**: Computes state root for each validated block

## Role in Architecture (V2.3 Choreography)

This subsystem is a **choreography participant**, NOT an orchestration target.

```
[Consensus (8)] ──BlockValidated──→ [Event Bus]
                                        │
                    ┌───────────────────┼───────────────────┐
                    ↓                   ↓                   ↓
           [Tx Indexing (3)]  [State Management (4)]  [Block Storage (2)]
                    │                   │              (Assembler)
                    ↓                   ↓                   ↑
           MerkleRootComputed   StateRootComputed           │
                    │                   │                   │
                    └──────→ [Event Bus] ←──────────────────┘
```

## IPC Authorization Matrix

| Message Type | Authorized Sender(s) | Purpose |
|--------------|---------------------|---------|
| `BlockValidatedEvent` | Subsystem 8 (Consensus) via Event Bus | Trigger state computation |
| `StateReadRequest` | Subsystems 6, 11, 12, 14 | Provide state data |
| `StateWriteRequest` | Subsystem 11 ONLY | Apply state changes |
| `BalanceCheckRequest` | Subsystem 6 ONLY | Validate tx balance |
| `ConflictDetectionRequest` | Subsystem 12 ONLY | Detect tx conflicts |

## Security

- **Centralized Security**: Uses `MessageVerifier` from `shared-types` crate
- **Envelope-Only Identity**: Identity derived solely from `AuthenticatedMessage.sender_id`
- **Strict Authorization**: Only authorized subsystems can access specific operations
- **Invariant Protection**:
  - Balance non-negativity (no underflow)
  - Strict nonce monotonicity (no gaps, no decrements)
  - Deterministic state root computation
  - Atomic state transitions

## Key Components

### Domain Layer

- `AccountState`: Balance, nonce, code hash, storage root
- `PatriciaMerkleTrie`: Main state trie structure
- `StateProof` / `StorageProof`: Cryptographic proofs for verification
- `ConflictInfo`: Transaction conflict detection for parallel execution

### Ports

- `StateManagementApi`: Primary API for state operations
- `TrieDatabase`: Abstraction for trie node storage
- `SnapshotStorage`: Abstraction for state snapshots

### IPC Handler

Uses the centralized `MessageVerifier` to validate all incoming messages:
- HMAC signature verification
- Timestamp validation (60-second window)
- Nonce replay prevention
- Sender authorization per IPC-MATRIX.md

## Usage

```rust
use qc_04_state_management::{IpcHandler, StaticKeyProvider, NonceCache};
use std::sync::Arc;

// Create handler with shared security
let nonce_cache = NonceCache::new_shared();
let key_provider = StaticKeyProvider::new(&[0x42; 32]);
let handler = IpcHandler::new(nonce_cache, key_provider);

// Handle BlockValidated event (from Consensus)
let result = handler.handle_block_validated(&msg, &msg_bytes)?;

// Handle balance check (from Mempool only)
let response = handler.handle_balance_check(&msg, &msg_bytes)?;
```

## Testing

```bash
# Run all tests
cargo test -p qc-04-state-management

# Run with verbose output
cargo test -p qc-04-state-management -- --nocapture
```

## Performance Targets

- State root computation: < 10 seconds (normal load)
- Proof generation: < 100ms per account
- Concurrent state reads: 1000+ supported

## References

- [SPEC-04-STATE-MANAGEMENT.md](../../SPECS/SPEC-04-STATE-MANAGEMENT.md)
- [Architecture.md](../../Documentation/Architecture.md) - Section 5.1 (Choreography)
- [IPC-MATRIX.md](../../Documentation/IPC-MATRIX.md) - Subsystem 4 section
- [System.md](../../Documentation/System.md) - Subsystem 4 algorithms
