# BLOCKCHAIN INTER-PROCESS COMMUNICATION (IPC) MATRIX
## Strict Message Types, Access Control, and Security Boundaries

**Security Principle:** Each subsystem is an isolated compartment. Even if one subsystem is compromised, attackers cannot access others without the correct message types and authentication.

---

## SUBSYSTEM 1: PEER DISCOVERY & ROUTING

### I Am Allowed To Talk To:
- **Subsystem 5 (Block Propagation)** - Provide peer list
- **Subsystem 7 (Bloom Filters)** - Provide full node connections
- **Subsystem 10 (Signature Verification)** - Verify node identity for DDoS defense
- **Subsystem 13 (Light Clients)** - Provide full node connections

### Who Is Allowed To Talk To Me:
- **External Bootstrap Nodes** (Initial network entry)
- **Subsystem 5 (Block Propagation)** - Request peer list
- **Subsystem 7 (Bloom Filters)** - Request full nodes
- **Subsystem 13 (Light Clients)** - Request full nodes

### Strict Message Types:

**OUTGOING:**
```rust
struct PeerList {
    version: u16,
    correlation_id: [u8; 16],
    peers: Vec<PeerInfo>,
    timestamp: u64,
    signature: Signature,  // Signed by this node
}

struct PeerInfo {
    node_id: [u8; 32],      // 256-bit Kademlia ID
    ip_address: IpAddr,
    port: u16,
    reputation_score: u8,   // 0-100
    last_seen: u64,
}

/// Request to Subsystem 10 for DDoS defense
struct VerifyNodeIdentityRequest {
    version: u16,
    requester_id: SubsystemId,  // Must be 1
    correlation_id: [u8; 16],
    reply_to: Topic,
    node_id: [u8; 32],
    claimed_pubkey: [u8; 33],
    signature: Signature,
}
```

**INCOMING:**
```rust
struct PeerListRequest {
    version: u16,
    requester_id: SubsystemId,  // Must be 5, 7, or 13
    correlation_id: [u8; 16],
    reply_to: Topic,
    request_id: u64,
    timestamp: u64,
    signature: Signature,       // Signed by requester
}

struct BootstrapRequest {
    version: u16,
    node_id: [u8; 32],
    ip_address: IpAddr,
    port: u16,
    proof_of_work: [u8; 32],   // Anti-Sybil
}

/// Response from Subsystem 10
struct NodeIdentityVerificationResult {
    version: u16,
    correlation_id: [u8; 16],
    node_id: [u8; 32],
    identity_valid: bool,
    verification_timestamp: u64,
}
```

### Security Boundaries:
- ✅ Accept: PeerListRequest from Subsystems 5, 7, 13 only
- ✅ Accept: BootstrapRequest from external nodes with valid PoW
- ✅ Accept: NodeIdentityVerificationResult from Subsystem 10 only
- ✅ Send: VerifyNodeIdentityRequest to Subsystem 10 (DDoS defense)
- ❌ Reject: Any message from Subsystems 2, 3, 4, 6, 8, 9, 11, 12, 14, 15
- ❌ Reject: Unsigned messages
- ❌ Reject: Messages older than 60 seconds

### DDoS Defense Flow:
```
External Peer ──BootstrapRequest──→ Peer Discovery (1) ──VerifyNodeIdentityRequest──→ Signature Verification (10)
                                                       ←──NodeIdentityVerificationResult──
                                    
If identity_valid == false: REJECT peer immediately (before it enters system)
If identity_valid == true:  Add to routing table
```

---

## SUBSYSTEM 2: BLOCK STORAGE ENGINE

**V2.3 ROLE: STATEFUL ASSEMBLER (Choreography Pattern)**
Block Storage is the **assembler** in the V2.3 choreography. It does NOT receive
direct write requests from Consensus. Instead, it subscribes to three independent
events and assembles them into a complete block for atomic storage.

### I Am Allowed To Talk To:
- **Subsystem 3 (Transaction Indexing)** - Respond to transaction location and hash queries for Merkle proof generation (V2.3)
- **Subsystem 6 (Mempool)** - Send BlockStorageConfirmation after successful block storage

### Who Is Allowed To Talk To Me:
- **Event Bus (Subsystem 8 origin)** - Subscribe to `BlockValidated` events (V2.3 Choreography)
- **Event Bus (Subsystem 3 origin)** - Subscribe to `MerkleRootComputed` events (V2.3 Choreography)
- **Event Bus (Subsystem 4 origin)** - Subscribe to `StateRootComputed` events (V2.3 Choreography)
- **Subsystem 3 (Transaction Indexing)** - Request transaction locations and hashes for Merkle proof generation (V2.3)
- **Subsystem 9 (Finality)** - Mark blocks as finalized

**REMOVED (V2.3 - Orchestrator Anti-Pattern Eliminated):**
- ~~Subsystem 8 (Consensus) - Store validated blocks~~ → Now via Event Bus subscription

**V2.3 CHOREOGRAPHY + DATA RETRIEVAL PATTERN:**
```
WRITE PATH (Choreography):
  Event Bus ──BlockValidated──────→ [Block Storage] ──buffers──→ PendingAssembly
  Event Bus ──MerkleRootComputed──→ [Block Storage] ──buffers──→ PendingAssembly
  Event Bus ──StateRootComputed───→ [Block Storage] ──buffers──→ PendingAssembly
                                          │
                                          ↓ (when all 3 arrive)
                                    ATOMIC WRITE + BlockStorageConfirmation

READ PATH (V2.3 Data Retrieval):
  [Subsystem 3] ──GetTransactionHashesRequest──→ [Block Storage]
  [Block Storage] ──TransactionHashesResponse──→ [Subsystem 3]
```

### Strict Message Types:

**OUTGOING:**
```rust
struct StorageResponse {
    version: u16,                    // Protocol version
    correlation_id: [u8; 16],        // Maps to original request
    request_id: u64,
    success: bool,
    data: Option<Vec<u8>>,
    error: Option<StorageError>,
}

/// NEW: Confirmation sent to Mempool after successful block storage
/// This completes the Two-Phase Commit for transaction removal
struct BlockStorageConfirmation {
    version: u16,
    sender_id: SubsystemId,          // Always 2 (Block Storage)
    correlation_id: [u8; 16],
    
    // Block that was successfully stored
    block_hash: [u8; 32],
    block_height: u64,
    
    // Transactions that were included and stored
    included_transactions: Vec<[u8; 32]>,
    
    // Storage timestamp (for audit trail)
    storage_timestamp: u64,
    signature: Signature,
}

/// V2.3: Response to transaction location query from Transaction Indexing
/// Enables Merkle proof generation by providing transaction position in block
/// 
/// SECURITY (Envelope-Only Identity): No requester_id in payload.
/// Identity derived from AuthenticatedMessage envelope.
struct TransactionLocationResponse {
    version: u16,
    correlation_id: [u8; 16],        // Maps to original request
    
    // Query result
    transaction_hash: [u8; 32],
    found: bool,
    
    // Location data (if found)
    block_hash: Option<[u8; 32]>,
    block_height: Option<u64>,
    transaction_index: Option<usize>,
    merkle_root: Option<[u8; 32]>,   // Cached for proof generation
    
    signature: Signature,
}

/// V2.3: Response to transaction hashes query from Transaction Indexing
/// Provides all transaction hashes for Merkle tree reconstruction
/// 
/// SECURITY (Envelope-Only Identity): No requester_id in payload.
struct TransactionHashesResponse {
    version: u16,
    correlation_id: [u8; 16],        // Maps to original request
    
    block_hash: [u8; 32],
    /// All transaction hashes in the block, in canonical order
    transaction_hashes: Vec<[u8; 32]>,
    /// Cached Merkle root for verification
    merkle_root: [u8; 32],
    
    signature: Signature,
}

enum StorageError {
    NotFound,
    Corrupted,
    DiskFull,
    PermissionDenied,
}
```

**INCOMING:**
```rust
/// WriteBlockRequest is the ONLY way to write a block.
/// This is a COMPLETE PACKAGE containing all data needed for atomic write.
/// 
/// ATOMICITY GUARANTEE: Either all of (block, merkle_root, state_root) are
/// written together, or none are written. Partial writes are impossible.
///
/// POST-STORAGE ACTION: Upon successful write, Block Storage MUST send
/// BlockStorageConfirmation to Mempool to complete Two-Phase Commit.
struct WriteBlockRequest {
    version: u16,                    // Protocol version - MUST be validated first
    requester_id: SubsystemId,       // MUST be 8 (Consensus) ONLY
    correlation_id: [u8; 16],        // For async response correlation
    reply_to: Topic,                 // Where to send response
    
    // === COMPLETE BLOCK PACKAGE ===
    block: ValidatedBlock,           // From Consensus
    merkle_root: [u8; 32],           // Consensus received this from Subsystem 3
    state_root: [u8; 32],            // Consensus received this from Subsystem 4
    
    // Transaction hashes for Mempool confirmation
    transaction_hashes: Vec<[u8; 32]>,
    
    signature: Signature,
}

// ============================================================
// THE FOLLOWING MESSAGE TYPES ARE REMOVED (Atomicity Violation):
// 
// - WriteMerkleRootRequest (REMOVED - merkle_root comes via WriteBlockRequest)
// - WriteStateRootRequest (REMOVED - state_root comes via WriteBlockRequest)
//
// RATIONALE: In an asynchronous system, separate messages arrive at different
// times. If power fails between Message 1 and Message 2, you have a block
// with no roots - the database is corrupted. By bundling into one message,
// we guarantee atomicity: either the whole block exists, or nothing does.
// ============================================================

struct ReadBlockRequest {
    version: u16,
    requester_id: SubsystemId,
    correlation_id: [u8; 16],
    reply_to: Topic,
    block_number: u64,
    signature: Signature,
}

struct ReadBlockRangeRequest {
    version: u16,
    requester_id: SubsystemId,
    correlation_id: [u8; 16],
    reply_to: Topic,
    start_height: u64,
    limit: u64,                      // Capped at 100
    signature: Signature,
}

struct MarkFinalizedRequest {
    version: u16,
    requester_id: SubsystemId,       // MUST be 9 (Finality) ONLY
    correlation_id: [u8; 16],
    reply_to: Topic,
    block_number: u64,
    finality_proof: FinalityProof,
    signature: Signature,
}

/// V2.3: Request for transaction location from Transaction Indexing
/// Enables Merkle proof generation by querying stored transaction positions
/// 
/// SECURITY (Envelope-Only Identity - V2.2):
/// No requester_id in payload per Architecture.md Section 3.2.1.
/// Identity is derived from AuthenticatedMessage envelope.sender_id.
struct GetTransactionLocationRequest {
    version: u16,
    // No requester_id per Envelope-Only Identity
    correlation_id: [u8; 16],
    reply_to: Topic,
    transaction_hash: [u8; 32],
    signature: Signature,
}

/// V2.3: Request for transaction hashes in a block from Transaction Indexing
/// Enables Merkle tree reconstruction for proof generation on cache miss
/// 
/// SECURITY (Envelope-Only Identity):
/// No requester_id in payload per Architecture.md Section 3.2.1.
struct GetTransactionHashesRequest {
    version: u16,
    // No requester_id per Envelope-Only Identity
    correlation_id: [u8; 16],
    reply_to: Topic,
    block_hash: [u8; 32],
    signature: Signature,
}
```

