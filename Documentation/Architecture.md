# ARCHITECTURE.md
## Modular Blockchain - Hybrid Architecture Specification

**Version:** 2.3  
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

**CRITICAL UPDATE (v2.2 Choreography Model):**
Subsystem 8 (Consensus) is NO LONGER an orchestrator. The system uses 
event-driven choreography where each subsystem reacts independently.
Block Storage (2) acts as a Stateful Assembler, buffering components
until a complete block can be written atomically.

```
Dependency Flow (A → B means "A depends on B"):

LEVEL 0 (No Dependencies):
├─ [1] Peer Discovery
└─ [10] Signature Verification

LEVEL 1 (Depends on Level 0):
├─ [1] Peer Discovery → [10] (DDoS defense - verify node identity at edge)
├─ [6] Mempool → [10]
├─ [7] Bloom Filters → [1]
└─ [13] Light Clients → [1]

LEVEL 2 (Depends on Level 0-1):
├─ [3] Transaction Indexing → [10]
├─ [5] Block Propagation → [1]
└─ [4] State Management (partial)

LEVEL 3 (Depends on Level 0-2):
├─ [8] Consensus → [5, 6, 10] (v2.2: Validation only, NOT orchestration)
└─ [11] Smart Contracts → [4, 10]

LEVEL 4 (Depends on Level 0-3):
├─ [2] Block Storage → subscribes to events from [3, 4, 8] (Stateful Assembler)
├─ [9] Finality → [8, 10]
├─ [12] Transaction Ordering → [4, 11]
└─ [14] Sharding → [4, 8]

LEVEL 5 (Depends on Level 0-4):
└─ [15] Cross-Chain → [8, 9, 11]
```

**Data Flow for Block Writes (v2.2 Choreography Pattern):**
```
[8] Consensus ─────BlockValidated────────→ [Event Bus]
                                               │
         ┌─────────────────────────────────────┼─────────────────────────┐
         ↓                                     ↓                         ↓
[3] Transaction Indexing          [4] State Management          [2] Block Storage
         │                                     │                    (buffers block)
         ↓                                     ↓
    MerkleRootComputed                 StateRootComputed
         │                                     │
         └──────────────→ [Event Bus] ←────────┘
                               ↓
                      [2] Block Storage
                      (receives all 3, writes atomically)

IMPORTANT: No subsystem "orchestrates". Each reacts to events independently.
           Block Storage assembles components, then writes atomically.
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

### 3.2.1 Envelope as Sole Source of Truth (v2.2 Security Mandate)

**SECURITY VULNERABILITY FIXED:** The "Payload Impersonation" attack.

In earlier versions, some message payloads contained redundant identity fields 
(e.g., `requester_id`, `source_subsystem`). This created an audit trail poisoning 
vulnerability where an attacker could set the envelope `sender_id` to pass 
signature verification but use a victim's ID in the payload to misdirect logging.

**MANDATORY RULES:**

| Rule | Description |
|------|-------------|
| **Rule 1: No Redundant Identity** | Payloads MUST NOT contain identity fields (e.g., `requester_id`, `source_id`) |
| **Rule 2: Envelope is Truth** | The `sender_id` in the AuthenticatedMessage envelope is the ONLY source of truth |
| **Rule 3: Logging/Metrics** | ALL logging, metrics, and authorization checks MUST use `msg.sender_id` |
| **Rule 4: Audit Trail** | Forensic audit trails MUST be based solely on envelope metadata |

**Attack Scenario (Now Prevented):**
```
BEFORE (Vulnerable):
1. Attacker controls Subsystem A
2. Attacker sends message with:
   - Envelope: sender_id = A (passes signature check)
   - Payload: requester_id = B (victim)
3. Logs show "Request from B" (WRONG!)
4. Forensic investigation misdirected

AFTER (Fixed):
1. Payloads have NO identity fields
2. All logging uses envelope.sender_id
3. Logs correctly show "Request from A"
4. Attacker cannot poison audit trail
```

**Implementation Requirement:**

```rust
// CORRECT: Payload has no identity field
struct ReadBlockRequestPayload {
    block_hash: [u8; 32],
    include_transactions: bool,
}

// WRONG: Do NOT include requester_id in payload
struct ReadBlockRequestPayload_FORBIDDEN {
    requester_id: SubsystemId,  // ❌ FORBIDDEN - redundant, unverified
    block_hash: [u8; 32],
    include_transactions: bool,
}

