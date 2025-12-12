# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

## SYSTEM ARCHITECT'S MANDATE

You are entering a **strictly governed codebase**. This is not a hobby project. Every architectural decision is intentional. Every rule exists because someone was burned by violating it. **Read this entire document before touching any code.**

---

## NON-NEGOTIABLE ARCHITECTURAL LAWS

These rules are **absolute**. Breaking them corrupts the entire system's security guarantees.

### LAW #1: Subsystem Isolation (Bounded Contexts)

Each subsystem (QC-01 through QC-17) is a **physically isolated Rust crate**. They know nothing about each other's internals.

```rust
// CORRECT: Subsystem defines its own domain entities
// crates/qc-08-consensus/src/domain/block.rs
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<SignedTransaction>,
}

// WRONG: Importing another subsystem's internal types
use qc_02_block_storage::internal::StorageBlock; // FORBIDDEN
```

### LAW #2: Event Bus Only Communication

Subsystems **NEVER** call each other directly. All communication flows through `BlockchainEvent` published to the shared bus.

```rust
// CORRECT: Publish event to trigger other subsystems
// crates/qc-08-consensus/src/service.rs
async fn after_validation(&self, block: ValidatedBlock) -> Result<(), ConsensusError> {
    self.event_bus.publish_block_validated(
        block.hash,
        block.height,
        block.clone(),
        consensus_proof,
        validated_at,
    ).await.map_err(|e| ConsensusError::EventBusError(e))?;
    Ok(())
}

// WRONG: Direct function call to another subsystem
async fn after_validation(&self, block: ValidatedBlock) -> Result<()> {
    self.block_storage.store_block(block).await?; // FORBIDDEN - Direct coupling!
    self.tx_indexer.index_transactions(&block)?;  // FORBIDDEN - Breaks isolation!
    Ok(())
}
```

### LAW #3: Envelope-Only Identity (Zero Trust)

The `AuthenticatedMessage<T>` envelope's `sender_id` is the **SOLE** source of truth for identity. Payloads must NEVER contain identity fields.

```rust
// CORRECT: Use envelope identity only
// crates/shared-types/src/envelope.rs
pub struct AuthenticatedMessage<T> {
    pub version: u16,
    pub sender_id: u8,        // THE ONLY source of truth
    pub recipient_id: u8,
    pub correlation_id: Uuid,
    pub timestamp: u64,
    pub nonce: Uuid,
    pub signature: [u8; 64],
    pub payload: T,           // Payload has NO identity fields
}

// WRONG: Redundant identity in payload
pub struct ValidateBlockRequest {
    pub requester_subsystem: u8,  // FORBIDDEN - duplicates envelope.sender_id
    pub block: Block,
}
```

### LAW #4: Test-Driven Development (TDD)

**No implementation code exists without a failing test first.** Domain logic must be testable in pure isolation.

```rust
// CORRECT: Test before implementation
#[test]
fn test_block_gas_limit_enforcement() {
    let block = Block::new_with_gas(30_000_001); // Over limit
    let result = validate_gas_limit(&block, 30_000_000);
    assert!(matches!(result, Err(ConsensusError::GasLimitExceeded { .. })));
}

// WRONG: Writing implementation without test coverage
pub fn validate_gas_limit(block: &Block, limit: u64) -> ConsensusResult<()> {
    // Implementation exists but no test verifies it
}
```

### LAW #5: Check Recent Commits

Before implementing anything, check git history to understand current patterns:

```bash
git log --oneline -15              # See recent changes
git diff HEAD~5..HEAD -- crates/   # See recent code changes
git show <commit-hash>             # Examine specific commit
```

---

## HEXAGONAL ARCHITECTURE (MANDATORY STRUCTURE)

**Every subsystem MUST follow this directory structure.** No exceptions.