### Security Boundaries (V2.3 Choreography):
- ✅ Subscribe: `BlockValidated` from Event Bus (Subsystem 8 origin) - triggers assembly
- ✅ Subscribe: `MerkleRootComputed` from Event Bus (Subsystem 3 origin) - assembly component
- ✅ Subscribe: `StateRootComputed` from Event Bus (Subsystem 4 origin) - assembly component
- ✅ Accept: MarkFinalizedRequest from Subsystem 9 (Finality) ONLY
- ✅ Accept: ReadBlockRequest from any authorized subsystem (read-only)
- ✅ Accept: ReadBlockRangeRequest from any authorized subsystem (read-only)
- ✅ Accept: GetTransactionLocationRequest from Subsystem 3 (Transaction Indexing) ONLY (V2.3)
- ✅ Accept: GetTransactionHashesRequest from Subsystem 3 (Transaction Indexing) ONLY (V2.3)
- ✅ Send: BlockStorageConfirmation to Subsystem 6 (Mempool) after successful assembly
- ✅ Send: TransactionLocationResponse to Subsystem 3 (Transaction Indexing) (V2.3)
- ✅ Send: TransactionHashesResponse to Subsystem 3 (Transaction Indexing) (V2.3)
- ❌ **REMOVED (V2.3): WriteBlockRequest from Consensus** - now via Event Bus subscription
- ❌ **REMOVED: WriteMerkleRootRequest (atomicity violation)**
- ❌ **REMOVED: WriteStateRootRequest (atomicity violation)**
- ❌ Reject: Events from unauthorized senders (verify envelope.sender_id)
- ❌ Reject: Duplicate block writes
- ❌ Reject: Writes when disk >95% full

### Post-Assembly Action (Two-Phase Commit):
After successfully assembling and writing a block (all 3 events received), Block Storage MUST:
1. Extract transaction hashes from the stored block
2. Send BlockStorageConfirmation to Mempool (Subsystem 6)
3. This allows Mempool to permanently delete included transactions

---

## SUBSYSTEM 3: TRANSACTION INDEXING

**V2.3 ROLE: CHOREOGRAPHY PARTICIPANT + DATA PROVIDER**
Transaction Indexing is a **choreography participant** that computes Merkle roots
and provides Merkle proofs. It subscribes to BlockValidated from the Event Bus
(NOT direct from Consensus) and publishes MerkleRootComputed.

### I Am Allowed To Talk To:
- **Event Bus** - Publish `MerkleRootComputed` events (V2.3 Choreography - PRIMARY OUTPUT)
- **Subsystem 2 (Block Storage)** - Query transaction locations and hashes for proof generation (V2.3 Data Retrieval)
- **Subsystem 7 (Bloom Filters)** - Provide transaction hashes
- **Subsystem 13 (Light Clients)** - Provide Merkle proofs

### Who Is Allowed To Talk To Me:
- **Event Bus (Subsystem 8 origin)** - Subscribe to `BlockValidated` events (V2.3 Choreography Trigger)
- **Subsystem 7 (Bloom Filters)** - Request transaction hashes
- **Subsystem 13 (Light Clients)** - Request Merkle proofs

**V2.3 CHOREOGRAPHY + DATA RETRIEVAL PATTERN:**
```
WRITE PATH (Choreography - Computing Merkle Root):
  Event Bus ──BlockValidated──→ [Transaction Indexing]
                                       │
                                       ↓ compute merkle tree
                                       │
                               MerkleRootComputed ──→ Event Bus ──→ [Block Storage]

READ PATH (Data Retrieval - Generating Proofs):
  [Light Client] ──MerkleProofRequest──→ [Transaction Indexing]
                                               │
                                               ↓ check local cache
                                               │
  ┌────────────────────────────────────────────┴─────────────────────────┐
  ↓ [Cache Hit]                                                   [Cache Miss] ↓
  Generate proof                                                          │
  from cache                                                              ↓
       │                           [Transaction Indexing] ──GetTransactionHashesRequest──→ [Block Storage]
       │                           [Block Storage] ──TransactionHashesResponse──→ [Transaction Indexing]
       │                                                                  │
       │                                                                  ↓ rebuild tree, cache it
       │                                                            Generate proof
       └──────────────────────────────────→ MerkleProofResponse ←─────────┘
```

### Strict Message Types:

**OUTGOING:**
```rust
/// V2.2: Published to event bus after computing Merkle root
/// Block Storage's Stateful Assembler consumes this event
struct MerkleRootComputedEvent {
    version: u16,
    block_hash: [u8; 32],
    block_height: u64,
    merkle_root: [u8; 32],
    transaction_count: u32,
    timestamp: u64,
    signature: Signature,
}

/// V2.3: Query to Block Storage for transaction location
/// SECURITY (Envelope-Only Identity): No requester_id in payload.
struct GetTransactionLocationRequest {
    version: u16,
    // No requester_id per Envelope-Only Identity
    correlation_id: [u8; 16],
    reply_to: Topic,
    transaction_hash: [u8; 32],
    signature: Signature,
}

struct MerkleProof {
    transaction_hash: [u8; 32],
    block_number: u64,
    proof_path: Vec<[u8; 32]>,  // Sibling hashes from leaf to root
    merkle_root: [u8; 32],
    signature: Signature,
}

struct TransactionHashList {
    block_number: u64,
    hashes: Vec<[u8; 32]>,
    timestamp: u64,
}
```

**INCOMING:**
```rust
/// V2.2: Subscribed from event bus (Choreography pattern)
/// This is the TRIGGER for Merkle tree computation
struct BlockValidatedEvent {
    version: u16,
    sender_id: SubsystemId,        // Must be 8 (Consensus)
    block: ValidatedBlock,
    block_hash: [u8; 32],
    block_height: u64,
    signature: Signature,
}

/// V2.3: Response from Block Storage for transaction location
struct TransactionLocationResponse {
    version: u16,
    correlation_id: [u8; 16],
    transaction_hash: [u8; 32],
    found: bool,
    block_hash: Option<[u8; 32]>,
    block_height: Option<u64>,
    transaction_index: Option<usize>,
    merkle_root: Option<[u8; 32]>,
    signature: Signature,
}

struct MerkleProofRequest {
    // SECURITY (Envelope-Only Identity): No requester_id in payload
    version: u16,
    correlation_id: [u8; 16],
    reply_to: Topic,
    transaction_hash: [u8; 32],
    signature: Signature,
}

struct TransactionHashRequest {
    // SECURITY (Envelope-Only Identity): No requester_id in payload
    version: u16,
    correlation_id: [u8; 16],
    reply_to: Topic,
    block_number: u64,
    signature: Signature,
}
```

### Security Boundaries:
- ✅ Accept: BlockValidatedEvent from Subsystem 8 (Consensus) only (V2.2 Choreography)
- ✅ Accept: MerkleProofRequest from Subsystem 13 (Light Clients) only
- ✅ Accept: TransactionHashRequest from Subsystem 7 (Bloom Filters) only
- ✅ Accept: TransactionLocationResponse from Subsystem 2 (Block Storage) only (V2.3)
- ✅ Send: MerkleRootComputedEvent to Event Bus (V2.2 Choreography)
- ✅ Send: GetTransactionLocationRequest to Subsystem 2 (Block Storage) (V2.3)
- ❌ Reject: BlockValidatedEvent from any subsystem other than Consensus
- ❌ Reject: Requests for non-existent blocks
- ❌ Reject: Tree depth >20 (1M+ transactions)

---

## SUBSYSTEM 4: STATE MANAGEMENT

### I Am Allowed To Talk To:
- **Event Bus** - Publish `StateRootComputed` events (V2.3 Choreography)
- **Subsystem 6 (Mempool)** - Provide balance/nonce checks
- **Subsystem 11 (Smart Contracts)** - Provide state reads
- **Subsystem 12 (Transaction Ordering)** - Provide conflict detection

### Who Is Allowed To Talk To Me:
- **Event Bus** - Subscribe to `BlockValidated` events from Subsystem 8 (V2.3 Choreography)
- **Subsystem 6 (Mempool)** - Check balance/nonce
- **Subsystem 11 (Smart Contracts)** - Read/write state
- **Subsystem 12 (Transaction Ordering)** - Detect conflicts
- **Subsystem 14 (Sharding)** - Access partitioned state

**V2.3 CHOREOGRAPHY PATTERN:**
State Management is a **choreography participant**, NOT an orchestrator target.
It subscribes to `BlockValidated` events, computes the state root, and publishes
`StateRootComputed` to the Event Bus. Block Storage (Subsystem 2) assembles this
with other components. There is NO direct State Management → Block Storage path.

### Strict Message Types:

**OUTGOING (V2.3 Choreography Event):**
```rust
/// V2.3: Published to Event Bus after computing state root for a validated block
/// Block Storage (Subsystem 2) subscribes to this as part of the Stateful Assembler pattern
/// 
/// SECURITY (Envelope-Only Identity): No requester_id in payload.
struct StateRootComputedPayload {
    /// Block hash this state root corresponds to (correlation key)
    block_hash: [u8; 32],
    /// Block height for ordering
    block_height: u64,
    /// The computed state root
    state_root: [u8; 32],
    /// Number of accounts modified
    accounts_modified: u32,
    /// Computation time in milliseconds (observability)
    computation_time_ms: u64,
}

struct AccountState {
    address: [u8; 20],
    balance: u256,
    nonce: u64,
    code_hash: [u8; 32],
    storage_root: [u8; 32],
}

struct StateReadResponse {
    request_id: u64,
    value: Option<Vec<u8>>,
    proof: Vec<[u8; 32]>,  // Patricia trie proof
}

struct ConflictDetectionResponse {
    request_id: u64,
    conflicts: Vec<(usize, usize)>,  // (tx_index_1, tx_index_2)
    conflict_type: Vec<ConflictType>,
}

enum ConflictType {
    ReadWrite,
    WriteWrite,
    NonceConflict,
}
```

**INCOMING:**
```rust
/// V2.3: Subscribed from Event Bus (published by Consensus, Subsystem 8)
/// This triggers state root computation as part of the choreography
struct BlockValidatedPayload {
    block_hash: [u8; 32],
    block_height: u64,
    block: ValidatedBlock,
    consensus_proof: ConsensusProof,
}

struct StateReadRequest {
    requester_id: SubsystemId,     // Must be 6, 11, 12, or 14
    address: [u8; 20],
    storage_key: Option<[u8; 32]>, // None for account balance/nonce
    block_number: u64,
    signature: Signature,
}

struct StateWriteRequest {
    requester_id: SubsystemId,     // Must be 11 only
    address: [u8; 20],
    storage_key: [u8; 32],
    value: Vec<u8>,
    signature: Signature,
}

struct BalanceCheckRequest {
    requester_id: SubsystemId,     // Must be 6
    address: [u8; 20],
    required_balance: u256,
    signature: Signature,
}

struct ConflictDetectionRequest {
    requester_id: SubsystemId,     // Must be 12
    transactions: Vec<TransactionAccessPattern>,
    signature: Signature,
}

struct TransactionAccessPattern {
    tx_hash: [u8; 32],
    reads: Vec<(Address, StorageKey)>,
    writes: Vec<(Address, StorageKey)>,
}
```

