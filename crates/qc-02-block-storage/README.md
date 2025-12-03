# qc-02-block-storage

**Block Storage Engine** - Authoritative persistence layer for blockchain data.

[![Crate](https://img.shields.io/badge/crate-qc--02--block--storage-blue.svg)](https://github.com/NerfedChou/Quantum-Chain)
[![Specification](https://img.shields.io/badge/spec-SPEC--02-green.svg)](../../SPECS/SPEC-02-BLOCK-STORAGE.md)
[![Architecture](https://img.shields.io/badge/arch-V2.3-orange.svg)](../../Documentation/Architecture.md)

---

## Overview

The Block Storage subsystem (ID: 2) is the **authoritative persistence layer** for all blockchain data. It implements the V2.3 **Stateful Assembler** pattern - receiving events from multiple subsystems and assembling them into complete blocks.

### Key Responsibilities

| Responsibility | Description |
|----------------|-------------|
| **Block Persistence** | Store validated blocks with merkle/state roots |
| **Sequential Chain** | Enforce parent-child block relationships |
| **Finalization Tracking** | Mark blocks as finalized (irreversible) |
| **Transaction Indexing** | Map transaction hashes to block locations |
| **Data Integrity** | Checksum verification on every read |

---

## Architecture

### V2.3 Choreography Pattern (Stateful Assembler)

Block Storage does NOT receive a pre-assembled package. Instead, it subscribes to THREE independent event streams:

```
Consensus (8) ────BlockValidated────→ ┐
                                       │
Tx Indexing (3) ──MerkleRootComputed──→├──→ Block Storage (2)
                                       │    [Buffers by block_hash]
State Mgmt (4) ───StateRootComputed───→┘
                                       ↓
                            [Atomic Write when all 3 present]
```

### Hexagonal Structure

```
src/
├── lib.rs                  # Public API and re-exports
├── domain/                 # Inner layer (pure logic)
│   ├── entities.rs         # StoredBlock, BlockIndex
│   ├── value_objects.rs    # StorageConfig, KeyPrefix
│   ├── assembler.rs        # BlockAssemblyBuffer (Stateful Assembler)
│   └── errors.rs           # StorageError enum
├── ports/                  # Middle layer (traits)
│   ├── inbound.rs          # BlockStorageApi trait
│   └── outbound.rs         # KeyValueStore, FileSystemAdapter traits
├── service.rs              # Application service
└── ipc/                    # IPC layer (security boundary)
    ├── envelope.rs         # AuthenticatedMessage validation
    ├── payloads.rs         # IPC payload types
    └── handlers.rs         # BlockStorageHandler
```

---

## Domain Invariants

| ID | Invariant | Enforcement |
|----|-----------|-------------|
| 1 | **Sequential Blocks** | Parent must exist for height > 0 |
| 2 | **Disk Space Safety** | Writes fail if disk < 5% available |
| 3 | **Data Integrity** | Checksum verified on every read |
| 4 | **Atomic Writes** | Batch operations - all or nothing |
| 5 | **Finalization Monotonicity** | Finalized height cannot decrease |
| 6 | **Genesis Immutability** | Genesis hash never changes |
| 7 | **Assembly Timeout** | Incomplete assemblies purged after 30s |
| 8 | **Bounded Buffer** | Max 1000 pending assemblies |

---

## IPC Authorization (per IPC-MATRIX.md)

| Event/Request | Authorized Sender |
|---------------|-------------------|
| `BlockValidated` | Consensus (8) |
| `MerkleRootComputed` | Transaction Indexing (3) |
| `StateRootComputed` | State Management (4) |
| `MarkFinalized` | Finality (9) |
| `ReadBlock` | Any authorized subsystem |
| `GetTransactionLocation` | Transaction Indexing (3) |

---

## Usage

### Basic Usage (Direct Service)

```rust
use qc_02_block_storage::{
    BlockStorageService, StorageConfig,
    domain::entities::ValidatedBlock,
};

// Create service with in-memory adapters (for testing)
let config = StorageConfig::default();
let service = BlockStorageService::new_in_memory(config);

// Write a block
let block = create_validated_block();
let merkle_root = compute_merkle_root(&block);
let state_root = compute_state_root(&block);

let hash = service.write_block(block, merkle_root, state_root)?;

// Read a block
let stored = service.read_block(&hash)?;
assert_eq!(stored.merkle_root, merkle_root);
```

### IPC Handler Usage (Security Boundary)

```rust
use qc_02_block_storage::{
    BlockStorageHandler, AuthenticatedMessage,
    BlockValidatedPayload, EnvelopeValidator,
};

// Create handler with security validator
let service = BlockStorageService::new_in_memory(StorageConfig::default());
let validator = EnvelopeValidator::new([0u8; 32]); // HMAC key
let mut handler = BlockStorageHandler::new(service, validator);

// Handle authenticated message
let msg: AuthenticatedMessage<BlockValidatedPayload> = receive_message();
let result = handler.handle_block_validated(msg)?;

if let Some(block_stored) = result {
    // Assembly complete - block was written
    publish_block_stored_event(block_stored);
}
```

### Event Bus Integration (Phase 7)

```rust
use qc_02_block_storage::bus::BlockStorageBusAdapter;
use shared_bus::EventBus;

// Create adapter
let bus = EventBus::new();
let adapter = BlockStorageBusAdapter::new(handler, bus.clone());

// Subscribe to events
adapter.subscribe("BlockValidated", subsystem_ids::CONSENSUS);
adapter.subscribe("MerkleRootComputed", subsystem_ids::TRANSACTION_INDEXING);
adapter.subscribe("StateRootComputed", subsystem_ids::STATE_MANAGEMENT);

// Run event loop
adapter.run().await?;
```

---

## Testing

```bash
# Run all unit tests
cargo test -p qc-02-block-storage

# Run with verbose output
cargo test -p qc-02-block-storage -- --nocapture

# Run specific test
cargo test -p qc-02-block-storage test_assembly_completes_with_all_three
```

### Test Coverage

| Category | Tests | Status |
|----------|-------|--------|
| Domain Logic | 50 | ✅ |
| IPC Security | 12 | ✅ |
| **Total** | **62** | ✅ |

---

## Configuration

```rust
use qc_02_block_storage::StorageConfig;

let config = StorageConfig {
    // Disk safety
    min_disk_space_percent: 5.0,
    max_block_size: 2 * 1024 * 1024, // 2MB
    
    // Assembly buffer
    assembly_timeout_secs: 30,
    max_pending_assemblies: 1000,
    
    // Batch read
    max_batch_size: 100,
};
```

---

## Related Documents

| Document | Description |
|----------|-------------|
| [SPEC-02-BLOCK-STORAGE.md](../../SPECS/SPEC-02-BLOCK-STORAGE.md) | Full specification |
| [Architecture.md](../../Documentation/Architecture.md) | System architecture |
| [IPC-MATRIX.md](../../Documentation/IPC-MATRIX.md) | IPC authorization rules |
| [System.md](../../Documentation/System.md) | Subsystem definitions |

---

## License

[Unlicense](../../LICENSE)