// When logging, ALWAYS use envelope
fn handle_request(msg: AuthenticatedMessage<ReadBlockRequestPayload>) {
    // ✅ CORRECT: Use envelope sender_id
    log::info!("Request from {:?}: read block {:?}", 
        msg.sender_id,  // From signed envelope
        msg.payload.block_hash
    );
    
    // ❌ WRONG: Do not use payload identity (doesn't exist now anyway)
    // log::info!("Request from {:?}", msg.payload.requester_id);
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
5. **REPLY_TO VALIDATION** - Responder MUST validate reply_to matches sender_id (see below)

### 3.3.1 Reply-To Forwarding Attack Prevention

**SECURITY VULNERABILITY:** The `reply_to` field creates an attack vector where a compromised
subsystem could forward requests with a malicious `reply_to` pointing to a victim subsystem.

**Attack Scenario:**
```
Attacker compromises Subsystem A
Attacker sends request to Subsystem B with:
  - sender_id: A (legitimate)
  - reply_to: C.responses (victim - Subsystem C)

Subsystem B processes request and sends response to C.responses
Subsystem C receives unsolicited responses, potentially:
  - Filling up its pending_requests map (DoS)
  - Causing confusion/state corruption
  - Enabling further attacks via crafted response payloads
```

**MANDATORY VALIDATION RULE:**

A responder subsystem MUST validate that the `reply_to.subsystem_id` field in a request 
message matches the `sender_id` of that same message. If they do not match, the message 
MUST be rejected as a malicious forwarding attempt.

```rust
fn validate_request<T>(msg: &AuthenticatedMessage<T>) -> Result<(), SecurityError> {
    // Standard envelope validation
    msg.verify_signature(&self.shared_secret)?;
    msg.verify_timestamp()?;
    msg.verify_nonce(&self.nonce_cache)?;
    
    // CRITICAL: Reply-To Forwarding Attack Prevention
    if let Some(ref reply_to) = msg.reply_to {
        if reply_to.subsystem_id != msg.sender_id {
            log::warn!(
                "SECURITY: Rejected forwarding attack. sender_id={:?}, reply_to={:?}",
                msg.sender_id, reply_to.subsystem_id
            );
            return Err(SecurityError::ReplyToMismatch {
                sender: msg.sender_id,
                reply_to: reply_to.subsystem_id,
            });
        }
    }
    
    Ok(())
}

// When sending a response, also validate the original request
fn send_response<T, R>(
    original_request: &AuthenticatedMessage<T>,
    response_payload: R,
) -> Result<(), Error> {
    // Validate reply_to matches sender_id
    let reply_to = original_request.reply_to.as_ref()
        .ok_or(Error::MissingReplyTo)?;
    
    if reply_to.subsystem_id != original_request.sender_id {
        return Err(Error::ReplyToMismatch);
    }
    
    // Safe to send response
    self.bus.publish(&reply_to.to_topic_string(), AuthenticatedMessage {
        version: PROTOCOL_VERSION,
        sender_id: self.subsystem_id,
        recipient_id: original_request.sender_id,  // Send to original sender
        correlation_id: original_request.correlation_id,
        reply_to: None,  // Responses don't need reply_to
        // ...
        payload: response_payload,
    });
    
    Ok(())
}
```

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
            sender_id: self.subsystem_id,  // Our ID
            correlation_id: correlation_id.as_bytes(),
            reply_to: Some(Topic { 
                subsystem_id: self.subsystem_id,  // MUST match sender_id!
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

### 3.5 Time-Bounded Replay Prevention

**CRITICAL SECURITY FIX (v2.1):** The original replay prevention design had a fatal flaw 
that could lead to unbounded memory growth and eventual node crash.

#### The Vulnerability: Nonce Cache Exhaustion

**The Problem:**
The original design validated timestamp and nonce independently:
1. Check if timestamp is within 60-second window → OK
2. Check if nonce exists in cache → If not, add to cache forever

**The Attack Vector:**
```
Attacker sends 1 million messages per second, each with:
- Valid timestamp (within 60s window)
- Unique nonce (incrementing counter)

Result: Nonce cache grows unboundedly at 1M entries/second
After 1 hour: 3.6 billion entries in cache (~144 GB RAM)
Node crashes due to OOM (Out of Memory)
```

**Why This Happens:**
- Timestamp check ensures message is "fresh" but doesn't limit cache
- Nonce check prevents replay but stores nonces FOREVER
- No garbage collection = unbounded memory growth
- This is a classic resource exhaustion DoS attack

#### The Solution: Time-Bounded Nonce Validity

**Core Insight:** A nonce only needs to be unique within the timestamp validity window.
After the window expires, the timestamp check will reject the message anyway.

**MANDATORY RULES:**

| Rule | Description |
|------|-------------|
| **Rule 1: Nonce Subservience** | A nonce's uniqueness is ONLY relevant within the valid timestamp window |
| **Rule 2: Bounded Window** | Nonce cache MUST only store nonces for 120 seconds (2x timestamp window) |
| **Rule 3: Garbage Collection** | Nonces with expired timestamps MUST be continuously purged from cache |

**Correct Validation Order (CRITICAL):**

```rust
/// Time-Bounded Nonce Cache
/// 
/// SECURITY: This cache automatically expires entries to prevent
/// unbounded memory growth (Nonce Cache Exhaustion attack).
struct TimeBoundedNonceCache {
    /// Map of nonce -> timestamp when nonce was first seen
    cache: HashMap<u64, u64>,
    
    /// Nonce validity window (2x message timestamp window)
    validity_window_secs: u64,  // Default: 120 seconds
    
    /// Last garbage collection timestamp
    last_gc: u64,
    
    /// GC interval
    gc_interval_secs: u64,  // Default: 10 seconds
}

impl TimeBoundedNonceCache {
    const DEFAULT_VALIDITY_WINDOW: u64 = 120;  // 2x the 60s message window
    const DEFAULT_GC_INTERVAL: u64 = 10;
    
    fn new() -> Self {
        Self {
            cache: HashMap::new(),
            validity_window_secs: Self::DEFAULT_VALIDITY_WINDOW,
            last_gc: current_timestamp(),
            gc_interval_secs: Self::DEFAULT_GC_INTERVAL,
        }
    }
    
    /// Check if nonce is valid (not seen before) and add to cache
    /// 
    /// PRECONDITION: Timestamp MUST be validated BEFORE calling this!
    fn check_and_add(&mut self, nonce: u64, timestamp: u64) -> Result<(), ReplayError> {
        let now = current_timestamp();
        
        // Garbage collect expired nonces periodically
        if now - self.last_gc > self.gc_interval_secs {
            self.garbage_collect(now);
            self.last_gc = now;
        }
        
        // Check if nonce exists in cache
        if self.cache.contains_key(&nonce) {
            return Err(ReplayError::NonceReused { nonce });
        }
        
        // Add nonce with its timestamp for later expiration
        self.cache.insert(nonce, timestamp);
        
        Ok(())
    }
    
    /// Remove all nonces whose timestamps have expired
    fn garbage_collect(&mut self, now: u64) {
        let expiry_threshold = now.saturating_sub(self.validity_window_secs);
        
        self.cache.retain(|_nonce, timestamp| {
            *timestamp > expiry_threshold
        });
        
        log::debug!(
            "Nonce cache GC complete. Remaining entries: {}",
            self.cache.len()
        );
    }
}
```

**Correct Message Verification (v2.1):**

```rust
impl<T> AuthenticatedMessage<T> {
    /// Verify message authenticity with time-bounded replay prevention
    /// 
    /// SECURITY: The order of checks is CRITICAL for DoS prevention.
    fn verify(
        &self, 
        expected_sender: SubsystemId, 
        shared_secret: &[u8],
        nonce_cache: &mut TimeBoundedNonceCache,
    ) -> Result<(), AuthError> {
        let now = current_timestamp();
        
        // ╔═══════════════════════════════════════════════════════════════╗
        // ║  STEP 1: TIMESTAMP CHECK (MUST BE FIRST!)                     ║
        // ║                                                               ║
        // ║  Reject messages outside the valid time window BEFORE any    ║
        // ║  other processing. This bounds all subsequent operations.    ║
        // ╚═══════════════════════════════════════════════════════════════╝
        
        // Allow 10s clock skew into future, 60s into past
        let min_valid_timestamp = now.saturating_sub(60);
        let max_valid_timestamp = now.saturating_add(10);
        
        if self.timestamp < min_valid_timestamp {
            return Err(AuthError::MessageTooOld { 
                timestamp: self.timestamp,
                threshold: min_valid_timestamp,
            });
        }
        
        if self.timestamp > max_valid_timestamp {
            return Err(AuthError::MessageFromFuture {
                timestamp: self.timestamp,
                threshold: max_valid_timestamp,
            });
        }
        
        // ╔═══════════════════════════════════════════════════════════════╗
        // ║  STEP 2: VERSION CHECK                                        ║
        // ║                                                               ║
        // ║  Reject unsupported versions before any deserialization.     ║
        // ╚═══════════════════════════════════════════════════════════════╝
        
        if self.version < MIN_SUPPORTED_VERSION || self.version > MAX_SUPPORTED_VERSION {
            return Err(AuthError::UnsupportedVersion { 
                received: self.version,
                supported_range: (MIN_SUPPORTED_VERSION, MAX_SUPPORTED_VERSION),
            });
        }
        
        // ╔═══════════════════════════════════════════════════════════════╗
        // ║  STEP 3: SENDER CHECK                                         ║
        // ║                                                               ║
        // ║  Verify the sender is who we expect (from IPC-MATRIX rules). ║
        // ╚═══════════════════════════════════════════════════════════════╝
        
        if self.sender_id != expected_sender {
            return Err(AuthError::InvalidSender {
                expected: expected_sender,
                received: self.sender_id,
            });
        }
        
        // ╔═══════════════════════════════════════════════════════════════╗
        // ║  STEP 4: SIGNATURE CHECK                                      ║
        // ║                                                               ║
        // ║  Verify HMAC before trusting any message content.            ║
        // ╚═══════════════════════════════════════════════════════════════╝
        
        let computed_hmac = compute_hmac(shared_secret, &self.serialize_without_sig());
        if !constant_time_eq(&computed_hmac, &self.signature) {
            return Err(AuthError::InvalidSignature);
        }
        
        // ╔═══════════════════════════════════════════════════════════════╗
        // ║  STEP 5: NONCE CHECK (ONLY AFTER TIMESTAMP!)                  ║
        // ║                                                               ║
        // ║  SECURITY: We only check/store nonces for messages that      ║
        // ║  passed the timestamp check. This bounds cache size.         ║
        // ║                                                               ║
        // ║  The cache will automatically expire this nonce after 120s.  ║
        // ╚═══════════════════════════════════════════════════════════════╝
        
        nonce_cache.check_and_add(self.nonce, self.timestamp)?;
        
        // ╔═══════════════════════════════════════════════════════════════╗
        // ║  STEP 6: REPLY-TO VALIDATION (for requests only)              ║
        // ║                                                               ║
        // ║  Prevent forwarding attacks by ensuring reply_to matches     ║
        // ║  sender_id. See Section 3.3.1 for details.                   ║
        // ╚═══════════════════════════════════════════════════════════════╝
        
        if let Some(ref reply_to) = self.reply_to {
            if reply_to.subsystem_id != self.sender_id {
                return Err(AuthError::ReplyToMismatch {
                    sender: self.sender_id,
                    reply_to: reply_to.subsystem_id,
                });
            }
        }
        
        Ok(())
    }
}
```

**Memory Bounds Analysis:**

```
Given:
- Message window: 60 seconds
- Nonce cache window: 120 seconds
- Max message rate per subsystem: 10,000/second (rate limited)
- Number of subsystems: 15

Worst case cache size:
= 10,000 msg/s × 120s × 15 subsystems
= 18,000,000 entries
× 16 bytes per entry (u64 nonce + u64 timestamp)
= 288 MB

With garbage collection every 10s:
- Cache never grows beyond 120s worth of messages
- Memory is bounded and predictable
- OOM attack is impossible
```

**Attack Mitigation Summary:**

| Attack | Before (Vulnerable) | After (Fixed) |
|--------|---------------------|---------------|
| Nonce flood | Cache grows forever → OOM crash | Cache bounded to 120s window |
| Replay attack | Blocked by nonce check | Still blocked (nonce valid for 120s) |
| Old message replay | Blocked by timestamp + nonce | Blocked by timestamp (first check) |
| Memory exhaustion | Possible with unique nonces | Impossible due to GC |

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

**Example: Block Validation Flow (v2.2 - Event-Driven Choreography)**

The block validation flow uses **decentralized choreography**, NOT centralized orchestration.
Each subsystem reacts to events independently and publishes its results to the bus.
Block Storage acts as a **Stateful Assembler**, buffering components until complete.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    BLOCK VALIDATION: CHOREOGRAPHY PATTERN                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   [Block Propagation]                                                       │
│          │                                                                  │
│          ↓ BlockReceived                                                    │
│   [Consensus (8)]                                                           │
│          │                                                                  │
│          ↓ BlockValidated { block, merkle_root: TBD, state_root: TBD }      │
│          │                                                                  │
│    ┌─────┴─────┬─────────────┐                                              │
│    ↓           ↓             ↓                                              │
│ [Subsystem 3] [Subsystem 4] [Block Storage (2)]                             │
│ Transaction   State         (buffers BlockValidated,                        │
│ Indexing      Management     waits for roots)                               │
│    │           │                   │                                        │
│    ↓           ↓                   │                                        │
│ MerkleRootComputed  StateRootComputed                                       │
│    │           │                   │                                        │
│    └─────┬─────┘                   │                                        │
│          ↓                         ↓                                        │
│    [Block Storage (2)] ────────────┘                                        │
│    Receives all 3 components                                                │
│          │                                                                  │
│          ↓ Atomic Write (only when block_hash matches all 3)                │
│    BlockStoredPayload                                                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**WHY CHOREOGRAPHY, NOT ORCHESTRATION (v2.2 Design Decision):**

The previous v2.0/v2.1 design made Consensus an "orchestrator" that collected roots
via request/response. This was rejected by the Architecture Council for these reasons:

| Problem with Orchestration | Why Choreography is Better |
|---------------------------|---------------------------|
| Single point of failure | Each subsystem operates independently |
| Performance bottleneck | Parallel processing, no waiting |
| Hidden latency source | Each component emits timing metrics |
| Complex retry logic in Consensus | Each component handles own failures |
| Violates EDA principles | True event-driven, loosely coupled |

```rust
// Step 1: Block Propagation receives block from network
// ↓ publishes to bus
BlockchainEvent::BlockReceived {
    block: Block,
    source_peer: PeerId,
}

// Step 2: Consensus subscribes to BlockReceived
// ↓ validates block cryptographically
// ↓ publishes BlockValidated (NOTE: roots are TBD)
BlockchainEvent::BlockValidated {
    block_hash: [u8; 32],
    block: ValidatedBlock,
    consensus_proof: ConsensusProof,
    // v2.2: These are NOT filled in by Consensus
    // They serve as placeholders indicating "to be computed"
    merkle_root: None,  // TBD by Subsystem 3
    state_root: None,   // TBD by Subsystem 4
}

// Step 3a: Transaction Indexing subscribes to BlockValidated
// ↓ computes Merkle root for block's transactions
// ↓ publishes result (PARALLEL with Step 3b)
TransactionIndexingEvent::MerkleRootComputed {
    block_hash: [u8; 32],  // Key for assembly
    merkle_root: [u8; 32],
    computation_time_ms: u64,  // For observability
}

// Step 3b: State Management subscribes to BlockValidated (PARALLEL)
// ↓ computes State root for block's state transitions
// ↓ publishes result
StateManagementEvent::StateRootComputed {
    block_hash: [u8; 32],  // Key for assembly
    state_root: [u8; 32],
    computation_time_ms: u64,  // For observability
}

// Step 4: Block Storage subscribes to ALL THREE events
// ↓ Uses Stateful Assembler pattern to buffer components
// ↓ Only writes when all 3 components with matching block_hash are received
```

### 5.1.1 Stateful Assembler Pattern (Block Storage)

Block Storage (Subsystem 2) implements the **Stateful Assembler** pattern to
enable decentralized choreography while maintaining atomicity guarantees.

```rust
/// Block Storage maintains a pending assembly buffer
/// 
/// This is a NECESSARY, CONTAINED piece of statefulness that enables
/// system-wide decentralization. Without it, we'd need a centralized
/// orchestrator (which we rejected in v2.2).
struct BlockAssemblyBuffer {
    /// Pending assemblies keyed by block_hash
    pending: HashMap<[u8; 32], PendingBlockAssembly>,
    
    /// Timeout for incomplete assemblies (default: 30 seconds)
    assembly_timeout: Duration,
}

struct PendingBlockAssembly {
    block_hash: [u8; 32],
    created_at: Instant,
    
    // The three components (None until received)
    validated_block: Option<ValidatedBlock>,
    merkle_root: Option<[u8; 32]>,
    state_root: Option<[u8; 32]>,
}

impl BlockAssemblyBuffer {
    /// Handle incoming BlockValidated event
    fn on_block_validated(&mut self, event: BlockValidated) -> Option<WriteAction> {
        let entry = self.pending
            .entry(event.block_hash)
            .or_insert_with(|| PendingBlockAssembly::new(event.block_hash));
        
        entry.validated_block = Some(event.block);
        self.try_complete_assembly(event.block_hash)
    }
    
    /// Handle incoming MerkleRootComputed event
    fn on_merkle_root(&mut self, event: MerkleRootComputed) -> Option<WriteAction> {
        if let Some(entry) = self.pending.get_mut(&event.block_hash) {
            entry.merkle_root = Some(event.merkle_root);
            return self.try_complete_assembly(event.block_hash);
        }
        // Event arrived before BlockValidated - buffer it
        let entry = self.pending
            .entry(event.block_hash)
            .or_insert_with(|| PendingBlockAssembly::new(event.block_hash));
        entry.merkle_root = Some(event.merkle_root);
        None
    }
    
    /// Handle incoming StateRootComputed event
    fn on_state_root(&mut self, event: StateRootComputed) -> Option<WriteAction> {
        if let Some(entry) = self.pending.get_mut(&event.block_hash) {
            entry.state_root = Some(event.state_root);
            return self.try_complete_assembly(event.block_hash);
        }
        // Event arrived before BlockValidated - buffer it
        let entry = self.pending
            .entry(event.block_hash)
            .or_insert_with(|| PendingBlockAssembly::new(event.block_hash));
        entry.state_root = Some(event.state_root);
        None
    }
    
    /// Check if all components are present and trigger atomic write
    fn try_complete_assembly(&mut self, block_hash: [u8; 32]) -> Option<WriteAction> {
        let entry = self.pending.get(&block_hash)?;
        
        // All three components must be present
        if entry.validated_block.is_some() 
            && entry.merkle_root.is_some() 
            && entry.state_root.is_some() 
        {
            // Remove from pending and return write action
            let complete = self.pending.remove(&block_hash)?;
            return Some(WriteAction::AtomicWrite {
                block: complete.validated_block.unwrap(),
                merkle_root: complete.merkle_root.unwrap(),
                state_root: complete.state_root.unwrap(),
            });
        }
        
        None  // Not yet complete, keep waiting
    }
    
    /// Garbage collect stale pending assemblies (called periodically)
    fn gc_stale_assemblies(&mut self) {
        let now = Instant::now();
        self.pending.retain(|hash, entry| {
            let is_stale = now.duration_since(entry.created_at) > self.assembly_timeout;
            if is_stale {
                log::warn!(
                    "Dropping incomplete block assembly {:?} after {:?} timeout. \
                     Had: block={}, merkle={}, state={}",
                    hash,
                    self.assembly_timeout,
                    entry.validated_block.is_some(),
                    entry.merkle_root.is_some(),
                    entry.state_root.is_some(),
                );
            }
            !is_stale
        });
    }
}
```

**Assembly Timeout Handling:**

| Scenario | Cause | Action |
|----------|-------|--------|
| Timeout with missing merkle_root | Subsystem 3 failed/slow | Log warning, drop assembly, alert ops |
| Timeout with missing state_root | Subsystem 4 failed/slow | Log warning, drop assembly, alert ops |
| Timeout with missing block | Race condition (roots arrived first) | Log debug, drop assembly |

**IMPORTANT: What Changed (v2.2 Choreography Fix)**

| v2.0/v2.1 (Orchestrator - REJECTED) | v2.2 (Choreography - ADOPTED) |
|-------------------------------------|-------------------------------|
| Consensus requests roots via RPC | Subsystems publish roots to bus |
| Consensus waits for responses | Block Storage buffers asynchronously |
| Single bottleneck in Consensus | Parallel independent processing |
| Consensus = god object | Consensus = validation only |
| Hidden latency | Observable per-component metrics |

### 5.1.2 Data Retrieval Pattern (V2.3 - Proof Generation)

In addition to the block creation choreography, the architecture supports a 
**Data Retrieval** workflow for generating Merkle proofs on demand.

**V2.3 Proof Generation Flow:**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│               PROOF GENERATION: DATA RETRIEVAL PATTERN (V2.3)               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   [Light Client (13)]                                                       │
│          │                                                                  │
│          ↓ MerkleProofRequest { tx_hash, reply_to }                         │
│          │                                                                  │
│   [Transaction Indexing (3)]                                                │
│          │                                                                  │
│          ↓ Check local cache for tx location                                │
│          │                                                                  │
│    ┌─────┴─────────────────────────────────────┐                            │
│    ↓ [Cache Hit]                               ↓ [Cache Miss]               │
│    │                                           │                            │
│    │                   GetTransactionHashesRequest { block_hash }           │
│    │                                           ↓                            │
│    │                                   [Block Storage (2)]                  │
│    │                                           │                            │
│    │                         TransactionHashesResponse { hashes }           │
│    │                                           ↓                            │
│    │                                   [Rebuild Merkle Tree]                │
│    │                                           │                            │
│    └─────────────────┬─────────────────────────┘                            │
│                      ↓                                                      │
│             [Generate Merkle Proof]                                         │
│                      │                                                      │
│                      ↓ MerkleProofResponse { proof, merkle_root }           │
│                      │                                                      │
│   [Light Client (13)] ←──────────────────────────────────────────           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**V2.3 Data Retrieval Contract (Block Storage → Transaction Indexing):**

```rust
/// Request for transaction hashes in a specific block
/// 
/// SECURITY (Envelope-Only Identity): No requester_id in payload.
/// Identity derived from AuthenticatedMessage envelope.
struct GetTransactionHashesRequest {
    version: u16,
    correlation_id: [u8; 16],
    reply_to: Topic,
    block_hash: [u8; 32],
    signature: Signature,
}

/// Response containing transaction hashes for a block
struct GetTransactionHashesResponse {
    version: u16,
    correlation_id: [u8; 16],
    block_hash: [u8; 32],
    transaction_hashes: Vec<[u8; 32]>,
    merkle_root: [u8; 32],  // Cached for verification
    signature: Signature,
}
```

**Why This Pattern is Necessary (V2.3 Amendment):**

| Requirement | Without V2.3 | With V2.3 |
|-------------|--------------|-----------|
| Proof for old transaction | Tx Indexing must store ALL tx hashes forever | Query Block Storage on cache miss |
| Memory usage | Unbounded growth | Bounded cache, cold storage fallback |
| Architectural consistency | Missing read path in dependency graph | Complete bidirectional contract |

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

### 5.4.1 Deterministic Trigger Conditions (v2.2 Testability Fix)

**PROBLEM (Fixed in v2.2):** The previous specification was untestable because it used
ambiguous language like "cannot find a chain with a higher finalized checkpoint" without
defining timeouts, peer quorum, or partial response handling.

**MANDATORY: All conditions MUST be expressed as deterministic, measurable predicates.**

```rust
/// Configuration constants for deterministic behavior
/// 
/// These values MUST be configurable but have sensible defaults.
/// All timeouts and thresholds are explicit and testable.
struct CircuitBreakerConfig {
    /// Timeout for sync operation (default: 120 seconds)
    /// After this duration, sync is considered failed.
    sync_timeout: Duration,
    
    /// Maximum sync attempts before entering HALTED state (default: 3)
    max_sync_attempts: u8,
    
    /// Minimum peer responses required for valid sync (default: 3)
    /// Prevents basing decisions on single malicious peer.
    min_peer_quorum: usize,
    
    /// Percentage of peers that must respond within timeout (default: 50%)
    /// If fewer than this respond, treat as network partition.
    peer_response_threshold: f64,
    
    /// Minimum checkpoint height advantage to consider sync successful (default: 1)
    /// Must find a chain at least this many checkpoints ahead.
    min_checkpoint_advantage: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            sync_timeout: Duration::from_secs(120),
            max_sync_attempts: 3,
            min_peer_quorum: 3,
            peer_response_threshold: 0.5,
            min_checkpoint_advantage: 1,
        }
    }
}
```

**ALGORITHM: Deterministic Sync Attempt (Testable)**

```rust
/// Sync Attempt Algorithm - Fully Deterministic
/// 
/// PRECONDITION: Node has detected a finality failure
/// POSTCONDITION: Either node syncs to better chain, or sync_failure_counter++
/// 
/// This algorithm is designed to be FULLY TESTABLE with mocked peers.
async fn execute_sync_attempt(&mut self) -> SyncResult {
    let start_time = Instant::now();
    let config = &self.config;
    
    // ═══════════════════════════════════════════════════════════════════
    // STEP 1: Broadcast ChainHeadRequest to all connected peers
    // ═══════════════════════════════════════════════════════════════════
    let connected_peers = self.peer_manager.get_connected_peers();
    let peer_count = connected_peers.len();
    
    log::info!(
        "Sync attempt {}/{}: broadcasting ChainHeadRequest to {} peers",
        self.sync_failure_counter + 1,
        config.max_sync_attempts,
        peer_count
    );
    
    for peer in &connected_peers {
        self.send_message(peer, ChainHeadRequest {
            our_finalized_checkpoint: self.current_chain.finalized_checkpoint,
            timestamp: now(),
        }).await;
    }
    
    // ═══════════════════════════════════════════════════════════════════
    // STEP 2: Wait for responses with DETERMINISTIC timeout
    // ═══════════════════════════════════════════════════════════════════
    let mut responses: Vec<ChainHeadResponse> = Vec::new();
    let deadline = start_time + config.sync_timeout;
    
    while Instant::now() < deadline {
        if let Some(response) = self.receive_chain_head_response(Duration::from_millis(100)).await {
            responses.push(response);
        }
        
        // Early exit if we have enough responses
        let response_ratio = responses.len() as f64 / peer_count as f64;
        if response_ratio >= config.peer_response_threshold 
            && responses.len() >= config.min_peer_quorum 
        {
            break;
        }
    }
    
    // ═══════════════════════════════════════════════════════════════════
    // STEP 3: Evaluate responses with DETERMINISTIC predicates
    // ═══════════════════════════════════════════════════════════════════
    
    // Predicate A: Did we receive enough responses?
    if responses.len() < config.min_peer_quorum {
        log::warn!(
            "Sync failed: insufficient peer responses ({}/{} required)",
            responses.len(),
            config.min_peer_quorum
        );
        return SyncResult::Failure(SyncFailureReason::InsufficientPeers {
            received: responses.len(),
            required: config.min_peer_quorum,
        });
    }
    
    // Predicate B: Is there a chain with a higher finalized checkpoint?
    let our_checkpoint = self.current_chain.finalized_checkpoint;
    let best_peer_chain = responses
        .iter()
        .max_by_key(|r| r.finalized_checkpoint);
    
    let Some(best) = best_peer_chain else {
        return SyncResult::Failure(SyncFailureReason::NoPeerData);
    };
    
    // Predicate C: Is the advantage significant enough?
    let advantage = best.finalized_checkpoint.saturating_sub(our_checkpoint);
    if advantage < config.min_checkpoint_advantage {
        log::warn!(
            "Sync failed: no chain with sufficient advantage \
             (best peer checkpoint: {}, ours: {}, min advantage: {})",
            best.finalized_checkpoint,
            our_checkpoint,
            config.min_checkpoint_advantage
        );
        return SyncResult::Failure(SyncFailureReason::NoSuperiorChain {
            our_checkpoint,
            best_found: best.finalized_checkpoint,
        });
    }
    
    // ═══════════════════════════════════════════════════════════════════
    // STEP 4: SUCCESS - Reorg to the better chain
    // ═══════════════════════════════════════════════════════════════════
    log::info!(
        "Sync success: found superior chain at checkpoint {} (ours: {})",
        best.finalized_checkpoint,
        our_checkpoint
    );
    
    SyncResult::Success {
        new_checkpoint: best.finalized_checkpoint,
        peer_id: best.peer_id,
    }
}

#[derive(Debug, Clone)]
enum SyncResult {
    Success {
        new_checkpoint: u64,
        peer_id: PeerId,
    },
    Failure(SyncFailureReason),
}

#[derive(Debug, Clone)]
enum SyncFailureReason {
    InsufficientPeers { received: usize, required: usize },
    NoPeerData,
    NoSuperiorChain { our_checkpoint: u64, best_found: u64 },
    Timeout,
}
```

**Testability Matrix:**

| Test Case | Mock Setup | Expected Outcome |
|-----------|------------|------------------|
| Happy path | 5 peers respond with checkpoint+2 | SyncResult::Success |
| Insufficient peers | 2 peers respond | SyncResult::Failure(InsufficientPeers) |
| No superior chain | 5 peers respond with same checkpoint | SyncResult::Failure(NoSuperiorChain) |
| Timeout | Peers don't respond within 120s | SyncResult::Failure(Timeout) |
| Quorum met early | 3 peers respond quickly, 2 slow | SyncResult evaluated at 3 responses |

**Circuit Breaker State Machine (with Deterministic Triggers):**
                }
            }
            Err(e) => {
                log::error!("Failed to identify majority chain: {:?}", e);
                self.sync_failure_counter += 1;
            }
        }
        
        // 5. LIVELOCK PREVENTION: Check if we've exceeded max attempts
        if self.sync_failure_counter >= Self::MAX_SYNC_ATTEMPTS {
            return self.enter_halted_state(reason).await;
        }
        
        // 6. Not halted yet - stay in sync mode, wait for network changes
        log::warn!("Sync attempt {} failed. Remaining in sync mode.", 
            self.sync_failure_counter);
        
        Ok(())
    }
    
    /// Enter HALTED_AWAITING_INTERVENTION state
    /// 
    /// In this state, the node:
    /// - Ceases ALL block production and validation
    /// - Only listens for network state changes
    /// - Requires manual intervention OR significant network recovery
    async fn enter_halted_state(&mut self, reason: SyncReason) -> Result<(), Error> {
        log::error!(
            "CRITICAL: Node entering HALTED state after {} failed sync attempts. \
             Reason: {:?}. Manual intervention required.",
            Self::MAX_SYNC_ATTEMPTS, reason
        );
        
        // Emit critical alert
        self.alert_ops(AlertLevel::Critical, format!(
            "Node halted due to unrecoverable finality failure: {:?}", reason
        ))?;
        
        // Transition to halted state
        self.emit_state_change(NodeState::HaltedAwaitingIntervention, reason).await?;
        
        // The node now enters a minimal operation mode:
        // - No block production
        // - No block validation
        // - Only listening for peer announcements of significantly higher finalized checkpoints
        // - Can be manually recovered via admin API
        
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
    HaltedAwaitingIntervention,  // NEW: Prevents livelock
}
```

**Livelock Prevention Logic:**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    FINALITY CIRCUIT BREAKER STATE MACHINE                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  [PRODUCING] ──finality failure──→ [SYNCING]                                │
│       ↑                                │                                    │
│       │                                ├── find better chain → [PRODUCING]  │
│       │                                │   (reset counter)                  │
│       │                                │                                    │
│       │                                └── no better chain                  │
│       │                                    (increment counter)              │
│       │                                         │                           │
│       │                                         ↓                           │
│       │                              counter < 3? ──yes──→ [SYNCING]        │
│       │                                         │          (retry)          │
│       │                                         no                          │
│       │                                         │                           │
│       │                                         ↓                           │
│       │                              [HALTED_AWAITING_INTERVENTION]         │
│       │                                         │                           │
│       └────────manual intervention──────────────┘                           │
│            OR network recovery with                                         │
│            significantly higher checkpoint                                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key Differences from Standard DLQ Handling:**

| Aspect | Standard DLQ | Finality Circuit Breaker |
|--------|-------------|-------------------------|
| Retry Count | 3-5 attempts | **0 retries (uses sync instead)** |
| Backoff | Exponential | **None** |
| DLQ Routing | Yes | **No** |
| Action | Wait and retry | **State change to Sync Mode** |
| Livelock Prevention | N/A | **HALTED after 3 failed syncs** |
| Goal | Eventually succeed | **Find correct chain or halt safely** |

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