### Security Boundaries:
- ✅ Accept: StateReadRequest from Subsystems 6, 11, 12, 14 only
- ✅ Accept: StateWriteRequest from Subsystem 11 only
- ✅ Accept: BalanceCheckRequest from Subsystem 6 only
- ✅ Accept: ConflictDetectionRequest from Subsystem 12 only
- ❌ Reject: Direct state writes from any subsystem except 11
- ❌ Reject: Unsigned state modifications
- ❌ Reject: Negative balances
- ❌ Reject: Nonce decrements

---

## SUBSYSTEM 5: BLOCK PROPAGATION

### I Am Allowed To Talk To:
- **Subsystem 1 (Peer Discovery)** - Request peer list
- **External Network Peers** - Broadcast blocks

### Who Is Allowed To Talk To Me:
- **Subsystem 8 (Consensus)** - Send validated blocks to propagate
- **External Network Peers** - Receive blocks

### Strict Message Types:

**OUTGOING:**
```rust
struct BlockAnnouncement {
    block_hash: [u8; 32],
    block_number: u64,
    parent_hash: [u8; 32],
    timestamp: u64,
    signature: Signature,
}

struct CompactBlock {
    header: BlockHeader,
    short_txids: Vec<u64>,      // Short transaction IDs
    prefilled_txs: Vec<Transaction>,
    signature: Signature,
}

struct BlockRequest {
    block_hash: [u8; 32],
    request_id: u64,
    timestamp: u64,
}
```

**INCOMING:**
```rust
struct PropagateBlockRequest {
    requester_id: SubsystemId,     // Must be 8
    block: ValidatedBlock,
    validation_proof: ConsensusProof,
    signature: Signature,
}

struct BlockReceived {
    sender_peer_id: [u8; 32],
    block_hash: [u8; 32],
    full_block: Option<Block>,
    compact_block: Option<CompactBlock>,
    timestamp: u64,
}

struct BlockRequestReceived {
    sender_peer_id: [u8; 32],
    block_hash: [u8; 32],
    timestamp: u64,
}
```

### Security Boundaries:
- ✅ Accept: PropagateBlockRequest from Subsystem 8 only
- ✅ Accept: BlockReceived from known peers in Subsystem 1
- ✅ Accept: BlockRequestReceived from known peers
- ❌ Reject: Blocks without ConsensusProof
- ❌ Reject: Blocks from unknown peers
- ❌ Reject: >1 block announcement per peer per second (rate limit)
- ❌ Reject: Blocks >10MB

---

## SUBSYSTEM 6: TRANSACTION POOL (MEMPOOL)

### I Am Allowed To Talk To:
- **Subsystem 4 (State Management)** - Check balance/nonce
- **Subsystem 8 (Consensus)** - Provide transactions for blocks (via ProposeTransactionBatch)

### Who Is Allowed To Talk To Me:
- **Subsystem 10 (Signature Verification)** - Add verified transactions
- **Subsystem 8 (Consensus)** - Request transactions for block
- **Subsystem 2 (Block Storage)** - Confirm transaction inclusion (BlockStorageConfirmation)

### Strict Message Types:

**OUTGOING:**
```rust
/// Standard batch of transactions for informational purposes
struct TransactionBatch {
    transactions: Vec<ValidatedTransaction>,
    total_gas: u64,
    highest_fee: u256,
    timestamp: u64,
}

/// NEW: Two-Phase Commit - Propose transactions for inclusion
/// Transactions are moved to pending_inclusion state, NOT deleted
struct ProposeTransactionBatch {
    version: u16,
    correlation_id: [u8; 16],
    reply_to: Topic,
    
    // Transactions proposed for this block
    transactions: Vec<ValidatedTransaction>,
    total_gas: u64,
    
    // Block context
    target_block_height: u64,
    proposal_timestamp: u64,
}

struct MempoolStatus {
    pending_count: u32,
    pending_inclusion_count: u32,  // NEW: Transactions awaiting confirmation
    total_gas: u64,
    memory_usage: u64,
}
```

**INCOMING:**
```rust
struct AddTransactionRequest {
    version: u16,
    requester_id: SubsystemId,        // Must be 10
    correlation_id: [u8; 16],
    transaction: SignedTransaction,
    signature_valid: bool,            // Pre-verified by Subsystem 10
    signature: Signature,
}

struct GetTransactionsRequest {
    version: u16,
    requester_id: SubsystemId,        // Must be 8
    correlation_id: [u8; 16],
    reply_to: Topic,
    max_count: u32,
    max_gas: u64,
    signature: Signature,
}

/// DEPRECATED: Direct removal is replaced by Two-Phase Commit
/// Kept for backward compatibility with InvalidTransaction/Expired reasons only
struct RemoveTransactionsRequest {
    version: u16,
    requester_id: SubsystemId,        // Must be 8
    correlation_id: [u8; 16],
    transaction_hashes: Vec<[u8; 32]>,
    reason: RemovalReason,
    signature: Signature,
}

enum RemovalReason {
    // Included,  // DEPRECATED - use BlockStorageConfirmation instead
    Invalid,
    Expired,
}

/// NEW: Two-Phase Commit - Confirmation from Block Storage
/// Only upon receiving this message are transactions permanently deleted
struct BlockStorageConfirmation {
    version: u16,
    requester_id: SubsystemId,        // Must be 2
    correlation_id: [u8; 16],
    
    // Block that was successfully stored
    block_hash: [u8; 32],
    block_height: u64,
    
    // Transactions that were included and stored
    included_transactions: Vec<[u8; 32]>,
    
    // Storage timestamp (for audit trail)
    storage_timestamp: u64,
    signature: Signature,
}

/// NEW: Block rejection notification (triggers rollback)
struct BlockRejectedNotification {
    version: u16,
    requester_id: SubsystemId,        // Must be 8 or 2
    correlation_id: [u8; 16],
    
    // Block that was rejected
    block_hash: [u8; 32],
    block_height: u64,
    
    // Transactions to roll back to pending pool
    affected_transactions: Vec<[u8; 32]>,
    
    rejection_reason: BlockRejectionReason,
    signature: Signature,
}

enum BlockRejectionReason {
    ConsensusRejected,
    StorageFailure,
    Timeout,
    Reorg,
}
```

### Two-Phase Transaction Removal Protocol

**CRITICAL:** Transactions are NEVER deleted upon proposal. They are only deleted upon
confirmed storage. This prevents the "Transaction Loss" vulnerability.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    TWO-PHASE TRANSACTION REMOVAL                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Phase 1: PROPOSE (Mempool → Consensus)                                     │
│  ├─ Mempool sends ProposeTransactionBatch                                   │
│  ├─ Transactions moved from 'pending' to 'pending_inclusion' state          │
│  └─ Transactions are NOT deleted                                            │
│                                                                             │
│  Phase 2a: CONFIRM (Block Storage → Mempool)                                │
│  ├─ Block Storage sends BlockStorageConfirmation                            │
│  ├─ Transactions in included_transactions are PERMANENTLY DELETED           │
│  └─ Space is freed in mempool                                               │
│                                                                             │
│  Phase 2b: ROLLBACK (Consensus/Storage → Mempool)                           │
│  ├─ If block is rejected, BlockRejectedNotification is sent                 │
│  ├─ Transactions moved from 'pending_inclusion' back to 'pending'           │
│  └─ Transactions become available for next block                            │
│                                                                             │
│  TIMEOUT HANDLING:                                                          │
│  ├─ If no confirmation/rejection within 30 seconds, auto-rollback           │
│  └─ Prevents transactions from being stuck in limbo                         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Security Boundaries:
- ✅ Accept: AddTransactionRequest from Subsystem 10 only (must be pre-verified)
- ✅ Accept: GetTransactionsRequest from Subsystem 8 only
- ✅ Accept: RemoveTransactionsRequest from Subsystem 8 only (Invalid/Expired reasons)
- ✅ Accept: BlockStorageConfirmation from Subsystem 2 only
- ✅ Accept: BlockRejectedNotification from Subsystems 2, 8 only
- ❌ Reject: Transactions with signature_valid=false
- ❌ Reject: Transactions with gas price <1 gwei
- ❌ Reject: >16 pending transactions per account
- ❌ Reject: Total mempool size >5000 transactions
- ❌ Reject: Direct transaction additions from Subsystem 11 or others
- ❌ Reject: BlockStorageConfirmation with unknown correlation_id (prevents replay)

---

## SUBSYSTEM 7: TRANSACTION FILTERING (BLOOM FILTERS)

### I Am Allowed To Talk To:
- **Subsystem 1 (Peer Discovery)** - Request full nodes
- **Subsystem 13 (Light Clients)** - Provide filtered transactions

### Who Is Allowed To Talk To Me:
- **Subsystem 3 (Transaction Indexing)** - Receive transaction hashes
- **Subsystem 13 (Light Clients)** - Request filter updates

### Strict Message Types:

**OUTGOING:**
```rust
struct BloomFilter {
    filter_id: u64,
    bit_array: Vec<u8>,          // m-bit array
    hash_count: u8,              // k hash functions
    false_positive_rate: f32,
    block_range: (u64, u64),     // (start, end)
    signature: Signature,
}

struct FilteredTransactions {
    block_number: u64,
    transactions: Vec<[u8; 32]>,  // Matching transaction hashes
    false_positives_included: bool,
}
```

**INCOMING:**
```rust
struct BuildFilterRequest {
    requester_id: SubsystemId,       // Must be 13
    watched_addresses: Vec<[u8; 20]>,
    start_block: u64,
    end_block: u64,
    target_fpr: f32,                 // Target false positive rate
    signature: Signature,
}

struct UpdateFilterRequest {
    requester_id: SubsystemId,       // Must be 13
    filter_id: u64,
    add_addresses: Vec<[u8; 20]>,
    remove_addresses: Vec<[u8; 20]>,
    signature: Signature,
}

struct TransactionHashUpdate {
    sender_id: SubsystemId,          // Must be 3
    block_number: u64,
    hashes: Vec<[u8; 32]>,
    signature: Signature,
}
```

### Security Boundaries:
- ✅ Accept: BuildFilterRequest from Subsystem 13 only
- ✅ Accept: UpdateFilterRequest from Subsystem 13 only
- ✅ Accept: TransactionHashUpdate from Subsystem 3 only
- ❌ Reject: Filters with >1000 watched addresses (privacy risk)
- ❌ Reject: FPR <0.01 or >0.1 (too precise or too noisy)
- ❌ Reject: >1 filter update per 10 blocks per client
- ❌ Reject: Direct filter access from external nodes

---

## SUBSYSTEM 8: CONSENSUS MECHANISM

**V2.3 CRITICAL DESIGN CHANGE (Choreography, NOT Orchestration):**
Consensus is a **validation-only** subsystem. It does NOT orchestrate block storage.
It validates blocks and publishes `BlockValidated` to the Event Bus. Subsystems 3, 4,
and 2 independently react to this event. This eliminates the single-point-of-failure
bottleneck that existed in the rejected v2.0/v2.1 Orchestrator pattern.

