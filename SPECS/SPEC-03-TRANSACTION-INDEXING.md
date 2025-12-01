# SPECIFICATION: TRANSACTION INDEXING

**Version:** 2.3  
**Subsystem ID:** 3  
**Bounded Context:** Data Retrieval & Cryptographic Proofs  
**Crate Name:** `crates/transaction-indexing`  
**Author:** Systems Architecture Team  
**Date:** 2024-12-01  
**Architecture Compliance:** Architecture.md v2.3 (Choreography Pattern, Envelope-Only Identity, Deterministic Failure Awareness, Transaction Location Lookup)

---

## 1. ABSTRACT

### 1.1 Purpose

The **Transaction Indexing** subsystem is the system's authority for proving transaction inclusion within a block. It computes Merkle roots for validated blocks and generates cryptographic Merkle proofs for any indexed transaction. This enables lightweight verification of transaction inclusion without downloading entire blocks.

### 1.2 Responsibility Boundaries

**In Scope:**
- Compute Merkle root for transactions in a validated block
- Generate Merkle proofs for transaction inclusion verification
- Verify Merkle proofs against known roots
- Index transactions by hash for efficient proof generation
- Maintain transaction location mappings (tx_hash → block_height, tx_index)

**Out of Scope:**
- Transaction validation or signature verification (handled by Consensus)
- Block storage or retrieval (handled by Subsystem 2)
- State trie computation (handled by Subsystem 4)
- Transaction execution (handled by Subsystem 11)
- Network I/O or peer communication

**CRITICAL DESIGN CONSTRAINT (V2.2 Choreography Pattern):**

This subsystem is a **participant** in the block processing choreography, NOT an orchestrator.

**Architecture Mandate (Architecture.md v2.2, Section 5.1):**
- This subsystem SUBSCRIBES to `BlockValidated` events from the event bus
- It performs its core computation (Merkle tree construction)
- It PUBLISHES `MerkleRootComputed` events back to the event bus
- Block Storage (Subsystem 2) consumes this event as part of its Stateful Assembler

**Choreography Flow:**
```
Consensus (8) ──BlockValidated──→ [Event Bus] ──→ Transaction Indexing (3)
                                                         │
                                                         ↓
                                                  [Compute Merkle Tree]
                                                         │
                                                         ↓
                                     ←──MerkleRootComputed──→ [Event Bus] ──→ Block Storage (2)
```

### 1.3 Key Design Principles

1. **Pure Domain Logic:** Merkle tree computation is pure Rust with no I/O dependencies
2. **Deterministic Computation:** Same transactions always produce same Merkle root
3. **Trust Boundary:** This is a "dumb calculator" - it trusts transaction validity from Consensus
4. **Proof Integrity:** All generated proofs are verifiable against their claimed root
5. **Canonical Serialization:** Transactions are hashed from canonical byte representation

### 1.4 Trust Model

This subsystem operates within a strict trust boundary under the V2.2 Choreography Pattern:

```
┌─────────────────────────────────────────────────────────────┐
│                    TRUST BOUNDARY (V2.2)                    │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  TRUSTED EVENT INPUTS (via AuthenticatedMessage envelope):  │
│  └─ Subsystem 8 (Consensus) → BlockValidated event         │
│                                                             │
│  CHOREOGRAPHY ROLE:                                         │
│  ├─ Compute Merkle root for block transactions             │
│  ├─ Publish MerkleRootComputed to event bus                │
│  └─ Respond to MerkleProofRequest from any subsystem       │
│                                                             │
│  TRUST ASSUMPTIONS:                                         │
│  ├─ Transactions in BlockValidated are already validated   │
│  ├─ This subsystem does NOT verify transaction signatures  │
│  └─ This subsystem does NOT validate transaction contents  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. DOMAIN MODEL (THE "INNER LAYER")

### 2.1 Shared Types (from `crates/shared-types`)

The following types are **NOT defined in this crate**. They are referenced from the `shared-types` crate:

```rust
// ============================================================
// FROM: crates/shared-types/src/lib.rs
// DO NOT REDEFINE - IMPORT ONLY
// ============================================================

/// 32-byte hash value
pub use shared_types::Hash;

/// Transaction structure
pub use shared_types::Transaction;

/// Validated block from Consensus
pub use shared_types::ValidatedBlock;

/// Subsystem identifier
pub use shared_types::SubsystemId;

/// Authenticated message envelope
pub use shared_types::AuthenticatedMessage;

/// Unix timestamp in seconds
pub use shared_types::Timestamp;
```

### 2.2 Core Domain Entities

```rust
/// A binary Merkle tree built from transaction hashes.
/// 
/// ALGORITHM: Binary hash tree where each non-leaf node is the hash
/// of its two children concatenated: H(left || right).
/// 
/// INVARIANT-1 (Power of Two): Leaves are padded to nearest power of two.
/// Empty slots are filled with a sentinel hash (all zeros).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerkleTree {
    /// All nodes in the tree, stored level by level (leaves at end)
    /// Tree is stored in array form: [root, level1..., level2..., leaves...]
    nodes: Vec<Hash>,
    /// Number of actual transactions (before padding)
    transaction_count: usize,
    /// Number of leaves after padding to power of two
    padded_leaf_count: usize,
    /// The computed root hash
    root: Hash,
}

impl MerkleTree {
    /// Build a Merkle tree from transaction hashes.
    /// 
    /// # Invariants Enforced
    /// - INVARIANT-1: Pads to nearest power of two
    /// - INVARIANT-3: Assumes input hashes are from canonical serialization
    pub fn build(transaction_hashes: Vec<Hash>) -> Self {
        // Implementation builds tree bottom-up
        unimplemented!()
    }
    
    /// Get the root hash of this tree
    pub fn root(&self) -> Hash {
        self.root
    }
    
    /// Generate a proof for the transaction at the given index
    pub fn generate_proof(&self, tx_index: usize) -> Result<MerkleProof, MerkleError> {
        unimplemented!()
    }
    
    /// Verify a proof against this tree's root
    pub fn verify_proof(&self, proof: &MerkleProof) -> bool {
        Self::verify_proof_static(&proof.leaf_hash, &proof.path, &self.root)
    }
    
    /// Static verification without tree instance
    /// 
    /// INVARIANT-2: If proof is valid, this returns true
    pub fn verify_proof_static(
        leaf_hash: &Hash,
        path: &[ProofNode],
        root: &Hash,
    ) -> bool {
        unimplemented!()
    }
}

