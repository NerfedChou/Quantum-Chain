# Changelog

All notable changes to Quantum-Chain will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Implemented Subsystems

#### Subsystem 10: Signature Verification (2024-12-02)
- **ADDED:** Complete domain layer implementation per SPEC-10
  - `SignatureType` enum (Ecdsa, Bls, Ed25519, Schnorr, MultiSig)
  - `VerificationResult` value object with security metadata
  - `SignatureVerificationService` with batch verification
- **ADDED:** Ports layer (Hexagonal Architecture)
  - `SignatureVerifier` inbound port
  - `KeyStore`, `SignatureCache`, `MetricsRecorder` outbound ports
- **ADDED:** Event definitions for shared bus
  - `SignatureVerifiedEvent`, `SignatureBatchVerifiedEvent`
  - `SignatureRejectedEvent`, `VerificationFailedEvent`
- **ADDED:** Comprehensive test suite (TDD compliant)
  - Unit tests for domain logic
  - Integration tests for port contracts
  - Property-based tests for cryptographic verification
- **SECURITY:** Zero external dependencies for core crypto (uses `k256`, `sha2`)

#### Infrastructure: Shared Bus (2024-12-02)
- **ADDED:** `shared-bus` crate implementing Choreography pattern per Architecture.md V2.3
  - `Event` domain entity with envelope metadata
  - `EventMetadata` with correlation_id, causation_id, timestamp
  - `EventBus` inbound port trait
  - `EventStore`, `EventSerializer`, `MetricsRecorder` outbound ports
  - `EventPublished`, `EventReceived`, `EventProcessingFailed` events
- **ALIGNED:** IPC-MATRIX.md communication rules enforced at type level

### DevOps

#### CI/CD Toolchain Update (2024-12-02)
- **FIXED:** Docker image tag `rust:stable-slim-bookworm` → `rust:slim-bookworm`
- **FIXED:** Updated MSRV from `1.82.0` to `1.85.0` for `edition2024` support
- **FIXED:** Removed `RUST_VERSION` build-arg from Dockerfiles (uses latest stable)
- **REASON:** `base64ct v1.8.0` requires `edition2024` feature, needs Rust 1.85+
- **UPDATED:** README.md to reflect Rust 1.85+ requirement

#### CI/CD Stabilization (2024-12-02)
- **FIXED:** Changed CI toolchain from `1.82.0` to `stable` in `rust.yml`
- **FIXED:** Changed Docker build toolchain to `stable` in `docker-publish.yml`
- **REASON:** `edition2024` feature in `base64ct v1.8.0` required stable toolchain support
- **NOTE:** Project now runs on stable Rust (1.85+), no nightly required

### Architecture Evolution

#### V2.4 - Hybrid Docker Architecture (2024-12-01)
- **ADDED:** Hybrid container architecture (Monolithic + Per-Subsystem modes)
- **ADDED:** `docker/docker-compose.yml` with 15 subsystem service definitions
- **ADDED:** `docker/Dockerfile.subsystem` for individual subsystem containers
- **ADDED:** Event Bus infrastructure (Redis Streams) for inter-container IPC
- **ADDED:** Prometheus/Grafana monitoring stack
- **ADDED:** Per-subsystem CI test matrix in `rust.yml`
- **ADDED:** IPC-MATRIX compliance validation in CI pipeline
- **UPDATED:** `docker-publish.yml` to support both deployment modes
- **UPDATED:** README.md with comprehensive DevOps documentation

#### V2.3 - Unified Workflow (2024-12-01)
- **ADDED:** Data Retrieval Pattern for Merkle proof generation
- **ADDED:** `GetTransactionHashesRequest` contract between Subsystem 3 → Subsystem 2
- **ADDED:** Bidirectional dependency graph (Block Storage ↔ Transaction Indexing)
- **FIXED:** Proof generation was logically impossible without transaction lookup path

#### V2.2 - Choreography Pattern (2024-11-30)
- **BREAKING:** Replaced Orchestrator pattern with Choreography pattern
- **ADDED:** Stateful Assembler in Block Storage (Subsystem 2)
- **ADDED:** Envelope-Only Identity mandate (no `requester_id` in payloads)
- **ADDED:** Time-Bounded Nonce Cache (prevents memory exhaustion attacks)
- **ADDED:** Zero-Trust Signature Re-Verification
- **ADDED:** Reply-To Forwarding Attack Prevention
- **ADDED:** Deterministic Circuit Breaker triggers for testability
- **REMOVED:** Centralized orchestration from Consensus (Subsystem 8)

#### V2.1 - Security Hardening (2024-11-29)
- **ADDED:** Time-bounded replay prevention (nonce cache expires after 120s)
- **ADDED:** Circuit breaker for Finality subsystem
- **ADDED:** Two-Phase Commit for Mempool transaction removal
- **FIXED:** Nonce Cache Exhaustion vulnerability (unbounded memory growth)
- **FIXED:** Transaction Loss vulnerability on storage failure

#### V2.0 - Initial Architecture (2024-11-28)
- **ADDED:** 15-subsystem modular architecture
- **ADDED:** Hexagonal Architecture (Ports & Adapters)
- **ADDED:** Event-Driven Architecture (Shared Bus)
- **ADDED:** IPC-MATRIX security boundaries

---

## [0.1.0] - 2024-12-01

### Added
- Initial project structure with Cargo workspace
- 15 subsystem crates (`qc-01` through `qc-15`)
- Shared types crate (`shared-types`)
- Node runtime binary (`node-runtime`)
- Master documentation:
  - `Architecture.md` (V2.3)
  - `System.md` (V2.3)
  - `IPC-MATRIX.md` (V2.3)