### I Am Allowed To Talk To:
- **Event Bus** - Publish `BlockValidated` events (V2.3 Choreography - PRIMARY OUTPUT)
- **Subsystem 5 (Block Propagation)** - Propagate validated blocks to network
- **Subsystem 6 (Mempool)** - Get transactions for block proposal
- **Subsystem 9 (Finality)** - Provide attestations for PoS finality
- **Subsystem 10 (Signature Verification)** - Verify validator signatures
- **Subsystem 12 (Transaction Ordering)** - Order transactions (optional)
- **Subsystem 14 (Sharding)** - Coordinate shards (optional)
- **Subsystem 15 (Cross-Chain)** - Provide finality proofs (optional)

### Who Is Allowed To Talk To Me:
- **Subsystem 5 (Block Propagation)** - Receive new blocks from network
- **Subsystem 6 (Mempool)** - Propose transaction batches for new blocks
- **Subsystem 10 (Signature Verification)** - Provide verified validator signatures
- **External Validators** - Receive attestations (PoS) or block proposals (PBFT)

**REMOVED (V2.3 - Orchestrator Anti-Pattern Eliminated):**
- ~~Subsystem 2 (Block Storage) - Store validated blocks~~ → Now via Event Bus
- ~~Subsystem 3 (Transaction Indexing) - Verify Merkle roots~~ → Subsystem 3 subscribes to Event Bus

### Strict Message Types:

**OUTGOING (V2.3 Choreography Event - PRIMARY):**
```rust
/// V2.3: Published to Event Bus after successful block validation
/// 
/// CHOREOGRAPHY: This is the trigger for the entire block processing flow.
/// - Subsystem 3 (Transaction Indexing) subscribes → computes MerkleRootComputed
/// - Subsystem 4 (State Management) subscribes → computes StateRootComputed
/// - Subsystem 2 (Block Storage) subscribes → buffers as Stateful Assembler
/// 
/// CRITICAL (V2.3): merkle_root and state_root are NOT computed by Consensus.
/// They are set to None/placeholder and filled in by the Stateful Assembler
/// after receiving MerkleRootComputed and StateRootComputed events.
/// 
/// SECURITY (Envelope-Only Identity): No requester_id in payload.
struct BlockValidatedPayload {
    /// Hash of the validated block (correlation key for assembly)
    block_hash: [u8; 32],
    /// Block height
    block_height: u64,
    /// The validated block (header + transactions)
    block: ValidatedBlock,
    /// Consensus proof (PoS attestations or PBFT votes)
    consensus_proof: ConsensusProof,
}

/// V2.3: Block structure - merkle_root and state_root are TBD
struct ValidatedBlock {
    header: BlockHeader,
    transactions: Vec<ValidatedTransaction>,
    /// V2.3: These are computed by Subsystems 3 and 4, NOT by Consensus
    /// Set to None here; Block Storage fills them in during assembly
    // merkle_root: REMOVED - computed by Subsystem 3
    // state_root: REMOVED - computed by Subsystem 4
    consensus_proof: ConsensusProof,
    validator_signatures: Vec<Signature>,
}

struct ConsensusProof {
    proof_type: ConsensusType,
    validator_set: Vec<[u8; 20]>,
    attestations: Vec<Attestation>,  // For PoS
    pbft_votes: Option<PBFTVotes>,   // For PBFT
}

enum ConsensusType {
    ProofOfStake,
    PBFT,
}

/// V2.3: Block header - merkle_root and state_root are placeholders
struct BlockHeader {
    parent_hash: [u8; 32],
    block_number: u64,
    timestamp: u64,
    /// V2.3: Placeholder - actual value comes from Subsystem 3 via Event Bus
    merkle_root: Option<[u8; 32]>,
    /// V2.3: Placeholder - actual value comes from Subsystem 4 via Event Bus
    state_root: Option<[u8; 32]>,
    difficulty: u256,
    gas_used: u64,
    gas_limit: u64,
}
```

**INCOMING:**
```rust
struct ValidateBlockRequest {
    requester_id: SubsystemId,          // Must be 5
    block: Block,
    received_from: [u8; 32],            // Peer ID
    timestamp: u64,
}

struct AttestationReceived {
    sender_id: SubsystemId,             // Must be 10 (verified signature)
    validator: [u8; 20],
    block_hash: [u8; 32],
    slot: u64,
    signature: Signature,
    signature_valid: bool,
}

struct PBFTMessage {
    sender_id: SubsystemId,             // Must be 10 (verified signature)
    message_type: PBFTPhase,
    block_hash: [u8; 32],
    view: u64,
    sender_validator: [u8; 20],
    signature: Signature,
}

enum PBFTPhase {
    PrePrepare,
    Prepare,
    Commit,
    ViewChange,
}
```

### Security Boundaries:
- ✅ Accept: ValidateBlockRequest from Subsystem 5 only
- ✅ Accept: AttestationReceived from Subsystem 10 only
- ✅ Accept: PBFTMessage from Subsystem 10 only
- ❌ Reject: Blocks without valid transactions
- ❌ Reject: Blocks with invalid Merkle root
- ❌ Reject: Blocks with invalid state transitions
- ❌ Reject: >33% Byzantine validators (safety threshold)
- ❌ Reject: Blocks older than 2 epochs

### Zero-Trust Signature Re-Verification (CRITICAL)

**SECURITY MANDATE:** Consensus MUST NOT trust the `signature_valid` flag from 
Signature Verification (Subsystem 10). All critical signatures MUST be 
independently re-verified before processing.

```rust
// WRONG - Trusting pre-validation flag
if attestation.signature_valid {
    process_attestation(attestation);  // VULNERABLE!
}

// CORRECT - Zero-trust re-verification
fn handle_attestation(attestation: AttestationReceived) -> Result<(), Error> {
    // Step 1: Verify envelope (standard checks)
    verify_envelope(&attestation)?;
    
    // Step 2: INDEPENDENTLY re-verify signature (ZERO TRUST)
    let message = attestation.block_hash;
    let recovered_signer = ecrecover(message, &attestation.signature)?;
    
    if recovered_signer != attestation.validator {
        return Err(Error::SignatureVerificationFailed);
    }
    
    // Step 3: Verify validator is in active set
    if !self.validator_set.contains(&recovered_signer) {
        return Err(Error::UnknownValidator);
    }
    
    // Now safe to process
    self.process_attestation(attestation)?;
    Ok(())
}
```

**Rationale:** If Subsystem 10 is compromised, an attacker could inject 
attestations with `signature_valid=true` for signatures they never verified.
By re-verifying, Consensus becomes independently secure.

---

## SUBSYSTEM 9: FINALITY MECHANISM

### I Am Allowed To Talk To:
- **Subsystem 2 (Block Storage)** - Mark blocks as finalized
- **Subsystem 15 (Cross-Chain)** - Provide finality proofs

### Who Is Allowed To Talk To Me:
- **Subsystem 8 (Consensus)** - Provide attestations for finality

### Strict Message Types:

**OUTGOING:**
```rust
struct FinalityProof {
    checkpoint: u64,                // Epoch boundary
    block_hash: [u8; 32],
    justification: Vec<Attestation>,
    supermajority_reached: bool,     // >2/3 validators
    timestamp: u64,
    signature: Signature,
}

struct FinalizedNotification {
    block_number: u64,
    block_hash: [u8; 32],
    finalized_at: u64,
}
```

**INCOMING:**
```rust
struct FinalityCheckRequest {
    requester_id: SubsystemId,      // Must be 8
    checkpoint: u64,
    attestations: Vec<Attestation>,
    signature: Signature,
}

struct FinalityProofRequest {
    requester_id: SubsystemId,      // Must be 15
    block_hash: [u8; 32],
    signature: Signature,
}

struct Attestation {
    validator: [u8; 20],
    source_checkpoint: u64,
    target_checkpoint: u64,
    block_hash: [u8; 32],
    signature: Signature,
}
```

### Security Boundaries:
- ✅ Accept: FinalityCheckRequest from Subsystem 8 only
- ✅ Accept: FinalityProofRequest from Subsystem 15 only
- ✅ Accept: Attestations with >2/3 validator support
- ❌ Reject: Attestations without 67%+ stake participation
- ❌ Reject: Conflicting finalizations (slashable offense)
- ❌ Reject: Finalization of non-consecutive checkpoints
- ❌ Reject: Attestations from unknown validators

### Zero-Trust Signature Re-Verification (CRITICAL)

**SECURITY MANDATE:** Finality MUST NOT trust any pre-validation flags. All 
attestation signatures MUST be independently re-verified before counting toward 
supermajority. This is especially critical for finality because:

1. Finality is IRREVERSIBLE - once finalized, blocks cannot be reverted
2. A compromised Subsystem 10 could fake 67% attestation support
3. Economic finality requires cryptographic certainty, not trust

```rust
fn verify_attestations_for_finality(
    attestations: &[Attestation],
    checkpoint: u64,
) -> Result<FinalityResult, Error> {
    let mut valid_stake = 0u128;
    let total_stake = self.get_total_active_stake();
    
    for attestation in attestations {
        // ZERO TRUST: Re-verify every signature independently
        let message = keccak256(&encode_attestation(attestation));
        let recovered = ecrecover(message, &attestation.signature)?;
        
        // Verify recovered address matches claimed validator
        if recovered != attestation.validator {
            log::warn!("Invalid attestation signature from {:?}", attestation.validator);
            continue;  // Skip invalid, don't fail entire batch
        }
        
        // Verify validator is in active set for this epoch
        if let Some(stake) = self.validator_stakes.get(&recovered) {
            valid_stake += stake;
        }
    }
    
    // Check supermajority (67%)
    if valid_stake * 3 >= total_stake * 2 {
        Ok(FinalityResult::Finalized { checkpoint })
    } else {
        Err(Error::InsufficientAttestations {
            have: valid_stake,
            need: (total_stake * 2) / 3,
        })
    }
}
```

### Circuit Breaker Integration

See Architecture.md Section 5.4 for Finality Circuit Breaker behavior.
If finality cannot be achieved after repeated attempts, the node transitions
to HALTED_AWAITING_INTERVENTION state to prevent livelock.

---

## SUBSYSTEM 10: SIGNATURE VERIFICATION

### I Am Allowed To Talk To:
- **Subsystem 6 (Mempool)** - Send verified transactions
- **Subsystem 8 (Consensus)** - Send verified validator signatures

### Who Is Allowed To Talk To Me:
- **External Network** - Receive signed transactions (via P2P gateway)
- **Subsystem 1 (Peer Discovery)** - Verify node identity signatures for DDoS defense
- **Subsystem 5 (Block Propagation)** - Verify block signatures from network peers
- **Subsystem 6 (Mempool)** - Verify transaction signatures before pool entry
- **Subsystem 8 (Consensus)** - Verify block/validator signatures
- **Subsystem 9 (Finality)** - Verify attestation signatures