/// A cryptographic proof of transaction inclusion in a Merkle tree.
/// 
/// This proof allows verification that a specific transaction is included
/// in a block without having access to all other transactions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerkleProof {
    /// Hash of the transaction being proven
    pub leaf_hash: Hash,
    /// Index of the transaction in the original list
    pub tx_index: usize,
    /// Block height where this transaction exists
    pub block_height: u64,
    /// Block hash for additional verification
    pub block_hash: Hash,
    /// The Merkle root this proof verifies against
    pub root: Hash,
    /// Path of sibling hashes from leaf to root
    pub path: Vec<ProofNode>,
}

/// A single node in the Merkle proof path
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofNode {
    /// The sibling hash at this level
    pub hash: Hash,
    /// Position of sibling (left or right)
    pub position: SiblingPosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiblingPosition {
    Left,
    Right,
}

/// Location of a transaction in the blockchain
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionLocation {
    /// Block height containing this transaction
    pub block_height: u64,
    /// Block hash containing this transaction
    pub block_hash: Hash,
    /// Index of transaction within the block
    pub tx_index: usize,
    /// Merkle root of the block (cached for proof generation)
    pub merkle_root: Hash,
}
```

### 2.3 Index Structures

```rust
use std::collections::HashMap;

/// Index for efficient transaction lookups and proof generation.
/// 
/// This structure maintains mappings from transaction hashes to their
/// locations in the blockchain, enabling O(1) proof generation.
pub struct TransactionIndex {
    /// Transaction hash → location mapping
    locations: HashMap<Hash, TransactionLocation>,
    /// Block hash → Merkle tree mapping (for proof generation)
    trees: HashMap<Hash, MerkleTree>,
    /// Configuration
    config: IndexConfig,
}

/// Configuration for the transaction index
#[derive(Debug, Clone)]
pub struct IndexConfig {
    /// Maximum number of Merkle trees to cache (default: 1000)
    /// 
    /// SECURITY: Bounds memory usage. Old trees are evicted LRU.
    pub max_cached_trees: usize,
    /// Whether to persist index to storage (default: true)
    pub persist_index: bool,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            max_cached_trees: 1000,
            persist_index: true,
        }
    }
}
```

### 2.4 Value Objects

```rust
/// Sentinel hash used for padding (all zeros)
pub const SENTINEL_HASH: Hash = Hash([0u8; 32]);

/// Configuration for Merkle tree computation
#[derive(Debug, Clone)]
pub struct MerkleConfig {
    /// Hash algorithm identifier (default: SHA3-256)
    pub hash_algorithm: HashAlgorithm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Sha3_256,
    Blake3,
}

impl Default for MerkleConfig {
    fn default() -> Self {
        Self {
            hash_algorithm: HashAlgorithm::Sha3_256,
        }
    }
}
```

### 2.5 Domain Invariants

**INVARIANT-1: Power of Two Padding**
```
∀ MerkleTree T:
    T.padded_leaf_count == 2^ceil(log2(T.transaction_count))
    
EXCEPTION: If transaction_count == 0, padded_leaf_count == 0 and root == SENTINEL_HASH

ENFORCEMENT:
    Before tree construction, pad leaf array with SENTINEL_HASH
    until length is a power of two.
```

**INVARIANT-2: Proof Validity**
```
∀ MerkleProof P generated by MerkleTree T:
    verify_proof_static(P.leaf_hash, P.path, T.root) == true
    
GUARANTEE: Any proof generated by this subsystem MUST be verifiable.
```

**INVARIANT-3: Deterministic Hashing (Canonical Serialization)**
```
∀ Transaction tx:
    hash(tx) == H(canonical_serialize(tx))
    
WHERE:
    canonical_serialize produces identical bytes regardless of
    in-memory representation, field ordering, or platform.
    
ENFORCEMENT:
    Transactions MUST be serialized using the canonical format
    defined in shared-types before hashing.
```

**INVARIANT-4: Index Consistency**
```
∀ TransactionLocation L in TransactionIndex:
    L.merkle_root == trees[L.block_hash].root
    
The cached Merkle root in location MUST match the actual tree.
```

**INVARIANT-5: Bounded Tree Cache (Memory Safety)**
```
ALWAYS: trees.len() ≤ max_cached_trees

ENFORCEMENT:
    IF trees.len() >= max_cached_trees THEN
        Evict least-recently-used tree before inserting new one
```

---

## 3. PORTS & INTERFACES (THE "HEXAGON")

### 3.1 Driving Ports (Inbound API)

These are the public APIs this library exposes to the application.

```rust
/// Primary API for the Transaction Indexing subsystem
pub trait TransactionIndexingApi {
    /// Generate a Merkle proof for a transaction by its hash.
    /// 
    /// # Parameters
    /// - `transaction_hash`: Hash of the transaction to prove
    /// 
    /// # Returns
    /// - `Ok(MerkleProof)`: Proof that can verify transaction inclusion
    /// - `Err(TransactionNotFound)`: Transaction not indexed
    /// - `Err(TreeNotCached)`: Merkle tree evicted, must rebuild
    fn generate_proof(
        &self,
        transaction_hash: Hash,
    ) -> Result<MerkleProof, IndexingError>;
    
    /// Verify a Merkle proof against a known root.
    /// 
    /// # INVARIANT-2 Guarantee
    /// If this returns true, the proof is cryptographically valid.
    fn verify_proof(&self, proof: &MerkleProof) -> bool;
    
    /// Get the location of a transaction by hash.
    fn get_transaction_location(
        &self,
        transaction_hash: Hash,
    ) -> Result<TransactionLocation, IndexingError>;
    
    /// Check if a transaction is indexed.
    fn is_indexed(&self, transaction_hash: Hash) -> bool;
    
    /// Get indexing statistics
    fn get_stats(&self) -> IndexingStats;
}

/// Statistics about the indexing subsystem
#[derive(Debug, Clone)]
pub struct IndexingStats {
    /// Total transactions indexed
    pub total_indexed: u64,
    /// Number of Merkle trees cached
    pub cached_trees: usize,
    /// Maximum cached trees allowed
    pub max_cached_trees: usize,
    /// Number of proofs generated
    pub proofs_generated: u64,
    /// Number of proofs verified
    pub proofs_verified: u64,
}

