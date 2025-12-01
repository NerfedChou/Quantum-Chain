# SPECIFICATION: BLOCK STORAGE ENGINE

**Version:** 2.2  
**Subsystem ID:** 2  
**Bounded Context:** Persistence & Data Reliability  
**Crate Name:** `crates/block-storage`  
**Author:** Systems Architecture Team  
**Date:** 2024-12-01  
**Architecture Compliance:** Architecture.md v2.3 (Choreography Pattern, Stateful Assembler, Envelope-Only Identity, Deterministic Failure Awareness, Transaction Location Lookup)

---

## 1. ABSTRACT

### 1.1 Purpose

The **Block Storage Engine** subsystem is the system's authoritative source of truth for all persisted blockchain data. It provides a reliable, atomic, and integrity-verified storage layer for blocks, transactions, and associated metadata. The subsystem abstracts over a key-value store backend optimized for LSM Tree implementations (e.g., RocksDB).

### 1.2 Responsibility Boundaries

**In Scope:**
- Persist validated blocks with atomic write guarantees
- Store and retrieve blocks by hash or height
- Maintain block index mapping (height → hash)
- Track finalized block height
- Verify data integrity via checksums on all read operations
- Monitor disk space and reject writes when capacity is critical
- Provide efficient batch/range reads for node syncing
- **V2.3: Provide transaction location lookups for Merkle proof generation**

**Out of Scope:**
- Block validation or consensus logic (handled by Subsystem 8)
- Transaction execution or state transitions (handled by Subsystem 11)
- Merkle tree computation (handled by Subsystem 3)
- State trie management (handled by Subsystem 4)
- Network I/O or peer communication
- Any business logic validation on block contents

**CRITICAL DESIGN CONSTRAINT (V2.3 Choreography + Transaction Lookup):**

This subsystem operates as a **Stateful Assembler** within an event-driven choreography. 
It does NOT receive a pre-assembled "complete package" from any orchestrator.

**Architecture Mandate (Architecture.md v2.2, Section 5.1):**
- The V2.2 architecture REJECTS the "Orchestrator" pattern where a single subsystem 
  assembles all components before storage.
- Instead, each subsystem publishes its results independently to the event bus.
- Block Storage subscribes to multiple event streams and assembles the components itself.

**Stateful Assembler Behavior:**
1. Subscribe to THREE independent events (no single orchestrator):
   - `BlockValidated` from Subsystem 8 (Consensus)
   - `MerkleRootComputed` from Subsystem 3 (Transaction Indexing)  
   - `StateRootComputed` from Subsystem 4 (State Management)
   
2. Buffer incoming components by `block_hash` key until all three arrive

3. When all three components are present for a given `block_hash`:
   - Perform atomic write of the complete block
   - Emit `BlockStored` event
   - Clear the assembly buffer entry

4. Implement Assembly Timeout (resource exhaustion defense):
   - Incomplete assemblies are purged after 30 seconds
   - This prevents memory exhaustion from orphaned partial blocks

### 1.3 Key Design Principles

1. **Dumb Storage Layer:** This subsystem trusts the validity of inputs from authorized sources. It does NOT perform cryptographic validation on block contents—only on the `AuthenticatedMessage` envelope.
2. **Atomic Writes:** Block data and all associated metadata are written as a single atomic batch. Partial writes are impossible.
3. **Integrity First:** Every read operation verifies the stored checksum. Corruption is detected immediately.
4. **Fail-Safe:** Operations fail explicitly when disk space is critical (<5%) rather than risking data corruption.

### 1.4 Trust Model

This subsystem operates within a strict trust boundary under the V2.2 Choreography Pattern:

```
┌─────────────────────────────────────────────────────────────┐
│                    TRUST BOUNDARY (V2.2)                    │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  TRUSTED EVENT INPUTS (via AuthenticatedMessage envelope):  │
│  ├─ Subsystem 8 (Consensus) → BlockValidated event         │
│  ├─ Subsystem 3 (Tx Indexing) → MerkleRootComputed event   │
│  ├─ Subsystem 4 (State Mgmt) → StateRootComputed event     │
│  └─ Subsystem 9 (Finality) → MarkFinalized request         │
│                                                             │
│  CHOREOGRAPHY PATTERN (V2.2 Mandate):                       │
│  ├─ NO single orchestrator assembles the complete package   │
│  ├─ Block Storage buffers components by block_hash         │
│  └─ Atomic write when all 3 components arrive              │
│                                                             │
│  READ ACCESS:                                               │
│  └─ Any authorized subsystem (read-only, no trust needed)   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. DOMAIN MODEL (THE "INNER LAYER")

### 2.1 Shared Types (from `crates/shared-types`)

The following types are **NOT defined in this crate**. They are referenced from the `shared-types` crate to ensure a single source of truth across all subsystems:

```rust
// ============================================================
// FROM: crates/shared-types/src/lib.rs
// DO NOT REDEFINE - IMPORT ONLY
// ============================================================

/// 32-byte hash value
pub use shared_types::Hash;

/// 20-byte address
pub use shared_types::Address;

/// Unix timestamp in seconds
pub use shared_types::Timestamp;

/// ECDSA signature
pub use shared_types::Signature;

/// A validated block received from Consensus
pub use shared_types::ValidatedBlock;

/// Block header containing essential metadata
pub use shared_types::BlockHeader;

/// Transaction structure
pub use shared_types::Transaction;

/// Consensus proof attached to validated block
pub use shared_types::ConsensusProof;

/// Consensus type enum
pub use shared_types::ConsensusType;

/// Validator signature
pub use shared_types::ValidatorSignature;

/// Subsystem identifier
pub use shared_types::SubsystemId;

/// Authenticated message envelope
pub use shared_types::AuthenticatedMessage;

/// Topic for event routing
pub use shared_types::Topic;
```

**Note:** The formal definitions of these types reside in `crates/shared-types`. See that crate's documentation for field-level details. This specification only uses these types by reference.

### 2.2 Domain-Specific Entities (Defined in This Crate)

The following types are **internal to Block Storage** and defined within this crate:

```rust
/// A block stored on disk with integrity checksum
/// 
/// This is the storage-layer wrapper around ValidatedBlock.
/// It adds storage-specific metadata (timestamp, checksum) that
/// are not part of the consensus-validated block structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredBlock {
    /// The complete block data (from shared-types)
    pub block: ValidatedBlock,
    /// Merkle root of transactions (provided by Consensus in WriteBlockRequest)
    pub merkle_root: Hash,
    /// State root after block execution (provided by Consensus in WriteBlockRequest)
    pub state_root: Hash,
    /// Timestamp when block was stored (local storage time, not block time)
    pub stored_at: Timestamp,
    /// CRC32C checksum computed at write time for integrity verification
    pub checksum: u32,
}
```

### 2.3 Index Structures

```rust
/// Mapping from block height to block hash
/// Stored separately for O(1) height-based lookups
pub struct BlockIndex {
    entries: Vec<BlockIndexEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockIndexEntry {
    pub height: u64,
    pub block_hash: Hash,
}

/// Global storage metadata
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageMetadata {
    /// Hash of the genesis block (immutable after first write)
    pub genesis_hash: Hash,
    /// Height of the latest stored block
    pub latest_height: u64,
    /// Height of the latest finalized block
    pub finalized_height: u64,
    /// Total number of blocks stored
    pub total_blocks: u64,
    /// Storage format version for migrations
    pub storage_version: u16,
}
```

### 2.4 Stateful Assembler Structures (V2.2 Choreography)

These structures implement the "Stateful Assembler" pattern mandated by Architecture.md v2.2.
Block Storage buffers incoming event components until all three are present, then performs
an atomic write.

```rust
use std::collections::HashMap;

/// Buffer for assembling block components from multiple subsystems.
/// 
/// ARCHITECTURE (V2.2 Choreography Pattern):
/// Unlike the rejected "Orchestrator" pattern where Consensus would assemble
/// a complete package, this subsystem receives THREE independent events:
/// - BlockValidated (from Consensus)
/// - MerkleRootComputed (from Transaction Indexing)
/// - StateRootComputed (from State Management)
/// 
/// Each event may arrive in any order. This buffer holds partial assemblies
/// until all components are present.
/// 
/// SECURITY (Resource Exhaustion Defense):
/// Entries are purged after `assembly_timeout_secs` to prevent memory exhaustion
/// from orphaned partial blocks (e.g., if one subsystem fails to emit its event).
pub struct BlockAssemblyBuffer {
    /// Pending assemblies keyed by block_hash
    pending: HashMap<Hash, PendingBlockAssembly>,
    /// Configuration for assembly behavior
    config: AssemblyConfig,
}

/// A partial block assembly awaiting completion
#[derive(Debug, Clone)]
pub struct PendingBlockAssembly {
    /// Block hash (key for this assembly)
    pub block_hash: Hash,
    /// Block height for ordering
    pub block_height: u64,
    /// When this assembly was first started (for timeout)
    pub started_at: Timestamp,
    /// The validated block (from Consensus, Subsystem 8)
    pub validated_block: Option<ValidatedBlock>,
    /// Correlation ID from BlockValidated event (for tracing)
    pub block_correlation_id: Option<[u8; 16]>,
    /// Merkle root of transactions (from Tx Indexing, Subsystem 3)
    pub merkle_root: Option<Hash>,
    /// Correlation ID from MerkleRootComputed event
    pub merkle_correlation_id: Option<[u8; 16]>,
    /// State root after execution (from State Management, Subsystem 4)
    pub state_root: Option<Hash>,
    /// Correlation ID from StateRootComputed event
    pub state_correlation_id: Option<[u8; 16]>,
}

impl PendingBlockAssembly {
    /// Check if all three components are present
    pub fn is_complete(&self) -> bool {
        self.validated_block.is_some() 
            && self.merkle_root.is_some() 
            && self.state_root.is_some()
    }
    