### FORBIDDEN Consumers (Principle of Least Privilege):
The following subsystems are EXPLICITLY FORBIDDEN from accessing SignatureVerification:
- ❌ Subsystem 2 (Block Storage) - Storage only, receives pre-verified data
- ❌ Subsystem 3 (Transaction Indexing) - Indexing only, receives pre-verified data
- ❌ Subsystem 4 (State Management) - State only, receives pre-verified data
- ❌ Subsystem 7 (Bloom Filters) - Filtering only, no signature needs
- ❌ Subsystem 11 (Smart Contracts) - Execution only, receives pre-verified transactions
- ❌ Subsystem 12 (Transaction Ordering) - Ordering only, receives pre-verified data
- ❌ Subsystem 13 (Light Clients) - Receives proofs, does not verify signatures directly
- ❌ Subsystem 14 (Sharding) - Coordination only, uses Consensus for verification
- ❌ Subsystem 15 (Cross-Chain) - Uses Finality proofs, not direct signature verification

**Security Rationale (Updated):** 
- Subsystem 1 (Peer Discovery) is NOW ALLOWED to verify signatures for **DDoS defense at the network edge**.
- If Peer Discovery cannot verify signatures, it must accept unverified data into the system.
- An attacker can flood the Mempool with invalid transactions, consuming all CPU.
- By allowing Peer Discovery to verify signatures **at the door**, we block attacks before they hit internal systems.
- Other low-priority subsystems remain forbidden to minimize attack surface.

### Strict Message Types:

**OUTGOING:**
```rust
struct VerifiedTransaction {
    version: u16,                    // Protocol version
    transaction: SignedTransaction,
    signer_address: [u8; 20],
    signature_valid: bool,
    verification_timestamp: u64,
}

struct VerifiedSignature {
    version: u16,                    // Protocol version
    message_hash: [u8; 32],
    signature: Signature,
    signer: [u8; 20],
    valid: bool,
    verification_timestamp: u64,
}

struct BatchVerificationResult {
    version: u16,                    // Protocol version
    request_id: u64,
    correlation_id: [u8; 16],        // Maps to original request
    results: Vec<bool>,              // One per signature
    total_valid: u32,
    total_invalid: u32,
}

/// Response for Peer Discovery node identity verification
struct NodeIdentityVerificationResult {
    version: u16,
    correlation_id: [u8; 16],
    node_id: [u8; 32],
    identity_valid: bool,
    verification_timestamp: u64,
}
```

**INCOMING:**
```rust
struct VerifyTransactionRequest {
    version: u16,                      // Protocol version - MUST be validated first
    requester_id: SubsystemId,         // MUST be 1, 5, 6, 8, or 9 ONLY
    correlation_id: [u8; 16],          // For async response correlation
    reply_to: Topic,                   // Where to send response
    transaction: SignedTransaction,
    expected_signer: Option<[u8; 20]>,
}

/// NEW: Node identity verification for DDoS defense
/// Allows Peer Discovery to verify node signatures before accepting peers
struct VerifyNodeIdentityRequest {
    version: u16,
    requester_id: SubsystemId,         // MUST be 1 (Peer Discovery) ONLY
    correlation_id: [u8; 16],
    reply_to: Topic,
    node_id: [u8; 32],                 // Kademlia node ID
    claimed_pubkey: [u8; 33],          // Compressed public key
    signature: Signature,              // Signature over node_id
}

struct SignedTransaction {
    nonce: u64,
    to: [u8; 20],
    value: u256,
    data: Vec<u8>,
    gas_limit: u64,
    gas_price: u256,
    signature: Signature,
}

struct Signature {
    r: [u8; 32],
    s: [u8; 32],
    v: u8,
}

struct VerifySignatureRequest {
    version: u16,                      // Protocol version - MUST be validated first
    requester_id: SubsystemId,         // MUST be 1, 5, 8, or 9 ONLY
    correlation_id: [u8; 16],          // For async response correlation
    reply_to: Topic,                   // Where to send response
    message_hash: [u8; 32],
    signature: Signature,
    expected_signer: [u8; 20],
}

struct BatchVerifyRequest {
    version: u16,                      // Protocol version - MUST be validated first
    requester_id: SubsystemId,         // MUST be 8 ONLY
    correlation_id: [u8; 16],          // For async response correlation
    reply_to: Topic,                   // Where to send response
    signatures: Vec<(MessageHash, Signature, ExpectedSigner)>,
}
```

### Security Boundaries:
- ✅ Accept: VerifyTransactionRequest from Subsystems 1, 5, 6, 8, 9 ONLY
- ✅ Accept: VerifyNodeIdentityRequest from Subsystem 1 (Peer Discovery) ONLY
- ✅ Accept: VerifySignatureRequest from Subsystems 1, 5, 8, 9 ONLY
- ✅ Accept: BatchVerifyRequest from Subsystem 8 ONLY
- ❌ **REJECT: ALL requests from Subsystems 2, 3, 4, 7, 11, 12, 13, 14, 15**
- ❌ Reject: Signatures with low s value (malleability)
- ❌ Reject: Signatures with v ∉ {27, 28}
- ❌ Reject: Batch verification with >1000 signatures (DoS risk)
- ❌ Reject: Invalid ECDSA points
- ❌ Reject: Messages with unsupported version field

### Rate Limiting (DDoS Defense):
- Subsystem 1 requests: Max 100 verifications/second (network edge protection)
- Subsystem 5, 6 requests: Max 1000 verifications/second (internal traffic)
- Subsystem 8, 9 requests: No limit (consensus-critical path)

---

## SUBSYSTEM 11: SMART CONTRACT EXECUTION

### I Am Allowed To Talk To:
- **Subsystem 4 (State Management)** - Read/write contract state
- **Subsystem 15 (Cross-Chain)** - Execute HTLC contracts

### Who Is Allowed To Talk To Me:
- **Subsystem 8 (Consensus)** - Execute transactions in validated block
- **Subsystem 12 (Transaction Ordering)** - Execute ordered transactions

### Strict Message Types:

**OUTGOING:**
```rust
struct ExecutionResult {
    transaction_hash: [u8; 32],
    success: bool,
    gas_used: u64,
    return_data: Vec<u8>,
    logs: Vec<Log>,
    state_changes: Vec<StateChange>,
    signature: Signature,
}

struct StateChange {
    address: [u8; 20],
    storage_key: [u8; 32],
    old_value: Vec<u8>,
    new_value: Vec<u8>,
}

struct Log {
    address: [u8; 20],
    topics: Vec<[u8; 32]>,
    data: Vec<u8>,
}

struct ContractDeployed {
    contract_address: [u8; 20],
    deployer: [u8; 20],
    code_hash: [u8; 32],
    gas_used: u64,
}
```

**INCOMING:**
```rust
struct ExecuteTransactionRequest {
    requester_id: SubsystemId,         // Must be 8 or 12
    transaction: ValidatedTransaction,
    block_context: BlockContext,
    gas_limit: u64,
    signature: Signature,
}

struct BlockContext {
    block_number: u64,
    timestamp: u64,
    coinbase: [u8; 20],
    difficulty: u256,
    gas_limit: u64,
}

struct ValidatedTransaction {
    from: [u8; 20],
    to: Option<[u8; 20]>,  // None for contract creation
    value: u256,
    data: Vec<u8>,
    nonce: u64,
    gas_price: u256,
    gas_limit: u64,
}

struct ExecuteHTLCRequest {
    requester_id: SubsystemId,         // Must be 15
    htlc_contract: [u8; 20],
    operation: HTLCOperation,
    signature: Signature,
}

enum HTLCOperation {
    Claim { secret: [u8; 32] },
    Refund,
}
```

### Security Boundaries:
- ✅ Accept: ExecuteTransactionRequest from Subsystems 8, 12 only
- ✅ Accept: ExecuteHTLCRequest from Subsystem 15 only
- ✅ Accept: Only transactions with validated signatures
- ❌ Reject: Execution without valid BlockContext
- ❌ Reject: Gas limit >30M (block gas limit)
- ❌ Reject: Recursive calls >1024 depth
- ❌ Reject: Execution time >5 seconds (timeout)
- ❌ Reject: Direct execution requests from external sources

---

## SUBSYSTEM 12: TRANSACTION ORDERING (DAG)

### I Am Allowed To Talk To:
- **Subsystem 4 (State Management)** - Detect conflicts
- **Subsystem 11 (Smart Contracts)** - Execute ordered transactions

### Who Is Allowed To Talk To Me:
- **Subsystem 8 (Consensus)** - Provide transactions to order

### Strict Message Types:

**OUTGOING:**
```rust
struct OrderedTransactions {
    transactions: Vec<ValidatedTransaction>,
    dependency_graph: DependencyGraph,
    parallel_batches: Vec<Vec<usize>>,  // Indices of parallelizable txs
    signature: Signature,
}

struct DependencyGraph {
    nodes: Vec<[u8; 32]>,        // Transaction hashes
    edges: Vec<(usize, usize)>,  // (from, to) dependencies
}

struct OrderingMetrics {
    total_transactions: u32,
    parallel_batches: u32,
    sequential_transactions: u32,
    conflicts_detected: u32,
    ordering_time_ms: u64,
}
```

**INCOMING:**
```rust
struct OrderTransactionsRequest {
    requester_id: SubsystemId,      // Must be 8
    transactions: Vec<ValidatedTransaction>,
    signature: Signature,
}
```

### Security Boundaries:
- ✅ Accept: OrderTransactionsRequest from Subsystem 8 only
- ❌ Reject: Transactions with circular dependencies
- ❌ Reject: Dependency graphs with >10,000 edges (complexity attack)
- ❌ Reject: Requests from subsystems other than Consensus

---

## SUBSYSTEM 13: LIGHT CLIENT SYNC

### I Am Allowed To Talk To:
- **Subsystem 1 (Peer Discovery)** - Request full nodes
- **Subsystem 7 (Bloom Filters)** - Setup filters
- **External Full Nodes** - Request headers and data

### Who Is Allowed To Talk To Me:
- **Subsystem 3 (Transaction Indexing)** - Receive Merkle proofs
- **External Full Nodes** - Receive headers and proofs

### Strict Message Types:

**OUTGOING:**
```rust
struct SyncRequest {
    start_block: u64,
    end_block: u64,
    checkpoint_hash: [u8; 32],
    signature: Signature,
}

struct ProofVerificationRequest {
    transaction_hash: [u8; 32],
    block_number: u64,
    signature: Signature,
}

struct FilterSetupRequest {
    watched_addresses: Vec<[u8; 20]>,
    target_fpr: f32,
    signature: Signature,
}
```

**INCOMING:**
```rust
struct HeaderChain {
    headers: Vec<BlockHeader>,
    checkpoint_proof: Vec<[u8; 32]>,
    signature: Signature,
}

struct MerkleProofReceived {
    sender_id: SubsystemId,           // Must be 3
    proof: MerkleProof,
    verified: bool,
}

struct FilterCreated {
    sender_id: SubsystemId,           // Must be 7
    filter: BloomFilter,
}
```

### Security Boundaries:
- ✅ Accept: MerkleProofReceived from Subsystem 3 only
- ✅ Accept: FilterCreated from Subsystem 7 only
- ✅ Accept: HeaderChain from trusted full nodes
- ❌ Reject: Headers without valid PoW/PoS proof
- ❌ Reject: Proofs that don't match stored Merkle root
- ❌ Reject: Headers from unknown full nodes
- ❌ Reject: Sync requests spanning >10,000 blocks

---

## SUBSYSTEM 14: SHARDING (OPTIONAL)

