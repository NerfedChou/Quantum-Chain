# Subsystem 03: Transaction Indexing

**Crate:** `qc-03-transaction-indexing`  
**Specification:** `SPECS/SPEC-03-TRANSACTION-INDEXING.md` v2.4  
**Architecture Compliance:** Architecture.md v2.2 (Hexagonal, DDD, EDA)

## Overview

This crate implements the Merkle tree construction and proof generation for transaction indexing in Quantum-Chain. It provides:

- **Merkle Tree Construction:** Power-of-two padded binary trees for O(log n) proofs
- **Proof Generation:** Cryptographic proofs for transaction inclusion
- **Proof Verification:** Validate proofs against known roots
- **Caching:** LRU cache for frequently accessed Merkle trees

## Architecture

The crate follows **Hexagonal Architecture** with strict layer separation:

```
┌─────────────────────────────────────────────────────────────────┐
│                        Adapters Layer                           │
│  ┌──────────────────┐  ┌──────────────────┐                    │
│  │ Event Publisher  │  │ Event Subscriber │  (shared-bus)      │
│  └──────────────────┘  └──────────────────┘                    │
├─────────────────────────────────────────────────────────────────┤
│                        IPC Layer                                │
│  ┌──────────────────┐  ┌──────────────────┐                    │
│  │     Payloads     │  │     Security     │  (per IPC-MATRIX)  │
│  └──────────────────┘  └──────────────────┘                    │
├─────────────────────────────────────────────────────────────────┤
│                        Service Layer                            │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │           TransactionIndexingService                      │  │
│  │           (implements TransactionIndexingApi)             │  │
│  └──────────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│                        Ports Layer                              │
│  ┌──────────────────┐  ┌──────────────────┐                    │
│  │TransactionIndex  │  │  TreeCache       │  (traits)          │
│  │ Api (Driving)    │  │  (Driven Port)   │                    │
│  └──────────────────┘  └──────────────────┘                    │
├─────────────────────────────────────────────────────────────────┤
│                        Domain Layer                             │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────┐ │
│  │   MerkleTree     │  │   MerkleProof    │  │  MerkleNode  │ │
│  │   (pure logic)   │  │   (with verify)  │  │  (hash calc) │ │
│  └──────────────────┘  └──────────────────┘  └──────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## Core Invariants

| Invariant | Description | Defense |
|-----------|-------------|---------|
| **INVARIANT-1** | Power-of-two leaf padding | Empty hash padding for non-power-of-two |
| **INVARIANT-2** | All proofs MUST verify | Proof generation guarantees validity |
| **INVARIANT-3** | Deterministic serialization | Canonical byte ordering |
| **INVARIANT-4** | Index consistency | Cached root = computed root |
| **INVARIANT-5** | Bounded cache | LRU eviction prevents memory exhaustion |

## Choreography Role

Transaction Indexing is a **Choreography Participant** in the V2.2 pattern:

```
              ┌─────────────────┐
              │  Consensus (8)  │
              └────────┬────────┘
                       │ BlockValidated
                       ▼
              ┌─────────────────┐
              │  Tx Indexing    │──────────────────┐
              │     (3)         │                  │
              └────────┬────────┘                  │
                       │ MerkleRootComputed        │ ProofRequest/Response
                       ▼                           ▼
              ┌─────────────────┐         ┌───────────────┐
              │ Block Storage   │         │ Light Clients │
              │     (2)         │         │     (13)      │
              └─────────────────┘         └───────────────┘
```

## IPC Integration

### Subscribed Events (per IPC-MATRIX.md)

| Event | From Subsystem | Handler |
|-------|----------------|---------|
| `BlockValidated` | 8 (Consensus) | Build Merkle tree, publish root |

### Published Events

| Event | To Subsystem | Trigger |
|-------|--------------|---------|
| `MerkleRootComputed` | 2 (Block Storage) | After tree construction |

### Request/Response

| Request | Allowed Subsystems | Response |
|---------|-------------------|----------|
| `ProofRequest` | 13 (Light Clients) | `MerkleProof` |

## Security Features

### 1. IPC Authentication (V2.4 Patch)

All IPC messages require HMAC signature and nonce validation:

```rust
// Envelope structure with security fields
pub struct SecureEnvelope<T> {
    pub sender_id: SubsystemId,
    pub timestamp: u64,
    pub nonce: u64,
    pub signature: [u8; 32],
    pub payload: T,
}
```

### 2. Replay Attack Prevention

Monotonic nonce tracking per sender:

```rust
impl NonceCache {
    pub fn validate_and_record(&mut self, sender: SubsystemId, nonce: u64) -> bool {
        // Reject if nonce <= last seen nonce
    }
}
```

### 3. Memory Bound (INVARIANT-5)

LRU cache with configurable max entries:

```rust
pub struct TreeCache {
    max_entries: usize,  // Default: 1024
    // Oldest entries evicted when full
}
```

## Usage

### Basic Usage

```rust
use qc_03_transaction_indexing::{
    MerkleTree, MerkleProof, Hash256,
};

// Create tree from transaction hashes
let tx_hashes = vec![hash1, hash2, hash3];
let tree = MerkleTree::from_leaves(tx_hashes);

// Get the root
let root = tree.root();

// Generate proof for tx at index 1
let proof = tree.generate_proof(1)?;

// Verify proof
assert!(proof.verify(&hash2, &root));
```

### Service Usage

```rust
use qc_03_transaction_indexing::{
    TransactionIndexingService, TransactionIndexingApi,
};

let service = TransactionIndexingService::new(config);

// Handle BlockValidated event
let root = service.on_block_validated(&block)?;

// Generate proof for light client
let proof = service.get_proof(block_hash, tx_index)?;
```

## Configuration

```rust
pub struct IndexingConfig {
    pub cache_max_entries: usize,     // Default: 1024
    pub cache_ttl_secs: u64,          // Default: 3600
    pub max_txs_per_block: usize,     // Default: 10000
}
```

## Testing

Run all tests:

```bash
cargo test -p qc-03-transaction-indexing
```

Run with verbose output:

```bash
cargo test -p qc-03-transaction-indexing -- --nocapture
```

## Test Coverage

| Test Group | Description | Count |
|------------|-------------|-------|
| Merkle Tree | Construction, padding, root | 8 |
| Proof Generation | Valid proofs, edge cases | 6 |
| Proof Verification | Valid/invalid proofs | 8 |
| Cache | LRU eviction, bounds | 5 |
| IPC Security | Auth, replay prevention | 10+ |

## Dependencies

The domain layer has **zero external dependencies** - pure Rust only.

```toml
[dependencies]
shared-types = { path = "../shared-types" }  # Hash256, SubsystemId

[dev-dependencies]
# Test utilities added as needed
```

## Related Documentation

- `SPECS/SPEC-03-TRANSACTION-INDEXING.md` - Full specification
- `Documentation/Architecture.md` - System architecture
- `Documentation/IPC-MATRIX.md` - IPC authorization matrix
- `Documentation/System.md` - Overall system design

## License

See repository LICENSE file.