/// Errors that can occur during indexing operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexingError {
    /// Transaction hash not found in index
    TransactionNotFound { tx_hash: Hash },
    /// Merkle tree was evicted from cache, must rebuild
    TreeNotCached { block_hash: Hash },
    /// Transaction index out of bounds
    InvalidIndex { index: usize, max: usize },
    /// Empty block (no transactions to index)
    EmptyBlock { block_hash: Hash },
    /// Serialization error
    SerializationError { message: String },
    /// Storage error
    StorageError { message: String },
}
```

### 3.2 Driven Ports (Outbound SPI)

These are the interfaces this library **requires** the host application to implement.

```rust
/// Abstract interface for transaction storage.
/// 
/// This port allows the indexing subsystem to persist transaction
/// locations for later proof generation.
pub trait TransactionStore: Send + Sync {
    /// Store a transaction location
    fn put_location(
        &mut self,
        tx_hash: Hash,
        location: TransactionLocation,
    ) -> Result<(), StoreError>;
    
    /// Get a transaction location by hash
    fn get_location(
        &self,
        tx_hash: Hash,
    ) -> Result<Option<TransactionLocation>, StoreError>;
    
    /// Check if a transaction exists
    fn exists(&self, tx_hash: Hash) -> Result<bool, StoreError>;
    
    /// Store a Merkle tree for a block (optional caching)
    fn put_tree(
        &mut self,
        block_hash: Hash,
        tree: MerkleTree,
    ) -> Result<(), StoreError>;
    
    /// Get a cached Merkle tree
    fn get_tree(
        &self,
        block_hash: Hash,
    ) -> Result<Option<MerkleTree>, StoreError>;
}

/// V2.3: Interface for querying Block Storage for transaction locations
/// 
/// This port allows Transaction Indexing to query stored transaction positions
/// for Merkle proof generation. In V2.3, Transaction Indexing can query Block
/// Storage instead of maintaining a separate transaction index.
pub trait BlockStorageClient: Send + Sync {
    /// Query Block Storage for a transaction's location
    /// 
    /// This is an async IPC call to Subsystem 2 (Block Storage).
    /// Uses the correlation_id pattern for request/response matching.
    async fn get_transaction_location(
        &self,
        transaction_hash: Hash,
    ) -> Result<TransactionLocation, BlockStorageError>;
}

#[derive(Debug)]
pub enum BlockStorageError {
    /// Transaction not found in any stored block
    TransactionNotFound { tx_hash: Hash },
    /// Block Storage communication error
    CommunicationError { message: String },
    /// Request timed out
    Timeout,
}

/// V2.3: Interface for querying Block Storage for transaction data
/// 
/// This port allows Transaction Indexing to query Block Storage for
/// transaction hashes needed to reconstruct Merkle trees for proof generation.
/// This enables bounded memory usage by fetching data on cache miss.
pub trait BlockDataProvider: Send + Sync {
    /// Get transaction hashes for a block (for Merkle tree reconstruction)
    /// 
    /// This is an async IPC call to Subsystem 2 (Block Storage).
    /// Uses the correlation_id pattern for request/response matching.
    /// 
    /// # Parameters
    /// - `block_hash`: Hash of the block to get transaction hashes for
    /// 
    /// # Returns
    /// - `Ok(TransactionHashesData)`: Transaction hashes and cached Merkle root
    /// - `Err(BlockStorageError)`: Block not found or communication error
    async fn get_transaction_hashes_for_block(
        &self,
        block_hash: Hash,
    ) -> Result<TransactionHashesData, BlockStorageError>;
    
    /// Get location of a specific transaction
    /// 
    /// # Parameters
    /// - `transaction_hash`: Hash of the transaction to locate
    /// 
    /// # Returns
    /// - `Ok(TransactionLocation)`: Location data for the transaction
    /// - `Err(BlockStorageError)`: Transaction not found or communication error
    async fn get_transaction_location(
        &self,
        transaction_hash: Hash,
    ) -> Result<TransactionLocation, BlockStorageError>;
}

/// V2.3: Transaction hashes data from Block Storage
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionHashesData {
    /// Block hash these hashes belong to
    pub block_hash: Hash,
    /// All transaction hashes in canonical order
    pub transaction_hashes: Vec<Hash>,
    /// Cached Merkle root for verification
    pub merkle_root: Hash,
}

#[derive(Debug)]
pub enum StoreError {
    IOError { message: String },
    SerializationError { message: String },
    NotFound,
}

/// Abstract interface for cryptographic hashing
pub trait HashProvider: Send + Sync {
    /// Hash arbitrary bytes
    fn hash(&self, data: &[u8]) -> Hash;
    
    /// Hash two concatenated hashes (for Merkle tree nodes)
    fn hash_pair(&self, left: &Hash, right: &Hash) -> Hash;
}

/// Abstract interface for transaction serialization
pub trait TransactionSerializer: Send + Sync {
    /// Serialize a transaction to canonical bytes.
    /// 
    /// INVARIANT-3: This MUST produce identical bytes for semantically
    /// identical transactions, regardless of in-memory representation.
    fn serialize(&self, tx: &Transaction) -> Result<Vec<u8>, SerializationError>;
    
    /// Compute hash of a transaction using canonical serialization
    fn hash_transaction(&self, tx: &Transaction) -> Result<Hash, SerializationError>;
}

#[derive(Debug)]
pub struct SerializationError {
    pub message: String,
}