```
crates/qc-XX-subsystem/
├── Cargo.toml
└── src/
    ├── lib.rs                 # Crate root with lint configs
    ├── service.rs             # Main service (orchestrates domain + adapters)
    │
    ├── domain/                # PURE BUSINESS LOGIC - NO I/O ALLOWED
    │   ├── mod.rs             # Re-exports
    │   ├── entities.rs        # Domain entities (Block, Transaction, etc.)
    │   ├── services.rs        # Pure functions (validate_*, compute_*)
    │   └── error.rs           # Domain-specific error types
    │
    ├── ports/                 # INTERFACES (traits) - Dependency Inversion
    │   ├── mod.rs
    │   ├── inbound.rs         # What this subsystem OFFERS (ConsensusApi)
    │   └── outbound.rs        # What this subsystem NEEDS (EventBus, StateReader)
    │
    ├── adapters/              # INFRASTRUCTURE - Implements ports
    │   ├── mod.rs
    │   └── ipc.rs             # Event bus adapter
    │
    └── events/                # Event types published by this subsystem
        └── mod.rs
```

### Domain Layer Rules

The `domain/` folder contains **pure business logic**. It must:

```rust
// CORRECT: Pure domain function - no I/O, no async, easily testable
// crates/qc-08-consensus/src/domain/services.rs
pub fn validate_proof_of_work(
    block_hash: &Hash,
    nonce: u64,
    difficulty_target: U256,
) -> Result<bool, ConsensusError> {
    let hash_value = compute_pow_hash(block_hash, nonce);
    Ok(U256::from_big_endian(&hash_value) <= difficulty_target)
}

// WRONG: Domain logic with I/O (async, database, network)
pub async fn validate_block(&self, block: Block) -> Result<bool> {
    let previous = self.db.get_block(block.parent_hash).await?; // I/O IN DOMAIN!
    // ...
}
```

### Ports Layer Rules

Ports are **traits** that define interfaces. Adapters implement these traits.

```rust
// CORRECT: Outbound port as trait
// crates/qc-08-consensus/src/ports/outbound.rs
#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish_block_validated(
        &self,
        block_hash: Hash,
        block_height: u64,
        block: ValidatedBlock,
        consensus_proof: ValidationProof,
        validated_at: u64,
    ) -> Result<(), String>;
}

// CORRECT: Inbound port as trait
// crates/qc-08-consensus/src/ports/inbound.rs
#[async_trait]
pub trait ConsensusApi: Send + Sync {
    async fn validate_block(
        &self,
        block: Block,
        source_peer: Option<[u8; 32]>,
    ) -> Result<ValidatedBlock, ConsensusError>;
}
```

---

## RUST CODE STANDARDS (ENFORCED)

### Lint Configuration

Every subsystem's `lib.rs` MUST have:

```rust
// crates/qc-XX-subsystem/src/lib.rs
#![warn(missing_docs)]
#![warn(clippy::all)]
#![deny(unsafe_code)]  // No unsafe unless absolutely necessary

// Test-only relaxations
#![cfg_attr(test, allow(clippy::unwrap_used))]
#![cfg_attr(test, allow(clippy::expect_used))]
#![cfg_attr(test, allow(clippy::panic))]
```

### Error Handling

Use `thiserror` for domain errors. Be explicit about failure modes.

```rust
// CORRECT: Exhaustive error types with context
// crates/qc-08-consensus/src/domain/error.rs
#[derive(Debug, thiserror::Error)]
pub enum ConsensusError {
    #[error("Invalid block height: expected {expected}, got {actual}")]
    InvalidHeight { expected: u64, actual: u64 },

    #[error("Insufficient attestations: {got}%, required {required}%")]
    InsufficientAttestations { got: u8, required: u8 },

    #[error("Duplicate vote from validator: {0:?}")]
    DuplicateVote(ValidatorId),
}

// WRONG: Stringly-typed errors
fn validate() -> Result<(), String> { // NEVER use String as error type
    Err("validation failed".into())
}
```

### Type Safety