- Micro-level specifications:
  - `SPEC-01-PEER-DISCOVERY.md` (V2.4)
  - `SPEC-02-BLOCK-STORAGE.md` (V2.3)
  - `SPEC-03-TRANSACTION-INDEXING.md` (V2.3)
  - `SPEC-04-STATE-MANAGEMENT.md` (V2.3)
  - `SPEC-05-BLOCK-PROPAGATION.md` (V2.3)
  - `SPEC-06-MEMPOOL.md` (V2.3)
  - `SPEC-07-BLOOM-FILTERS.md` (V2.3)
  - `SPEC-08-CONSENSUS.md` (V2.3)
  - `SPEC-09-FINALITY.md` (V2.3)
  - `SPEC-10-SIGNATURE-VERIFICATION.md` (V2.3)
  - `SPEC-11-SMART-CONTRACTS.md` (V2.3)
  - `SPEC-12-TRANSACTION-ORDERING.md` (V2.3)
  - `SPEC-13-LIGHT-CLIENT-SYNC.md` (V2.3)
  - `SPEC-14-SHARDING.md` (V2.3)
  - `SPEC-15-CROSS-CHAIN.md` (V2.3)

### DevOps
- CI/CD workflows:
  - `rust.yml` - Rust build, test, lint with per-subsystem isolation testing
  - `docker-publish.yml` - Hybrid container build (Monolithic + Per-Subsystem)
- Docker infrastructure:
  - `Dockerfile` - Multi-stage production build (~50MB image)
  - `docker/Dockerfile.subsystem` - Individual subsystem containers
  - `docker/docker-compose.yml` - Full orchestration with 3 profiles:
    - Default: Monolithic node
    - `dev`: Per-subsystem containers with Event Bus
    - `monitoring`: Prometheus + Grafana stack
  - `docker/monitoring/prometheus.yml` - Metrics collection for all 15 subsystems

### Security
- Implemented Envelope-Only Identity pattern
- Added Time-Bounded Nonce Cache
- Added Zero-Trust Signature Verification mandate
- Added Reply-To Forwarding Attack Prevention
- Added Circuit Breaker with deterministic triggers
- Added Two-Phase Transaction Removal Protocol
- Added IPC-MATRIX compliance validation in CI

---

## Blocking Flags Resolved

### Flag #23 - Incomplete Envelope-Only Identity Implementation
- **Status:** ✅ RESOLVED
- **Resolution:** Updated all SPEC payload structs to remove `requester_id` fields
- **Commit:** TBD

### Flag #24 - Missing Stateful Assembler Acknowledgment
- **Status:** ✅ RESOLVED
- **Resolution:** Added architectural context notes to SPEC-01 and SPEC-02
- **Commit:** TBD

### Flag #28 - Missing BlockDataProvider Contract
- **Status:** ✅ RESOLVED
- **Resolution:** Added V2.3 Data Retrieval pattern with transaction lookup path
- **Commit:** TBD

---

## Document Version Matrix

| Document | Current Version | Last Updated |
|----------|-----------------|--------------|
| Architecture.md | V2.3 | 2024-12-01 |
| System.md | V2.3 | 2024-12-01 |
| IPC-MATRIX.md | V2.3 | 2024-12-01 |
| SPEC-01 | V2.4 | 2024-12-01 |
| SPEC-02 | V2.3 | 2024-12-01 |
| SPEC-03 | V2.3 | 2024-12-01 |
| SPEC-04 through SPEC-09 | V2.3 | 2024-12-01 |
| **SPEC-10** | **V2.3** | **2024-12-02** |
| SPEC-11 through SPEC-15 | V2.3 | 2024-12-01 |

---

## Migration Notes

### From V2.2 to V2.3

If upgrading from V2.2 architecture:

1. **Block Storage (Subsystem 2):**
   - Add `GetTransactionHashesRequest` handler
   - Add `TransactionHashesResponse` publisher
   - Update security boundaries to accept requests from Subsystem 3

2. **Transaction Indexing (Subsystem 3):**
   - Add `BlockDataProvider` port
   - Implement cache-miss logic to query Block Storage
   - Update dependency graph to show bidirectional relationship

3. **IPC-MATRIX:**
   - Add Subsystem 3 → Subsystem 2 communication path
   - Update security boundaries for both subsystems

### From V2.1 to V2.2

If upgrading from V2.1 architecture:

1. **Remove all `requester_id` fields from payloads**
2. **Implement Stateful Assembler in Block Storage**
3. **Convert Consensus from orchestrator to event publisher**
4. **Update message verification to use TimeBoundedNonceCache**

---

## Dependency Graph (V2.3)

```
LEVEL 0 (No Dependencies):
├─ [10] Signature Verification

LEVEL 1 (Depends on Level 0):
├─ [1] Peer Discovery → [10]
├─ [6] Mempool → [10]
└─ [7] Bloom Filters → [1]

LEVEL 2 (Depends on Level 0-1):
├─ [3] Transaction Indexing → [10], [2]
├─ [5] Block Propagation → [1]
└─ [4] State Management (partial)

LEVEL 3 (Depends on Level 0-2):
├─ [8] Consensus → [5, 6, 10]
└─ [11] Smart Contracts → [4, 10]

LEVEL 4 (Depends on Level 0-3):
├─ [2] Block Storage → subscribes to [3, 4, 8], provides to [3]
├─ [9] Finality → [8, 10]
├─ [12] Transaction Ordering → [4, 11]
└─ [14] Sharding → [4, 8]

LEVEL 5 (Depends on Level 0-4):
└─ [15] Cross-Chain → [8, 9, 11]
```

---

**Maintained by:** Quantum-Chain Contributors