/// Abstract interface for time operations (for testability)
pub trait TimeSource: Send + Sync {
    /// Get current timestamp
    fn now(&self) -> Timestamp;
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

**ARCHITECTURAL MANDATE:** This subsystem subscribes to events as part of the block processing choreography. It does NOT orchestrate.

```rust
/// Events this subsystem SUBSCRIBES TO
/// 
/// CHOREOGRAPHY PATTERN (V2.2 - Architecture.md Section 5.1):
/// Transaction Indexing reacts to BlockValidated events by computing
/// the Merkle root and publishing MerkleRootComputed.
/// 
/// SECURITY (Envelope-Only Identity):
/// Sender identity is derived from envelope.sender_id only.
/// These payloads contain NO identity fields.

// ============================================================
// EVENT: BlockValidated (from Consensus, Subsystem 8)
// ============================================================

/// Published by Consensus when a block passes validation.
/// This is the TRIGGER for Merkle tree computation.
/// 
/// SECURITY: Only accept from sender_id == SubsystemId::Consensus
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockValidatedPayload {
    /// The validated block containing transactions to index
    pub block: ValidatedBlock,
    /// Block hash for correlation
    pub block_hash: Hash,
    /// Block height for ordering
    pub block_height: u64,
}
```

### 4.2 Incoming Request Payloads

These are request payloads (requiring a response) that Transaction Indexing handles:

```rust
/// Request payloads this subsystem handles
/// 
/// SECURITY (Envelope-Only Identity - V2.2):
/// These payloads contain NO identity fields. Sender identity
/// is derived from the AuthenticatedMessage envelope only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionIndexingRequestPayload {
    /// Request a Merkle proof for a transaction
    /// Allowed sender: Any authorized subsystem (Light Clients, etc.)
    MerkleProofRequest(MerkleProofRequestPayload),
    
    /// Request transaction location
    /// Allowed sender: Any authorized subsystem
    TransactionLocationRequest(TransactionLocationRequestPayload),
}

/// Request for a Merkle proof
/// 
/// SECURITY (Envelope-Only Identity): No requester_id field.
/// Sender verified via envelope.sender_id.
/// 
/// The response will be sent to the topic specified in envelope.reply_to
/// with the same envelope.correlation_id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerkleProofRequestPayload {
    /// Hash of the transaction to generate proof for
    pub transaction_hash: Hash,
}

/// Request for transaction location
/// 
/// SECURITY (Envelope-Only Identity): No requester_id field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionLocationRequestPayload {
    /// Hash of the transaction to locate
    pub transaction_hash: Hash,
}
```

### 4.3 Outgoing Event Publications

These are the payload types that Transaction Indexing publishes:

```rust
/// Events emitted by the Transaction Indexing subsystem
/// 
/// USAGE: These are payloads wrapped in AuthenticatedMessage<T>.
/// Example: AuthenticatedMessage<MerkleRootComputedPayload>
/// 
/// SECURITY (Envelope-Only Identity): No identity fields in payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionIndexingEventPayload {
    /// Merkle root computed for a block (CHOREOGRAPHY EVENT)
    /// 
    /// CRITICAL: This event is consumed by Block Storage's Stateful Assembler.
    /// It must be published for every BlockValidated event received.
    MerkleRootComputed(MerkleRootComputedPayload),
    
    /// Response to a Merkle proof request
    MerkleProofResponse(MerkleProofResponsePayload),
    
    /// Response to a transaction location request
    TransactionLocationResponse(TransactionLocationResponsePayload),
    
    /// Indexing error occurred
    IndexingError(IndexingErrorPayload),
}

/// Published after computing Merkle root for a validated block.
/// 
/// V2.2 CHOREOGRAPHY: This is a critical event in the block processing flow.
/// Block Storage (Subsystem 2) buffers this event by block_hash and waits
/// for BlockValidated and StateRootComputed to complete the assembly.
/// 
/// ARCHITECTURAL CONTEXT (Stateful Assembler):
/// If this event is not emitted within 30 seconds of BlockValidated,
/// Block Storage will time out the assembly for this block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerkleRootComputedPayload {
    /// Block hash this Merkle root corresponds to
    pub block_hash: Hash,
    /// Block height for ordering
    pub block_height: u64,
    /// The computed Merkle root
    pub merkle_root: Hash,
    /// Number of transactions in the block
    pub transaction_count: usize,
}

/// Response to a Merkle proof request
/// 
/// The correlation_id in the envelope links this to the original request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerkleProofResponsePayload {
    /// The result of proof generation
    pub result: Result<MerkleProof, IndexingErrorPayload>,
}

/// Response to a transaction location request
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionLocationResponsePayload {
    /// The transaction hash that was queried
    pub transaction_hash: Hash,
    /// The result of location lookup
    pub result: Result<TransactionLocation, IndexingErrorPayload>,
}

/// Serializable indexing error for IPC
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexingErrorPayload {
    pub error_type: IndexingErrorType,
    pub message: String,
    pub transaction_hash: Option<Hash>,
    pub block_hash: Option<Hash>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexingErrorType {
    TransactionNotFound,
    TreeNotCached,
    InvalidIndex,
    EmptyBlock,
    SerializationError,
    StorageError,
}
```

### 4.4 Choreography Event Handling (V2.2)

This section describes how Transaction Indexing handles the BlockValidated event and participates in the choreography:

```rust
impl TransactionIndexing {
    /// Handle incoming BlockValidated event from Consensus (Subsystem 8)
    /// 
    /// CHOREOGRAPHY: This is the main trigger for this subsystem's work.
    /// After processing, we MUST publish MerkleRootComputed.
    async fn handle_block_validated(
        &mut self,
        msg: AuthenticatedMessage<BlockValidatedPayload>
    ) -> Result<(), Error> {
        // Step 1: Validate envelope (version, signature, timestamp)
        self.verify_envelope(&msg)?;
        
        // Step 2: Verify sender is Consensus (Subsystem 8)
        if msg.sender_id != SubsystemId::Consensus {
            log::warn!(
                "BlockValidated from unauthorized sender {:?} - REJECTED",
                msg.sender_id
            );
            return Err(Error::UnauthorizedSender {
                sender: msg.sender_id,
                expected: SubsystemId::Consensus,
            });
        }
        
        // Step 3: Extract transaction hashes with canonical serialization
        let tx_hashes: Vec<Hash> = msg.payload.block.transactions
            .iter()
            .map(|tx| self.serializer.hash_transaction(tx))
            .collect::<Result<Vec<_>, _>>()?;
        
        // Step 4: Build Merkle tree (enforces INVARIANT-1: power of two)
        let tree = MerkleTree::build(tx_hashes.clone());
        
        // Step 5: Index all transactions
        for (index, tx) in msg.payload.block.transactions.iter().enumerate() {
            let tx_hash = tx_hashes[index];
            let location = TransactionLocation {
                block_height: msg.payload.block_height,
                block_hash: msg.payload.block_hash,
                tx_index: index,
                merkle_root: tree.root(),
            };
            self.store.put_location(tx_hash, location)?;
        }
        
        // Step 6: Cache the Merkle tree (for proof generation)
        self.cache_tree(msg.payload.block_hash, tree.clone())?;
        
        // Step 7: Publish MerkleRootComputed event (CHOREOGRAPHY OUTPUT)
        let result_payload = MerkleRootComputedPayload {
            block_hash: msg.payload.block_hash,
            block_height: msg.payload.block_height,
            merkle_root: tree.root(),
            transaction_count: msg.payload.block.transactions.len(),
        };
        
        self.publish_event(
            TransactionIndexingEventPayload::MerkleRootComputed(result_payload)
        ).await?;
        
        log::info!(
            "Computed Merkle root for block {} (height {}, {} txs): {:?}",
            hex::encode(&msg.payload.block_hash.0[..8]),
            msg.payload.block_height,
            msg.payload.block.transactions.len(),
            hex::encode(&tree.root().0[..8])
        );
        
        Ok(())
    }
    
    /// Cache a Merkle tree with bounded memory (INVARIANT-5)
    fn cache_tree(&mut self, block_hash: Hash, tree: MerkleTree) -> Result<(), Error> {
        // Enforce max cache size
        while self.trees.len() >= self.config.max_cached_trees {
            // Evict LRU tree
            if let Some(oldest_key) = self.lru_order.pop_front() {
                self.trees.remove(&oldest_key);
            }
        }
        
        self.trees.insert(block_hash, tree);
        self.lru_order.push_back(block_hash);
        
        Ok(())
    }
}
```

### 4.5 Request/Response Flow Example (MerkleProofRequest)

Per Architecture.md Section 3.3, all request/response flows MUST use the correlation ID pattern:

```rust
// ============================================================
// REQUESTER SIDE (e.g., Light Client - Subsystem 13)
// ============================================================

impl LightClient {
    /// Request a Merkle proof from Transaction Indexing (NON-BLOCKING)
    async fn request_merkle_proof(&self, tx_hash: Hash) -> Result<(), Error> {
        // Step 1: Generate unique correlation ID
        let correlation_id = Uuid::new_v4();
        
        // Step 2: Store pending request for later matching
        self.pending_requests.insert(correlation_id, PendingRequest {
            created_at: Instant::now(),
            timeout: Duration::from_secs(30),
            request_type: RequestType::MerkleProof,
        });
        
        // Step 3: Construct the full authenticated message
        let message = AuthenticatedMessage {
            version: PROTOCOL_VERSION,
            sender_id: SubsystemId::LightClients,
            recipient_id: SubsystemId::TransactionIndexing,
            correlation_id: correlation_id.as_bytes().clone(),
            reply_to: Some(Topic {
                subsystem_id: SubsystemId::LightClients,
                channel: "responses".into(),
            }),
            timestamp: self.time_source.now(),
            nonce: self.nonce_generator.next(),
            signature: [0u8; 32],
            
            // SECURITY: No requester_id in payload
            payload: MerkleProofRequestPayload {
                transaction_hash: tx_hash,
            },
        };
        
        // Step 4: Sign and publish
        let signed_message = message.sign(&self.shared_secret);
        self.event_bus.publish("transaction-indexing.requests", signed_message).await?;
        
        Ok(())
    }
}

// ============================================================
// RESPONDER SIDE (Transaction Indexing - Subsystem 3)
// ============================================================

impl TransactionIndexing {
    /// Handle incoming Merkle proof requests
    async fn handle_merkle_proof_request(
        &self,
        msg: AuthenticatedMessage<MerkleProofRequestPayload>
    ) -> Result<(), Error> {
        // Step 1: Validate envelope
        self.verify_envelope(&msg)?;
        
        // Step 2: Generate proof (reads are allowed from any authorized subsystem)
        let result = self.generate_proof(msg.payload.transaction_hash);
        
        // Step 3: Construct response with SAME correlation_id
        let response = AuthenticatedMessage {
            version: PROTOCOL_VERSION,
            sender_id: SubsystemId::TransactionIndexing,
            recipient_id: msg.sender_id,
            correlation_id: msg.correlation_id, // CRITICAL: Same as request
            reply_to: None,
            timestamp: self.time_source.now(),
            nonce: self.nonce_generator.next(),
            signature: [0u8; 32],
            
            payload: MerkleProofResponsePayload {
                result: result.map_err(|e| IndexingErrorPayload {
                    error_type: match &e {
                        IndexingError::TransactionNotFound { .. } => IndexingErrorType::TransactionNotFound,
                        IndexingError::TreeNotCached { .. } => IndexingErrorType::TreeNotCached,
                        _ => IndexingErrorType::StorageError,
                    },
                    message: format!("{:?}", e),
                    transaction_hash: Some(msg.payload.transaction_hash),
                    block_hash: None,
                }),
            },
        };
        
        // Step 4: Sign and publish to requester's reply_to topic
        let signed_response = response.sign(&self.shared_secret);
        let reply_topic = msg.reply_to.ok_or(Error::MissingReplyTo)?;
        
        self.event_bus.publish(&reply_topic.to_string(), signed_response).await
    }
}
```

### 4.6 Message Envelope Compliance Checklist

For every IPC message sent or received by this subsystem:

| Field | Required | Validation |
|-------|----------|------------|
| `version` | ✅ YES | Must be within `[MIN_SUPPORTED_VERSION, MAX_SUPPORTED_VERSION]` |
| `sender_id` | ✅ YES | Must match expected sender per IPC Matrix |
| `recipient_id` | ✅ YES | Must be `SubsystemId::TransactionIndexing` for incoming |
| `correlation_id` | ✅ YES | UUID v4, used to match request/response pairs |
| `reply_to` | ✅ For requests | Topic where response should be published |
| `timestamp` | ✅ YES | Must be within 60 seconds of current time |
| `nonce` | ✅ For requests | Must not be reused (replay prevention via TimeBoundedNonceCache) |
| `signature` | ✅ YES | HMAC-SHA256, verified before processing |

**REQUEST vs RESPONSE Verification Differences:**

| Check | Request | Response |
|-------|---------|----------|
| Version | ✅ Required | ✅ Required |
| Sender ID | ✅ Required (per IPC Matrix) | ✅ Required (must be expected responder) |
| Timestamp | ✅ Required (60s window) | ✅ Required (60s window) |
| Signature | ✅ Required (HMAC) | ✅ Required (HMAC) |
| Nonce Cache | ✅ Required (TimeBoundedNonceCache) | ❌ NOT required |
| Correlation ID | ✅ Generate new UUID | ✅ Must match pending request |
| Reply-To | ✅ Required | ❌ Not applicable |

---

## 5. TDD VALIDATION STRATEGY

### 5.1 Critical Domain Logic Tests (Red Phase)

Before implementing any function bodies, we must write these failing tests:

#### Test Group 1: Merkle Tree Construction

```rust
#[test]
fn test_merkle_tree_single_transaction()
// Verify: Tree with 1 tx, root == tx_hash (padded to 2)
// Setup: Build tree from [tx1_hash]
// Assert: Tree has 2 leaves (1 real + 1 sentinel)

#[test]
fn test_merkle_tree_two_transactions()
// Verify: Tree with 2 txs, root == H(tx1 || tx2)
// Setup: Build tree from [tx1_hash, tx2_hash]
// Assert: Root is hash of concatenated leaves

#[test]
fn test_merkle_tree_three_transactions()
// Verify: INVARIANT-1 - padding to power of two (4)
// Setup: Build tree from [tx1, tx2, tx3]
// Assert: Tree has 4 leaves (3 real + 1 sentinel)

#[test]
fn test_merkle_tree_power_of_two_no_padding()
// Verify: 4 transactions needs no padding
// Setup: Build tree from [tx1, tx2, tx3, tx4]
// Assert: Tree has exactly 4 leaves

#[test]
fn test_merkle_tree_empty_transactions()
// Verify: Empty block handling
// Setup: Build tree from []
// Assert: Root == SENTINEL_HASH

#[test]
fn test_merkle_tree_deterministic()
// Verify: INVARIANT-3 - same input = same output
// Setup: Build tree twice with same transactions
// Assert: Both trees have identical roots
```

#### Test Group 2: Proof Generation

```rust
#[test]
fn test_proof_generation_first_transaction()
// Verify: Proof for tx at index 0
// Setup: Build tree, generate proof for index 0
// Assert: Proof path has correct length (log2(n))

#[test]
fn test_proof_generation_last_transaction()
// Verify: Proof for tx at last index
// Setup: Build tree with 4 txs, generate proof for index 3
// Assert: Proof is valid and verifiable

#[test]
fn test_proof_generation_middle_transaction()
// Verify: Proof for tx in middle
// Setup: Build tree with 8 txs, generate proof for index 4
// Assert: Proof path has correct sibling positions

#[test]
fn test_proof_generation_invalid_index()
// Verify: Error on out-of-bounds index
// Setup: Build tree with 4 txs
// Action: Generate proof for index 10
// Assert: Returns IndexingError::InvalidIndex

#[test]
fn test_proof_contains_correct_metadata()
// Verify: Proof includes block_hash, block_height, root
// Setup: Generate proof
// Assert: All metadata fields populated correctly
```

#### Test Group 3: Proof Verification (INVARIANT-2)

```rust
#[test]
fn test_proof_verification_valid_proof()
// Verify: INVARIANT-2 - valid proofs verify
// Setup: Generate proof from tree
// Action: Verify proof against tree root
// Assert: Returns true

#[test]
fn test_proof_verification_tampered_leaf()
// Verify: Tampered leaf hash fails
// Setup: Generate valid proof, modify leaf_hash
// Action: Verify tampered proof
// Assert: Returns false

#[test]
fn test_proof_verification_tampered_path()
// Verify: Tampered path fails
// Setup: Generate valid proof, modify one path hash
// Action: Verify tampered proof
// Assert: Returns false

#[test]
fn test_proof_verification_wrong_root()
// Verify: Proof against wrong root fails
// Setup: Generate proof, verify against different root
// Assert: Returns false

#[test]
fn test_proof_verification_static_without_tree()
// Verify: Static verification works without tree instance
// Setup: Generate proof, extract components
// Action: Call verify_proof_static directly
// Assert: Returns true for valid proof
```

#### Test Group 4: Power of Two Padding (INVARIANT-1)

```rust
#[test]
fn test_padding_1_to_2()
// Verify: 1 tx pads to 2 leaves
// Assert: padded_leaf_count == 2

#[test]
fn test_padding_3_to_4()
// Verify: 3 txs pad to 4 leaves
// Assert: padded_leaf_count == 4

#[test]
fn test_padding_5_to_8()
// Verify: 5 txs pad to 8 leaves
// Assert: padded_leaf_count == 8

#[test]
fn test_padding_17_to_32()
// Verify: 17 txs pad to 32 leaves
// Assert: padded_leaf_count == 32

#[test]
fn test_sentinel_hash_used_for_padding()
// Verify: Padding uses SENTINEL_HASH
// Setup: Build tree with 3 txs
// Assert: 4th leaf is SENTINEL_HASH
```

#### Test Group 5: Transaction Indexing

```rust
#[test]
fn test_transaction_location_stored()
// Verify: Transaction location saved after BlockValidated
// Setup: Process BlockValidated with 5 txs
// Assert: All 5 txs have locations in store

#[test]
fn test_transaction_location_correct()
// Verify: Location contains correct data
// Setup: Process BlockValidated
// Assert: Location has correct block_height, tx_index, merkle_root

#[test]
fn test_transaction_not_found()
// Verify: Unknown tx returns error
// Setup: Empty index
// Action: Query non-existent tx_hash
// Assert: Returns IndexingError::TransactionNotFound
```

#### Test Group 6: Choreography Event Handling

```rust
#[test]
fn test_block_validated_triggers_merkle_computation()
// Verify: BlockValidated → MerkleRootComputed
// Setup: Mock event bus
// Action: Handle BlockValidated event
// Assert: MerkleRootComputed published with correct data

#[test]
fn test_block_validated_rejects_non_consensus_sender()
// Verify: Only Consensus can send BlockValidated
// Setup: BlockValidated with sender_id = SubsystemId::Mempool
// Action: Handle event
// Assert: Rejected with UnauthorizedSender

#[test]
fn test_block_validated_indexes_all_transactions()
// Verify: All txs in block are indexed
// Setup: BlockValidated with 10 txs
// Action: Handle event
// Assert: All 10 txs queryable via generate_proof

#[test]
fn test_merkle_root_computed_payload_correct()
// Verify: Published event has correct fields
// Setup: Handle BlockValidated
// Assert: MerkleRootComputed has matching block_hash, height, root
```

#### Test Group 7: Cache Management (INVARIANT-5)

```rust
#[test]
fn test_tree_cache_bounded()
// Verify: INVARIANT-5 - cache respects max size
// Setup: config.max_cached_trees = 3
// Action: Process 5 blocks
// Assert: trees.len() == 3

#[test]
fn test_tree_cache_lru_eviction()
// Verify: LRU tree evicted when full
// Setup: Process blocks A, B, C (max = 3)
// Action: Process block D
// Assert: Block A's tree evicted

#[test]
fn test_tree_not_cached_error()
// Verify: Proper error when tree evicted
// Setup: Evict tree via cache pressure
// Action: Generate proof for evicted block's tx
// Assert: Returns IndexingError::TreeNotCached
```

#### Test Group 8: Canonical Serialization (INVARIANT-3)

```rust
#[test]
fn test_canonical_serialization_deterministic()
// Verify: INVARIANT-3 - same tx = same hash
// Setup: Create transaction, hash twice
// Assert: Both hashes identical

#[test]
fn test_canonical_serialization_field_order_independent()
// Verify: Field order doesn't affect hash
// Setup: Create semantically identical txs with different construction
// Assert: Same hash

#[test]
fn test_different_transactions_different_hashes()
// Verify: Different txs have different hashes
// Setup: Create two different transactions
// Assert: Hashes differ
```

### 5.2 Integration Tests (Port Contracts)

```rust
#[test]
fn test_transaction_store_adapter_roundtrip()
// Verify: Store adapter persists and retrieves correctly
// Setup: Put location, get location
// Assert: Retrieved == stored

#[test]
fn test_hash_provider_sha3_correctness()
// Verify: SHA3-256 implementation
// Setup: Known input and expected hash
// Assert: Computed hash matches expected

#[test]
fn test_hash_pair_commutative_check()
// Verify: H(a||b) != H(b||a) (order matters)
// Setup: hash_pair(a, b) and hash_pair(b, a)
// Assert: Results differ

#[test]
fn test_serializer_canonical_format()
// Verify: Serializer produces canonical bytes
// Setup: Serialize transaction
// Assert: Matches expected canonical format
```

---

## 6. SECURITY & CONSTRAINTS

### 6.1 Access Control Matrix (V2.2 Choreography Pattern)

**Event Subscriptions (Choreography):**

| Event Type | Allowed Sender | Rejection Action |
|------------|----------------|------------------|
| BlockValidated | Subsystem 8 (Consensus) ONLY | Log warning + reject |

**Request/Response Handlers:**

| Request Type | Allowed Sender(s) | Rejection Action |
|--------------|-------------------|------------------|
| MerkleProofRequest | Any authorized subsystem | N/A (permissive) |
| TransactionLocationRequest | Any authorized subsystem | N/A (permissive) |

### 6.2 Trust Boundary Enforcement

```rust
// This subsystem does NOT:
// ❌ Validate transaction signatures
// ❌ Verify transaction contents
// ❌ Execute transactions
// ❌ Check consensus rules

// This subsystem ONLY:
// ✅ Verifies AuthenticatedMessage envelope (HMAC, timestamp, nonce)
// ✅ Verifies sender_id == Consensus for BlockValidated events
// ✅ Computes Merkle trees from trusted transaction data
// ✅ Generates cryptographic proofs
// ✅ Publishes MerkleRootComputed for Block Storage assembler
```

### 6.3 Deterministic Failure Awareness (V2.2)

**ARCHITECTURAL CONTEXT (Finality Circuit Breaker):**

This subsystem participates in the block processing choreography that eventually leads to finalization. Per Architecture.md Section 5.4.1, the Finality subsystem (Subsystem 9) uses a deterministic circuit breaker.

**Implications for Transaction Indexing:**

1. **Normal Operation:** BlockValidated → MerkleRootComputed → Block Storage assembles → Eventually finalized

2. **Finality Halted State:** If the Finality circuit breaker triggers:
   - Blocks continue to be validated by Consensus
   - This subsystem continues to compute Merkle roots
   - Block Storage continues to assemble and store blocks
   - BUT blocks will NOT be marked as finalized until recovery

3. **Operational Awareness:**
   - If operators observe blocks being stored but not finalized, check Finality circuit breaker state
   - This subsystem's health is NOT affected by Finality failures
   - Monitor: `MerkleRootComputed` events should be emitted for every `BlockValidated`

### 6.4 Panic Policy

**Principle:** This library must NEVER panic in production.

**Rules:**
1. All array accesses use `.get()` with `Result` return
2. All integer operations checked for overflow (especially tree indices)
3. All `unwrap()` calls replaced with proper error handling

```rust
// ❌ FORBIDDEN
let node = self.nodes[index];  // Can panic

// ✅ REQUIRED
let node = self.nodes
    .get(index)
    .ok_or(IndexingError::InvalidIndex { index, max: self.nodes.len() })?;
```

### 6.5 Memory Constraints

**Limits:**
- **Tree Node Size:** 32 bytes (Hash)
- **Tree Size for N txs:** O(2N) nodes = O(64N) bytes
- **Max Cached Trees:** 1000 (default, configurable)
- **Max Memory:** ~1000 blocks × 1000 txs × 64 bytes = ~64 MB worst case

**Enforcement:**
```rust
// INVARIANT-5: Bounded cache
if self.trees.len() >= self.config.max_cached_trees {
    self.evict_lru_tree();
}
```

---

## 7. DEPENDENCIES & REFERENCES

### 7.1 Internal Dependencies

- **Shared Types Crate** (`crates/shared-types`):
    - `SubsystemId` enum
    - `Hash` type
    - `Transaction`, `ValidatedBlock` types
    - `AuthenticatedMessage<T>` envelope

- **Shared Bus Crate** (`crates/shared-bus`):
    - `EventPublisher` trait
    - `EventSubscriber` trait
    - `Topic` struct

### 7.2 External Crate Dependencies (Minimal)

```toml
[dependencies]
# Cryptographic hashing
sha3 = "0.10"  # For SHA3-256

# No other dependencies allowed in domain layer
```

### 7.3 References

- **IPC Matrix Document (V2.3):** Section "SUBSYSTEM 3: TRANSACTION INDEXING"
- **Architecture Document (V2.3):**
    - Section 3.2 (AuthenticatedMessage envelope)
    - Section 3.2.1 (Envelope as Sole Source of Truth - V2.2 Security Mandate)
    - Section 3.3 (Request/Response Correlation Pattern)
    - Section 5.1 (Block Validation Flow - Event-Driven Choreography)
    - Section 5.4.1 (Deterministic Trigger Conditions - Finality Circuit Breaker)
- **System.md:** Merkle Tree specification

### 7.4 Related Specifications

- **SPEC-02-BLOCK-STORAGE.md v2.3** (consumes MerkleRootComputed; provides transaction location lookups)
- **SPEC-08-CONSENSUS.md** (publishes BlockValidated events that trigger this subsystem)
- **SPEC-13-LIGHT-CLIENT.md** (requests Merkle proofs from this subsystem)

**Data Flow (V2.3 Choreography + Transaction Lookup):**
```
┌──────────────────────────────────────────────────────────────────────────┐
│                     V2.3 TRANSACTION INDEXING FLOW                       │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  CHOREOGRAPHY (Block Processing):                                        │
│  Consensus (8) ────BlockValidated────→ [Event Bus] ──→ Tx Indexing (3)   │
│                                                              │           │
│                                                              ↓           │
│                                                   [Compute Merkle Tree]  │
│                                                   [Cache Transactions]   │
│                                                              │           │
│                                                              ↓           │
│                                   ←──MerkleRootComputed──→ [Event Bus]   │
│                                                              │           │
│                                                              ↓           │
│                                                   Block Storage (2)      │
│                                                  [Stateful Assembler]    │
│                                                                          │
│  PROOF GENERATION (V2.3 Transaction Lookup):                             │
│  Light Client (13) ──MerkleProofRequest──→ Tx Indexing (3)               │
│                                                    │                     │
│                                                    ↓                     │
│                               [Check local cache for tx location]        │
│                                                    │                     │
│                         ┌──────────────────────────┴─────────────────┐   │
│                         │ If not cached:                             │   │
│                         │ GetTransactionLocationRequest ──→ Block    │   │
│                         │ Storage (2)                                │   │
│                         │ ←── TransactionLocationResponse            │   │
│                         └────────────────────────────────────────────┘   │
│                                                    │                     │
│                                                    ↓                     │
│                                    [Generate Merkle Proof]               │
│                                                    │                     │
│                                                    ↓                     │
│  Light Client (13) ←──MerkleProofResponse─── Tx Indexing (3)             │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## 8. IMPLEMENTATION CHECKLIST

### Phase 1: Domain Logic (Pure)
- [ ] Import shared types from `crates/shared-types` (DO NOT redefine)
- [ ] Implement `MerkleTree::build()` with power-of-two padding (INVARIANT-1)
- [ ] Implement `MerkleTree::generate_proof()` 
- [ ] Implement `MerkleTree::verify_proof_static()` (INVARIANT-2)
- [ ] Implement `MerkleProof` structure with all metadata
- [ ] Implement `TransactionIndex` with LRU cache (INVARIANT-5)
- [ ] Implement canonical serialization hashing (INVARIANT-3)
- [ ] Write all TDD tests from Section 5.1

### Phase 2: Port Definitions
- [ ] Define `TransactionIndexingApi` trait
- [ ] Define `TransactionStore` trait
- [ ] Define `HashProvider` trait
- [ ] Define `TransactionSerializer` trait
- [ ] Define `TimeSource` trait
- [ ] Define `BlockStorageClient` trait (V2.3 - for transaction location queries)

### Phase 3: Event Integration (V2.3 Choreography + Transaction Lookup)
- [ ] Define `BlockValidatedPayload` (incoming event)
- [ ] Define `MerkleRootComputedPayload` (outgoing choreography event)
- [ ] Define `MerkleProofRequestPayload` and response
- [ ] Define `GetTransactionLocationRequest` (V2.3 - query to Block Storage)
- [ ] Implement `handle_block_validated()` choreography handler
- [ ] Implement `handle_merkle_proof_request()` with correlation_id pattern
- [ ] Implement Block Storage location query for proof generation (V2.3)
- [ ] Implement `AuthenticatedMessage<T>` envelope handling
- [ ] Implement sender verification (Consensus only for BlockValidated)
- [ ] Verify Envelope-Only Identity (no requester_id in payloads)

### Phase 4: Adapters (Separate Crate)
- [ ] Create `transaction-indexing-adapters` crate
- [ ] Implement `RocksDBTransactionStore` adapter
- [ ] Implement `BlockStorageClientAdapter` (V2.3 - IPC client for Block Storage)
- [ ] Implement `Sha3HashProvider`
- [ ] Implement `CanonicalTransactionSerializer`
- [ ] Implement `SystemTimeSource`
- [ ] Write integration tests

---

## 9. OPEN QUESTIONS & DESIGN DECISIONS

### Q1: Tree Persistence Strategy?
**Question:** Should Merkle trees be persisted to disk or rebuilt on demand?

**Options:**
- A) Cache only (rebuild from transactions if evicted)
- B) Full persistence (store all trees)
- C) Hybrid (persist finalized blocks, cache pending)

**Decision:** Cache with LRU eviction for v1.0. Trees can be rebuilt from transactions if needed.

### Q2: Proof Format?
**Question:** What serialization format for proofs in responses?

**Options:**
- A) Custom binary format (compact)
- B) Standard format (e.g., SSZ for Ethereum compatibility)

**Decision:** Use shared-types serialization format for consistency.

### Q3: Multi-Block Proof Batching?
**Question:** Support batched proof requests for multiple transactions?

**Decision:** Defer to future version. Single-transaction proofs in v1.0.

---

## 10. ACCEPTANCE CRITERIA

This specification is considered **complete** when:

1. ✅ All domain entities defined with no implementation
2. ✅ All invariants explicitly stated (5 invariants)
3. ✅ All ports (Driving + Driven) defined as traits
4. ✅ All events defined as payloads for AuthenticatedMessage<T>
5. ✅ Choreography pattern implemented (BlockValidated → MerkleRootComputed)
6. ✅ Request/Response pattern demonstrated with correlation_id
7. ✅ All TDD tests listed (names only, no code)
8. ✅ Security constraints documented (access control matrix)
9. ✅ Memory limits specified (bounded tree cache)
10. ✅ Panic policy stated
11. ✅ Envelope-Only Identity enforced (no requester_id in payloads)
12. ✅ Deterministic Failure Awareness documented (Finality circuit breaker)

This specification is considered **approved** when:

1. ✅ Reviewed by senior architect
2. ✅ Confirmed to match IPC Matrix requirements
3. ✅ Confirmed to follow Architecture.md v2.2 Choreography Pattern
4. ✅ Confirmed AuthenticatedMessage envelope used correctly
5. ✅ Confirmed Envelope-Only Identity (no payload identity fields)
6. ✅ Confirmed integration with Block Storage Stateful Assembler
7. ✅ No implementation code present (only signatures)

---

**END OF SPECIFICATION**

**Next Steps:**
1. Review this specification for completeness
2. Approve for TDD Phase (write tests)
3. Implement domain logic (pass tests)
4. Implement adapters (wire to runtime)