```rust
// CORRECT: Newtype pattern for domain types
pub struct BlockHeight(u64);
pub struct ValidatorId([u8; 32]);
pub struct Hash([u8; 32]);

// WRONG: Primitive obsession
fn get_block(height: u64, validator: &[u8], hash: &[u8]) -> Block { ... }
```

---

## EVENT-DRIVEN CHOREOGRAPHY (V2.3)

The system uses **choreography**, not orchestration. Each subsystem reacts to events independently.

### Event Flow: Block Lifecycle

```
QC-17 (Mining)                    QC-08 (Consensus)                   QC-02/03/04
     │                                  │                                   │
     │──BlockProducedEvent──────────────▶│                                   │
     │                                  │                                   │
     │                                  │──BlockValidated────────────────────▶│
     │                                  │        (triggers parallel work)    │
     │                                  │                                   │
     │                                  │◀──MerkleRootComputed (QC-03)────────│
     │                                  │◀──StateRootComputed (QC-04)─────────│
     │                                  │◀──BlockStored (QC-02)───────────────│
```

### Subscribing to Events

```rust
// CORRECT: Subscribe to specific topics
// crates/qc-02-block-storage/src/service.rs
let filter = EventFilter::topics(vec![EventTopic::Consensus]);
let mut stream = event_bus.subscribe(filter).await?;

while let Some(event) = stream.next().await {
    match event {
        BlockchainEvent::BlockValidated(block) => {
            self.store_block(block).await?;
        }
        _ => {} // Ignore events we don't care about
    }
}
```

---

## SECURITY MODEL (IPC-MATRIX.md)

### Zero-Trust Signature Re-Verification

Even if Subsystem 10 (Signature Verification) says a signature is valid, Consensus **RE-VERIFIES** all signatures independently.

```rust
// From crates/qc-08-consensus/src/lib.rs:30-38
// Zero-Trust Signature Re-Verification (CRITICAL)
//
// Per IPC-MATRIX.md, Consensus MUST NOT trust pre-validation flags from
// Subsystem 10. All signatures are independently re-verified because
// if Subsystem 10 is compromised, attackers could inject fake attestations.
```

### Message Authentication

All IPC messages use `AuthenticatedMessage<T>` with:

- **Timestamp validation**: `now - 60s <= timestamp <= now + 10s`
- **Nonce cache**: Prevents replay attacks (120s TTL)
- **Ed25519 signature**: Over serialized header + payload

```rust
// Verification result types
pub enum VerificationResult {
    Valid,
    UnsupportedVersion { received: u16, supported: u16 },
    TimestampOutOfRange { timestamp: u64, now: u64 },
    ReplayDetected { nonce: Uuid },
    InvalidSignature,
    ReplyToMismatch { reply_to_subsystem: u8, sender_id: u8 },
}
```

---

## COMMON COMMANDS

### Building

```bash
cargo build                           # Development build
cargo build --release                 # Optimized build
cargo build --release --features rocksdb  # With production storage
cargo build --features "qc-02,qc-08,qc-17" # Minimal node
```

### Testing

```bash
cargo test --all                      # Run all tests
cargo test -p qc-08-consensus         # Test specific subsystem
cargo test -p qc-08-consensus domain::  # Test domain module only
cargo test --all -- --nocapture       # With output
```

### Docker Development

```bash
cargo build --release && docker compose -f docker-compose.dev.yml up   # Dev mode
docker compose -f docker-compose.dev.yml --profile monitoring up        # With Grafana
docker compose -f docker-compose.dev.yml --profile gpu-nvidia up        # GPU mining
```

### Monitoring

```bash
./tools/event-flow-logger.sh          # Watch event flow
./tools/quantum-flow-monitor.sh       # Advanced monitoring
docker logs -f qc-dev-node            # Docker logs
```

### Frontend (Controls Panel)

```bash
cd controls && npm run dev            # Development server
cd controls && npm run build          # Production build
```

---

## SUBSYSTEM REGISTRY

