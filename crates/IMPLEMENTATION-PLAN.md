# QUANTUM-CHAIN IMPLEMENTATION PLAN
## A High-Level Strategic Roadmap for Phased Implementation

**Version:** 2.0  
**Created:** 2025-12-02  
**Branch:** develop  
**Architecture Reference:** V2.3 (Choreography + Data Retrieval Pattern)

---

## DOCUMENT PURPOSE AND SCOPE

**This document defines the ORDER of implementation, NOT the technical details.**

- **What this document IS:** A strategic roadmap defining which subsystems to build first, their dependencies, and the overall phasing strategy.
- **What this document is NOT:** A technical specification. It does not contain code, struct definitions, trait signatures, or detailed algorithms.

**All technical specifications are located in the `/SPECS` directory.** Each subsystem has a dedicated `SPEC-XX-NAME.md` file that serves as the single source of truth for its implementation.

When implementing a subsystem:
1. Consult this plan to understand the phase and dependencies
2. Read the corresponding SPEC file for technical details
3. Create a TODO.md in the subsystem's crate folder to track progress

---

## TABLE OF CONTENTS

1. [Executive Summary](#1-executive-summary)
2. [Current State Assessment](#2-current-state-assessment)
3. [Implementation Phases](#3-implementation-phases)
4. [Phase 1: Foundation](#4-phase-1-foundation)
5. [Phase 2: Consensus Infrastructure](#5-phase-2-consensus-infrastructure)
6. [Phase 3: Advanced Features](#6-phase-3-advanced-features)
7. [Phase 4: Optional Subsystems](#7-phase-4-optional-subsystems)
8. [Cross-Cutting Concerns](#8-cross-cutting-concerns)
9. [Testing Strategy](#9-testing-strategy)
10. [Risk Assessment](#10-risk-assessment)

---

## 1. EXECUTIVE SUMMARY

### 1.1 Vision

Quantum-Chain is a modular blockchain system following:
- **Domain-Driven Design (DDD)** - Business logic as first-class citizens
- **Hexagonal Architecture** - Dependency inversion via Ports & Adapters  
- **Event-Driven Architecture (EDA)** - Asynchronous, decoupled communication
- **Test-Driven Development (TDD)** - Design validated by tests first

### 1.2 Core Constraints

```
RULE #1: Libraries have ZERO knowledge of the binary/CLI/Docker
RULE #2: Direct subsystem-to-subsystem calls are FORBIDDEN
RULE #3: Implementation code CANNOT be written without tests first
RULE #4: All inter-subsystem communication via Shared Bus ONLY
```

### 1.3 Implementation Priority Summary

| Priority | Phase | Subsystems | Duration |
|----------|-------|------------|----------|
| P0 | Foundation | 10, shared-bus | Weeks 1-2 |
| P1 | Foundation | 1, 6 | Weeks 3-4 |
| P2 | Consensus | 3, 4, 8 | Weeks 5-6 |
| P3 | Consensus | 2, 5, 9 | Weeks 7-8 |
| P4 | Advanced | 11, 7, 13 | Weeks 9-12 |
| P5 | Optional | 12, 14, 15 | Weeks 13+ |

---

## 2. CURRENT STATE ASSESSMENT

### 2.1 Completed Work

| Component | Status | Notes |
|-----------|--------|-------|
| `shared-types` | ✅ Complete | Entities, Envelope, Errors, IPC types defined |
| `shared-bus` | ✅ Complete | Event bus, nonce cache, 26 tests |
| `qc-10-signature-verification` | ✅ Complete | ECDSA/BLS verification, 53 tests |
| Architecture Docs | ✅ Complete | Architecture.md, System.md, IPC-MATRIX.md at V2.3 |
| SPECS Directory | ✅ Complete | All 15 subsystem specifications |
| Crate Structure | ✅ Scaffolded | All 15 subsystem folders created |

### 2.2 What's NOT Implemented

| Component | Status | Required For |
|-----------|--------|--------------|
| Peer Discovery (1) | 🔴 Stub | Networking |
| Mempool (6) | 🔴 Stub | Transactions |
| All other subsystems | 🔴 Stub | Phase 2+ |

---

## 3. IMPLEMENTATION PHASES

### 3.1 Phase Overview Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          IMPLEMENTATION PHASES                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  PHASE 1: FOUNDATION (Weeks 1-4)                                            │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │ [shared-types] ──→ [10: Signature] ──→ [shared-bus] ──→ [1: Peers] │    │
│  │                                                      └──→ [6: Mempool]│   │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    ↓                                         │
│  PHASE 2: CONSENSUS INFRASTRUCTURE (Weeks 5-8)                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │ [3: Tx Indexing] ──→ [4: State Mgmt] ──→ [8: Consensus]            │    │
│  │         ↓                    ↓                  ↓                   │    │
│  │ [2: Block Storage - Stateful Assembler] ←───────┘                  │    │
│  │         ↓                                                          │    │
│  │ [5: Propagation] ──→ [9: Finality]                                 │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    ↓                                         │
│  PHASE 3: ADVANCED FEATURES (Weeks 9-12)                                    │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │ [11: Smart Contracts] ──→ [7: Bloom Filters] ──→ [13: Light Clients]│   │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    ↓                                         │
│  PHASE 4: OPTIONAL (Weeks 13+)                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │ [12: Tx Ordering] ──→ [14: Sharding] ──→ [15: Cross-Chain]         │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. PHASE 1: FOUNDATION

Phase 1 establishes the cryptographic and communication foundations that all other subsystems depend on.

### 4.1 Subsystem 10: Signature Verification (P0 - Week 1-2)

**Priority:** Highest. No dependencies and required by nearly all other subsystems.

**Architectural Role:** A pure cryptographic service for verifying transaction and block signatures. Provides DDoS defense at the network edge.

**Dependencies:** None (Level 0 in dependency graph)

**Implementation Details:**
- **Formal Design:** See `SPECS/SPEC-10-SIGNATURE-VERIFICATION.md`
- **Key Deliverables:** Domain logic for Ed25519 verification, `SignatureVerifier` port, batch verification support, node identity verification for DDoS defense.

---

### 4.2 Shared Bus Infrastructure (P0 - Week 2)

**Priority:** Highest. Required for all inter-subsystem communication per Architecture.md Rule #4.

**Architectural Role:** The event bus that enables the V2.3 Choreography pattern. All subsystems publish and subscribe to events through this bus.

**Dependencies:** shared-types (for AuthenticatedMessage envelope)

**Implementation Details:**
- **Formal Design:** See `Documentation/Architecture.md` Section 5 and `Documentation/IPC-MATRIX.md`
- **Key Deliverables:** EventBus trait, TimeBoundedNonceCache (v2.1), Dead Letter Queue handling, message verification logic.

---

### 4.3 Subsystem 1: Peer Discovery (P1 - Week 3)

**Priority:** High. Required for networking. First subsystem to use Signature Verification for DDoS defense.

**Architectural Role:** Kademlia DHT-based peer discovery. Provides peer lists to Block Propagation, Bloom Filters, and Light Clients.

**Dependencies:** Subsystem 10 (for node identity verification)

**Implementation Details:**
- **Formal Design:** See `SPECS/SPEC-01-PEER-DISCOVERY.md`
- **Key Deliverables:** Kademlia routing table, XOR distance calculation, peer reputation system, signature verification integration at network edge.

---

### 4.4 Subsystem 6: Mempool (P1 - Week 4)

**Priority:** High. Core infrastructure for transaction handling and consensus.

**Architectural Role:** Transaction queue with Two-Phase Commit for safe transaction removal. Prevents transaction loss on storage failures.

**Dependencies:** Subsystem 10 (signature verification), Subsystem 4 (balance/nonce checks - can be mocked initially)

**Implementation Details:**
- **Formal Design:** See `SPECS/SPEC-06-MEMPOOL.md`
- **Key Deliverables:** Priority queue, Two-Phase Commit protocol, transaction state machine (Pending → PendingInclusion → Confirmed), BlockStorageConfirmation handling.

---

## 5. PHASE 2: CONSENSUS INFRASTRUCTURE

Phase 2 implements the V2.3 Choreography pattern where Consensus validates blocks and publishes events, while Block Storage acts as a Stateful Assembler.

### 5.1 Subsystem 3: Transaction Indexing (P2 - Week 5)

**Priority:** Medium-High. Choreography participant for Merkle root computation.

**Architectural Role (V2.3):** Subscribes to `BlockValidated` events, computes Merkle roots, publishes `MerkleRootComputed`. Also provides Merkle proofs via Data Retrieval pattern.

**Dependencies:** Subsystem 10, Event Bus, Subsystem 2 (for Data Retrieval on cache miss)

**Implementation Details:**
- **Formal Design:** See `SPECS/SPEC-03-TRANSACTION-INDEXING.md`
- **Key Deliverables:** Merkle tree construction, proof generation, event subscription/publishing, Data Retrieval integration with Block Storage.

---

### 5.2 Subsystem 4: State Management (P2 - Week 5-6)

**Priority:** Medium-High. Choreography participant for state root computation.

**Architectural Role (V2.3):** Subscribes to `BlockValidated` events, computes state roots from transaction execution, publishes `StateRootComputed`.

**Dependencies:** Subsystem 11 (for state updates - can be stubbed), Event Bus

**Implementation Details:**
- **Formal Design:** See `SPECS/SPEC-04-STATE-MANAGEMENT.md`
- **Key Deliverables:** Patricia Merkle Trie, account state management, state root computation, event subscription/publishing.

---

### 5.3 Subsystem 8: Consensus (P2 - Week 6)

**Priority:** Medium-High. Validation only - NOT an orchestrator in V2.3.

**Architectural Role (V2.3):** Validates blocks cryptographically and publishes `BlockValidated` to the Event Bus. Does NOT orchestrate storage writes. Implements Zero-Trust signature re-verification.

**Dependencies:** Subsystem 5 (receives blocks), Subsystem 6 (gets transactions), Subsystem 10 (signature verification)

**Implementation Details:**
- **Formal Design:** See `SPECS/SPEC-08-CONSENSUS.md`
- **Key Deliverables:** Block validation, PoS/PBFT support, Zero-Trust re-verification of all validator signatures, `BlockValidated` event publishing.

---

### 5.4 Subsystem 2: Block Storage (P3 - Week 7)

**Priority:** Medium. Stateful Assembler in V2.3 Choreography pattern.

**Architectural Role (V2.3):** Subscribes to three events (`BlockValidated`, `MerkleRootComputed`, `StateRootComputed`), buffers components, performs atomic write when all three arrive. Also serves as Data Provider for Transaction Indexing.

**Dependencies:** Event Bus (subscribes to events from 3, 4, 8), Subsystem 9 (finality marking)

**Implementation Details:**
- **Formal Design:** See `SPECS/SPEC-02-BLOCK-STORAGE.md`
- **Key Deliverables:** PendingBlockAssembly buffer, atomic write logic, assembly timeout handling, BlockStorageConfirmation publishing, Data Provider ports.

---

### 5.5 Subsystem 5: Block Propagation (P3 - Week 7-8)

**Priority:** Medium. Network distribution of validated blocks.

**Architectural Role:** Gossip protocol for distributing blocks across the network. Compact block relay for bandwidth efficiency.

**Dependencies:** Subsystem 1 (peer list), Subsystem 8 (receives validated blocks)

**Implementation Details:**
- **Formal Design:** See `SPECS/SPEC-05-BLOCK-PROPAGATION.md`
- **Key Deliverables:** Gossip protocol, compact block relay (BIP152-style), rate limiting, peer reputation integration.

---

### 5.6 Subsystem 9: Finality (P3 - Week 8)

**Priority:** Medium. Casper FFG with Circuit Breaker for livelock prevention.

**Architectural Role:** Provides economic finality guarantees. Implements Circuit Breaker pattern with deterministic trigger conditions for testability.

**Dependencies:** Subsystem 8 (attestations), Subsystem 10 (Zero-Trust signature verification)

**Implementation Details:**
- **Formal Design:** See `SPECS/SPEC-09-FINALITY.md`
- **Key Deliverables:** Justification/finalization logic, Circuit Breaker (MAX_SYNC_ATTEMPTS=3, SYNC_TIMEOUT=120s), Zero-Trust signature re-verification, HALTED state handling.

---

## 6. PHASE 3: ADVANCED FEATURES

Phase 3 adds programmability and light client support.

### 6.1 Subsystem 11: Smart Contracts (P4 - Week 9-10)

**Priority:** Lower. Enables programmable transactions.

**Architectural Role:** Virtual machine for deterministic contract execution with gas metering.

**Dependencies:** Subsystem 4 (state read/write), Subsystem 10 (sender verification)

**Implementation Details:**
- **Formal Design:** See `SPECS/SPEC-11-SMART-CONTRACTS.md`
- **Key Deliverables:** Stack-based VM, gas metering, storage operations, HTLC support for cross-chain.

---

### 6.2 Subsystem 7: Bloom Filters (P4 - Week 11)

**Priority:** Lower. Light client support infrastructure.

**Architectural Role:** Probabilistic membership tests for efficient transaction filtering.

**Dependencies:** Subsystem 3 (transaction hashes), Subsystem 1 (full node connections)

**Implementation Details:**
- **Formal Design:** See `SPECS/SPEC-07-BLOOM-FILTERS.md`
- **Key Deliverables:** Bloom filter construction, false positive rate tuning, filter rotation for privacy.

---

### 6.3 Subsystem 13: Light Client Sync (P4 - Week 11-12)

**Priority:** Lower. Enables resource-constrained clients.

**Architectural Role:** SPV-style verification without full chain download.

**Dependencies:** Subsystem 1 (full node connections), Subsystem 3 (Merkle proofs), Subsystem 7 (Bloom filters)

**Implementation Details:**
- **Formal Design:** See `SPECS/SPEC-13-LIGHT-CLIENT.md`
- **Key Deliverables:** Header chain sync, Merkle proof verification, Bloom filter setup.

---

## 7. PHASE 4: OPTIONAL SUBSYSTEMS

Phase 4 subsystems are optional and can be implemented based on project requirements.

### 7.1 Subsystem 12: Transaction Ordering (Weeks 13+)

**Architectural Role:** DAG-based transaction ordering for parallel execution.

**Implementation Details:**
- **Formal Design:** See `SPECS/SPEC-12-TRANSACTION-ORDERING.md`

---

### 7.2 Subsystem 14: Sharding (Weeks 14+)

**Architectural Role:** Horizontal scaling via state partitioning.

**Implementation Details:**
- **Formal Design:** See `SPECS/SPEC-14-SHARDING.md`

---

### 7.3 Subsystem 15: Cross-Chain (Weeks 15+)

**Architectural Role:** HTLC-based atomic swaps for interoperability.

**Implementation Details:**
- **Formal Design:** See `SPECS/SPEC-15-CROSS-CHAIN.md`

---

## 8. CROSS-CUTTING CONCERNS

### 8.1 Observability

Each subsystem MUST emit:
- Metrics (processing time, event counts, error rates)
- Structured logs with trace IDs for correlation
- Health check endpoints

### 8.2 Configuration

Each subsystem MUST support:
- Runtime configuration via TOML config file
- Environment variable overrides
- Sensible defaults for development

### 8.3 Error Handling

Follow Architecture.md DLQ strategy:

| Criticality | Retry Count | Backoff | DLQ Action |
|-------------|-------------|---------|------------|
| CRITICAL (Block Storage, State Write) | 5 | Exponential | Alert + Manual Review |
| HIGH (Consensus) | 3 | Exponential | Alert + Auto-Retry 1hr |
| FINALITY | 0 | None | Circuit Breaker |
| MEDIUM (Mempool, Propagation) | 2 | Linear | Auto-Discard 24hrs |
| LOW (Metrics, Logging) | 0 | None | Discard immediately |

---

## 9. TESTING STRATEGY

### 9.1 Test Pyramid

```
                    ┌────────────────┐
                    │  E2E Tests     │  ← Few (expensive, slow)
                    │  Full node     │
                    └────────────────┘
                ┌──────────────────────┐
                │ Integration Tests    │  ← Some (moderate cost)
                │ Port contracts       │
                └──────────────────────┘
        ┌──────────────────────────────────┐
        │      Unit Tests                  │  ← Many (cheap, fast)
        │      Domain logic                │
        └──────────────────────────────────┘
```

### 9.2 Testing Requirements by Layer

| Layer | Test Type | Focus |
|-------|-----------|-------|
| Domain | Unit | Pure business logic, no I/O |
| Ports | Integration | Adapter contract compliance |
| Subsystem | Integration | Event subscription/publishing |
| System | E2E | Full node behavior |

---

## 10. RISK ASSESSMENT

### 10.1 Technical Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Cryptography bugs | Critical | Use audited libraries (ed25519-dalek) |
| State trie complexity | High | Start simple, optimize later |
| Event ordering issues | High | Comprehensive integration tests |
| Memory exhaustion | Medium | Time-bounded caches, periodic GC |

### 10.2 Schedule Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Underestimated complexity | High | Buffer time between phases |
| Integration issues | Medium | Early integration testing |
| Scope creep | Medium | Strict adherence to SPEC docs |

### 10.3 Architectural Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| V2.3 Choreography complexity | Medium | Follow SPEC patterns exactly |
| Two-Phase Commit failures | High | Comprehensive timeout handling |
| Circuit Breaker edge cases | Medium | Deterministic, testable triggers |

---

## NEXT STEPS

1. **Read this plan** to understand the phase and dependencies
2. **Start with Phase 1, P0:** Subsystem 10 (Signature Verification)
3. **Read the SPEC:** `SPECS/SPEC-10-SIGNATURE-VERIFICATION.md`
4. **Create TODO.md** in `crates/qc-10-signature-verification/TODO.md`
5. **Implement using TDD:** RED → GREEN → REFACTOR
6. **Proceed to next subsystem** in priority order

---

**END OF IMPLEMENTATION PLAN**

*This document is a strategic roadmap. For technical details, see the SPECS directory.*