### I Am Allowed To Talk To:
- **Subsystem 4 (State Management)** - Access partitioned state
- **Subsystem 8 (Consensus)** - Report shard status

### Who Is Allowed To Talk To Me:
- **Subsystem 8 (Consensus)** - Beacon chain coordination

### Strict Message Types:

**OUTGOING:**
```rust
struct ShardAssignment {
    account: [u8; 20],
    shard_id: u16,
    signature: Signature,
}

struct CrossShardTransaction {
    from_shard: u16,
    to_shard: u16,
    transaction: ValidatedTransaction,
    two_phase_commit_id: u64,
    signature: Signature,
}

struct ShardStatus {
    shard_id: u16,
    validator_count: u32,
    transaction_count: u64,
    state_size: u64,
    load_percentage: u8,
}
```

**INCOMING:**
```rust
struct AssignShardRequest {
    requester_id: SubsystemId,        // Must be 8
    account: [u8; 20],
    signature: Signature,
}

struct CrossShardCommitRequest {
    requester_id: SubsystemId,        // Must be 8
    commit_id: u64,
    phase: CommitPhase,
    participating_shards: Vec<u16>,
    signature: Signature,
}

enum CommitPhase {
    Prepare,
    Commit,
    Abort,
}

struct RebalanceRequest {
    requester_id: SubsystemId,        // Must be 8
    target_load: u8,
    signature: Signature,
}
```

### Security Boundaries:
- ✅ Accept: AssignShardRequest from Subsystem 8 only
- ✅ Accept: CrossShardCommitRequest from Subsystem 8 only
- ✅ Accept: RebalanceRequest from Subsystem 8 only
- ❌ Reject: Cross-shard transactions without 2PC
- ❌ Reject: Shards with <128 validators (security threshold)
- ❌ Reject: Shard assignments from non-Consensus subsystems
- ❌ Reject: Rebalancing that creates load imbalance >20%

---

## SUBSYSTEM 15: CROSS-CHAIN COMMUNICATION

### I Am Allowed To Talk To:
- **Subsystem 8 (Consensus)** - Request finality proofs
- **Subsystem 11 (Smart Contracts)** - Execute HTLC operations

### Who Is Allowed To Talk To Me:
- **Subsystem 9 (Finality)** - Provide finality proofs
- **External Blockchains** - Receive cross-chain messages

### Strict Message Types:

**OUTGOING:**
```rust
struct HTLCInitiate {
    hashlock: [u8; 32],          // H(secret)
    timelock: u64,               // Expiration timestamp
    amount: u256,
    recipient_chain: ChainId,
    recipient: [u8; 20],
    signature: Signature,
}

struct HTLCClaim {
    htlc_id: [u8; 32],
    secret: [u8; 32],            // Reveals the secret
    signature: Signature,
}

struct HTLCRefund {
    htlc_id: [u8; 32],
    reason: RefundReason,
    signature: Signature,
}

enum RefundReason {
    Timeout,
    Failed,
}

struct ChainId {
    chain_name: String,
    chain_id: u64,
}
```

**INCOMING:**
```rust
struct InitiateSwapRequest {
    requester_id: SubsystemId,        // External or validated
    amount: u256,
    target_chain: ChainId,
    target_recipient: [u8; 20],
    hashlock: [u8; 32],
    timelock: u64,
    signature: Signature,
}

struct ClaimSwapRequest {
    requester_id: SubsystemId,        // External or validated
    htlc_id: [u8; 32],
    secret: [u8; 32],
    signature: Signature,
}

struct FinalityProofReceived {
    sender_id: SubsystemId,           // Must be 9
    proof: FinalityProof,
    chain: ChainId,
}

struct CrossChainMessage {
    source_chain: ChainId,
    message_type: CrossChainMessageType,
    payload: Vec<u8>,
    signature: Signature,
}

enum CrossChainMessageType {
    HTLCInitiated,
    HTLCClaimed,
    HTLCRefunded,
}
```

### Security Boundaries:
- ✅ Accept: FinalityProofReceived from Subsystem 9 only
- ✅ Accept: InitiateSwapRequest from validated external sources
- ✅ Accept: ClaimSwapRequest with valid secret reveal
- ❌ Reject: HTLCs with timelock <6 hours
- ❌ Reject: HTLCs without finality proof on both chains
- ❌ Reject: Secret reveals after timelock expiration
- ❌ Reject: Cross-chain messages without valid signatures
- ❌ Reject: Claims without valid hash preimage (H(secret) == hashlock)

---

## COMMUNICATION FLOW DIAGRAM

```
┌─────────────────────────────────────────────────────────────────┐
│                     EXTERNAL SOURCES                            │
├─────────────────────────────────────────────────────────────────┤
│  • Bootstrap Nodes → [1]                                        │
│  • Network Peers → [1, 5]                                       │
│  • Transactions → [10]                                          │
│  • Validators → [8, 10]                                         │
│  • Full Nodes → [13]                                            │
│  • Other Blockchains → [15]                                     │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                    SUBSYSTEM INTERACTIONS                       │
└─────────────────────────────────────────────────────────────────┘

[1] Peer Discovery
     ↓ PeerList
     ├──→ [5] Block Propagation
     ├──→ [7] Bloom Filters
     └──→ [13] Light Clients

[10] Signature Verification
     ↓ VerifiedTransaction
     ├──→ [6] Mempool
     │
     ↓ VerifiedSignature
     └──→ [8] Consensus

[6] Mempool
     ↓ BalanceCheckRequest
     ├──→ [4] State Management
     │
     ↓ TransactionBatch
     └──→ [8] Consensus

[8] Consensus
     ↓ BuildMerkleTreeRequest
     ├──→ [3] Transaction Indexing
     │
     ↓ StateReadRequest
     ├──→ [4] State Management
     │
     ↓ ValidatedBlock
     ├──→ [2] Block Storage
     ├──→ [5] Block Propagation
     │
     ↓ Attestations
     ├──→ [9] Finality
     │
     ↓ OrderTransactionsRequest (optional)
     ├──→ [12] Transaction Ordering
     │
     └──→ [14] Sharding (optional)

[3] Transaction Indexing
     ↓ MerkleRootStored
     ├──→ [2] Block Storage
     │
     ↓ TransactionHashList
     ├──→ [7] Bloom Filters
     │
     ↓ MerkleProof
     └──→ [13] Light Clients

[4] State Management
     ↓ StateRootStored
     ├──→ [2] Block Storage
     │
     ↓ AccountState / StateReadResponse
     ├──→ [6] Mempool
     ├──→ [11] Smart Contracts
     ├──→ [12] Transaction Ordering
     └──→ [14] Sharding

[9] Finality
     ↓ FinalityProof
     ├──→ [2] Block Storage
     └──→ [15] Cross-Chain

[11] Smart Contracts
     ↓ StateWriteRequest
     ├──→ [4] State Management
     │
     ↓ ExecutionResult
     └──→ [15] Cross-Chain (HTLC)

[12] Transaction Ordering
     ↓ ConflictDetectionRequest
     ├──→ [4] State Management
     │
     ↓ OrderedTransactions
     └──→ [11] Smart Contracts

[7] Bloom Filters
     ↓ BloomFilter
     └──→ [13] Light Clients

[13] Light Clients
     ↓ PeerListRequest
     ├──→ [1] Peer Discovery
     │
     ↓ MerkleProofRequest
     ├──→ [3] Transaction Indexing
     │
     └──→ [7] Bloom Filters

[14] Sharding
     ↓ ShardStatus
     ├──→ [8] Consensus
     │
     └──→ [4] State Management

[15] Cross-Chain
     ↓ FinalityProofRequest
     ├──→ [9] Finality
     │
     ↓ ExecuteHTLCRequest
     └──→ [11] Smart Contracts
```

---

## IPC SECURITY SUMMARY TABLE

| Subsystem | Allowed Senders | Allowed Recipients | Critical Security Check |
|-----------|----------------|-------------------|------------------------|
| 1 (Peer Discovery) | 5, 7, 13, External | 5, 7, 10, 13 | PoW for bootstrap, reputation scoring |
| 2 (Block Storage) | **3, 8, 9** | **3**, 6 | **V2.2 Choreography + V2.3 Transaction Lookup** |
| 3 (Transaction Indexing) | **2**, 7, 8, 13 | **2**, 7, 13, Event Bus | **V2.2: Subscribes to BlockValidated, publishes MerkleRootComputed; V2.3: Queries Block Storage for tx locations** |
| 4 (State Management) | 6, 11, 12, 14 | 6, 8, 11, 12 | Only Subsystem 11 can write |
| 5 (Block Propagation) | 8, External Peers | 1, 10, External Peers | ConsensusProof required |
| 6 (Mempool) | 2, 8, 10 | 4, 8, 10 | Only pre-verified transactions, Two-Phase Commit |
| 7 (Bloom Filters) | 3, 13 | 1, 13 | FPR limits, address count limits |
| 8 (Consensus) | 5, 10, External | 2, 3, 4, 5, 6, 9, 10, 12, 14, 15, Event Bus | **V2.2: Publishes BlockValidated to Event Bus** |
| 9 (Finality) | 8 | 2, 10, 15 | Supermajority (>2/3) required, **circuit breaker on failure** |
| 10 (Signature Verification) | **1, 5, 6, 8, 9** | 6, 8 | **DDoS defense: Peer Discovery can verify at edge** |
| 11 (Smart Contracts) | 8, 12 | 4, 15 | Gas limits, depth limits |
| 12 (Transaction Ordering) | 8 | 4, 11 | Cycle detection |
| 13 (Light Clients) | 1, 3, 7, External | 1, 3, 7 | Merkle proof validation |
| 14 (Sharding) | 8 | 4, 8 | Minimum validator count |
| 15 (Cross-Chain) | 9, 11, External | 8, 11 | Finality proofs, timelock enforcement |
| **16 (API Gateway)** | **External HTTP/WS, Event Bus** | **1, 2, 3, 4, 6, 8, 10, 11, Event Bus** | **Rate limiting, method whitelist, timeout protection** |
| **17 (Block Production)** | **6, 4, 9, 8 (events), Admin CLI** | **6, 4, 8, 10, Event Bus** | **Gas limit enforcement, nonce ordering, state validity, censorship detection** |

---

## SUBSYSTEM 16: API GATEWAY (NEW)

**Purpose:** Single entry point for all external interactions with the node.
External-facing HTTP/WebSocket server exposing JSON-RPC, REST, and subscription APIs.

### I Am Allowed To Talk To:
- **Subsystem 1 (Peer Discovery)** - Peer info queries, admin operations
- **Subsystem 2 (Block Storage)** - Block queries (eth_getBlock*)
- **Subsystem 3 (Transaction Indexing)** - Transaction/receipt queries, logs
- **Subsystem 4 (State Management)** - State queries (eth_getBalance, eth_getCode)
- **Subsystem 6 (Mempool)** - Transaction submission, status queries
- **Subsystem 8 (Consensus)** - Admin: start/stop mining (protected)
- **Subsystem 10 (Signature Verification)** - Transaction validation before mempool
- **Subsystem 11 (Smart Contracts)** - eth_call, eth_estimateGas
- **Event Bus** - Subscribe to events for WebSocket notifications

