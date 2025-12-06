# Quantum-Chain

**A Production-Ready Modular Blockchain System with Quantum-Inspired Architecture**

[![Rust](https://img.shields.io/badge/rust-stable%20(1.85%2B)-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-Unlicense-blue.svg)](LICENSE)
[![Architecture](https://img.shields.io/badge/architecture-v2.4-green.svg)](Documentation/Architecture.md)
[![Tests](https://img.shields.io/badge/tests-1180%20passing-brightgreen.svg)](#test-coverage)
[![CI](https://github.com/NerfedChou/Quantum-Chain/actions/workflows/rust.yml/badge.svg)](https://github.com/NerfedChou/Quantum-Chain/actions/workflows/rust.yml)

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [API Gateway](#api-gateway)
4. [Subsystems](#subsystems)
5. [Test Coverage](#test-coverage)
6. [Quick Start](#quick-start)
7. [Development](#development)
8. [Security](#security)
9. [Documentation](#documentation)
10. [Contributing](#contributing)

---

## Overview

Quantum-Chain is a **production-ready modular blockchain system** built with Rust, implementing a hybrid architecture that combines:

- **Domain-Driven Design (DDD)** - Business logic as first-class citizens
- **Hexagonal Architecture** - Dependency inversion via Ports & Adapters
- **Event-Driven Architecture (EDA)** - Asynchronous, decoupled communication
- **Zero-Trust Security** - Independent signature re-verification at every layer

### Key Design Principles

```
RULE #1: Libraries have ZERO knowledge of the binary/CLI/Docker
RULE #2: Direct subsystem-to-subsystem calls are FORBIDDEN
RULE #3: All inter-subsystem communication via Shared Bus ONLY
RULE #4: Consensus-critical signatures are re-verified independently
```

### Production Readiness (December 2025)

| Component | Status | Tests |
|-----------|--------|-------|
| Core Subsystems (1-10) | ✅ Production Ready | 531 |
| Bloom Filters (7) | ✅ Production Ready | 61 |
| API Gateway (16) | ✅ Production Ready | 111 |
| Integration Tests | ✅ All Passing | 281 |
| Node Runtime Wiring | ✅ Complete | 87 |
| Infrastructure | ✅ Ready | 54 |
| **Total** | **✅ Ready** | **1180** |

---

## Architecture

### System Topology

Quantum-Chain is architected as a **fortress of isolated subsystems**, each representing a distinct business capability (Bounded Context):

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           EXTERNAL WORLD                                    │
│         (Wallets, dApps, Block Explorers, CLI Tools, Monitoring)           │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                     SUBSYSTEM 16: API GATEWAY                               │
│     ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐        │
│     │JSON-RPC │  │WebSocket│  │  REST   │  │ Metrics │  │ Health  │        │
│     │ :8545   │  │ :8546   │  │ :8080   │  │ :9090   │  │ :8081   │        │
│     └─────────┘  └─────────┘  └─────────┘  └─────────┘  └─────────┘        │
│         │             │            │            │            │              │
│         └─────────────┴────────────┴────────────┴────────────┘              │
│                                    │                                        │
│              Tower Middleware: Rate Limit → Timeout → CORS                  │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         QUANTUM-CHAIN NODE RUNTIME                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                         SHARED EVENT BUS                             │    │
│  │            (HMAC-authenticated, Time-bounded Nonces)                 │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│       │              │              │              │              │         │
│       ▼              ▼              ▼              ▼              ▼         │
│  ┌─────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐       │
│  │ Peer    │   │ Block   │   │ Tx      │   │ State   │   │ Block   │       │
│  │ Disc(1) │   │ Store(2)│   │ Index(3)│   │ Mgmt(4) │   │ Prop(5) │       │
│  └─────────┘   └─────────┘   └─────────┘   └─────────┘   └─────────┘       │
│       │              │              │              │              │         │
│       ▼              ▼              ▼              ▼              ▼         │
│  ┌─────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐       │
│  │ Mempool │   │ Bloom   │   │Consensus│   │Finality │   │ Sig     │       │
│  │   (6)   │   │Filters(7)│  │   (8)   │   │   (9)   │   │ Ver(10) │       │
│  └─────────┘   └─────────┘   └─────────┘   └─────────┘   └─────────┘       │
│                                                                             │
│                           [ LGTM Telemetry ]                                │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### V2.4 Choreography Pattern

The system uses **event-driven choreography** for block processing:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    BLOCK VALIDATION: CHOREOGRAPHY PATTERN                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   [Consensus (8)] ──BlockValidated──→ [Event Bus]                           │
│                                            │                                │
│          ┌─────────────────────────────────┼─────────────────────┐          │
│          ↓                                 ↓                     ↓          │
│   [Tx Indexing (3)]              [State Mgmt (4)]        [Block Storage (2)]│
│          │                                 │              (Stateful Assembler)
│          ↓                                 ↓                     ↑          │
│   MerkleRootComputed              StateRootComputed              │          │
│          └─────────────────────────────────┴─────────────────────┘          │
│                                            │                                │
│                                            ↓                                │
│                              [Atomic Write + Finality (9)]                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Security Model

| Layer | Protection | Implementation |
|-------|------------|----------------|
| **API Security** | Rate limiting, method whitelists, CORS | `qc-16` Tower |
| **IPC Security** | HMAC-SHA256 authenticated envelopes | `shared-bus` |
| **Replay Prevention** | Time-bounded nonce cache (120s) | `TimeBoundedNonceCache` |
| **Zero-Trust** | Signatures re-verified at Consensus & Finality | `qc-08`, `qc-09` |
| **Side-Channel** | Constant-time cryptographic operations | `subtle` crate |
| **Memory Safety** | Zeroization of sensitive data | `zeroize` crate |
| **Malleability** | EIP-2 low-S enforcement | `qc-10` |

---

## API Gateway

### Subsystem 16: External Interface

The API Gateway (`qc-16-api-gateway`) is the **single entry point** for all external interactions:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         API GATEWAY INTERFACES                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   JSON-RPC 2.0 (:8545)        Ethereum-compatible API                       │
│   ├─ eth_getBalance           → qc-04 State Management                      │
│   ├─ eth_sendRawTransaction   → qc-06 Mempool                               │
│   ├─ eth_getBlock*            → qc-02 Block Storage                         │
│   ├─ eth_getTransaction*      → qc-03 Transaction Indexing                  │
│   ├─ eth_call                 → qc-11 Smart Contracts                       │
│   └─ eth_subscribe            → Event Bus                                   │
│                                                                             │
│   WebSocket (:8546)           Real-time subscriptions                       │
│   ├─ newHeads                 Block notifications                           │
│   ├─ logs                     Event log notifications                       │
│   └─ pendingTransactions      Mempool notifications                         │
│                                                                             │
│   REST API (:8080)            Admin endpoints (protected)                   │
│   ├─ /admin/peers             Node peer management                          │
│   └─ /admin/status            Node status                                   │
│                                                                             │
│   Prometheus (:9090)          Metrics for Grafana/Mimir                     │
│   └─ /metrics                 Request counts, latencies, errors             │
│                                                                             │
│   Health (:8081)              Kubernetes/Docker probes                      │
│   ├─ /health/live             Liveness probe                                │
│   └─ /health/ready            Readiness probe                               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Method Security Tiers

| Tier | Access | Examples |
|------|--------|----------|
| **Tier 1: Public** | No auth | `eth_getBalance`, `eth_sendRawTransaction`, `eth_call` |
| **Tier 2: Protected** | API key or localhost | `admin_peers`, `txpool_status` |
| **Tier 3: Admin** | Localhost + auth | `admin_addPeer`, `miner_start`, `debug_*` |

### Stack

- **Axum** - HTTP/WebSocket framework
- **Tower** - Middleware (rate limiting, timeout, CORS)
- **jsonrpsee** - JSON-RPC 2.0 protocol

---

## Subsystems

### Core Subsystems (Production Ready)

| ID | Crate | Description | Tests | Status |
|----|-------|-------------|-------|--------|
| 1 | `qc-01-peer-discovery` | Kademlia DHT, DDoS defense | 86 | ✅ |
| 2 | `qc-02-block-storage` | Choreography assembler, atomic writes | 66 | ✅ |
| 3 | `qc-03-transaction-indexing` | Merkle trees, inclusion proofs | 40 | ✅ |
| 4 | `qc-04-state-management` | Patricia Merkle Trie | 22 | ✅ |
| 5 | `qc-05-block-propagation` | Gossip protocol, compact blocks | 37 | ✅ |
| 6 | `qc-06-mempool` | Priority queue, two-phase commit | 95 | ✅ |
| 7 | `qc-07-bloom-filters` | SPV filtering, O(1) membership tests | 61 | ✅ |
| 8 | `qc-08-consensus` | PoS/PBFT, 2/3 attestation threshold | 30 | ✅ |
| 9 | `qc-09-finality` | Casper FFG, slashing, circuit breaker | 33 | ✅ |
| 10 | `qc-10-signature-verification` | ECDSA/BLS, batch verification | 61 | ✅ |

### External Interface

| ID | Crate | Description | Tests | Status |
|----|-------|-------------|-------|--------|
| 16 | `qc-16-api-gateway` | JSON-RPC/WebSocket/REST API | 111 | ✅ |

### Infrastructure

| Crate | Purpose | Tests | Status |
|-------|---------|-------|--------|
| `shared-types` | Common types (Hash, Address, Signature, SubsystemId) | 12 | ✅ |
| `shared-bus` | HMAC-authenticated event bus, nonce cache | 26 | ✅ |
| `quantum-telemetry` | LGTM observability (Loki, Grafana, Tempo, Mimir) | 16 | ✅ |
| `node-runtime` | Application binary, subsystem wiring | 87 | ✅ |
| `integration-tests` | End-to-end exploit & choreography tests | 281 | ✅ |

### Future Subsystems

| ID | Name | Status |
|----|------|--------|
| 11-15 | Advanced (Sharding, Cross-chain, etc.) | Planned |

---

## Test Coverage

### Summary (December 2025)

```
┌────────────────────────────────────────────────────────────────┐
│                    TEST RESULTS: 1180 PASSING                  │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│  Core Subsystems (Unit Tests)                                  │
│  ├── qc-01-peer-discovery ................ 86 tests ✅        │
│  ├── qc-02-block-storage ................. 66 tests ✅        │
│  ├── qc-03-transaction-indexing .......... 40 tests ✅        │
│  ├── qc-04-state-management .............. 22 tests ✅        │
│  ├── qc-05-block-propagation ............. 37 tests ✅        │
│  ├── qc-06-mempool ....................... 95 tests ✅        │
│  ├── qc-07-bloom-filters ................. 61 tests ✅        │
│  ├── qc-08-consensus ..................... 30 tests ✅        │
│  ├── qc-09-finality ...................... 33 tests ✅        │
│  └── qc-10-signature-verification ........ 61 tests ✅        │
│                                                                │
│  External Interface                                            │
│  └── qc-16-api-gateway .................. 111 tests ✅        │
│                                                                │
│  Integration Tests                                             │
│  └── integration-tests .................. 281 tests ✅        │
│                                                                │
│  Infrastructure                                                │
│  ├── node-runtime ....................... 87 tests ✅         │
│  ├── shared-types ....................... 12 tests ✅         │
│  ├── shared-bus ......................... 26 tests ✅         │
│  └── quantum-telemetry .................. 16 tests ✅         │
│                                                                │
│  TOTAL: 1180 tests passing                                     │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

### Test Categories

| Category | Coverage | Description |
|----------|----------|-------------|
| **Unit Tests** | 531 | Domain logic, ports, services |
| **API Gateway** | 111 | JSON-RPC, WebSocket, middleware |
| **Integration Tests** | 281 | Cross-subsystem flows, exploit scenarios |
| **Infrastructure Tests** | 141 | Wiring, event routing, shared components |
| **Invariant Tests** | ✅ | Determinism, no false positives, no malleability |
| **Security Tests** | ✅ | IPC auth, replay prevention, rate limiting |

---

## Quick Start

### Prerequisites

- **Rust** stable toolchain (1.85+)
- **Cargo** (comes with Rust)
- **Docker** (optional)

### Build from Source

```bash
# Clone the repository
git clone https://github.com/NerfedChou/Quantum-Chain.git
cd Quantum-Chain

# Build all crates
cargo build --release

# Run all tests (1180 tests)
cargo test --all

# Run the node
cargo run --release --bin node-runtime
```

### Docker Deployment with LGTM Monitoring

```bash
# Build the Docker image
docker build -t quantum-chain:latest .

# Run with full LGTM monitoring stack
docker compose -f docker/docker-compose.yml --profile monitoring up

# Access monitoring dashboards:
# - Grafana: http://localhost:3000 (admin/admin)
# - Prometheus: http://localhost:9090
# - Tempo: http://localhost:3200
# - Loki: http://localhost:3100
```

### Verify Installation

```bash
# Run core subsystem tests
cargo test -p qc-01-peer-discovery -p qc-02-block-storage \
           -p qc-03-transaction-indexing -p qc-04-state-management \
           -p qc-05-block-propagation -p qc-06-mempool \
           -p qc-07-bloom-filters -p qc-08-consensus \
           -p qc-09-finality -p qc-10-signature-verification

# Run integration tests
cargo test -p integration-tests

# Run node-runtime tests
cargo test -p node-runtime
```

---

## Development

### Project Structure

```
Quantum-Chain/
├── Cargo.toml                    # Workspace root
├── Documentation/                # Master architecture documents
│   ├── Architecture.md          # V2.4 - Hybrid Architecture Spec
│   ├── System.md                # V2.4 - Subsystem Definitions
│   └── IPC-MATRIX.md            # V2.4 - Inter-Process Communication
├── SPECS/                        # Micro-level specifications
│   ├── SPEC-01-PEER-DISCOVERY.md
│   ├── SPEC-02-BLOCK-STORAGE.md
│   ├── SPEC-16-API-GATEWAY.md   # NEW: External API specification
│   └── ...
└── crates/                       # Rust library crates
    ├── node-runtime/            # Main binary (wiring layer)
    ├── shared-types/            # Common types
    ├── shared-bus/              # Event bus infrastructure
    ├── quantum-telemetry/       # LGTM observability
    ├── integration-tests/       # Cross-subsystem tests
    └── qc-XX-*/                  # Subsystem implementations
```

### Subsystem Architecture (Hexagonal)

```
crates/qc-XX-subsystem-name/
├── Cargo.toml
├── src/
│   ├── lib.rs                   # Public API exports
│   ├── domain/                  # Inner layer (pure logic)
│   │   ├── entities.rs          # Core domain objects
│   │   ├── services.rs          # Business logic
│   │   └── errors.rs            # Domain errors
│   ├── ports/                   # Middle layer (traits)
│   │   ├── inbound.rs           # Driving ports (API)
│   │   └── outbound.rs          # Driven ports (SPI)
│   ├── adapters/                # Outer layer
│   │   ├── ipc.rs               # IPC handler with auth
│   │   └── bus.rs               # Event bus adapter
│   ├── service.rs               # Application service
│   └── events.rs                # Event definitions
└── tests/
```

### Running Tests

```bash
# Run all tests
cargo test --all

# Run specific subsystem
cargo test -p qc-10-signature-verification

# Run with output
cargo test --all -- --nocapture

# Run clippy lints
cargo clippy --all -- -D warnings

# Check formatting
cargo fmt -- --check
```

---

## Security

### Defense in Depth

| Layer | Protection |
|-------|------------|
| **API Gateway** | Rate limiting, method whitelists, CORS |
| **Cryptographic** | ECDSA/BLS with EIP-2 malleability protection |
| **Constant-Time** | Side-channel resistant comparisons (`subtle`) |
| **Memory Safety** | Zeroization of sensitive buffers (`zeroize`) |
| **IPC Security** | HMAC-SHA256 authenticated messages |
| **Replay Prevention** | Time-bounded nonce cache (120s window) |
| **Rate Limiting** | Per-subsystem configurable limits |
| **Zero-Trust** | Independent signature re-verification |
| **Circuit Breaker** | Finality halt protection |

### Security Features by Subsystem

| Subsystem | Security Features |
|-----------|-------------------|
| **qc-16** | Rate limiting, method tiers, CORS, request validation |
| **qc-10** | Constant-time ops, EIP-2, zeroization, batch verification |
| **qc-08** | Zero-trust re-verification, PBFT signature validation |
| **qc-09** | Slashing detection, inactivity leak, circuit breaker |
| **qc-07** | Privacy-preserving filters, false positive tuning, filter rotation |
| **qc-05** | Signature verification at edge, rate limiting |
| **qc-06** | Signature validation before pool admission |

### Reporting Vulnerabilities

Please report security vulnerabilities responsibly via GitHub Security Advisories.

---

## Documentation

### Master Documents

| Document | Description |
|----------|-------------|
| [Architecture.md](Documentation/Architecture.md) | V2.4 Hybrid Architecture Specification |
| [System.md](Documentation/System.md) | Subsystem Definitions & Algorithms |
| [IPC-MATRIX.md](Documentation/IPC-MATRIX.md) | Inter-Process Communication Rules |

### Specifications

Each subsystem has a detailed specification in `SPECS/`:

- `SPEC-01-PEER-DISCOVERY.md` - Kademlia DHT
- `SPEC-02-BLOCK-STORAGE.md` - Storage engine
- `SPEC-07-BLOOM-FILTERS.md` - SPV transaction filtering
- `SPEC-08-CONSENSUS.md` - PoS/PBFT validation
- `SPEC-09-FINALITY.md` - Casper FFG
- `SPEC-10-SIGNATURE-VERIFICATION.md` - ECDSA/BLS
- **`SPEC-16-API-GATEWAY.md`** - External API (NEW)

---

## Contributing

### Getting Started

1. Read [Architecture.md](Documentation/Architecture.md)
2. Review [IPC-MATRIX.md](Documentation/IPC-MATRIX.md)
3. Pick a subsystem and read its SPEC
4. Write tests first (TDD)
5. Implement domain logic
6. Submit PR

### Pull Request Requirements

- [ ] All tests pass: `cargo test --all`
- [ ] No clippy warnings: `cargo clippy --all -- -D warnings`
- [ ] Code formatted: `cargo fmt`
- [ ] SPEC compliance verified

---

## License

This project is licensed under the [Unlicense](LICENSE).

---

## Acknowledgments

- **Domain-Driven Design:** Eric Evans
- **Hexagonal Architecture:** Alistair Cockburn
- **Casper FFG:** Vitalik Buterin, Virgil Griffith
- **Rust Ecosystem:** k256, blst, subtle, zeroize, axum, tower, jsonrpsee

---

**Version:** 0.5.0 | **Architecture:** V2.4 | **Last Updated:** 2025-12-06

**Status:** ✅ Production Ready (1180 tests passing)