    /// Check if this assembly has timed out
    pub fn is_expired(&self, now: Timestamp, timeout_secs: u64) -> bool {
        now.0.saturating_sub(self.started_at.0) > timeout_secs
    }
}

/// Configuration for the assembly buffer
#[derive(Debug, Clone)]
pub struct AssemblyConfig {
    /// Maximum time to wait for all components before purging (default: 30 seconds)
    /// 
    /// SECURITY: This prevents memory exhaustion from orphaned partial blocks.
    /// If a subsystem fails to emit its event, the partial assembly is cleaned up.
    pub assembly_timeout_secs: u64,
    /// Maximum number of pending assemblies (default: 1000)
    /// 
    /// SECURITY: Bounds memory usage. If exceeded, oldest entries are purged.
    pub max_pending_assemblies: usize,
}

impl Default for AssemblyConfig {
    fn default() -> Self {
        Self {
            assembly_timeout_secs: 30,
            max_pending_assemblies: 1000,
        }
    }
}
```

### 2.5 Value Objects

```rust
/// Configuration for the storage engine
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// Minimum required disk space percentage (default: 5%)
    pub min_disk_space_percent: u8,
    /// Enable checksum verification on reads (default: true)
    pub verify_checksums: bool,
    /// Maximum block size in bytes (default: 10MB)
    pub max_block_size: usize,
    /// Compaction strategy
    pub compaction_strategy: CompactionStrategy,
    /// Assembly buffer configuration (V2.2 Choreography)
    pub assembly_config: AssemblyConfig,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            min_disk_space_percent: 5,
            verify_checksums: true,
            max_block_size: 10 * 1024 * 1024, // 10 MB
            compaction_strategy: CompactionStrategy::LeveledCompaction,
            assembly_config: AssemblyConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionStrategy {
    LeveledCompaction,
    SizeTieredCompaction,
}

/// Key prefixes for the key-value store
#[derive(Debug, Clone, Copy)]
pub enum KeyPrefix {
    Block,           // b:{hash} -> StoredBlock
    BlockByHeight,   // h:{height} -> Hash
    Metadata,        // m:metadata -> StorageMetadata
    MerkleRoot,      // r:{height} -> Hash
    StateRoot,       // s:{height} -> Hash
}

impl KeyPrefix {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            KeyPrefix::Block => b"b:",
            KeyPrefix::BlockByHeight => b"h:",
            KeyPrefix::Metadata => b"m:",
            KeyPrefix::MerkleRoot => b"r:",
            KeyPrefix::StateRoot => b"s:",
        }
    }
}
```

### 2.6 Domain Invariants

**INVARIANT-1: Sequential Blocks (Parent Chain Continuity)**
```
∀ block B at height N where N > 0:
    ∃ block P at height N-1 where P.block_hash == B.parent_hash
    
Exception: Genesis block (height = 0) has no parent requirement
```

**INVARIANT-2: Disk Space Safety**
```
∀ write operation W:
    available_disk_space() >= min_disk_space_percent (5%)
    OR W fails with StorageError::DiskFull
```

**INVARIANT-3: Data Integrity (Checksum Verification)**
```
∀ read operation R on StoredBlock B:
    compute_checksum(B.block, B.merkle_root, B.state_root) == B.checksum
    OR R fails with StorageError::DataCorruption
```

**INVARIANT-4: Atomic Writes**
```
∀ write_block operation:
    ALL of (block, index_entry, merkle_root, state_root) are written
    OR NONE are written
    
No partial state is possible.
```

**INVARIANT-5: Finalization Monotonicity**
```
∀ mark_finalized(height):
    height > current_finalized_height
    AND block_exists(height)
    
Finalization cannot regress.
```

**INVARIANT-6: Genesis Immutability**
```
Once StorageMetadata.genesis_hash is set:
    StorageMetadata.genesis_hash NEVER changes
```

**INVARIANT-7: Assembly Timeout (V2.2 Resource Exhaustion Defense)**
```
∀ pending_assembly A in BlockAssemblyBuffer:
    IF now - A.started_at > assembly_timeout_secs THEN
        A is PURGED from buffer
        Log warning with block_hash for debugging
        
RATIONALE:
    - Prevents memory exhaustion from orphaned partial blocks
    - If one of the three subsystems fails to emit, assembly is cleaned up
    - Default timeout: 30 seconds (configurable)
```

**INVARIANT-8: Bounded Assembly Buffer (V2.2 Memory Safety)**
```
ALWAYS: pending_assemblies.len() ≤ max_pending_assemblies (default: 1000)

ENFORCEMENT:
    IF pending_assemblies.len() >= max_pending_assemblies THEN
        Purge OLDEST incomplete assembly (by started_at)
        Log warning with purged block_hash
```

---

## 3. PORTS & INTERFACES (THE "HEXAGON")

### 3.1 Driving Ports (Inbound API)

These are the public APIs this library exposes to the application.

```rust
/// Primary API for the Block Storage subsystem
pub trait BlockStorageApi {
    /// Write a validated block with its associated roots
    /// 
    /// # Atomicity
    /// This operation is atomic. Either all data is written or none.
    /// 
    /// # Errors
    /// - `DiskFull`: Available disk space < 5%
    /// - `ParentNotFound`: Parent block does not exist (violates INVARIANT-1)
    /// - `BlockExists`: Block with this hash already stored
    /// - `BlockTooLarge`: Block exceeds maximum size limit
    fn write_block(
        &mut self,
        block: ValidatedBlock,
        merkle_root: Hash,
        state_root: Hash,
    ) -> Result<(), StorageError>;
    
    /// Read a block by its hash
    /// 
    /// # Integrity
    /// Checksum is verified before returning. Corrupted data raises error.
    /// 
    /// # Errors
    /// - `BlockNotFound`: No block with this hash exists
    /// - `DataCorruption`: Checksum mismatch detected
    fn read_block(&self, hash: Hash) -> Result<StoredBlock, StorageError>;
    
    /// Read a block by its height
    /// 
    /// # Errors
    /// - `BlockNotFound`: No block at this height
    /// - `DataCorruption`: Checksum mismatch detected
    fn read_block_by_height(&self, height: u64) -> Result<StoredBlock, StorageError>;
    
    /// Read a range of blocks by height (for node syncing)
    /// 
    /// # Performance
    /// This is optimized for sequential reads and is the preferred
    /// method for syncing nodes that need multiple consecutive blocks.
    /// 
    /// # Parameters
    /// - `start_height`: First block height to read (inclusive)
    /// - `limit`: Maximum number of blocks to return (capped at 100)
    /// 
    /// # Returns
    /// Vector of StoredBlocks in ascending height order.
    /// May return fewer blocks than `limit` if end of chain reached.
    /// 
    /// # Errors
    /// - `HeightNotFound`: start_height does not exist
    /// - `DataCorruption`: Checksum mismatch detected in any block
    fn read_block_range(&self, start_height: u64, limit: u64) -> Result<Vec<StoredBlock>, StorageError>;
    
    /// Mark a block height as finalized
    /// 
    /// # Errors
    /// - `BlockNotFound`: No block at this height
    /// - `InvalidFinalization`: Height <= current finalized height
    fn mark_finalized(&mut self, height: u64) -> Result<(), StorageError>;
    
    /// Get the current storage metadata
    fn get_metadata(&self) -> Result<StorageMetadata, StorageError>;
    
    /// Get the latest block height
    fn get_latest_height(&self) -> Result<u64, StorageError>;
    
    /// Get the finalized block height
    fn get_finalized_height(&self) -> Result<u64, StorageError>;
    
    /// Check if a block exists by hash
    fn block_exists(&self, hash: Hash) -> bool;
    
    /// Check if a block exists at height
    fn block_exists_at_height(&self, height: u64) -> bool;
    
    /// V2.3: Get the location of a transaction by its hash
    /// 
    /// This API supports Transaction Indexing (Subsystem 3) for Merkle proof generation.
    /// Returns the block and position where a transaction is stored.
    /// 
    /// # Parameters
    /// - `transaction_hash`: Hash of the transaction to locate
    /// 
    /// # Returns
    /// - `Ok(TransactionLocation)`: Location data including block_hash, height, and index
    /// - `Err(TransactionNotFound)`: Transaction not in any stored block
    fn get_transaction_location(
        &self,
        transaction_hash: Hash,
    ) -> Result<TransactionLocation, StorageError>;
    
    /// V2.3: Get ONLY the list of transaction hashes for a given block.
    /// 
    /// This is a performance-optimized endpoint for the Transaction Indexing
    /// subsystem to use for rebuilding Merkle trees for proof generation.
    /// Returns transaction hashes in canonical order (same order as stored).
    /// 
    /// # Parameters
    /// - `block_hash`: Hash of the block to get transaction hashes for
    /// 
    /// # Returns
    /// - `Ok(Vec<Hash>)`: Transaction hashes in canonical order
    /// - `Err(BlockNotFound)`: Block with this hash not found
    fn get_transaction_hashes_for_block(
        &self,
        block_hash: Hash,
    ) -> Result<Vec<Hash>, StorageError>;
}

/// V2.3: Location of a transaction within a stored block
/// Used by Transaction Indexing for Merkle proof generation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionLocation {
    /// Hash of the block containing this transaction
    pub block_hash: Hash,
    /// Height of the block containing this transaction
    pub block_height: u64,
    /// Index of the transaction within the block's transaction list
    pub transaction_index: usize,
    /// Cached Merkle root (for efficient proof generation)
    pub merkle_root: Hash,
}