### Who Is Allowed To Talk To Me:
- **External HTTP/WS Clients** - All public JSON-RPC methods
- **Localhost Admin Clients** - Protected and admin methods
- **Event Bus** - Subscription notifications

### Strict Message Types:

**OUTGOING:**
```rust
/// Request to qc-06 Mempool for transaction submission
struct SubmitTransactionRequest {
    version: u16,
    correlation_id: [u8; 16],
    reply_to: Topic,
    raw_transaction: Vec<u8>,  // RLP encoded signed transaction
    signature: Signature,
}

/// Request to qc-04 State Management
struct GetBalanceRequest {
    version: u16,
    correlation_id: [u8; 16],
    reply_to: Topic,
    address: [u8; 20],
    block_number: Option<u64>,
    signature: Signature,
}

/// Request to qc-02 Block Storage
struct GetBlockRequest {
    version: u16,
    correlation_id: [u8; 16],
    reply_to: Topic,
    block_id: BlockId,
    include_transactions: bool,
    signature: Signature,
}

/// Request to qc-11 Smart Contracts
struct ExecuteCallRequest {
    version: u16,
    correlation_id: [u8; 16],
    reply_to: Topic,
    from: Option<[u8; 20]>,
    to: [u8; 20],
    gas: Option<u64>,
    value: Option<u256>,
    data: Vec<u8>,
    block_number: Option<u64>,
    signature: Signature,
}
```

**INCOMING:**
```rust
/// Response from internal subsystems
struct SubsystemResponse<T> {
    version: u16,
    correlation_id: [u8; 16],
    result: Option<T>,
    error: Option<String>,
    signature: Signature,
}

/// Event Bus subscription notification
struct SubscriptionNotification {
    version: u16,
    subscription_id: u64,
    event_type: EventType,
    payload: Vec<u8>,
    signature: Signature,
}
```

### Security Boundaries:
- ✅ Accept: HTTP/WS connections from any IP (public methods)
- ✅ Accept: Admin methods from localhost only
- ✅ Accept: Event notifications from Event Bus
- ✅ Send: Authenticated IPC messages to internal subsystems
- ❌ Reject: Requests exceeding rate limits
- ❌ Reject: Requests exceeding size limits (1MB)
- ❌ Reject: Batch requests >100 items
- ❌ Reject: Disabled methods (configurable)
- ❌ Reject: Protected methods without API key (non-localhost)
- ❌ Reject: Admin methods from non-localhost

### Method Tiers:

| Tier | Access Level | Examples |
|------|--------------|----------|
| **Tier 1: Public** | No auth required | eth_getBalance, eth_sendRawTransaction, eth_call |
| **Tier 2: Protected** | API key or localhost | admin_peers, txpool_status |
| **Tier 3: Admin** | Localhost + auth | admin_addPeer, miner_start, debug_* |

### Rate Limiting:
- Public methods: 100 req/s per IP
- Write operations: 10 req/s per IP
- Heavy operations: 20 req/s per IP
- Localhost: Higher limits (1000 req/s)

### Summary of Re-Alignments (System.md Compliance)

| Fix | Change | Rationale |
|-----|--------|-----------|
| **V2.2 Choreography Fix** | Block Storage is Stateful Assembler, not Consensus orchestrator | Decentralization: No single point of failure in block assembly |
| **V2.3 Transaction Lookup** | Subsystem 3 can query Subsystem 2 for tx locations | Merkle proof generation requires knowing transaction position in block |
| **DDoS Defense Fix** | Subsystem 1 can now access Subsystem 10 | Network edge protection: Verify signatures before accepting data into system |
| **Finality Fix** | See Architecture.md DLQ section | Circuit breaker: Don't retry mathematical impossibilities |

---

## SUBSYSTEM 17: BLOCK PRODUCTION ENGINE

### I Am Allowed To Talk To:
- **Subsystem 6 (Mempool)** - Request pending transactions
- **Subsystem 4 (State Management)** - State prefetch for transaction simulation
- **Subsystem 8 (Consensus)** - Submit produced blocks
- **Subsystem 10 (Signature Verification)** - Sign blocks (PoS validator key)
- **Event Bus** - Publish `BlockProduced`, `MiningMetrics` events

### Who Is Allowed To Talk To Me:
- **Subsystem 9 (Finality)** - `BlockFinalized` event (triggers next block)
- **Subsystem 8 (Consensus)** - `SlotAssigned` event (PoS proposer duty)
- **Admin CLI** - Start/stop mining, change settings (localhost only)

### Strict Message Types:

**OUTGOING:**
```rust
/// Request to Mempool for pending transactions
struct GetPendingTransactionsRequest {
    version: u16,
    correlation_id: [u8; 16],
    reply_to: Topic,
    max_count: u32,              // Maximum transactions to return
    min_gas_price: u256,         // Minimum acceptable gas price
    signature: Signature,
}

/// Request to State Management for state prefetch
struct StatePrefetchRequest {
    version: u16,
    correlation_id: [u8; 16],
    reply_to: Topic,
    parent_state_root: [u8; 32],
    transactions: Vec<SignedTransaction>,
    signature: Signature,
}

/// Produced block submitted to Consensus
struct ProduceBlockRequest {
    version: u16,
    sender_id: SubsystemId,      // Must be 17
    correlation_id: [u8; 16],
    reply_to: Topic,
    
    // Block data
    block_template: BlockTemplate,
    consensus_mode: ConsensusMode,  // PoW, PoS, or PBFT
    
    // PoW specific
    nonce: Option<u64>,          // Only for PoW
    
    // PoS specific
    vrf_proof: Option<VRFProof>, // Only for PoS
    validator_signature: Option<Signature>,
    
    signature: Signature,
}

struct BlockTemplate {
    header: BlockHeader,
    transactions: Vec<SignedTransaction>,
    total_gas_used: u64,
    total_fees: u256,
}

enum ConsensusMode {
    ProofOfWork,
    ProofOfStake,
    PBFT,
}

/// Metrics published to Event Bus
struct MiningMetrics {
    version: u16,
    
    // Transaction selection metrics
    transactions_considered: u32,
    transactions_selected: u32,
    total_gas_used: u64,
    total_fees: u256,
    selection_time_ms: u64,
    
    // PoW specific
    hashrate: Option<f64>,       // H/s
    mining_time_ms: Option<u64>,
    
    // PoS specific
    slot_number: Option<u64>,
    
    // Profitability
    expected_reward: u256,
    mev_profit: u256,
    
    timestamp: u64,
}

/// Event published when block is produced
struct BlockProducedEvent {
    version: u16,
    sender_id: SubsystemId,      // Always 17
    
    block_hash: [u8; 32],
    block_number: u64,
    transaction_count: u32,
    total_gas_used: u64,
    total_fees: u256,
    production_time_ms: u64,
    
    consensus_mode: ConsensusMode,
    timestamp: u64,
}
```

**INCOMING:**
```rust
/// Response from Mempool with pending transactions
struct PendingTransactionsResponse {
    version: u16,
    correlation_id: [u8; 16],
    
    transactions: Vec<VerifiedTransaction>,
    total_count: u32,            // Total in mempool
    returned_count: u32,         // Number returned
    signature: Signature,
}

struct VerifiedTransaction {
    transaction: SignedTransaction,
    from: [u8; 20],              // Recovered sender
    nonce: u64,
    gas_price: u256,
    gas_limit: u64,
    signature_valid: bool,       // Pre-verified by Subsystem 10
}

/// Response from State Management with simulation results
struct StatePrefetchResponse {
    version: u16,
    correlation_id: [u8; 16],
    
    simulations: Vec<TransactionSimulation>,
    state_cache: Vec<u8>,        // Serialized cache for reuse
    signature: Signature,
}

struct TransactionSimulation {
    tx_hash: [u8; 32],
    success: bool,
    gas_used: u64,
    state_changes: Vec<StateChange>,
    error: Option<String>,
}

struct StateChange {
    address: [u8; 20],
    storage_key: Option<[u8; 32]>,
    old_value: Vec<u8>,
    new_value: Vec<u8>,
}

/// Event from Finality: Block finalized, produce next
struct BlockFinalizedEvent {
    version: u16,
    sender_id: SubsystemId,      // Must be 9
    
    block_hash: [u8; 32],
    block_number: u64,
    finalized_at: u64,
}

/// Event from Consensus: PoS proposer duty assigned
struct SlotAssignedEvent {
    version: u16,
    sender_id: SubsystemId,      // Must be 8
    
    slot: u64,
    epoch: u64,
    validator_index: u32,
    vrf_proof: VRFProof,
}

struct VRFProof {
    output: [u8; 32],
    proof: [u8; 80],
}

/// Admin command to start/stop mining
struct MiningControlCommand {
    version: u16,
    requester_id: SubsystemId,   // Must be Admin CLI (localhost)
    correlation_id: [u8; 16],
    
    command: MiningCommand,
    signature: Signature,
}

enum MiningCommand {
    Start {
        mode: ConsensusMode,
        threads: u8,             // For PoW
        validator_key: Option<[u8; 32]>,  // For PoS
    },
    Stop,
    UpdateGasLimit(u64),
    UpdateMinGasPrice(u256),
}
```

### Security Boundaries:
- ✅ Accept: `PendingTransactionsResponse` from Subsystem 6 only
- ✅ Accept: `StatePrefetchResponse` from Subsystem 4 only
- ✅ Accept: `BlockFinalizedEvent` from Subsystem 9 only
- ✅ Accept: `SlotAssignedEvent` from Subsystem 8 only
- ✅ Accept: `MiningControlCommand` from Admin CLI (localhost) only
- ✅ Send: `ProduceBlockRequest` to Subsystem 8 only
- ✅ Publish: `BlockProducedEvent` to Event Bus
- ✅ Publish: `MiningMetrics` to Event Bus
- ❌ Reject: Block production requests from external sources
- ❌ Reject: Transactions without valid signatures
- ❌ Reject: Blocks exceeding gas limit
- ❌ Reject: Duplicate transaction hashes
- ❌ Reject: Admin commands from non-localhost

### Rate Limiting:
- Block production: Max 1 block per slot (PoS) or per difficulty target (PoW)
- Transaction queries: Max 100 req/s to Mempool
- State prefetch: Max 50 req/s to State Management
- Admin commands: Max 10 req/s (localhost only)

### Invariants:
```rust
// Gas limit enforcement
invariant!(block.total_gas_used <= BLOCK_GAS_LIMIT);

// Nonce ordering
invariant!(all_transactions_have_sequential_nonces_per_sender());

// State validity
invariant!(all_transactions_simulate_successfully());

// Timestamp monotonicity
invariant!(block.timestamp >= parent_block.timestamp);
invariant!(block.timestamp <= current_time + 15);  // Max 15s into future

// No duplicates
invariant!(no_duplicate_transaction_hashes());

// Fee profitability
invariant!(selected_txs_sorted_by_gas_price_descending());
```

### Attack Scenario: Compromised Block Producer

**Attack:** Attacker gains control of Subsystem 17

**Attempt 1:** Produce blocks with invalid transactions
- ❌ BLOCKED: Consensus (8) re-validates all transactions
- ❌ BLOCKED: Invalid blocks rejected