| ID | Crate | Status | Purpose |
|----|-------|--------|---------|
| QC-01 | `qc-01-peer-discovery` | Active | Kademlia DHT, bootstrap |
| QC-02 | `qc-02-block-storage` | Active | RocksDB persistence |
| QC-03 | `qc-03-transaction-indexing` | Active | Merkle trees |
| QC-04 | `qc-04-state-management` | Active | Account state |
| QC-05 | `qc-05-block-propagation` | Active | Gossip protocol |
| QC-06 | `qc-06-mempool` | Active | Transaction pool |
| QC-07 | `qc-07-bloom-filters` | Active | SPV support |
| QC-08 | `qc-08-consensus` | Active | PoW/PoS validation |
| QC-09 | `qc-09-finality` | Active | Checkpoints |
| QC-10 | `qc-10-signature-verification` | Active | ECDSA/BLS |
| QC-11 | `qc-11-smart-contracts` | Planned | EVM execution |
| QC-12 | `qc-12-transaction-ordering` | Planned | MEV protection |
| QC-13 | `qc-13-light-client-sync` | Planned | SPV proofs |
| QC-14 | `qc-14-sharding` | Planned | Cross-shard |
| QC-15 | `qc-15-cross-chain` | Planned | IBC bridges |
| QC-16 | `qc-16-api-gateway` | Active | JSON-RPC/WebSocket |
| QC-17 | `qc-17-block-production` | Active | PoW mining |

### Shared Crates

| Crate | Purpose |
|-------|---------|
| `shared-types` | Domain entities, `AuthenticatedMessage<T>`, `Subsystem` trait |
| `shared-bus` | Event bus, `BlockchainEvent`, choreography |
| `shared-crypto` | Cryptographic primitives |
| `qc-compute` | GPU-accelerated compute (OpenCL) |
| `quantum-telemetry` | LGTM stack integration |

---

## ADDING A NEW SUBSYSTEM

1. **Create crate** following hexagonal structure:
```bash
cargo new --lib crates/qc-XX-my-subsystem
mkdir -p crates/qc-XX-my-subsystem/src/{domain,ports,adapters,events}
```

2. **Implement `Subsystem` trait** (mandatory for plug-and-play):
```rust
use shared_types::{Subsystem, SubsystemId, SubsystemStatus, SubsystemError};

#[async_trait]
impl Subsystem for MySubsystem {
    fn id(&self) -> SubsystemId { SubsystemId::MySubsystem }
    fn name(&self) -> &'static str { "My Subsystem" }

    async fn start(&self) -> Result<(), SubsystemError> {
        // 1. Validate config
        // 2. Subscribe to events
        // 3. Start background tasks
        Ok(())
    }

    async fn stop(&self) -> Result<(), SubsystemError> { Ok(()) }
    async fn health_check(&self) -> SubsystemStatus { SubsystemStatus::Healthy }
}
```

3. **Register in workspace** (`Cargo.toml`):
```toml
[workspace]
members = ["crates/qc-XX-my-subsystem"]
```

4. **Add feature flag** (`crates/node-runtime/Cargo.toml`):
```toml
[features]
qc-XX = ["dep:qc-XX-my-subsystem"]
```

5. **Update IPC-MATRIX.md** with allowed message types

---

## DOCUMENTATION REFERENCE

| Document | Purpose |
|----------|---------|
| `Documentation/Architecture.md` | Architectural patterns (DDD + Hexagonal + EDA) |
| `Documentation/IPC-MATRIX.md` | Security boundaries, allowed message types |
| `Documentation/System.md` | Subsystem specifications |
| `Documentation/DATA-ARCHITECTURE.md` | RocksDB storage design |

---

## FINAL WARNING

If you find yourself doing any of these, **STOP and reconsider**:

- Importing internal types from another subsystem crate
- Adding async I/O to the `domain/` layer
- Using `String` as an error type
- Writing implementation before tests
- Calling subsystem methods directly instead of publishing events
- Adding identity fields to IPC payloads

**The architecture is the product.** Violating it doesn't just create tech debt - it compromises the security model that protects billions in potential value.