/// Errors that can occur during storage operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    /// Block with this hash was not found
    BlockNotFound { hash: Hash },
    /// No block exists at this height
    HeightNotFound { height: u64 },
    /// Block with this hash already exists
    BlockExists { hash: Hash },
    /// Parent block not found (INVARIANT-1 violation)
    ParentNotFound { parent_hash: Hash },
    /// Disk space below minimum threshold (INVARIANT-2 violation)
    DiskFull { available_percent: u8 },
    /// Checksum mismatch detected (INVARIANT-3 violation)
    DataCorruption { 
        block_hash: Hash, 
        expected_checksum: u32, 
        actual_checksum: u32 
    },
    /// Block exceeds maximum size limit
    BlockTooLarge { size: usize, max_size: usize },
    /// Finalization height invalid (must be > current)
    InvalidFinalization { 
        requested: u64, 
        current: u64 
    },
    /// V2.3: Transaction not found in any stored block
    TransactionNotFound { tx_hash: Hash },
    /// Database I/O error
    DatabaseError { message: String },
    /// Serialization/deserialization error
    SerializationError { message: String },
}
```

### 3.2 Driven Ports (Outbound SPI)

These are the interfaces this library **requires** the host application to implement.

```rust
/// Abstract interface for key-value database operations
/// Implementations: RocksDB, LevelDB, LMDB, etc.
pub trait KeyValueStore: Send + Sync {
    /// Get a value by key
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, KVStoreError>;
    
    /// Put a single key-value pair
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), KVStoreError>;
    
    /// Delete a key
    fn delete(&mut self, key: &[u8]) -> Result<(), KVStoreError>;
    
    /// Execute an atomic batch write
    /// 
    /// # Atomicity Guarantee
    /// Either ALL operations in the batch succeed, or NONE are applied.
    /// This is critical for INVARIANT-4.
    fn atomic_batch_write(&mut self, operations: Vec<BatchOperation>) -> Result<(), KVStoreError>;
    
    /// Check if a key exists
    fn exists(&self, key: &[u8]) -> Result<bool, KVStoreError>;
    
    /// Iterate over keys with a prefix
    fn prefix_scan(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, KVStoreError>;
}

/// Batch operation for atomic writes
#[derive(Debug, Clone)]
pub enum BatchOperation {
    Put { key: Vec<u8>, value: Vec<u8> },
    Delete { key: Vec<u8> },
}

#[derive(Debug)]
pub enum KVStoreError {
    IOError { message: String },
    CorruptionError { message: String },
    NotFound,
}

/// Abstract interface for filesystem operations
pub trait FileSystemAdapter: Send + Sync {
    /// Get available disk space as a percentage (0-100)
    fn available_disk_space_percent(&self) -> Result<u8, FSError>;
    
    /// Get available disk space in bytes
    fn available_disk_space_bytes(&self) -> Result<u64, FSError>;
    
    /// Get total disk space in bytes
    fn total_disk_space_bytes(&self) -> Result<u64, FSError>;
}

#[derive(Debug)]
pub enum FSError {
    IOError { message: String },
    PermissionDenied,
}

/// Abstract interface for checksum computation
pub trait ChecksumProvider: Send + Sync {
    /// Compute CRC32C checksum of data
    fn compute_crc32c(&self, data: &[u8]) -> u32;
    
    /// Verify CRC32C checksum matches
    fn verify_crc32c(&self, data: &[u8], expected: u32) -> bool;
}

/// Abstract interface for time operations (for testability)
pub trait TimeSource: Send + Sync {
    /// Get current timestamp
    fn now(&self) -> Timestamp;
}

/// Abstract interface for serialization
pub trait BlockSerializer: Send + Sync {
    /// Serialize a StoredBlock to bytes
    fn serialize(&self, block: &StoredBlock) -> Result<Vec<u8>, SerializationError>;
    
    /// Deserialize bytes to a StoredBlock
    fn deserialize(&self, data: &[u8]) -> Result<StoredBlock, SerializationError>;
}

#[derive(Debug)]
pub struct SerializationError {
    pub message: String,
}
```

---

## 4. EVENT SCHEMA (EDA)

**IMPORTANT:** All events in this section are **payloads** within the `AuthenticatedMessage<T>` envelope defined in Architecture.md Section 3.2. They are NOT standalone structs. Every IPC message MUST include the mandatory envelope fields: `version`, `correlation_id`, `reply_to`, `sender_id`, `recipient_id`, `timestamp`, `nonce`, and `signature`.

**SECURITY (Envelope-Only Identity - Architecture.md v2.2 Amendment 3.2.1):**
All payloads in this section contain NO identity fields (e.g., `requester_id`, `sender_id`).
The sender's identity is derived SOLELY from the `AuthenticatedMessage` envelope's `sender_id`
field, which is cryptographically signed. This prevents "Payload Impersonation" attacks.

### 4.1 Incoming Event Subscriptions (V2.2 Choreography Pattern)

**ARCHITECTURAL MANDATE:** This subsystem does NOT listen for a single `WriteBlockRequest`.
Instead, it subscribes to THREE independent event streams per the V2.2 Choreography Pattern.
Each event may arrive in any order; the Stateful Assembler buffers them by `block_hash`.

```rust
/// Events this subsystem SUBSCRIBES TO (not requests, but published events)
/// 
/// CHOREOGRAPHY PATTERN (V2.2 - Architecture.md Section 5.1):
/// Block Storage assembles blocks from three independent event streams.
/// There is NO orchestrator - each subsystem publishes independently.
/// 
/// SECURITY (Envelope-Only Identity):
/// Sender identity is derived from envelope.sender_id only.
/// These payloads contain NO identity fields.

// ============================================================
// EVENT 1: BlockValidated (from Consensus, Subsystem 8)
// ============================================================

/// Published by Consensus when a block passes validation.
/// Block Storage buffers this until MerkleRootComputed and StateRootComputed arrive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockValidatedPayload {
    /// The consensus-validated block
    pub block: ValidatedBlock,
    /// Block hash for correlation with other events
    pub block_hash: Hash,
    /// Block height for ordering
    pub block_height: u64,
}

// ============================================================
// EVENT 2: MerkleRootComputed (from Transaction Indexing, Subsystem 3)
// ============================================================

/// Published by Transaction Indexing when Merkle tree is computed.
/// Block Storage buffers this until BlockValidated and StateRootComputed arrive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerkleRootComputedPayload {
    /// Block hash to correlate with other components
    pub block_hash: Hash,
    /// The computed Merkle root of transactions
    pub merkle_root: Hash,
}

// ============================================================
// EVENT 3: StateRootComputed (from State Management, Subsystem 4)
// ============================================================

/// Published by State Management after state transitions are applied.
/// Block Storage buffers this until BlockValidated and MerkleRootComputed arrive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateRootComputedPayload {
    /// Block hash to correlate with other components
    pub block_hash: Hash,
    /// The state root after executing this block
    pub state_root: Hash,
}
```

### 4.2 Incoming Request Payloads

These are request payloads (requiring a response) that Block Storage handles:

```rust
/// Request payloads this subsystem handles
/// 
/// SECURITY (Envelope-Only Identity - V2.2):
/// These payloads contain NO identity fields. Sender identity
/// is derived from the AuthenticatedMessage envelope only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockStorageRequestPayload {
    /// Mark a block as finalized
    /// Allowed sender: Subsystem 9 (Finality) ONLY
    MarkFinalized(MarkFinalizedRequestPayload),
    
    /// Read a single block (request/response pattern)
    /// Allowed sender: Any authorized subsystem
    ReadBlock(ReadBlockRequestPayload),
    
    /// Read a range of blocks (for node syncing)
    /// Allowed sender: Any authorized subsystem
    ReadBlockRange(ReadBlockRangeRequestPayload),
    
    /// V2.3: Get transaction location for Merkle proof generation
    /// Allowed sender: Subsystem 3 (Transaction Indexing) ONLY
    GetTransactionLocation(GetTransactionLocationRequestPayload),
    
    /// V2.3: Get transaction hashes for a block (Merkle tree reconstruction)
    /// Allowed sender: Subsystem 3 (Transaction Indexing) ONLY
    GetTransactionHashes(GetTransactionHashesRequestPayload),
}

/// V2.3: Request for transaction location from Transaction Indexing
/// Enables Merkle proof generation by querying stored transaction positions
/// 
/// SECURITY (Envelope-Only Identity): No requester_id in payload.
/// Identity is derived from AuthenticatedMessage envelope.sender_id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetTransactionLocationRequestPayload {
    /// Hash of the transaction to locate
    pub transaction_hash: Hash,
}

/// V2.3: Request for transaction hashes in a block from Transaction Indexing
/// Enables Merkle tree reconstruction for proof generation on cache miss
/// 
/// SECURITY (Envelope-Only Identity): No requester_id in payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetTransactionHashesRequestPayload {
    /// Hash of the block to get transaction hashes for
    pub block_hash: Hash,
}

/// Mark finalized request from Finality subsystem
/// 
/// SECURITY (Envelope-Only Identity): No requester_id field.
/// Sender verified via envelope.sender_id.
/// 
/// ARCHITECTURAL CONTEXT (Finality Liveness - V2.2):
/// The Finality subsystem (Subsystem 9) uses a deterministic circuit breaker
/// as defined in Architecture.md Section 5.4.1. If the Finality subsystem
/// enters a HALTED state (after exceeding the maximum failed sync attempts),
/// this MarkFinalizedRequest will CEASE to be emitted until the node is
/// manually recovered. 
/// 
/// Operators diagnosing "blocks not being finalized" should check:
/// 1. Finality subsystem circuit breaker state
/// 2. Finality sync attempt counters
/// 3. Network connectivity to other validators
/// 
/// The circuit breaker triggers are deterministic and testable:
/// - Consecutive failures threshold (configurable)
/// - Time-based failure window
/// - Manual reset required after HALTED state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkFinalizedRequestPayload {
    pub block_height: u64,
    /// FinalityProof from shared-types (not redefined here)
    pub finality_proof: FinalityProof,
}

/// Read block request (requires response via correlation_id)
/// 
/// SECURITY (Envelope-Only Identity): No requester_id field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadBlockRequestPayload {
    /// Read by hash or by height
    pub query: BlockQuery,
}