**Attempt 2:** Censor specific transactions
- ⚠️ PARTIAL SUCCESS: Can exclude transactions
- ✅ MITIGATED: Cryptographic inclusion proofs reveal censorship
- ✅ MITIGATED: Community can detect and penalize

**Attempt 3:** Front-run transactions (MEV exploitation)
- ⚠️ PARTIAL SUCCESS: Can reorder within gas price tier
- ✅ MITIGATED: Fair ordering enforcement (FIFO within tier)
- ✅ MITIGATED: MEV metrics publicly visible

**Result:** Attack limited; cannot produce invalid blocks, censorship detectable

---

## IMPLEMENTATION CHECKLIST

For each subsystem, implement:

### 1. Message Type Validation
```rust
fn validate_message_type<T: MessageType>(msg: &T) -> Result<(), ValidationError> {
    // Check message type matches expected struct
    // Verify all required fields present
    // Validate field constraints (ranges, lengths)
}
```

### 2. Subsystem ID Verification
```rust
fn verify_sender(sender_id: SubsystemId, allowed: &[SubsystemId]) -> Result<(), AuthError> {
    if !allowed.contains(&sender_id) {
        return Err(AuthError::UnauthorizedSender);
    }
    Ok(())
}
```

### 3. Signature Verification
```rust
fn verify_signature(msg: &SignedMessage) -> Result<(), CryptoError> {
    // Verify message signature
    // Check signature timestamp (reject if >60s old)
    // Verify signer authority
}
```

### 4. Rate Limiting
```rust
struct RateLimiter {
    limits: HashMap<SubsystemId, (u32, Duration)>,
    counters: HashMap<SubsystemId, (u32, Instant)>,
}

impl RateLimiter {
    fn check_limit(&mut self, sender: SubsystemId) -> Result<(), RateLimitError> {
        // Implement token bucket or sliding window
    }
}
```

### 5. Input Sanitization
```rust
fn sanitize_input<T>(input: &T) -> Result<T, SanitizationError> {
    // Check bounds (array lengths, numeric ranges)
    // Validate addresses (20 bytes)
    // Validate hashes (32 bytes)
    // Check for null bytes, invalid UTF-8, etc.
}
```

---

## ATTACK SCENARIO EXAMPLES

### Scenario 1: Compromised Mempool
**Attack:** Attacker gains control of Subsystem 6 (Mempool)
**Attempt:** Try to add malicious transactions directly to Consensus

**Defense:**
- ❌ Subsystem 8 REJECTS: `AddTransactionRequest` from Subsystem 6
- ✅ Only accepts `TransactionBatch` when requested via `GetTransactionsRequest`
- ✅ All transactions must come through Subsystem 10 (Signature Verification) first

**Result:** Attack contained to Mempool, cannot affect consensus

---

### Scenario 2: Compromised Smart Contract Executor
**Attack:** Attacker gains control of Subsystem 11
**Attempt:** Directly modify state without transaction validation

**Defense:**
- ❌ Subsystem 4 REJECTS: `StateWriteRequest` without valid `ExecuteTransactionRequest` from Subsystem 8
- ✅ Only Subsystem 11 can write, but only after receiving execution request from Consensus
- ✅ All state changes signed and auditable

**Result:** Cannot modify state without going through Consensus

---

### Scenario 3: Compromised Peer Discovery
**Attack:** Attacker controls Subsystem 1
**Attempt:** Provide malicious peers to isolate node (Eclipse attack)

**Defense:**
- ✅ Subsystem 5 validates peer reputation before connecting
- ✅ Maintains connections to known good peers (checkpoint peers)
- ✅ Random peer selection prevents full eclipse
- ✅ Peer diversity requirements (IP subnet limits)

**Result:** Partial mitigation, cannot fully eclipse node

---

## CROSS-SUBSYSTEM AUTHENTICATION

Every message between subsystems must include:

```rust
struct AuthenticatedMessage<T> {
    // === VERSION (MANDATORY - MUST BE FIRST) ===
    version: u16,                    // Protocol version - deserialize and validate FIRST
    
    // === ROUTING ===
    sender_id: SubsystemId,
    recipient_id: SubsystemId,
    
    // === CORRELATION (FOR REQUEST/RESPONSE) ===
    correlation_id: [u8; 16],        // UUID v4 for request/response mapping
    reply_to: Option<Topic>,         // Topic for async response delivery
    
    // === PAYLOAD ===
    payload: T,
    
    // === SECURITY ===
    timestamp: u64,
    nonce: u64,
    signature: [u8; 32],             // HMAC-SHA256 with shared secret
}

struct Topic {
    subsystem_id: SubsystemId,
    channel: String,                 // e.g., "responses", "dlq.errors"
}

impl<T> AuthenticatedMessage<T> {
    /// Verify message authenticity with time-bounded replay prevention
    /// 
    /// CRITICAL: See Architecture.md Section 3.5 for full implementation.
    /// The order of checks is security-critical to prevent DoS attacks.
    fn verify(
        &self, 
        expected_sender: SubsystemId, 
        shared_secret: &[u8],
        nonce_cache: &mut TimeBoundedNonceCache,  // v2.1: Time-bounded cache
    ) -> Result<(), AuthError> {
        let now = current_timestamp();
        
        // 0. TIMESTAMP CHECK FIRST (bounds all subsequent operations)
        // This MUST come before nonce check to prevent cache exhaustion attacks
        let min_valid = now.saturating_sub(60);
        let max_valid = now.saturating_add(10);
        if self.timestamp < min_valid || self.timestamp > max_valid {
            return Err(AuthError::TimestampOutOfRange);
        }
        
        // 1. Check version (before any payload deserialization)
        if self.version < MIN_SUPPORTED_VERSION || self.version > MAX_SUPPORTED_VERSION {
            return Err(AuthError::UnsupportedVersion { 
                received: self.version,
                supported_range: (MIN_SUPPORTED_VERSION, MAX_SUPPORTED_VERSION),
            });
        }
        
        // 2. Check sender_id matches expected
        if self.sender_id != expected_sender {
            return Err(AuthError::InvalidSender);
        }
        
        // 3. Verify HMAC
        let computed_hmac = compute_hmac(shared_secret, &self.serialize_without_sig());
        if !constant_time_eq(&computed_hmac, &self.signature) {
            return Err(AuthError::InvalidSignature);
        }
        
        // 4. Check nonce AFTER timestamp (v2.1: time-bounded cache)
        // Cache auto-expires entries after 120s, preventing memory exhaustion
        nonce_cache.check_and_add(self.nonce, self.timestamp)?;
        
        // 5. Reply-to validation (for requests with reply_to)
        if let Some(ref reply_to) = self.reply_to {
            if reply_to.subsystem_id != self.sender_id {
                return Err(AuthError::ReplyToMismatch);
            }
        }
        
        Ok(())
    }
}
```

---

## DEFENSE IN DEPTH SUMMARY

1. **Type Safety** - Strict message types prevent wrong data structures
2. **Sender Verification** - Only authorized subsystems can communicate
3. **Signature Validation** - Cryptographic proof of authenticity
4. **Rate Limiting** - Prevents flooding attacks
5. **Input Sanitization** - Bounds checking on all inputs
6. **Time-Bounded Nonce Tracking** - Prevents replay attacks with bounded memory (v2.1)
7. **Timestamp-First Validation** - Bounds all operations, prevents cache exhaustion (v2.1)
8. **Compartmentalization** - Breaching one subsystem doesn't compromise others
9. **Version Validation** - Protocol version checked BEFORE payload deserialization
10. **Correlation IDs** - Enables non-blocking request/response without deadlocks
11. **Dead Letter Queues** - Critical events never lost, always recoverable
12. **Atomicity Enforcement** - Block writes assembled by Stateful Assembler (v2.2 choreography)
13. **Edge Defense** - Peer Discovery can verify signatures to block DDoS at network edge
14. **Finality Circuit Breaker** - Consensus failures trigger sync mode with deterministic triggers
15. **Two-Phase Transaction Removal** - Mempool never deletes until storage confirmed (Transaction Loss Fix)
16. **Zero-Trust Signature Verification** - Consensus/Finality re-verify all signatures independently
17. **Reply-To Validation** - Prevents forwarding attacks by validating reply_to matches sender_id
18. **Livelock Prevention** - Circuit breaker halts node after 3 failed sync attempts (deterministic)
19. **Time-Bounded Nonce Cache** - Nonces expire after 120s, preventing memory exhaustion attacks (v2.1)
20. **Envelope-Only Identity** - Payloads have NO identity fields; envelope.sender_id is sole truth (v2.2)
21. **Choreography Pattern** - Decentralized event flow, no centralized orchestrator (v2.2)

**System.md Compliance Achieved (V2.2):**

| System.md Requirement | IPC-MATRIX Implementation |
|-----------------------|---------------------------|
| Atomic Writes (Subsystem 2) | Stateful Assembler buffers components, writes atomically |
| DDoS Defense (Subsystem 6) | Peer Discovery can verify signatures before accepting data |
| Safety over Liveness (Subsystem 9) | Finality failures trigger circuit breaker with deterministic triggers |
| Transaction Integrity (Subsystem 6) | Two-Phase Commit prevents transaction loss on storage failure |
| Zero-Trust Security (Subsystems 8, 9) | Critical signatures re-verified independently |
| Livelock Prevention (Subsystem 9) | HALTED state after 3 failed sync attempts (testable conditions) |
| Memory Safety (All subsystems) | Time-bounded nonce cache prevents OOM attacks |
| Decentralization (All subsystems) | Choreography pattern, no orchestrator bottleneck |

**Architecture v2.2 Amendments Applied:**
- Amendment 1.1: Two-Phase Mempool Protocol (Transaction Loss Fix)
- Amendment 1.2: Enhanced Circuit Breaker with Livelock Prevention
- Amendment 2.1: Zero-Trust Signature Re-Verification
- Amendment 2.2: Reply-To Forwarding Attack Prevention
- Amendment 2.3: Time-Bounded Nonce Cache (Memory Exhaustion Fix) (v2.1)
- Amendment 3: Future Scalability Considerations (documented in System.md)
- **Amendment 4.1: Choreography Pattern (Orchestrator Decentralization)** (v2.2 NEW)
- **Amendment 4.2: Envelope-Only Identity (Payload Impersonation Fix)** (v2.2 NEW)
- **Amendment 4.3: Deterministic Circuit Breaker Triggers (Testability Fix)** (v2.2 NEW)

**IMPORTANT (v2.2): Payload Identity Fields Deprecated**
Some legacy message definitions contain `requester_id` fields. These are DEPRECATED.
The `sender_id` in the `AuthenticatedMessage` envelope is the ONLY source of truth.
All new implementations MUST use envelope identity, not payload identity.
See Architecture.md Section 3.2.1 for details.

**Result:** Even if an attacker fully compromises one subsystem, they are trapped in that compartment and cannot spread to others without breaking multiple layers of authentication. The system now correctly prioritizes data integrity (atomicity + two-phase commit), network defense (edge verification), consensus safety (circuit breaker + livelock prevention + deterministic triggers), zero-trust security (independent signature verification), resource safety (bounded memory via time-limited nonce caching), decentralization (choreography, not orchestration), and audit integrity (envelope-only identity).