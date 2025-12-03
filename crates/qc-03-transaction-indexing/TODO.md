# TODO: qc-03 Transaction Indexing Implementation

**Subsystem ID:** 3  
**Crate:** `qc-03-transaction-indexing`  
**Status:** 🟢 PHASE 1-6 COMPLETE  
**Last Updated:** 2025-12-03

---

## Reference Documents

| Document | Key Sections |
|----------|-------------|
| System.md | Subsystem 3: Transaction Indexing (Merkle Tree) |
| Architecture.md | Section 5.1 (Choreography), Section 3.2 (AuthenticatedMessage) |
| IPC-MATRIX.md | Subsystem 3 message flows and authorization |
| SPEC-03-TRANSACTION-INDEXING.md | Complete specification |

---

## Domain Invariants (From SPEC-03 Section 2.5)

| ID | Invariant | Status |
|----|-----------|--------|
| INVARIANT-1 | Power of Two Padding | ✅ Implemented |
| INVARIANT-2 | Proof Validity | ✅ Implemented |
| INVARIANT-3 | Deterministic Hashing | ✅ Implemented |
| INVARIANT-4 | Index Consistency | ✅ Implemented |
| INVARIANT-5 | Bounded Tree Cache | ✅ Implemented |

---

## Choreography Role (V2.2)

```
Consensus (8) ──BlockValidated──→ [Event Bus]
                                       │
                                       ↓
                           Transaction Indexing (3)
                                       │
                                       ↓
                            [Compute Merkle Tree]
                            [Index Transactions]
                                       │
                                       ↓
                       ←──MerkleRootComputed──→ [Event Bus]
                                       │
                                       ↓
                              Block Storage (2)
                             [Stateful Assembler]
```

---

## Implementation Phases

### ✅ Phase 1: RED (Write Failing Tests)

Location: `src/domain/entities.rs` (test module)

| Test Group | Tests | Target Invariant | Status |
|------------|-------|------------------|--------|
| Merkle Tree Construction | 6 tests | INVARIANT-1 | ✅ PASS |
| Proof Generation | 3 tests | INVARIANT-2 | ✅ PASS |
| Proof Verification | 5 tests | INVARIANT-2 | ✅ PASS |
| Power of Two Padding | 4 tests | INVARIANT-1 | ✅ PASS |
| Transaction Index | 2 tests | INVARIANT-4 | ✅ PASS |
| Cache Management | 2 tests | INVARIANT-5 | ✅ PASS |

**Checklist:**
- [x] Create unit tests for MerkleTree
- [x] Create unit tests for MerkleProof
- [x] Create unit tests for TransactionIndex
- [x] All tests PASS (implementation complete)

---

### ✅ Phase 2: Domain Entities (Pure Logic)

Location: `src/domain/`

| Entity | Description | File | Status |
|--------|-------------|------|--------|
| `MerkleTree` | Binary hash tree with power-of-two padding | `entities.rs` | ✅ |
| `MerkleProof` | Proof of inclusion (leaf_hash, path, root) | `entities.rs` | ✅ |
| `ProofNode` | Single node in proof path (hash + position) | `entities.rs` | ✅ |
| `SiblingPosition` | Left/Right enum | `entities.rs` | ✅ |
| `TransactionLocation` | tx_hash → (block_height, tx_index, merkle_root) | `entities.rs` | ✅ |
| `TransactionIndex` | HashMap + LRU cache | `entities.rs` | ✅ |
| `IndexConfig` | max_cached_trees, persist_index | `value_objects.rs` | ✅ |
| `MerkleConfig` | hash_algorithm | `value_objects.rs` | ✅ |
| `IndexingError` | Error enum | `errors.rs` | ✅ |
| `IndexingErrorPayload` | IPC error payload | `errors.rs` | ✅ |

**Checklist:**
- [x] Create `src/domain/mod.rs`
- [x] Create `src/domain/entities.rs`
- [x] Create `src/domain/value_objects.rs`
- [x] Create `src/domain/errors.rs`
- [x] Import shared types (do NOT redefine Hash, Transaction, etc.)

---

### ✅ Phase 3: Port Definitions (Traits)

Location: `src/ports/`

| Port Type | Trait | Purpose | Status |
|-----------|-------|---------|--------|
| Inbound (API) | `TransactionIndexingApi` | generate_proof, verify_proof, get_location | ✅ |
| Outbound (SPI) | `TransactionStore` | put_location, get_location, put_tree, get_tree | ✅ |
| Outbound (SPI) | `HashProvider` | hash, hash_pair | ✅ |
| Outbound (SPI) | `TransactionSerializer` | serialize, hash_transaction | ✅ |
| Outbound (SPI) | `BlockDataProvider` | get_transaction_hashes_for_block (V2.3) | ✅ |
| Outbound (SPI) | `TimeSource` | now() | ✅ |

**Checklist:**
- [x] Create `src/ports/mod.rs`
- [x] Create `src/ports/inbound.rs` (TransactionIndexingApi)
- [x] Create `src/ports/outbound.rs` (all SPI traits)

---

### ✅ Phase 4: Event Schema (IPC Payloads)

Location: `src/ipc/`

| Payload Type | Direction | Purpose | Status |
|--------------|-----------|---------|--------|
| `BlockValidatedPayload` | Incoming | Trigger for Merkle computation | ✅ |
| `MerkleRootComputedPayload` | Outgoing | Choreography output to Block Storage | ✅ |
| `MerkleProofRequestPayload` | Incoming | Request from Light Clients | ✅ |
| `MerkleProofResponsePayload` | Outgoing | Response to Light Clients | ✅ |
| `TransactionLocationRequestPayload` | Incoming | Query for tx location | ✅ |
| `TransactionLocationResponsePayload` | Outgoing | Response with location | ✅ |