/// Read block range request for efficient batch reads (node syncing)
/// 
/// SECURITY (Envelope-Only Identity): No requester_id field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadBlockRangeRequestPayload {
    /// First block height to read (inclusive)
    pub start_height: u64,
    /// Maximum number of blocks to return (capped at 100 by subsystem)
    pub limit: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockQuery {
    ByHash(Hash),
    ByHeight(u64),
}

// Note: FinalityProof, Attestation are from shared-types crate
// They are NOT redefined here to maintain single source of truth
```

### 4.3 Outgoing Event Payloads

These are the payload types that Block Storage publishes:

```rust
/// Events emitted by the Block Storage subsystem
/// 
/// USAGE: These are payloads wrapped in AuthenticatedMessage<T>.
/// Example: AuthenticatedMessage<BlockStoredPayload>
/// 
/// SECURITY (Envelope-Only Identity): No identity fields in payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockStorageEventPayload {
    /// A block was successfully stored (all 3 components assembled)
    BlockStored(BlockStoredPayload),
    
    /// A block was marked as finalized
    BlockFinalized(BlockFinalizedPayload),
    
    /// Response to a read block request (single block)
    ReadBlockResponse(ReadBlockResponsePayload),
    
    /// Response to a read block range request (batch)
    ReadBlockRangeResponse(ReadBlockRangeResponsePayload),
    
    /// V2.3: Response to transaction location request
    TransactionLocationResponse(TransactionLocationResponsePayload),
    
    /// V2.3: Response to transaction hashes request
    TransactionHashesResponse(TransactionHashesResponsePayload),
    
    /// Critical storage error occurred
    StorageCritical(StorageCriticalPayload),
    
    /// Assembly timeout warning (V2.2 - partial block purged)
    AssemblyTimeout(AssemblyTimeoutPayload),
}

/// Emitted when a block is successfully written
/// 
/// V2.2: This is emitted only after ALL THREE components are assembled:
/// - BlockValidated from Consensus (8)
/// - MerkleRootComputed from Transaction Indexing (3)
/// - StateRootComputed from State Management (4)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockStoredPayload {
    pub block_height: u64,
    pub block_hash: Hash,
    pub merkle_root: Hash,
    pub state_root: Hash,
    pub stored_at: Timestamp,
}

/// Emitted when a block is marked as finalized
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockFinalizedPayload {
    pub block_height: u64,
    pub block_hash: Hash,
    pub previous_finalized_height: u64,
}

/// Response to ReadBlockRequest (correlated via correlation_id)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadBlockResponsePayload {
    pub result: Result<StoredBlock, StorageErrorPayload>,
}

/// Response to ReadBlockRangeRequest (correlated via correlation_id)
/// Used for efficient batch reads during node syncing
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadBlockRangeResponsePayload {
    /// Blocks in ascending height order
    pub blocks: Vec<StoredBlock>,
    /// The height of the last block in the chain (for pagination)
    pub chain_tip_height: u64,
    /// Whether more blocks exist after this range
    pub has_more: bool,
}

/// V2.3: Response to GetTransactionLocationRequest
/// Enables Transaction Indexing to generate Merkle proofs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionLocationResponsePayload {
    /// The transaction hash that was queried
    pub transaction_hash: Hash,
    /// The result of the location lookup
    pub result: Result<TransactionLocation, StorageErrorPayload>,
}

/// V2.3: Response to GetTransactionHashesRequest
/// Provides all transaction hashes for Merkle tree reconstruction
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionHashesResponsePayload {
    /// The block hash that was queried
    pub block_hash: Hash,
    /// Transaction hashes in canonical order, or error
    pub result: Result<TransactionHashesData, StorageErrorPayload>,
}

/// V2.3: Transaction hashes data for Merkle tree reconstruction
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionHashesData {
    /// All transaction hashes in the block, in canonical order
    pub transaction_hashes: Vec<Hash>,
    /// Cached Merkle root for verification
    pub merkle_root: Hash,
}

