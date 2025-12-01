# ARCHITECTURE.md
## Modular Blockchain - Hybrid Architecture Specification

**Version:** 1.0  
**Project:** Rust-based Modular Blockchain  
**Enforcement Level:** STRICT / ZERO TOLERANCE  
**Architecture Patterns:** DDD + Hexagonal + EDA + TDD

---

## TABLE OF CONTENTS

1. [Executive Summary](#1-executive-summary)
2. [Architectural Principles](#2-architectural-principles)
3. [System Topology](#3-system-topology)
4. [Subsystem Catalog](#4-subsystem-catalog)
5. [Communication Protocol](#5-communication-protocol)
6. [Development Workflow](#6-development-workflow)
7. [Security Architecture](#7-security-architecture)
8. [Testing Strategy](#8-testing-strategy)
9. [Deployment Model](#9-deployment-model)

---

## 1. EXECUTIVE SUMMARY

### 1.1 Vision

This blockchain system is architected as a **fortress of isolated subsystems**, each representing a distinct business capability (Bounded Context). The system achieves:

- **Modularity:** Each subsystem is a standalone Rust library crate
- **Security:** Compartmentalized design prevents cascade failures
- **Maintainability:** Pure domain logic separated from infrastructure
- **Testability:** Test-driven development enforced at every layer

### 1.2 Core Architecture Decision

We employ a **Hybrid Architecture** combining:

1. **Domain-Driven Design (DDD)** - Business logic as first-class citizens
2. **Hexagonal Architecture** - Dependency inversion via Ports & Adapters
3. **Event-Driven Architecture (EDA)** - Asynchronous, decoupled communication
4. **Test-Driven Development (TDD)** - Design validated by tests first

### 1.3 Key Constraints

```
RULE #1: Libraries have ZERO knowledge of the binary/CLI/Docker
RULE #2: Direct subsystem-to-subsystem calls are FORBIDDEN
RULE #3: Implementation code CANNOT be written without tests first
RULE #4: All inter-subsystem communication via Shared Bus ONLY
```

---

## 2. ARCHITECTURAL PRINCIPLES

### 2.1 Domain-Driven Design (DDD)

#### Bounded Contexts = Physical Crates

**Principle:** Each business capability is isolated into its own Rust crate.

```
crates/
├── peer-discovery/          # Subsystem 1
├── block-storage/           # Subsystem 2
├── transaction-indexing/    # Subsystem 3
├── state-management/        # Subsystem 4
├── block-propagation/       # Subsystem 5
├── mempool/                 # Subsystem 6
├── bloom-filters/           # Subsystem 7
├── consensus/               # Subsystem 8
├── finality/                # Subsystem 9
├── signature-verification/  # Subsystem 10
├── smart-contracts/         # Subsystem 11
├── transaction-ordering/    # Subsystem 12 (optional)
├── light-client/            # Subsystem 13 (optional)
├── sharding/                # Subsystem 14 (optional)
├── cross-chain/             # Subsystem 15 (optional)
└── shared-bus/              # Event communication layer
```

#### Ubiquitous Language

**Principle:** Code structure mirrors business language exactly.

```rust
// ✅ CORRECT: Matches business terminology
struct Block { ... }
struct Transaction { ... }
struct Validator { ... }

// ❌ WRONG: Implementation details leak into domain
struct BlockDataSchema { ... }
struct TxRecord { ... }
struct ValidatorNode { ... }
```

**Constraint:** Every struct, enum, and function name must be understandable by a blockchain domain expert without programming knowledge.

---

### 2.2 Hexagonal Architecture (Ports & Adapters)

#### Layer Hierarchy

```
┌─────────────────────────────────────────────────────────┐
│                    OUTER LAYER                          │
│              (Adapters - Infrastructure)                │
│  RocksDB, LibP2P, Tokio, HTTP Server, CLI              │
└─────────────────────────────────────────────────────────┘
                         ↑ depends on ↑
┌─────────────────────────────────────────────────────────┐
│                   MIDDLE LAYER                          │
│               (Ports - Interfaces/Traits)               │
│  trait BlockPersister, trait NetworkSocket              │
└─────────────────────────────────────────────────────────┘
                         ↑ depends on ↑
┌─────────────────────────────────────────────────────────┐
│                    INNER LAYER                          │
│              (Domain - Pure Business Logic)             │
│  struct Block, fn validate_block(), enum State          │
│  NO I/O, NO Async, NO External Dependencies             │
└─────────────────────────────────────────────────────────┘
```

#### Dependency Rule

**CRITICAL:** Dependencies point INWARD ONLY.

```rust
// ✅ ALLOWED: Adapter depends on Port
impl BlockPersister for RocksDBAdapter { ... }

// ✅ ALLOWED: Port defined by Domain
trait BlockPersister {
    fn save(&self, block: Block) -> Result<()>;
}

// ❌ FORBIDDEN: Domain depends on Adapter
struct Block {
    db: RocksDB,  // ← VIOLATION! Domain knows about infrastructure
}
```

#### Port Types

**Driving Ports (API - Inbound):**
- Functions the subsystem exposes to the application
- Example: `fn find_peer(id: PeerId) -> Option<PeerInfo>`

**Driven Ports (SPI - Outbound):**
- Interfaces the subsystem needs from external systems
- Example: `trait Storage { fn write(&self, data: &[u8]) -> Result<()>; }`

---

### 2.3 Event-Driven Architecture (EDA)

#### The Shared Bus Pattern

**Principle:** Subsystems communicate ONLY via asynchronous events through a shared bus.

```rust
// Shared Bus Definition (crates/shared-bus/src/lib.rs)
pub enum BlockchainEvent {
    // From Subsystem 1: Peer Discovery
    PeerDiscovered(PeerInfo),
    PeerDisconnected(PeerId),
    
    // From Subsystem 8: Consensus
    BlockValidated(ValidatedBlock),
    BlockRejected { hash: Hash, reason: String },
    
    // From Subsystem 10: Signature Verification
    TransactionVerified(VerifiedTransaction),
    TransactionInvalid { hash: Hash, reason: String },
}

pub trait EventPublisher {
    fn publish(&self, event: BlockchainEvent);
}

pub trait EventSubscriber {
    fn subscribe(&self, filter: EventFilter) -> EventStream;
}
```

#### Communication Rules

```
┌──────────────┐                    ┌──────────────┐
│ Subsystem A  │                    │ Subsystem B  │
│              │                    │              │
│              │  ❌ FORBIDDEN      │              │
│              │ ────────────────→  │              │
│              │  Direct Call       │              │
└──────────────┘                    └──────────────┘

         ✅ REQUIRED PATTERN

┌──────────────┐                    ┌──────────────┐
│ Subsystem A  │                    │ Subsystem B  │
│              │    publish()       │              │
│              │ ──────┐            │              │
└──────────────┘       │            └──────────────┘
                       ↓                    ↑
                 ┌──────────────┐          │
                 │ Shared Bus   │          │
                 │              │          │
                 │  Event Queue │  subscribe()
                 └──────────────┘
```

#### Benefits

1. **Loose Coupling:** Subsystem A crashes → Subsystem B continues running
2. **Scalability:** Events can be processed in parallel
3. **Auditability:** Every action is an event with timestamp
4. **Replay:** Events can be stored and replayed for debugging

---

### 2.4 Test-Driven Development (TDD)

#### Strict TDD Workflow

**ENFORCEMENT:** The AI/Developer is **FORBIDDEN** from writing implementation code without first writing a failing test.

```
Phase 1: RED
├─ Write a test that fails
├─ Example: test_block_hash_validation_fails_on_empty_data()
└─ Run: cargo test → FAIL

Phase 2: GREEN
├─ Write MINIMUM code to pass the test
├─ No optimization, no extras
└─ Run: cargo test → PASS

Phase 3: REFACTOR
├─ Clean up code while keeping tests green
├─ Extract functions, improve names
└─ Run: cargo test → PASS (still)
```

#### Example TDD Cycle

```rust
// Step 1: RED - Write failing test
#[test]
fn test_block_validation_rejects_invalid_merkle_root() {
    let block = Block {
        merkle_root: [0u8; 32],  // Invalid
        transactions: vec![tx1, tx2],
    };
    
    assert!(validate_block(&block).is_err());  // ← FAILS (no validate_block yet)
}

// Step 2: GREEN - Minimum implementation
pub fn validate_block(block: &Block) -> Result<(), ValidationError> {
    let computed_root = compute_merkle_root(&block.transactions);
    if computed_root != block.merkle_root {
        return Err(ValidationError::InvalidMerkleRoot);
    }
    Ok(())
}

// Step 3: REFACTOR - Extract logic
pub fn validate_block(block: &Block) -> Result<(), ValidationError> {
    verify_merkle_root(block)?;
    verify_state_transitions(block)?;
    verify_signatures(block)?;
    Ok(())
}
```

---

## 3. SYSTEM TOPOLOGY

### 3.1 Subsystem Dependency Graph

**Reference:** See IPC-MATRIX.md for complete message type definitions.

**CRITICAL UPDATE (Atomicity Enforcement):**
Subsystem 2 (Block Storage) no longer depends on Subsystems 3 and 4 directly.
Subsystem 8 (Consensus) is the orchestrator that collects roots from 3 and 4,
then sends a single atomic WriteBlockRequest to Subsystem 2.

```
Dependency Flow (A → B means "A depends on B"):

LEVEL 0 (No Dependencies):
├─ [1] Peer Discovery
└─ [10] Signature Verification

LEVEL 1 (Depends on Level 0):
├─ [1] Peer Discovery → [10] (NEW: DDoS defense - verify node identity at edge)
├─ [6] Mempool → [10]
├─ [7] Bloom Filters → [1]
└─ [13] Light Clients → [1]

LEVEL 2 (Depends on Level 0-1):
├─ [3] Transaction Indexing → [10]
├─ [5] Block Propagation → [1]
└─ [4] State Management (partial)

LEVEL 3 (Depends on Level 0-2):
├─ [8] Consensus → [3, 4, 5, 6, 10] (ORCHESTRATOR: collects merkle_root from 3, state_root from 4)
└─ [11] Smart Contracts → [4, 10]

LEVEL 4 (Depends on Level 0-3):
├─ [2] Block Storage → [8] (UPDATED: receives complete package from Consensus ONLY)
├─ [9] Finality → [8, 10]
├─ [12] Transaction Ordering → [4, 11]
└─ [14] Sharding → [4, 8]

LEVEL 5 (Depends on Level 0-4):
└─ [15] Cross-Chain → [8, 9, 11]
```

**Data Flow for Block Writes (Atomicity Guarantee):**
```
[3] Transaction Indexing ──merkle_root──→ [8] Consensus ──WriteBlockRequest──→ [2] Block Storage
[4] State Management ────state_root────→ [8] Consensus ──(complete package)──→ [2] Block Storage

IMPORTANT: Subsystems 3 and 4 do NOT write directly to Block Storage.
           Consensus assembles the complete package to ensure atomicity.
```

### 3.2 Message Flow Architecture

**Reference:** See IPC Matrix document for complete message type definitions.

**Key Principle:** Every message has:
1. **Version** (protocol version for forward/backward compatibility)
2. **Sender ID** (which subsystem sent it)
3. **Recipient ID** (which subsystem should receive it)
4. **Correlation ID** (unique identifier for request/response mapping)
5. **Reply-To Topic** (where responses should be published)
6. **Payload** (strictly typed struct)
7. **Timestamp** (for replay prevention)
8. **Signature** (HMAC for authentication)

```rust
// Every inter-subsystem message follows this pattern
struct AuthenticatedMessage<T> {
    // === HEADER (MANDATORY) ===
    version: u16,                    // Protocol version - MUST be checked before deserialization
    sender_id: SubsystemId,
    recipient_id: SubsystemId,
    correlation_id: [u8; 16],        // UUID v4 for request/response correlation
    reply_to: Option<Topic>,         // Topic for async response delivery
    timestamp: u64,
    nonce: u64,
    signature: [u8; 32],             // HMAC-SHA256
    
    // === PAYLOAD ===
    payload: T,
}

// Topic definition for reply routing
struct Topic {
    subsystem_id: SubsystemId,
    channel: String,                 // e.g., "responses", "dlq.errors"
}
```

### 3.3 Request/Response Correlation Pattern

**CRITICAL:** All request/response flows MUST use the correlation ID pattern to maintain EDA principles while enabling stateful conversations.

```
┌──────────────┐                         ┌──────────────┐
│ Subsystem A  │                         │ Subsystem B  │
│ (Requester)  │                         │ (Responder)  │
│              │                         │              │
│ 1. Generate  │                         │              │
│    UUID      │                         │              │
│              │    2. Publish Request   │              │
│              │ ─────────────────────→  │              │
│              │    correlation_id: X    │              │
│              │    reply_to: A.responses│              │
│              │                         │              │
│ 3. Continue  │                         │ 4. Process   │
│    other work│                         │    request   │
│    (NON-     │                         │              │
│    BLOCKING) │                         │              │
│              │    5. Publish Response  │              │
│              │ ←─────────────────────  │              │
│              │    correlation_id: X    │              │
│              │    to: A.responses      │              │
│              │                         │              │
│ 6. Match X   │                         │              │
│    to pending│                         │              │
│    request   │                         │              │
└──────────────┘                         └──────────────┘
```

**Rules:**
1. **NEVER BLOCK** - Requester publishes and immediately continues processing
2. **ALWAYS CORRELATE** - Every response MUST include the original `correlation_id`
3. **TIMEOUT HANDLING** - Requester MUST implement timeout for pending requests (default: 30s)
4. **ORPHAN CLEANUP** - Pending request map MUST be garbage-collected periodically

```rust
// Requester implementation pattern
impl Subsystem {
    async fn request_peer_list(&self) -> Result<(), Error> {
        let correlation_id = Uuid::new_v4();
        
        // Store pending request with timeout
        self.pending_requests.insert(correlation_id, PendingRequest {
            created_at: Instant::now(),
            timeout: Duration::from_secs(30),
        });
        
        // Publish request (NON-BLOCKING)
        self.bus.publish(AuthenticatedMessage {
            version: PROTOCOL_VERSION,
            correlation_id: correlation_id.as_bytes(),
            reply_to: Some(Topic { 
                subsystem_id: SubsystemId::BlockPropagation,
                channel: "responses".into(),
            }),
            // ... other fields
        });
        
        // Return immediately - DO NOT AWAIT RESPONSE HERE
        Ok(())
    }
    
    // Separate handler for responses
    async fn handle_response(&self, msg: AuthenticatedMessage<PeerListResponse>) {
        if let Some(pending) = self.pending_requests.remove(&msg.correlation_id) {
            // Process the response
        }
        // Ignore orphaned responses (request already timed out)
    }
}
```

### 3.4 Message Versioning Protocol

**CRITICAL:** All messages MUST include a version field to enable rolling upgrades and prevent deserialization attacks.

**Version Handling Rules:**

```rust
const CURRENT_VERSION: u16 = 1;
const MIN_SUPPORTED_VERSION: u16 = 1;
const MAX_SUPPORTED_VERSION: u16 = 2;

fn deserialize_message<T>(bytes: &[u8]) -> Result<AuthenticatedMessage<T>, Error> {
    // Step 1: Read version FIRST (always at offset 0, always 2 bytes)
    let version = u16::from_be_bytes([bytes[0], bytes[1]]);
    
    // Step 2: Version gate BEFORE any payload deserialization
    if version < MIN_SUPPORTED_VERSION {
        return Err(Error::VersionTooOld { 
            received: version, 
            minimum: MIN_SUPPORTED_VERSION 
        });
    }
    if version > MAX_SUPPORTED_VERSION {
        return Err(Error::VersionTooNew { 
            received: version, 
            maximum: MAX_SUPPORTED_VERSION 
        });
    }
    
    // Step 3: Version-specific deserialization
    match version {
        1 => deserialize_v1(bytes),
        2 => deserialize_v2(bytes),
        _ => unreachable!(), // Guarded by checks above
    }
}
```

**Upgrade Strategy:**
| Scenario | Action |
|----------|--------|
| Adding optional field | Bump minor version, old deserializers ignore new field |
| Adding required field | Bump major version, maintain V1 deserializer for 2 epochs |
| Removing field | Bump major version, deprecate for 4 epochs before removal |
| Changing field type | FORBIDDEN - create new field, deprecate old |

---

## 4. SUBSYSTEM CATALOG

### 4.1 Core Subsystems (Required)

| ID | Name | Bounded Context | Primary Responsibility |
|----|------|----------------|----------------------|
| 1 | Peer Discovery | Network Topology | Find and maintain peer connections |
| 2 | Block Storage | Persistence | Store blockchain data efficiently |
| 3 | Transaction Indexing | Data Retrieval | Provide O(log n) transaction proofs |
| 4 | State Management | Account State | Track balances, nonces, storage |
| 5 | Block Propagation | Network Broadcast | Distribute blocks across network |
| 6 | Mempool | Transaction Queue | Prioritize pending transactions |
| 8 | Consensus | Agreement | Achieve network-wide agreement |
| 10 | Signature Verification | Cryptography | Validate ECDSA/Schnorr signatures |

### 4.2 Optional Subsystems (Advanced Features)

| ID | Name | Bounded Context | Primary Responsibility |
|----|------|----------------|----------------------|
| 7 | Bloom Filters | Light Client Support | Fast probabilistic membership tests |
| 9 | Finality | Economic Security | Guarantee transaction irreversibility |
| 11 | Smart Contracts | Programmability | Execute deterministic code |
| 12 | Transaction Ordering | Parallel Execution | Order transactions via DAG |
| 13 | Light Clients | Resource Efficiency | Sync without full chain download |
| 14 | Sharding | Horizontal Scaling | Split state across shards |
| 15 | Cross-Chain | Interoperability | Atomic swaps via HTLC |

### 4.3 Infrastructure Crates

```
crates/
├── shared-bus/          # Event bus for inter-subsystem communication
├── shared-types/        # Common types (Hash, Address, Signature)
├── crypto/              # Cryptographic primitives (wrapper around libsecp256k1)
└── node-runtime/        # Application binary that wires everything together
```

---

## 5. COMMUNICATION PROTOCOL

### 5.1 Event Schema Design

**Reference:** See IPC Matrix for complete message type catalog.

**Example: Block Validation Flow (Updated for Atomicity)**

The block validation flow has been updated to ensure atomicity. Consensus now acts as the
orchestrator, collecting roots from Subsystems 3 and 4 before sending a single atomic
WriteBlockRequest to Block Storage.

```rust
// Step 1: Block Propagation receives block from network
// ↓ publishes to bus
BlockchainEvent::BlockReceived {
    block: Block,
    source_peer: PeerId,
}

// Step 2: Consensus subscribes to BlockReceived
// ↓ validates block cryptographically
// ↓ publishes result to indicate block is valid
BlockchainEvent::BlockValidated {
    block: ValidatedBlock,
    consensus_proof: ConsensusProof,
}

// Step 3: Consensus orchestrates data collection (NON-BLOCKING, parallel)
// ↓ requests merkle_root from Transaction Indexing
// ↓ requests state_root from State Management

// 3a: Request to Transaction Indexing
ConsensusToTransactionIndexing::BuildMerkleTreeRequest {
    correlation_id: uuid_a,
    reply_to: "consensus.responses",
    block_number: block.header.block_height,
    transactions: block.transactions,
}

// 3b: Request to State Management (parallel with 3a)
ConsensusToStateManagement::ComputeStateRootRequest {
    correlation_id: uuid_b,
    reply_to: "consensus.responses",
    block_number: block.header.block_height,
    state_transitions: block.state_transitions,
}

// Step 4: Consensus receives responses (via correlation IDs)
// ↓ from Transaction Indexing
TransactionIndexingToConsensus::MerkleRootResponse {
    correlation_id: uuid_a,  // Matches request
    merkle_root: [u8; 32],
}

// ↓ from State Management
StateManagementToConsensus::StateRootResponse {
    correlation_id: uuid_b,  // Matches request
    state_root: [u8; 32],
}

// Step 5: Consensus assembles COMPLETE PACKAGE and sends to Block Storage
// This is the ONLY message that triggers a block write.
// ATOMICITY GUARANTEE: Either all data is written, or none.
ConsensusToBlockStorage::WriteBlockRequest {
    correlation_id: uuid_c,
    reply_to: "consensus.responses",
    
    // === COMPLETE PACKAGE ===
    block: ValidatedBlock,
    merkle_root: [u8; 32],   // From Step 4 (Transaction Indexing)
    state_root: [u8; 32],    // From Step 4 (State Management)
}

// Step 6: Block Storage writes atomically
// ↓ Either all data (block + merkle_root + state_root) is written
// ↓ Or nothing is written (rollback on failure)
// ↓ Emits BlockStoredPayload on success

// Step 7: Finality checks if epoch boundary reached (separate flow)
// ↓ This happens after block is stored, not in parallel
```

**IMPORTANT: What Changed (Atomicity Fix)**

| Before (Flawed) | After (Correct) |
|-----------------|-----------------|
| Subsystems 3, 4 wrote roots directly to Storage | Subsystems 3, 4 send roots to Consensus |
| Three separate writes (potential partial failure) | Single atomic write (all or nothing) |
| Power failure = corrupted database | Power failure = clean rollback |
| Block Storage depended on 3 and 4 | Block Storage depends only on 8 |

### 5.2 Security Boundaries

**Reference:** See IPC Matrix Section "Security Boundaries" for each subsystem.

**Key Rules:**
1. Only whitelisted subsystems can send specific message types
2. Every message must be signed (HMAC-SHA256)
3. Timestamps must be within 60-second window
4. Nonces prevent replay attacks
5. Rate limiting per subsystem (e.g., max 100 msgs/sec)
6. **Version field MUST be validated before payload deserialization**

**Example:**

```rust
// Mempool can ONLY accept transactions from Signature Verification
impl Mempool {
    fn handle_message(&mut self, msg: AuthenticatedMessage<AddTransactionRequest>) 
        -> Result<(), SecurityError> 
    {
        // Security check 0: Verify protocol version FIRST
        if msg.version < MIN_SUPPORTED_VERSION || msg.version > MAX_SUPPORTED_VERSION {
            return Err(SecurityError::UnsupportedVersion);
        }
        
        // Security check 1: Verify sender
        if msg.sender_id != SubsystemId::SignatureVerification {
            return Err(SecurityError::UnauthorizedSender);
        }
        
        // Security check 2: Verify signature
        msg.verify_hmac(&self.shared_secret)?;
        
        // Security check 3: Check timestamp
        if now() - msg.timestamp > 60 {
            return Err(SecurityError::MessageTooOld);
        }
        
        // Security check 4: Verify payload
        if !msg.payload.signature_valid {
            return Err(SecurityError::UnverifiedTransaction);
        }
        
        // Now safe to process
        self.add_transaction(msg.payload.transaction)?;
        Ok(())
    }
}
```

### 5.3 Dead Letter Queue (DLQ) Strategy

**CRITICAL:** At-most-once delivery is UNACCEPTABLE for critical blockchain state. Failed events MUST NOT be dropped.

**DLQ Architecture:**

```
┌──────────────────────────────────────────────────────────────┐
│                     EVENT BUS                                 │
├──────────────────────────────────────────────────────────────┤
│                                                               │
│   ┌─────────────┐      ┌─────────────┐      ┌─────────────┐  │
│   │ Main Topic  │      │ Main Topic  │      │ Main Topic  │  │
│   │ block.      │      │ state.      │      │ tx.         │  │
│   │ validated   │      │ updated     │      │ verified    │  │
│   └──────┬──────┘      └──────┬──────┘      └──────┬──────┘  │
│          │                    │                    │          │
│          ↓                    ↓                    ↓          │
│   ┌──────────────────────────────────────────────────────┐   │
│   │              Consumer Processing                      │   │
│   │  ┌─────────┐  ┌─────────┐  ┌─────────┐              │   │
│   │  │ Success │  │ Retry   │  │ Failure │              │   │
│   │  │   ✓     │  │ (3x)    │  │   ✗     │              │   │
│   │  └────┬────┘  └────┬────┘  └────┬────┘              │   │
│   │       │            │            │                    │   │
│   │       ↓            ↓            ↓                    │   │
│   │   [Commit]    [Backoff]   [Dead Letter]             │   │
│   └──────────────────────────────────────────────────────┘   │
│                                      │                        │
│                                      ↓                        │
│   ┌──────────────────────────────────────────────────────┐   │
│   │                  DLQ Topics                           │   │
│   │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐   │   │
│   │  │ dlq.block.  │  │ dlq.state.  │  │ dlq.tx.     │   │   │
│   │  │ validated   │  │ updated     │  │ verified    │   │   │
│   │  └─────────────┘  └─────────────┘  └─────────────┘   │   │
│   └──────────────────────────────────────────────────────┘   │
│                                                               │
└──────────────────────────────────────────────────────────────┘
```

**DLQ Processing Rules:**

| Event Criticality | Retry Count | Backoff Strategy | DLQ Action |
|-------------------|-------------|------------------|------------|
| CRITICAL (Block Storage, State Write) | 5 | Exponential (1s, 2s, 4s, 8s, 16s) | Alert + Manual Review Required |
| HIGH (Consensus) | 3 | Exponential (1s, 2s, 4s) | Alert + Auto-Retry after 1 hour |
| **FINALITY (Special Case)** | **0** | **NONE - Circuit Breaker** | **Trigger Sync Mode (see below)** |
| MEDIUM (Mempool, Propagation) | 2 | Linear (1s, 2s) | Log + Auto-Discard after 24 hours |
| LOW (Metrics, Logging) | 0 | None | Discard immediately |

### 5.4 Finality Circuit Breaker (Casper FFG Compliance)

**CRITICAL:** Finality failures require special handling that differs from standard retry logic.

**The Problem:**
- System.md (Subsystem 9) uses Casper FFG consensus
- Casper FFG prioritizes **Safety over Liveness**: the chain MUST stop finalizing if 33% of validators disagree
- Standard retry logic (Auto-Retry after 1 hour) is inappropriate for mathematical impossibilities

**Why Retrying Finality Failures is Harmful:**
1. If >33% of validators reject finalization, consensus is mathematically impossible
2. Retrying a mathematical impossibility wastes CPU and fills logs with errors
3. The node may be on a minority fork and needs to sync with the majority chain
4. Continuous retries create a "Zombie State" - the node appears alive but cannot make progress

**Circuit Breaker Implementation:**

```rust
/// Finality failure handling - DO NOT RETRY
impl FinalityCircuitBreaker {
    /// Handle finality failure events
    async fn handle_finality_failure(
        &mut self,
        failure: FinalityFailureEvent
    ) -> Result<(), Error> {
        // Step 1: DO NOT RETRY - this is a circuit breaker, not a retry handler
        log::error!("Finality circuit breaker triggered: {:?}", failure);
        
        // Step 2: Determine failure type
        match failure.failure_type {
            FinalityFailureType::InsufficientAttestations => {
                // <67% attestations - likely on minority fork
                self.trigger_sync_mode(SyncReason::MinorityFork).await?;
            }
            FinalityFailureType::ConflictingCheckpoints => {
                // Validators disagree on checkpoint - network partition
                self.trigger_sync_mode(SyncReason::NetworkPartition).await?;
            }
            FinalityFailureType::StaleCheckpoint => {
                // Checkpoint too old - node fell behind
                self.trigger_sync_mode(SyncReason::NodeBehind).await?;
            }
            FinalityFailureType::InvalidProof => {
                // Byzantine behavior detected
                self.alert_security("Invalid finality proof detected", &failure)?;
                self.trigger_sync_mode(SyncReason::ByzantineBehavior).await?;
            }
        }
        
        // Step 3: Emit state change event
        self.emit_event(NodeStateChanged {
            previous_state: NodeState::Finalizing,
            new_state: NodeState::Syncing,
            reason: format!("Finality circuit breaker: {:?}", failure.failure_type),
            timestamp: now(),
        }).await?;
        
        // Step 4: DO NOT send to DLQ - this is not a retry-able error
        // The message is acknowledged and the node transitions to sync mode
        
        Ok(())
    }
    
    /// Transition node to sync mode to find majority chain
    async fn trigger_sync_mode(&mut self, reason: SyncReason) -> Result<(), Error> {
        log::warn!("Entering sync mode: {:?}", reason);
        
        // 1. Pause block production
        self.pause_block_production()?;
        
        // 2. Request peer chain heads
        self.request_chain_heads_from_peers().await?;
        
        // 3. Identify longest finalized chain
        let majority_chain = self.identify_majority_chain().await?;
        
        // 4. If our chain diverges, reorg to majority
        if majority_chain.tip != self.current_chain.tip {
            self.reorg_to_chain(majority_chain).await?;
        }
        
        // 5. Resume normal operation
        self.resume_block_production()?;
        
        Ok(())
    }
}

#[derive(Debug, Clone)]
enum FinalityFailureType {
    InsufficientAttestations,  // <67% validators attested
    ConflictingCheckpoints,    // Validators disagree on checkpoint
    StaleCheckpoint,           // Checkpoint is too old
    InvalidProof,              // Proof cryptographically invalid
}

#[derive(Debug, Clone)]
enum SyncReason {
    MinorityFork,
    NetworkPartition,
    NodeBehind,
    ByzantineBehavior,
}

#[derive(Debug, Clone)]
enum NodeState {
    Syncing,
    Finalizing,
    Producing,
    Halted,
}
```

**Key Differences from Standard DLQ Handling:**

| Aspect | Standard DLQ | Finality Circuit Breaker |
|--------|-------------|-------------------------|
| Retry Count | 3-5 attempts | **0 attempts** |
| Backoff | Exponential | **None** |
| DLQ Routing | Yes | **No** |
| Action | Wait and retry | **State change to Sync Mode** |
| Goal | Eventually succeed | **Find correct chain** |

**DLQ Message Format:**

```rust
struct DeadLetterMessage<T> {
    // Original message (preserved exactly)
    original_message: AuthenticatedMessage<T>,
    
    // DLQ metadata
    dlq_metadata: DLQMetadata,
}

struct DLQMetadata {
    original_topic: String,
    failure_reason: FailureReason,
    failure_timestamp: u64,
    retry_count: u8,
    last_error: String,
    stack_trace: Option<String>,
    consumer_id: SubsystemId,
}

enum FailureReason {
    DeserializationError,
    ValidationError,
    StorageError,
    TimeoutError,
    UnknownError,
}
```

**Consumer Implementation Pattern:**

```rust
impl EventConsumer for BlockStorage {
    async fn consume(&mut self, msg: AuthenticatedMessage<BlockValidated>) -> Result<(), Error> {
        let mut retry_count = 0;
        let max_retries = 5;
        
        loop {
            match self.process_block(&msg.payload) {
                Ok(()) => {
                    // Success - commit offset
                    self.commit_offset(msg.offset)?;
                    return Ok(());
                }
                Err(e) if retry_count < max_retries => {
                    // Transient failure - retry with backoff
                    retry_count += 1;
                    let backoff = Duration::from_secs(2_u64.pow(retry_count));
                    tokio::time::sleep(backoff).await;
                    continue;
                }
                Err(e) => {
                    // Permanent failure - send to DLQ
                    self.publish_to_dlq(DeadLetterMessage {
                        original_message: msg,
                        dlq_metadata: DLQMetadata {
                            original_topic: "block.validated".into(),
                            failure_reason: FailureReason::StorageError,
                            failure_timestamp: now(),
                            retry_count,
                            last_error: e.to_string(),
                            stack_trace: Some(e.backtrace().to_string()),
                            consumer_id: SubsystemId::BlockStorage,
                        },
                    }).await?;
                    
                    // Alert operations team for CRITICAL events
                    self.alert_ops("Block storage failed after 5 retries", &e)?;
                    
                    // Commit offset to prevent infinite loop
                    // The message is now safely in DLQ for manual review
                    self.commit_offset(msg.offset)?;
                    return Err(e);
                }
            }
        }
    }
}
```

**DLQ Monitoring Requirements:**
- DLQ depth MUST be monitored with alerting threshold (>10 messages = WARN, >100 = CRITICAL)
- DLQ age MUST be monitored (oldest message >1 hour = WARN, >24 hours = CRITICAL)
- DLQ replay tooling MUST be provided for manual intervention
- DLQ messages MUST be retained for minimum 7 days
```

---

## 6. DEVELOPMENT WORKFLOW

### 6.1 Specification-First Approach

**RULE:** No implementation code before the specification document exists.

**Workflow:**

```
Step 1: Write Specification
├─ Create SPEC-[ID]-[NAME].md
├─ Define Domain Model (structs, enums)
├─ Define Ports (traits)
├─ Define Events
└─ Define TDD Strategy

Step 2: Write Tests (Red Phase)
├─ Implement tests from TDD Strategy section
├─ All tests must fail (no implementation yet)
└─ Run: cargo test → FAIL

Step 3: Implement Domain (Green Phase)
├─ Write MINIMUM code to pass tests
├─ Pure domain logic only
└─ Run: cargo test → PASS

Step 4: Implement Ports (Hexagonal)
├─ Define trait signatures
├─ No concrete implementations yet
└─ Traits live in the library crate

Step 5: Implement Adapters (Outer Layer)
├─ Create adapter crate (e.g., peer-discovery-libp2p)
├─ Implement traits using external dependencies
└─ Wire into node-runtime binary

Step 6: Integration Testing
├─ Test subsystem via events
├─ Mock other subsystems
└─ Verify event emissions
```

### 6.2 Crate Structure Template

Every subsystem follows this structure:

```
crates/subsystem-name/
├── Cargo.toml
├── SPEC-[ID]-[NAME].md          # ← The specification
├── src/
│   ├── lib.rs                   # Public API
│   ├── domain/                  # Inner layer (pure logic)
│   │   ├── mod.rs
│   │   ├── entities.rs          # Core structs
│   │   ├── value_objects.rs     # Immutable data
│   │   └── services.rs          # Business logic functions
│   ├── ports/                   # Middle layer (traits)
│   │   ├── mod.rs
│   │   ├── inbound.rs           # Driving ports (API)
│   │   └── outbound.rs          # Driven ports (SPI)
│   └── events.rs                # Event definitions for shared bus
└── tests/
    ├── unit/                    # Domain logic tests
    ├── integration/             # Port contract tests
    └── fixtures/                # Test data
```

### 6.3 AI Development Rules

When acting as the implementation assistant:

1. **Design Phase:**
    - ✅ Generate specification documents
    - ✅ Define domain models (struct definitions)
    - ✅ Define port traits (no implementations)
    - ❌ FORBIDDEN: Write impl blocks, write function bodies

2. **Test Phase:**
    - ✅ Write failing unit tests
    - ✅ Define test fixtures
    - ❌ FORBIDDEN: Write passing tests before implementation

3. **Implementation Phase:**
    - ✅ Write minimum code to pass tests
    - ✅ Refactor while keeping tests green
    - ❌ FORBIDDEN: Add features not covered by tests

---

## 7. SECURITY ARCHITECTURE

### 7.1 Defense in Depth

**Reference:** See IPC Matrix "Defense in Depth Summary" section.

**8 Security Layers:**

```
Layer 8: Social Layer (Community governance)
Layer 7: Application Logic (Smart contract safety)
Layer 6: Consensus Rules (51% attack prevention)
Layer 5: Network Security (DDoS mitigation)
Layer 4: Cryptographic Security (Signature verification)
Layer 3: IPC Security (Message authentication)
Layer 2: Memory Safety (Rust borrow checker)
Layer 1: Hardware Security (TEE, SGX - optional)
```

### 7.2 Compartmentalization

**Principle:** Compromising one subsystem CANNOT compromise others.

**Attack Scenario Example:**

```
Scenario: Attacker gains full control of Mempool (Subsystem 6)

Attacker attempts:
1. Inject malicious transaction into Consensus
   → ❌ BLOCKED: Consensus only accepts from Signature Verification
   
2. Modify state directly
   → ❌ BLOCKED: State Management rejects writes from Mempool
   
3. Read private keys
   → ❌ BLOCKED: Keys stored in separate process (not in Mempool)
   
4. Flood network with spam
   → ✅ PARTIAL SUCCESS: Can spam mempool
   → ⚠️ CONTAINED: Rate limiting prevents network flood
   
Result: Attack contained to Mempool subsystem
```

### 7.3 Critical Security Invariants

Each subsystem must maintain invariants documented in its specification:

```rust
// Example: Mempool invariants
invariant!(pending_transactions.len() <= MAX_MEMPOOL_SIZE);
invariant!(all_transactions_have_valid_signatures());
invariant!(total_gas_does_not_exceed_block_limit());
```

**Enforcement:**

```rust
#[cfg(debug_assertions)]
fn check_invariants(&self) {
    assert!(self.pending_transactions.len() <= MAX_MEMPOOL_SIZE);
    // ... other checks
}

// Called after every state mutation
fn add_transaction(&mut self, tx: Transaction) -> Result<()> {
    // ... add logic
    #[cfg(debug_assertions)]
    self.check_invariants();
    Ok(())
}
```

---

## 8. TESTING STRATEGY

### 8.1 Test Pyramid

```
                    ┌────────────────┐
                    │  E2E Tests     │  ← Few (expensive, slow)
                    │  (10 tests)    │
                    └────────────────┘
                ┌──────────────────────┐
                │ Integration Tests    │  ← Some (moderate cost)
                │ (50 tests)           │
                └──────────────────────┘
        ┌──────────────────────────────────┐
        │      Unit Tests                  │  ← Many (cheap, fast)
        │      (500+ tests)                │
        └──────────────────────────────────┘
```

### 8.2 Unit Testing (Domain Layer)

**Target:** Pure domain logic, no I/O

```rust
// Example: Transaction Indexing domain tests
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_merkle_tree_rejects_empty_transactions() {
        let result = MerkleTree::build(&[]);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_merkle_tree_produces_correct_root() {
        let txs = vec![tx1, tx2, tx3];
        let tree = MerkleTree::build(&txs).unwrap();
        assert_eq!(tree.root(), expected_root);
    }
    
    #[test]
    fn test_merkle_proof_verification() {
        let tree = MerkleTree::build(&txs).unwrap();
        let proof = tree.generate_proof(tx_hash);
        assert!(tree.verify_proof(&proof));
    }
}
```

### 8.3 Integration Testing (Port Layer)

**Target:** Verify adapters satisfy port contracts

```rust
// Test that RocksDB adapter implements BlockPersister correctly
#[cfg(test)]
mod integration_tests {
    #[test]
    fn test_rocksdb_adapter_persists_blocks() {
        let adapter = RocksDBAdapter::new_in_memory();
        let block = create_test_block();
        
        adapter.save(&block).unwrap();
        let loaded = adapter.load(block.hash()).unwrap();
        
        assert_eq!(block, loaded);
    }
    
    #[test]
    fn test_rocksdb_adapter_handles_disk_full() {
        let adapter = RocksDBAdapter::with_limit(1024); // 1KB limit
        let large_block = create_block_of_size(2048);
        
        let result = adapter.save(&large_block);
        assert!(matches!(result, Err(StorageError::DiskFull)));
    }
}
```

### 8.4 End-to-End Testing

**Target:** Full system behavior via event bus

```rust
#[tokio::test]
async fn test_block_validation_pipeline() {
    // Setup full node with all subsystems
    let node = TestNode::new().await;
    
    // Inject block from network
    node.inject_block(create_test_block()).await;
    
    // Verify events emitted in correct order
    let events = node.collect_events().await;
    assert_eq!(events[0], BlockchainEvent::BlockReceived { .. });
    assert_eq!(events[1], BlockchainEvent::BlockValidated { .. });
    assert_eq!(events[2], BlockchainEvent::BlockStored { .. });
}
```

---

## 9. DEPLOYMENT MODEL

### 9.1 Single-Binary Architecture

Despite the modular design, we compile to a **single binary** for production:

```
node-runtime (binary)
├─ Links all subsystem libraries
├─ Wires adapters to ports
├─ Configures shared bus
└─ Starts async runtime (Tokio)
```

**Why not microservices?**
- Lower latency (in-process communication)
- Simpler deployment (single binary)
- Easier debugging
- Can scale to microservices later if needed

### 9.2 Configuration

```toml
# config.toml
[peer_discovery]
bootstrap_nodes = ["node1.example.com:30303"]
max_peers = 50

[consensus]
type = "pos"  # or "pbft"
validator_key = "path/to/key.pem"

[storage]
backend = "rocksdb"
data_dir = "/var/blockchain/data"
max_size_gb = 500

[mempool]
max_transactions = 5000
min_gas_price = "1gwei"
```

### 9.3 Monitoring & Observability

Each subsystem emits metrics:

```rust
// Every subsystem implements
trait Metrics {
    fn report_metrics(&self) -> SubsystemMetrics;
}

struct SubsystemMetrics {
    subsystem_id: SubsystemId,
    uptime_seconds: u64,
    events_processed: u64,
    errors_count: u64,
    custom_metrics: HashMap<String, f64>,
}
```

---

## 10. REFERENCES

### 10.1 Related Documents

1. **IPC Matrix** (`IPC-MATRIX.md`) - Complete message type catalog and security boundaries
2. **Subsystem Specifications** (`SPEC-[ID]-[NAME].md`) - Individual subsystem designs
3. **Dependency Graph** - See Section 3.1 and previous conversation

### 10.2 External Resources

- **Domain-Driven Design:** Eric Evans, "Domain-Driven Design: Tackling Complexity"
- **Hexagonal Architecture:** Alistair Cockburn, "Hexagonal Architecture"
- **Event-Driven Architecture:** Martin Fowler, "Event Sourcing"
- **Rust Patterns:** https://rust-unofficial.github.io/patterns/

### 10.3 Changelog

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2024-11-30 | Initial architecture document |

---

## APPENDIX A: QUICK START GUIDE

### For New Developers

1. **Read this document** (you are here)
2. **Review IPC Matrix** to understand subsystem communication
3. **Pick a subsystem** from the catalog (start with #10 Signature Verification - no dependencies)
4. **Read its SPEC document** (or create one if missing)
5. **Write tests first** (TDD Phase 1: Red)
6. **Implement domain logic** (TDD Phase 2: Green)
7. **Refactor** (TDD Phase 3: Clean)

### For AI Assistants

```
if (task == "design new subsystem") {
    output = generate_specification_document();
    verify(output.has_domain_model);
    verify(output.has_ports);
    verify(output.has_events);
    verify(output.has_tdd_strategy);
} else if (task == "implement subsystem") {
    verify(specification_exists);
    write_failing_tests();
    wait_for_approval();
    write_minimum_implementation();
} else {
    error("Unknown task");
}
```

---

**END OF ARCHITECTURE.md**

*This document is the constitution of the codebase. All code must conform to these principles.*