**SECURITY (Envelope-Only Identity):**
- [x] NO `requester_id` or `sender_id` in payloads
- [x] Identity from `AuthenticatedMessage` envelope ONLY

**Checklist:**
- [x] Create `src/ipc/mod.rs`
- [x] Create `src/ipc/payloads.rs` (all payload structs)
- [x] Create `src/ipc/handler.rs` (event handlers)
- [x] Verify NO identity fields in payloads

---

### ✅ Phase 5: GREEN (Implement Domain Logic)

Location: `src/domain/` and `src/ipc/handler.rs`

| Implementation | Key Logic | Status |
|----------------|-----------|--------|
| `MerkleTree::build()` | Power-of-two padding, bottom-up construction | ✅ |
| `MerkleTree::generate_proof()` | Extract sibling path from leaf to root | ✅ |
| `MerkleTree::verify_proof_static()` | Recompute root from leaf + path | ✅ |
| `TransactionIndex::cache_tree()` | LRU eviction when full | ✅ |
| `handle_block_validated()` | Compute tree, index txs, return MerkleRootComputed | ✅ |
| `handle_merkle_proof_request()` | Generate proof, handle cache miss | ✅ |

**Checklist:**
- [x] Implement `MerkleTree::build()` (INVARIANT-1)
- [x] Implement `MerkleTree::generate_proof()` (INVARIANT-2)
- [x] Implement `MerkleTree::verify_proof_static()` (INVARIANT-2)
- [x] Implement `TransactionIndex` with LRU cache (INVARIANT-5)
- [x] Implement canonical hashing (INVARIANT-3)
- [x] All 36 tests PASS

---

### ✅ Phase 6: IPC Handler Implementation

Location: `src/ipc/handler.rs`

| Handler | Authorization | Action | Status |
|---------|---------------|--------|--------|
| `handle_block_validated()` | sender_id == Consensus(8) | Compute tree, return MerkleRootComputed | ✅ |
| `handle_merkle_proof_request()` | Any authorized subsystem | Generate proof, respond | ✅ |
| `handle_transaction_location_request()` | Any authorized subsystem | Lookup location, respond | ✅ |

**Security Checks:**
- [x] Version validation
- [x] Timestamp validation (60s window)
- [x] Nonce validation (replay prevention)
- [x] Sender authorization (per IPC-MATRIX)
- [x] Recipient validation

**Checklist:**
- [x] Implement `EnvelopeValidator` with all security checks
- [x] Implement `handle_block_validated()` with sender verification
- [x] Implement `handle_merkle_proof_request()` with correlation_id
- [x] Handler tests PASS

---

### ⬜ Phase 7: Integration Tests

Location: `integration-tests/tests/qc-03/`

| Test | Description | Status |
|------|-------------|--------|
| `test_choreography_flow` | BlockValidated → MerkleRootComputed published | ⬜ TODO |
| `test_proof_generation_e2e` | Request proof, receive valid response | ⬜ TODO |
| `test_cache_miss_fallback` | Query BlockDataProvider when tree evicted | ⬜ TODO |
| `test_sender_authorization` | Reject BlockValidated from non-Consensus | ⬜ TODO |

**Checklist:**
- [ ] Create `integration-tests/tests/qc-03/mod.rs`
- [ ] Create `integration-tests/tests/qc-03/choreography.rs`
- [ ] Create `integration-tests/tests/qc-03/proof_generation.rs`
- [ ] Create `integration-tests/tests/qc-03/authorization.rs`

---

## Brutal Test Scenarios (Security)

| Attack | Target Invariant | Expected Defense | Status |
|--------|------------------|------------------|--------|
| Forge BlockValidated from Mempool | IPC Authorization | Reject: sender_id != Consensus | ✅ Tested |
| Flood with 10,000 blocks | INVARIANT-5 | LRU eviction, bounded memory | ✅ Tested |
| Tampered proof verification | INVARIANT-2 | verify_proof returns false | ✅ Tested |
| Non-deterministic hash | INVARIANT-3 | Canonical serialization | ✅ Tested |
| Replay old BlockValidated | Timestamp/Nonce | Nonce cache rejects | ✅ Tested |
| Memory exhaustion (no eviction) | INVARIANT-5 | max_cached_trees enforced | ✅ Tested |

---

## Test Results

```
running 36 tests
test result: ok. 36 passed; 0 failed; 0 ignored
```

---

## Progress Tracking

| Phase | Status | Completion Date |
|-------|--------|-----------------|
| Phase 1: RED | ✅ Complete | 2025-12-03 |
| Phase 2: Domain | ✅ Complete | 2025-12-03 |
| Phase 3: Ports | ✅ Complete | 2025-12-03 |
| Phase 4: Events | ✅ Complete | 2025-12-03 |
| Phase 5: GREEN | ✅ Complete | 2025-12-03 |
| Phase 6: IPC | ✅ Complete | 2025-12-03 |
| Phase 7: Integration | ⬜ Not Started | - |

---

## Notes

1. **V2.3 Cache Miss Handling**: When tree is evicted, query Block Storage for transaction hashes and rebuild tree
2. **Canonical Serialization**: Uses pre-computed tx_hash from ValidatedTransaction
3. **No Panic Policy**: All array accesses via `.get()`, no `unwrap()` in production code
4. **Memory Budget**: ~64 MB worst case (1000 blocks × 1000 txs × 64 bytes)

---

**END OF TODO**