/// Serializable version of StorageError for IPC
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageErrorPayload {
    pub error_type: StorageErrorType,
    pub message: String,
    pub block_hash: Option<Hash>,
    pub block_height: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageErrorType {
    BlockNotFound,
    HeightNotFound,
    DataCorruption,
    DatabaseError,
    /// V2.3: Transaction not found in any stored block
    TransactionNotFound,
}

/// Critical error that may require DLQ handling
/// 
/// DLQ CANDIDATE: If processing of this event fails downstream,
/// it MUST be routed to dlq.storage.critical for manual intervention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageCriticalPayload {
    pub error_type: CriticalErrorType,
    pub message: String,
    pub affected_block: Option<Hash>,
    pub affected_height: Option<u64>,
    pub timestamp: Timestamp,
    pub requires_manual_intervention: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CriticalErrorType {
    /// Disk space critically low
    DiskFull,
    /// Data corruption detected
    DataCorruption,
    /// Database engine failure
    DatabaseFailure,
    /// Unrecoverable I/O error
    IOFailure,
}

/// Emitted when a partial block assembly times out (V2.2 Choreography)
/// 
/// This indicates that one or more of the three required events
/// (BlockValidated, MerkleRootComputed, StateRootComputed) did not arrive
/// within the assembly timeout window.
/// 
/// This may indicate:
/// - A subsystem failure (Consensus, TxIndexing, or StateManagement)
/// - Network partition between subsystems
/// - Event bus delivery failure
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblyTimeoutPayload {
    /// Block hash of the incomplete assembly
    pub block_hash: Hash,
    /// Block height (if BlockValidated was received)
    pub block_height: Option<u64>,
    /// Which components were present at timeout
    pub had_validated_block: bool,
    pub had_merkle_root: bool,
    pub had_state_root: bool,
    /// How long the assembly was pending
    pub pending_duration_secs: u64,
    /// Timestamp of purge
    pub purged_at: Timestamp,
}
```

### 4.4 Stateful Assembler Event Handling (V2.2 Choreography)

This section describes how Block Storage handles the three incoming events and assembles them into a complete block:

```rust
impl BlockStorage {
    /// Handle incoming BlockValidated event from Consensus (Subsystem 8)
    async fn handle_block_validated(
        &mut self,
        msg: AuthenticatedMessage<BlockValidatedPayload>
    ) -> Result<(), Error> {
        // Step 1: Validate envelope (version, signature, timestamp)
        self.verify_envelope(&msg)?;
        
        // Step 2: Verify sender is Consensus (Subsystem 8)
        if msg.sender_id != SubsystemId::Consensus {
            return Err(Error::UnauthorizedSender { 
                sender: msg.sender_id,
                expected: SubsystemId::Consensus,
            });
        }
        
        // Step 3: Get or create pending assembly for this block_hash
        let assembly = self.assembly_buffer
            .entry(msg.payload.block_hash)
            .or_insert_with(|| PendingBlockAssembly {
                block_hash: msg.payload.block_hash,
                block_height: msg.payload.block_height,
                started_at: self.time_source.now(),
                validated_block: None,
                block_correlation_id: None,
                merkle_root: None,
                merkle_correlation_id: None,
                state_root: None,
                state_correlation_id: None,
            });
        
        // Step 4: Store the validated block component
        assembly.validated_block = Some(msg.payload.block.clone());
        assembly.block_correlation_id = Some(msg.correlation_id);
        
        // Step 5: Check if assembly is now complete
        self.try_complete_assembly(msg.payload.block_hash).await
    }
    
    /// Handle incoming MerkleRootComputed event from Transaction Indexing (Subsystem 3)
    async fn handle_merkle_root_computed(
        &mut self,
        msg: AuthenticatedMessage<MerkleRootComputedPayload>
    ) -> Result<(), Error> {
        self.verify_envelope(&msg)?;
        
        if msg.sender_id != SubsystemId::TransactionIndexing {
            return Err(Error::UnauthorizedSender { 
                sender: msg.sender_id,
                expected: SubsystemId::TransactionIndexing,
            });
        }
        
        let assembly = self.assembly_buffer
            .entry(msg.payload.block_hash)
            .or_insert_with(|| PendingBlockAssembly::new_empty(
                msg.payload.block_hash, 
                self.time_source.now()
            ));
        
        assembly.merkle_root = Some(msg.payload.merkle_root);
        assembly.merkle_correlation_id = Some(msg.correlation_id);
        
        self.try_complete_assembly(msg.payload.block_hash).await
    }
    
    /// Handle incoming StateRootComputed event from State Management (Subsystem 4)
    async fn handle_state_root_computed(
        &mut self,
        msg: AuthenticatedMessage<StateRootComputedPayload>
    ) -> Result<(), Error> {
        self.verify_envelope(&msg)?;
        
        if msg.sender_id != SubsystemId::StateManagement {
            return Err(Error::UnauthorizedSender { 
                sender: msg.sender_id,
                expected: SubsystemId::StateManagement,
            });
        }
        
        let assembly = self.assembly_buffer
            .entry(msg.payload.block_hash)
            .or_insert_with(|| PendingBlockAssembly::new_empty(
                msg.payload.block_hash, 
                self.time_source.now()
            ));
        
        assembly.state_root = Some(msg.payload.state_root);
        assembly.state_correlation_id = Some(msg.correlation_id);
        
        self.try_complete_assembly(msg.payload.block_hash).await
    }
    
    /// Attempt to complete assembly if all components are present
    async fn try_complete_assembly(&mut self, block_hash: Hash) -> Result<(), Error> {
        let assembly = match self.assembly_buffer.get(&block_hash) {
            Some(a) if a.is_complete() => self.assembly_buffer.remove(&block_hash).unwrap(),
            _ => return Ok(()), // Not yet complete, wait for more events
        };
        
        // All three components present - perform atomic write
        let validated_block = assembly.validated_block.unwrap();
        let merkle_root = assembly.merkle_root.unwrap();
        let state_root = assembly.state_root.unwrap();
        
        // Check disk space (INVARIANT-2)
        self.check_disk_space()?;
        
        // Verify parent exists (INVARIANT-1)
        self.verify_parent_exists(&validated_block)?;
        
        // Compute checksum and prepare StoredBlock
        let stored_block = StoredBlock {
            block: validated_block.clone(),
            merkle_root,
            state_root,
            stored_at: self.time_source.now(),
            checksum: self.compute_checksum(&validated_block, &merkle_root, &state_root),
        };
        
        // Atomic batch write (INVARIANT-4)
        let operations = self.prepare_write_operations(&stored_block)?;
        self.kv_store.atomic_batch_write(operations)?;
        
        // Emit BlockStored event
        self.emit_block_stored(assembly.block_height, block_hash, merkle_root, state_root).await
    }
    
    /// Periodic garbage collection of timed-out assemblies (INVARIANT-7)
    async fn gc_expired_assemblies(&mut self) {
        let now = self.time_source.now();
        let timeout = self.config.assembly_config.assembly_timeout_secs;
        
        let expired: Vec<Hash> = self.assembly_buffer
            .iter()
            .filter(|(_, a)| a.is_expired(now, timeout))
            .map(|(h, _)| *h)
            .collect();
        
        for block_hash in expired {
            if let Some(assembly) = self.assembly_buffer.remove(&block_hash) {
                // Emit warning event
                self.emit_assembly_timeout(AssemblyTimeoutPayload {
                    block_hash,
                    block_height: assembly.validated_block.as_ref().map(|b| b.header.block_height),
                    had_validated_block: assembly.validated_block.is_some(),
                    had_merkle_root: assembly.merkle_root.is_some(),
                    had_state_root: assembly.state_root.is_some(),
                    pending_duration_secs: now.0.saturating_sub(assembly.started_at.0),
                    purged_at: now,
                }).await;
                
                log::warn!(
                    "Assembly timeout: block_hash={:?}, had_block={}, had_merkle={}, had_state={}",
                    block_hash,
                    assembly.validated_block.is_some(),
                    assembly.merkle_root.is_some(),
                    assembly.state_root.is_some()
                );
            }
        }
    }
}
```

### 4.5 Request/Response Flow Example (ReadBlock)

Per Architecture.md Section 3.3, all request/response flows MUST use the correlation ID pattern. Here is the complete flow for reading a block:

```rust
// ============================================================
// REQUESTER SIDE (e.g., Consensus - Subsystem 8)
// ============================================================

impl Consensus {
    /// Request a block from Block Storage (NON-BLOCKING)
    async fn request_block(&self, block_hash: Hash) -> Result<(), Error> {
        // Step 1: Generate unique correlation ID
        let correlation_id = Uuid::new_v4();
        
        // Step 2: Store pending request for later matching
        self.pending_requests.insert(correlation_id, PendingRequest {
            created_at: Instant::now(),
            timeout: Duration::from_secs(30),
            request_type: RequestType::ReadBlock,
        });
        
        // Step 3: Construct the full authenticated message
        let message = AuthenticatedMessage {
            // === MANDATORY HEADER FIELDS ===
            version: PROTOCOL_VERSION,           // e.g., 1
            sender_id: SubsystemId::Consensus,
            recipient_id: SubsystemId::BlockStorage,
            correlation_id: correlation_id.as_bytes().clone(),
            reply_to: Some(Topic {
                subsystem_id: SubsystemId::Consensus,
                channel: "responses".into(),
            }),
            timestamp: self.time_source.now(),
            nonce: self.nonce_generator.next(),
            signature: [0u8; 32],                // Computed below
            
            // === PAYLOAD ===
            payload: ReadBlockRequestPayload {
                query: BlockQuery::ByHash(block_hash),
            },
        };
        
        // Step 4: Sign the message
        let signed_message = message.sign(&self.shared_secret);
        
        // Step 5: Publish to event bus (NON-BLOCKING - returns immediately)
        self.event_bus.publish("block-storage.requests", signed_message).await?;
        
        // DO NOT AWAIT RESPONSE HERE - continue processing other work
        Ok(())
    }
    
    /// Handle responses from Block Storage (separate async handler)
    async fn handle_read_block_response(
        &mut self, 
        msg: AuthenticatedMessage<ReadBlockResponsePayload>
    ) {
        // Step 1: Validate the envelope (version, signature, timestamp, etc.)
        if let Err(e) = msg.verify(SubsystemId::BlockStorage, &self.shared_secret) {
            log::warn!("Invalid message from BlockStorage: {:?}", e);
            return;
        }
        
        // Step 2: Match correlation_id to pending request
        let correlation_id = Uuid::from_bytes(msg.correlation_id);
        if let Some(pending) = self.pending_requests.remove(&correlation_id) {
            // Step 3: Check if request timed out
            if pending.created_at.elapsed() > pending.timeout {
                log::warn!("Response arrived after timeout for {:?}", correlation_id);
                return;
            }
            
            // Step 4: Process the response
            match msg.payload.result {
                Ok(stored_block) => {
                    self.process_retrieved_block(stored_block).await;
                }
                Err(error) => {
                    log::error!("Block read failed: {:?}", error);
                    self.handle_block_read_error(error).await;
                }
            }
        } else {
            // Orphaned response - request already timed out or never existed
            log::debug!("Orphaned response for correlation_id {:?}", correlation_id);
        }
    }
}

// ============================================================
// RESPONDER SIDE (Block Storage - Subsystem 2)
// ============================================================

impl BlockStorage {
    /// Handle incoming read block requests
    async fn handle_read_block_request(
        &self,
        msg: AuthenticatedMessage<ReadBlockRequestPayload>
    ) -> Result<(), Error> {
        // Step 1: Validate the envelope
        if let Err(e) = msg.verify_envelope() {
            return Err(Error::InvalidMessage(e));
        }
        
        // Step 2: Verify version is supported
        if msg.version < MIN_SUPPORTED_VERSION || msg.version > MAX_SUPPORTED_VERSION {
            return Err(Error::UnsupportedVersion(msg.version));
        }
        
        // Step 3: Process the request (read is allowed from any authorized subsystem)
        let result = match msg.payload.query {
            BlockQuery::ByHash(hash) => self.read_block(hash),
            BlockQuery::ByHeight(height) => self.read_block_by_height(height),
        };
        
        // Step 4: Convert result to response payload
        let response_payload = ReadBlockResponsePayload {
            result: result.map_err(|e| StorageErrorPayload {
                error_type: match &e {
                    StorageError::BlockNotFound { .. } => StorageErrorType::BlockNotFound,
                    StorageError::HeightNotFound { .. } => StorageErrorType::HeightNotFound,
                    StorageError::DataCorruption { .. } => StorageErrorType::DataCorruption,
                    _ => StorageErrorType::DatabaseError,
                },
                message: format!("{:?}", e),
                block_hash: match &e {
                    StorageError::BlockNotFound { hash } => Some(*hash),
                    StorageError::DataCorruption { block_hash, .. } => Some(*block_hash),
                    _ => None,
                },
                block_height: match &e {
                    StorageError::HeightNotFound { height } => Some(*height),
                    _ => None,
                },
            }),
        };
        
        // Step 5: Construct response with SAME correlation_id
        let response = AuthenticatedMessage {
            version: PROTOCOL_VERSION,
            sender_id: SubsystemId::BlockStorage,
            recipient_id: msg.sender_id,
            correlation_id: msg.correlation_id,      // CRITICAL: Same as request!
            reply_to: None,                          // This is a response, not a request
            timestamp: self.time_source.now(),
            nonce: self.nonce_generator.next(),
            signature: [0u8; 32],
            
            payload: response_payload,
        };
        
        // Step 6: Sign and publish to the requester's reply_to topic
        let signed_response = response.sign(&self.shared_secret);
        let reply_topic = msg.reply_to
            .ok_or(Error::MissingReplyTo)?;
        
        self.event_bus.publish(&reply_topic.to_string(), signed_response).await?;
        
        Ok(())
    }
}
```

### 4.4 Write Request Handling (Sender Verification)

Write operations have strict sender requirements per IPC-MATRIX.md:

```rust
impl BlockStorage {
    /// Handle incoming write block request
    async fn handle_write_block_request(
        &mut self,
        msg: AuthenticatedMessage<WriteBlockRequestPayload>
    ) -> Result<(), Error> {
        // Step 1: Validate envelope (version, signature, timestamp, nonce)
        if let Err(e) = msg.verify_envelope() {
            return Err(Error::InvalidMessage(e));
        }
        
        // Step 2: STRICT SENDER VERIFICATION (IPC-MATRIX.md requirement)
        // WriteBlockRequest is ONLY accepted from Subsystem 8 (Consensus)
        if msg.sender_id != SubsystemId::Consensus {
            log::warn!(
                "Unauthorized WriteBlockRequest from {:?} - REJECTED",
                msg.sender_id
            );
            return Err(Error::UnauthorizedSender {
                sender: msg.sender_id,
                allowed: vec![SubsystemId::Consensus],
            });
        }
        
        // Step 3: Check disk space (INVARIANT-2)
        let available = self.fs_adapter.available_disk_space_percent()?;
        if available < self.config.min_disk_space_percent {
            // Emit critical error event
            self.emit_critical_error(CriticalErrorType::DiskFull, 
                format!("Disk space at {}%", available)).await?;
            return Err(Error::DiskFull { available_percent: available });
        }
        
        // Step 4: Verify parent exists (INVARIANT-1)
        let parent_hash = msg.payload.block.header.parent_hash;
        let block_height = msg.payload.block.header.block_height;
        
        if block_height > 0 && !self.block_exists(parent_hash) {
            return Err(Error::ParentNotFound { parent_hash });
        }
        
        // Step 5: Compute checksum and prepare StoredBlock
        let stored_block = StoredBlock {
            block: msg.payload.block.clone(),
            merkle_root: msg.payload.merkle_root,
            state_root: msg.payload.state_root,
            stored_at: self.time_source.now(),
            checksum: self.compute_checksum(&msg.payload),
        };
        
        // Step 6: Atomic batch write (INVARIANT-4)
        let operations = self.prepare_write_operations(&stored_block)?;
        self.kv_store.atomic_batch_write(operations)?;
        
        // Step 7: Emit success event
        let event_payload = BlockStoredPayload {
            block_height,
            block_hash: msg.payload.block.header.block_hash,
            merkle_root: msg.payload.merkle_root,
            state_root: msg.payload.state_root,
            stored_at: stored_block.stored_at,
        };
        
        self.emit_event(BlockStorageEventPayload::BlockStored(event_payload)).await?;
        
        Ok(())
    }
    
    /// Verify sender for different request types
    /// 
    /// IMPORTANT: This subsystem only accepts writes from:
    /// - WriteBlock: Consensus (Subsystem 8) ONLY
    /// - MarkFinalized: Finality (Subsystem 9) ONLY
    /// 
    /// There are NO handlers for WriteMerkleRoot or WriteStateRoot.
    /// Consensus assembles the complete package before requesting storage.
    fn verify_sender(
        &self,
        sender_id: SubsystemId,
        request_type: &BlockStorageRequestPayload
    ) -> Result<(), Error> {
        let allowed = match request_type {
            BlockStorageRequestPayload::WriteBlock(_) => {
                // ONLY Consensus can write blocks
                // The WriteBlockRequest contains the complete package:
                // ValidatedBlock + merkle_root + state_root
                vec![SubsystemId::Consensus]  // Subsystem 8 ONLY
            }
            BlockStorageRequestPayload::MarkFinalized(_) => {
                vec![SubsystemId::Finality]  // Subsystem 9 ONLY
            }
            BlockStorageRequestPayload::ReadBlock(_) |
            BlockStorageRequestPayload::ReadBlockRange(_) => {
                // Read operations are allowed from any authorized subsystem
                return Ok(());
            }
        };
        
        if !allowed.contains(&sender_id) {
            return Err(Error::UnauthorizedSender { sender: sender_id, allowed });
        }
        
        Ok(())
    }
}
```

### 4.5 Dead Letter Queue (DLQ) Integration

Per Architecture.md Section 5.3, critical events MUST be routed to DLQ on failure:

```rust
impl BlockStorage {
    /// Emit critical error with DLQ metadata
    async fn emit_critical_error(
        &self,
        error_type: CriticalErrorType,
        message: String,
    ) -> Result<(), Error> {
        let payload = StorageCriticalPayload {
            error_type,
            message: message.clone(),
            affected_block: None,
            affected_height: None,
            timestamp: self.time_source.now(),
            requires_manual_intervention: matches!(
                error_type,
                CriticalErrorType::DataCorruption | CriticalErrorType::DatabaseFailure
            ),
        };
        
        let event = AuthenticatedMessage {
            version: PROTOCOL_VERSION,
            sender_id: SubsystemId::BlockStorage,
            recipient_id: SubsystemId::NodeRuntime,  // Broadcast to runtime
            correlation_id: Uuid::new_v4().as_bytes().clone(),
            reply_to: None,
            timestamp: self.time_source.now(),
            nonce: self.nonce_generator.next(),
            signature: [0u8; 32],
            payload: BlockStorageEventPayload::StorageCritical(payload),
        };
        
        // Publish to critical events topic
        // If this fails, it MUST go to DLQ
        if let Err(e) = self.event_bus.publish("storage.critical", event.sign(&self.shared_secret)).await {
            // Route to Dead Letter Queue
            self.publish_to_dlq(DeadLetterMessage {
                original_message: event,
                dlq_metadata: DLQMetadata {
                    original_topic: "storage.critical".into(),
                    failure_reason: FailureReason::PublishError,
                    failure_timestamp: self.time_source.now().0,
                    retry_count: 0,
                    last_error: e.to_string(),
                    stack_trace: None,
                    consumer_id: SubsystemId::BlockStorage,
                },
            }).await?;
        }
        
        Ok(())
    }
}
```

### 4.6 Message Envelope Compliance Checklist

For every IPC message sent or received by this subsystem:

| Field | Required | Validation |
|-------|----------|------------|
| `version` | ✅ YES | Must be within `[MIN_SUPPORTED_VERSION, MAX_SUPPORTED_VERSION]` |
| `sender_id` | ✅ YES | Must match expected sender per IPC Matrix (strict for writes) |
| `recipient_id` | ✅ YES | Must be `SubsystemId::BlockStorage` for incoming |
| `correlation_id` | ✅ YES | UUID v4, used to match request/response pairs |
| `reply_to` | ✅ For reads | Topic where response should be published |
| `timestamp` | ✅ YES | Must be within 60 seconds of current time |
| `nonce` | ✅ YES | Must not be reused (replay prevention) |
| `signature` | ✅ YES | HMAC-SHA256, verified before processing |

---

## 5. TDD VALIDATION STRATEGY

### 5.1 Critical Domain Logic Tests (Red Phase)

Before implementing any function bodies, we must write these failing tests:

#### Test Group 1: Atomic Write Guarantees

```rust
#[test]
fn test_atomic_write_succeeds_completely_or_not_at_all()
// Verify: INVARIANT-4
// Setup: Write a block
// Action: Simulate crash during atomic_batch_write
// Assert: Either all data present or none

#[test]
fn test_partial_write_not_possible_on_simulated_crash()
// Verify: INVARIANT-4
// Setup: Mock KVStore to fail after 2 of 4 operations
// Action: Attempt write_block
// Assert: No data written (rollback occurred)

#[test]
fn test_write_includes_all_required_entries()
// Verify: Block, height index, merkle root, state root all written
// Setup: Successful write_block
// Assert: All 4 key types present in store
```

#### Test Group 2: Disk Space Safety

```rust
#[test]
fn test_write_fails_when_disk_below_5_percent()
// Verify: INVARIANT-2
// Setup: Mock FileSystemAdapter returns 4% available
// Action: Attempt write_block
// Assert: Returns StorageError::DiskFull

#[test]
fn test_write_succeeds_when_disk_at_5_percent()
// Verify: INVARIANT-2 boundary
// Setup: Mock FileSystemAdapter returns 5% available
// Action: Attempt write_block
// Assert: Write succeeds

#[test]
fn test_disk_full_emits_critical_event()
// Verify: DLQ integration
// Setup: Mock FileSystemAdapter returns 4% available
// Action: Attempt write_block
// Assert: StorageCriticalPayload emitted with DiskFull type
```

#### Test Group 3: Data Integrity (Checksum Verification)

```rust
#[test]
fn test_read_detects_corrupted_checksum()
// Verify: INVARIANT-3
// Setup: Write block, then manually alter stored checksum
// Action: Read block
// Assert: Returns StorageError::DataCorruption

#[test]
fn test_read_detects_corrupted_data()
// Verify: INVARIANT-3
// Setup: Write block, then manually alter stored data
// Action: Read block
// Assert: Returns StorageError::DataCorruption

#[test]
fn test_corruption_emits_critical_event()
// Verify: DLQ integration
// Setup: Corrupt a stored block
// Action: Read block
// Assert: StorageCriticalPayload emitted with DataCorruption type

#[test]
fn test_valid_checksum_passes_verification()
// Verify: Happy path
// Setup: Write block normally
// Action: Read block
// Assert: Block returned without error
```

#### Test Group 4: Sequential Block Requirement

```rust
#[test]
fn test_write_fails_without_parent_block()
// Verify: INVARIANT-1
// Setup: Empty storage
// Action: Attempt write_block at height 5
// Assert: Returns StorageError::ParentNotFound

#[test]
fn test_genesis_block_has_no_parent_requirement()
// Verify: INVARIANT-1 exception
// Setup: Empty storage
// Action: Write block at height 0
// Assert: Write succeeds

#[test]
fn test_write_succeeds_with_parent_present()
// Verify: INVARIANT-1 happy path
// Setup: Write genesis block
// Action: Write block at height 1 with parent = genesis
// Assert: Write succeeds
```

#### Test Group 5: Finalization Logic

```rust
#[test]
fn test_finalization_rejects_lower_height()
// Verify: INVARIANT-5
// Setup: Finalize height 10
// Action: Attempt finalize height 5
// Assert: Returns StorageError::InvalidFinalization

#[test]
fn test_finalization_rejects_same_height()
// Verify: INVARIANT-5
// Setup: Finalize height 10
// Action: Attempt finalize height 10 again
// Assert: Returns StorageError::InvalidFinalization

#[test]
fn test_finalization_requires_block_exists()
// Verify: Finalize only existing blocks
// Setup: Storage with blocks 0-10
// Action: Attempt finalize height 15
// Assert: Returns StorageError::HeightNotFound

#[test]
fn test_finalization_emits_event()
// Verify: Event emission
// Setup: Storage with blocks 0-10
// Action: Finalize height 5
// Assert: BlockFinalizedPayload emitted
```

#### Test Group 6: Access Control

```rust
#[test]
fn test_write_block_rejects_non_consensus_sender()
// Verify: IPC-MATRIX compliance - ONLY Consensus can write blocks
// Setup: Message with sender_id = SubsystemId::Mempool
// Action: Handle WriteBlockRequest
// Assert: Returns Error::UnauthorizedSender

#[test]
fn test_write_block_rejects_transaction_indexing_sender()
// Verify: IPC-MATRIX compliance - Transaction Indexing cannot write directly
// Setup: Message with sender_id = SubsystemId::TransactionIndexing
// Action: Handle WriteBlockRequest
// Assert: Returns Error::UnauthorizedSender
// Note: merkle_root comes via WriteBlockRequest from Consensus, not directly

#[test]
fn test_write_block_rejects_state_management_sender()
// Verify: IPC-MATRIX compliance - State Management cannot write directly
// Setup: Message with sender_id = SubsystemId::StateManagement
// Action: Handle WriteBlockRequest
// Assert: Returns Error::UnauthorizedSender
// Note: state_root comes via WriteBlockRequest from Consensus, not directly

#[test]
fn test_mark_finalized_rejects_non_finality_sender()
// Verify: IPC-MATRIX compliance - Only Finality can mark finalized
// Setup: Message with sender_id = SubsystemId::Consensus
// Action: Handle MarkFinalizedRequest
// Assert: Returns Error::UnauthorizedSender

#[test]
fn test_read_block_accepts_any_authorized_sender()
// Verify: Read is permissive
// Setup: Message with sender_id = SubsystemId::LightClients
// Action: Handle ReadBlockRequest
// Assert: Request processed (not rejected for sender)

#[test]
fn test_read_block_range_accepts_any_authorized_sender()
// Verify: Batch read is permissive
// Setup: Message with sender_id = SubsystemId::BlockPropagation
// Action: Handle ReadBlockRangeRequest
// Assert: Request processed successfully
```

#### Test Group 7: Batch Read (Node Syncing)

```rust
#[test]
fn test_read_block_range_returns_sequential_blocks()
// Verify: Batch read returns blocks in order
// Setup: Store blocks 0-100
// Action: read_block_range(start_height: 10, limit: 20)
// Assert: Returns blocks 10-29 in ascending order

#[test]
fn test_read_block_range_respects_limit_cap()
// Verify: Limit is capped at 100
// Setup: Store blocks 0-200
// Action: read_block_range(start_height: 0, limit: 500)
// Assert: Returns only 100 blocks (capped)

#[test]
fn test_read_block_range_returns_partial_if_chain_end()
// Verify: Returns fewer blocks if chain ends
// Setup: Store blocks 0-50
// Action: read_block_range(start_height: 40, limit: 20)
// Assert: Returns blocks 40-50 (11 blocks, not 20)

#[test]
fn test_read_block_range_fails_on_invalid_start()
// Verify: Error if start_height doesn't exist
// Setup: Store blocks 0-10
// Action: read_block_range(start_height: 100, limit: 10)
// Assert: Returns StorageError::HeightNotFound
```

#### Test Group 8: Concurrency Safety

```rust
#[tokio::test]
async fn test_concurrent_reads_do_not_block()
// Verify: Read concurrency
// Setup: Store 100 blocks
// Action: Spawn 50 concurrent read tasks
// Assert: All complete without deadlock or error

#[tokio::test]
async fn test_concurrent_reads_during_write()
// Verify: Read/write isolation
// Setup: Store 10 blocks
// Action: Start long write, spawn concurrent reads
// Assert: Reads complete with consistent data

#[tokio::test]
async fn test_writes_are_serialized()
// Verify: Write ordering
// Setup: Empty storage
// Action: Attempt 10 concurrent writes
// Assert: All succeed without corruption

#[tokio::test]
async fn test_concurrent_batch_reads()
// Verify: Batch read concurrency for node syncing
// Setup: Store 1000 blocks
// Action: Spawn 10 concurrent read_block_range requests
// Assert: All complete without deadlock or error
```

#### Test Group 9: Message Envelope Validation

```rust
#[test]
fn test_rejects_message_with_invalid_version()
// Verify: Version gate
// Setup: Message with version = 999
// Action: Process message
// Assert: Rejected with UnsupportedVersion error

#[test]
fn test_rejects_message_with_expired_timestamp()
// Verify: Timestamp validation
// Setup: Message with timestamp 120 seconds old
// Action: Process message
// Assert: Rejected with MessageTooOld error

#[test]
fn test_rejects_message_with_reused_nonce()
// Verify: Replay prevention
// Setup: Process message with nonce X
// Action: Process another message with same nonce X
// Assert: Rejected with NonceReused error

#[test]
fn test_rejects_message_with_invalid_signature()
// Verify: HMAC validation
// Setup: Message with tampered signature
// Action: Process message
// Assert: Rejected with InvalidSignature error
```

### 5.2 Integration Tests (Port Contracts)

```rust
#[test]
fn test_rocksdb_adapter_atomic_batch_write()
// Verify: RocksDB implements atomic semantics
// Setup: RocksDB adapter with test database
// Action: Execute batch with 4 operations
// Assert: All or none committed

#[test]
fn test_filesystem_adapter_reports_disk_space()
// Verify: FileSystemAdapter implementation
// Setup: SystemFileSystemAdapter
// Action: Call available_disk_space_percent()
// Assert: Returns valid percentage (0-100)

#[test]
fn test_checksum_provider_crc32c_correctness()
// Verify: CRC32C implementation
// Setup: Known data and expected CRC32C
// Action: Compute checksum
// Assert: Matches expected value

#[test]
fn test_serialization_roundtrip()
// Verify: BlockSerializer implementation
// Setup: Create StoredBlock
// Action: Serialize then deserialize
// Assert: Result equals original
```

#### Test Group 10: Stateful Assembler (V2.2 Choreography)

```rust
#[test]
fn test_assembly_completes_when_all_three_events_arrive()
// Verify: Choreography pattern - complete assembly
// Setup: Send BlockValidated, MerkleRootComputed, StateRootComputed (any order)
// Assert: Block is written and BlockStored event emitted

#[test]
fn test_assembly_buffers_partial_components()
// Verify: Buffering of incomplete assemblies
// Setup: Send only BlockValidated and MerkleRootComputed
// Assert: Block is NOT written yet; assembly is buffered

#[test]
fn test_assembly_works_regardless_of_event_order()
// Verify: Order independence
// Setup: Send events in order: StateRootComputed, BlockValidated, MerkleRootComputed
// Assert: Block is written correctly when third event arrives

#[test]
fn test_assembly_timeout_purges_incomplete_blocks()
// Verify: INVARIANT-7 - timeout defense
// Setup: Send only BlockValidated; wait > assembly_timeout_secs
// Action: Trigger GC
// Assert: Assembly is purged; AssemblyTimeout event emitted

#[test]
fn test_assembly_buffer_respects_max_pending_limit()
// Verify: INVARIANT-8 - memory safety
// Setup: Create max_pending_assemblies + 1 partial assemblies
// Assert: Oldest assembly is purged; newest is accepted

#[test]
fn test_assembly_rejects_wrong_sender_for_block_validated()
// Verify: Sender verification for choreography events
// Setup: Send BlockValidated with sender_id != Consensus
// Assert: Rejected with UnauthorizedSender

#[test]
fn test_assembly_rejects_wrong_sender_for_merkle_root()
// Verify: Sender verification
// Setup: Send MerkleRootComputed with sender_id != TransactionIndexing
// Assert: Rejected with UnauthorizedSender

#[test]
fn test_assembly_rejects_wrong_sender_for_state_root()
// Verify: Sender verification
// Setup: Send StateRootComputed with sender_id != StateManagement
// Assert: Rejected with UnauthorizedSender
```

#### Test Group 11: Transaction Data Retrieval (V2.3)

```rust
#[test]
fn test_get_transaction_location_returns_correct_position()
// Verify: Transaction location lookup
// Setup: Store block with 10 transactions
// Action: Query location for transaction at index 5
// Assert: Returns correct block_hash, height, and index

#[test]
fn test_get_transaction_location_returns_not_found()
// Verify: Transaction not found error
// Setup: Store block with known transactions
// Action: Query location for unknown transaction hash
// Assert: Returns TransactionNotFound error

#[test]
fn test_get_transaction_hashes_for_block_returns_ordered_hashes()
// Verify: Transaction hashes retrieval
// Setup: Store block with 20 transactions
// Action: Call get_transaction_hashes_for_block
// Assert: Returns all 20 hashes in canonical order

#[test]
fn test_get_transaction_hashes_for_block_not_found()
// Verify: Block not found error
// Setup: Empty storage
// Action: Query transaction hashes for unknown block
// Assert: Returns BlockNotFound error

#[test]
fn test_get_transaction_hashes_sender_verification()
// Verify: Only Transaction Indexing can query
// Setup: Send GetTransactionHashesRequest with sender_id != TransactionIndexing
// Assert: Rejected with UnauthorizedSender
```

---

## 6. SECURITY & CONSTRAINTS

### 6.1 Access Control Matrix (V2.3 Choreography + Data Retrieval)

**Event Subscriptions (Choreography - Block Assembly):**

| Event Type | Allowed Sender | Rejection Action |
|------------|----------------|------------------|
| BlockValidated | Subsystem 8 (Consensus) ONLY | Log warning + reject |
| MerkleRootComputed | Subsystem 3 (Transaction Indexing) ONLY | Log warning + reject |
| StateRootComputed | Subsystem 4 (State Management) ONLY | Log warning + reject |

**Request/Response Handlers:**

| Request Type | Allowed Sender(s) | Rejection Action |
|--------------|-------------------|------------------|
| MarkFinalizedRequest | Subsystem 9 (Finality) ONLY | Log warning + reject |
| ReadBlockRequest | Any authorized subsystem | N/A (permissive) |
| ReadBlockRangeRequest | Any authorized subsystem | N/A (permissive) |
| GetTransactionLocationRequest | Subsystem 3 (Transaction Indexing) ONLY | Log warning + reject |
| GetTransactionHashesRequest | Subsystem 3 (Transaction Indexing) ONLY | Log warning + reject |

**V2.3 Architecture Amendments:**
- **V2.2:** Choreography pattern - Block Storage is Stateful Assembler
- **V2.3:** Transaction Indexing can query transaction locations AND hashes for Merkle proof generation
- No `WriteBlockRequest` - the orchestrator pattern is REJECTED
- Block Storage SUBSCRIBES to three independent events and ASSEMBLES them
- Block Storage PROVIDES transaction data to Transaction Indexing on demand

### 6.2 Trust Boundary Enforcement

```rust
// This subsystem does NOT:
// ❌ Validate block signatures
// ❌ Verify transaction validity
// ❌ Execute smart contracts
// ❌ Check consensus rules
// ❌ Validate state transitions

// This subsystem ONLY:
// ✅ Verifies AuthenticatedMessage envelope (HMAC, timestamp, nonce)
// ✅ Verifies sender_id matches expected source for each event type
// ✅ Buffers incoming event components by block_hash
// ✅ Performs atomic write when all 3 components arrive
// ✅ Purges timed-out assemblies (resource exhaustion defense)
// ✅ Verifies data integrity via checksums on read
// ✅ Provides efficient batch reads for node syncing
```

### 6.3 Panic Policy

**Principle:** This library must NEVER panic in production.

**Rules:**
1. All array accesses use `.get()` with `Result` return
2. All integer operations checked for overflow
3. All `unwrap()` calls replaced with proper error handling
4. Serialization failures return errors, not panics

```rust
// ❌ FORBIDDEN
let block = self.blocks[hash];  // Can panic

// ✅ REQUIRED
let block = self.blocks
    .get(&hash)
    .ok_or(StorageError::BlockNotFound { hash })?;
```

### 6.4 Memory Constraints

**Limits:**
- **Maximum Block Size:** 10 MB (configurable)
- **Read Buffer Size:** 16 MB (for large block reads)
- **Batch Write Size:** 1000 operations maximum

**Enforcement:**
```rust
const MAX_BLOCK_SIZE: usize = 10 * 1024 * 1024;  // 10 MB
const MAX_BATCH_OPERATIONS: usize = 1000;

fn validate_block_size(&self, block: &ValidatedBlock) -> Result<(), StorageError> {
    let size = self.serializer.estimate_size(block);
    if size > self.config.max_block_size {
        return Err(StorageError::BlockTooLarge { 
            size, 
            max_size: self.config.max_block_size 
        });
    }
    Ok(())
}
```

### 6.5 Data Retention Policy

- **Block Data:** Retained indefinitely (or until pruning policy defined in future spec)
- **Finalized Blocks:** Never pruned
- **Orphaned Blocks:** May be pruned after configurable retention period

---

## 7. DEPENDENCIES & REFERENCES

### 7.1 Internal Dependencies

- **Shared Types Crate** (`crates/shared-types`):
    - `SubsystemId` enum
    - `Hash`, `Address`, `Signature` types
    - `AuthenticatedMessage<T>` envelope
    - Common error types

- **Shared Bus Crate** (`crates/shared-bus`):
    - `EventPublisher` trait
    - `EventSubscriber` trait
    - `Topic` struct
    - `DeadLetterMessage` struct

### 7.2 External Crate Dependencies (Minimal)

```toml
[dependencies]
# Checksum computation
crc32c = "0.6"

# Serialization (domain layer uses traits, adapters use concrete impl)
# serde = "1.0"  # Only in adapter crate

# No other dependencies allowed in domain layer
```

### 7.3 References

- **IPC Matrix Document:** Section "SUBSYSTEM 2: BLOCK STORAGE ENGINE"
- **Architecture Document (V2.2):** 
    - Section 3.2 (AuthenticatedMessage envelope)
    - Section 3.2.1 (Envelope as Sole Source of Truth - V2.2 Security Mandate)
    - Section 3.3 (Request/Response Correlation Pattern)
    - Section 3.4 (Message Versioning Protocol)
    - Section 5.1 (Block Validation Flow - Event-Driven Choreography)
    - Section 5.3 (Dead Letter Queue Strategy)
    - Section 5.4.1 (Deterministic Trigger Conditions - Finality Circuit Breaker)
    - Section 4.1 (Subsystem Catalog - Block Storage)

### 7.4 Related Specifications

- **SPEC-08-CONSENSUS.md** (publishes BlockValidated events to event bus)
- **SPEC-09-FINALITY.md** (triggers finalization in this subsystem)
- **SPEC-03-TRANSACTION-INDEXING.md** (publishes MerkleRootComputed events to event bus)
- **SPEC-04-STATE-MANAGEMENT.md** (publishes StateRootComputed events to event bus)

**Data Flow (V2.2 Choreography Pattern):**
```
Consensus (8) ────BlockValidated────→ [Event Bus] ──→ Block Storage (2)
                                                         │
Tx Indexing (3) ──MerkleRootComputed──→ [Event Bus] ──→ │ (Stateful Assembler)
                                                         │
State Mgmt (4) ───StateRootComputed───→ [Event Bus] ──→ │
                                                         ↓
                                              [Atomic Write when all 3 present]
```

---

## 8. IMPLEMENTATION CHECKLIST

### Phase 1: Domain Logic (Pure)
- [ ] Import shared types from `crates/shared-types` (DO NOT redefine)
- [ ] Implement `StoredBlock` with checksum computation (domain-specific type)
- [ ] Implement `BlockIndex` for height → hash mapping
- [ ] Implement `StorageMetadata` tracking
- [ ] Implement `BlockAssemblyBuffer` and `PendingBlockAssembly` (V2.2 Choreography)
- [ ] Implement all invariant checks (1-8, including assembly timeout)
- [ ] Implement `StorageError` enum with all variants
- [ ] Implement `read_block_range` for efficient batch reads
- [ ] Write all TDD tests from Section 5.1 (including Test Group 10)

### Phase 2: Port Definitions
- [ ] Define `BlockStorageApi` trait (including `read_block_range`)
- [ ] Define `KeyValueStore` trait
- [ ] Define `FileSystemAdapter` trait
- [ ] Define `ChecksumProvider` trait
- [ ] Define `TimeSource` trait
- [ ] Define `BlockSerializer` trait

### Phase 3: Event Integration (V2.2 Choreography)
- [ ] Define incoming event payloads: `BlockValidatedPayload`, `MerkleRootComputedPayload`, `StateRootComputedPayload`
- [ ] Define `BlockStorageRequestPayload` enum (MarkFinalized, ReadBlock, ReadBlockRange)
- [ ] Define `BlockStorageEventPayload` enum (including AssemblyTimeout)
- [ ] Implement event handlers for three choreography events
- [ ] Implement Stateful Assembler logic (buffer, complete, GC)
- [ ] Implement `AuthenticatedMessage<T>` envelope handling
- [ ] Implement correlation ID tracking for ReadBlock/ReadBlockRange request/response
- [ ] Implement sender verification per event type
- [ ] Implement event publishing for BlockStored, BlockFinalized, AssemblyTimeout
- [ ] Implement StorageCritical event emission with DLQ integration
- [ ] Implement response routing via `reply_to` topic

### Phase 4: Adapters (Separate Crate)
- [ ] Create `block-storage-adapters` crate
- [ ] Implement `RocksDBKeyValueStore` adapter
- [ ] Implement `SystemFileSystemAdapter`
- [ ] Implement `Crc32cChecksumProvider`
- [ ] Implement `SystemTimeSource`
- [ ] Implement `BincodeBlockSerializer` (or similar)
- [ ] Write integration tests

---

## 9. OPEN QUESTIONS & DESIGN DECISIONS

### Q1: Block Pruning Strategy?
**Question:** Should we support pruning old, non-finalized blocks?

**Options:**
- A) No pruning (archive node mode only)
- B) Optional pruning via `PruningPolicy` configuration
- C) Automatic pruning of orphaned blocks after N epochs

**Decision:** Defer to future specification. Archive mode assumed for v1.0.

### Q2: Compression?
**Question:** Should blocks be compressed before storage?

**Options:**
- A) No compression (simpler, faster reads)
- B) Optional Snappy compression (RocksDB native support)
- C) LZ4 compression for better ratios

**Decision:** Defer to adapter implementation. Domain layer stores raw bytes.

### Q3: Read Caching?
**Question:** Should we cache recently read blocks in memory?

**Options:**
- A) No caching (rely on RocksDB block cache)
- B) LRU cache for hot blocks
- C) Configurable caching policy

**Decision:** Rely on RocksDB's built-in block cache. No application-level cache in v1.0.

---

## 10. ACCEPTANCE CRITERIA

This specification is considered **complete** when:

1. ✅ All domain entities defined with no implementation
2. ✅ All invariants explicitly stated (8 invariants, including V2.2 assembly timeout)
3. ✅ All ports (Driving + Driven) defined as traits
4. ✅ All events defined as payloads for AuthenticatedMessage<T>
5. ✅ Choreography pattern implemented (3 independent event subscriptions)
6. ✅ Stateful Assembler structures defined (BlockAssemblyBuffer, PendingBlockAssembly)
7. ✅ Assembly timeout logic specified (resource exhaustion defense)
8. ✅ Request/Response pattern demonstrated with correlation_id
9. ✅ All TDD tests listed (names only, no code) including Test Group 10
10. ✅ Security constraints documented (access control matrix for choreography)
11. ✅ Memory limits specified
12. ✅ Panic policy stated
13. ✅ DLQ integration documented for critical errors
14. ✅ Shared types referenced from shared-types crate (not redefined)
15. ✅ Batch read capability (read_block_range) defined
16. ✅ Envelope-Only Identity enforced (no requester_id in payloads)
17. ✅ Finality Liveness context documented (circuit breaker awareness)

This specification is considered **approved** when:

1. ✅ Reviewed by senior architect
2. ✅ Confirmed to match IPC Matrix requirements
3. ✅ Confirmed to follow Architecture.md v2.2 Choreography Pattern
4. ✅ Confirmed AuthenticatedMessage envelope used correctly
5. ✅ Confirmed Envelope-Only Identity (no payload identity fields)
6. ✅ Confirmed Stateful Assembler pattern implemented
7. ✅ Confirmed assembly timeout defense against resource exhaustion
8. ✅ No implementation code present (only signatures)

---

**END OF SPECIFICATION**

**Next Steps:**
1. Review this specification for completeness
2. Approve for TDD Phase (write tests)
3. Implement domain logic (pass tests)
4. Implement adapters (wire to runtime)
