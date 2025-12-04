# Quantum-Chain

**A Production-Ready Modular Blockchain System with Quantum-Inspired Architecture**

[![Rust](https://img.shields.io/badge/rust-stable%20(1.85%2B)-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-Unlicense-blue.svg)](LICENSE)
[![Architecture](https://img.shields.io/badge/architecture-v2.3-green.svg)](Documentation/Architecture.md)
[![Tests](https://img.shields.io/badge/tests-651%20passing-brightgreen.svg)](#test-coverage)
[![CI](https://github.com/NerfedChou/Quantum-Chain/actions/workflows/rust.yml/badge.svg)](https://github.com/NerfedChou/Quantum-Chain/actions/workflows/rust.yml)

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Subsystems](#subsystems)
4. [Test Coverage](#test-coverage)
5. [Quick Start](#quick-start)
6. [Development](#development)
7. [Security](#security)
8. [Documentation](#documentation)
9. [Contributing](#contributing)

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
| Core Subsystems (1-6, 8-10) | ✅ Production Ready | 432 |
| Integration Tests | ✅ All Passing | 219 |
| Node Runtime Wiring | ✅ Complete | 34 |
| **Total** | **✅ Ready** | **651** |

---

## Architecture

### System Topology

Quantum-Chain is architected as a **fortress of isolated subsystems**, each representing a distinct business capability (Bounded Context):

```
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
│  │ Mempool │   │Consensus│   │Finality │   │ Sig     │   │  Node   │       │
│  │   (6)   │   │   (8)   │   │   (9)   │   │ Ver(10) │   │ Runtime │       │
│  └─────────┘   └─────────┘   └─────────┘   └─────────┘   └─────────┘       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### V2.3 Choreography Pattern

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
| **IPC Security** | HMAC-SHA256 authenticated envelopes | `shared-bus` |
| **Replay Prevention** | Time-bounded nonce cache (120s) | `TimeBoundedNonceCache` |
| **Zero-Trust** | Signatures re-verified at Consensus & Finality | `qc-08`, `qc-09` |
| **Side-Channel** | Constant-time cryptographic operations | `subtle` crate |
| **Memory Safety** | Zeroization of sensitive data | `zeroize` crate |
| **Malleability** | EIP-2 low-S enforcement | `qc-10` |

---

## Subsystems

### Core Subsystems (Production Ready)

| ID | Crate | Description | Tests | Status |
|----|-------|-------------|-------|--------|
| 1 | `qc-01-peer-discovery` | Kademlia DHT, DDoS defense | 74 | ✅ |
| 2 | `qc-02-block-storage` | Choreography assembler, atomic writes | 62 | ✅ |
| 3 | `qc-03-transaction-indexing` | Merkle trees, inclusion proofs | 36 | ✅ |
| 4 | `qc-04-state-management` | Patricia Merkle Trie | 22 | ✅ |
| 5 | `qc-05-block-propagation` | Gossip protocol, compact blocks | 33 | ✅ |
| 6 | `qc-06-mempool` | Priority queue, two-phase commit | 84 | ✅ |
| 8 | `qc-08-consensus` | PoS/PBFT, 2/3 attestation threshold | 29 | ✅ |
| 9 | `qc-09-finality` | Casper FFG, slashing, circuit breaker | 32 | ✅ |
| 10 | `qc-10-signature-verification` | ECDSA/BLS, batch verification | 60 | ✅ |

### Infrastructure

| Crate | Purpose | Status |
|-------|---------|--------|
| `shared-types` | Common types (Hash, Address, Signature, SubsystemId) | ✅ |
| `shared-bus` | HMAC-authenticated event bus, nonce cache | ✅ |
| `node-runtime` | Application binary, subsystem wiring | ✅ |
| `integration-tests` | End-to-end exploit & choreography tests | ✅ |

### Future Subsystems

| ID | Name | Status |
|----|------|--------|
| 7 | Bloom Filters | Planned |
| 11-15 | Advanced (Sharding, Cross-chain, etc.) | Planned |

---

## Test Coverage

### Summary (December 2025)

```
┌────────────────────────────────────────────────────────────────┐
│                    TEST RESULTS: 651 PASSING                   │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│  Core Subsystems (Unit Tests)                                  │
│  ├── qc-01-peer-discovery ................ 74 tests ✅        │
│  ├── qc-02-block-storage ................. 62 tests ✅        │
│  ├── qc-03-transaction-indexing .......... 36 tests ✅        │
│  ├── qc-04-state-management .............. 22 tests ✅        │
│  ├── qc-05-block-propagation ............. 33 tests ✅        │
│  ├── qc-06-mempool ....................... 84 tests ✅        │
│  ├── qc-08-consensus ..................... 29 tests ✅        │
│  ├── qc-09-finality ...................... 32 tests ✅        │
│  └── qc-10-signature-verification ........ 60 tests ✅        │
│                                                                │
│  Integration Tests                                             │
│  └── integration-tests .................. 219 tests ✅        │
│                                                                │
│  Node Runtime                                                  │
│  └── node-runtime ....................... 34 tests ✅         │
│                                                                │
│  TOTAL: 651 tests passing                                      │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

### Test Categories

| Category | Coverage | Description |
|----------|----------|-------------|
| **Unit Tests** | 432 | Domain logic, ports, services |
| **Integration Tests** | 219 | Cross-subsystem flows, exploit scenarios |
| **Runtime Tests** | 34 | Wiring, event routing, authorization |
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

# Run all tests (651 tests)
cargo test --all

# Run the node
cargo run --release --bin node-runtime
```

### Docker Deployment

```bash
# Build the Docker image
docker build -t quantum-chain:latest .

# Run with monitoring
docker compose -f docker/docker-compose.yml --profile monitoring up
```

### Verify Installation

```bash
# Run core subsystem tests
cargo test -p qc-01-peer-discovery -p qc-02-block-storage \
           -p qc-03-transaction-indexing -p qc-04-state-management \
           -p qc-05-block-propagation -p qc-06-mempool \
           -p qc-08-consensus -p qc-09-finality -p qc-10-signature-verification

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
│   ├── Architecture.md          # V2.3 - Hybrid Architecture Spec
│   ├── System.md                # V2.3 - Subsystem Definitions
│   └── IPC-MATRIX.md            # V2.3 - Inter-Process Communication
├── SPECS/                        # Micro-level specifications
│   ├── SPEC-01-PEER-DISCOVERY.md
│   ├── SPEC-02-BLOCK-STORAGE.md
│   └── ...
└── crates/                       # Rust library crates
    ├── node-runtime/            # Main binary (wiring layer)
    ├── shared-types/            # Common types
    ├── shared-bus/              # Event bus infrastructure
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
| **qc-10** | Constant-time ops, EIP-2, zeroization, batch verification |
| **qc-08** | Zero-trust re-verification, PBFT signature validation |
| **qc-09** | Slashing detection, inactivity leak, circuit breaker |
| **qc-05** | Signature verification at edge, rate limiting |
| **qc-06** | Signature validation before pool admission |

### Reporting Vulnerabilities

Please report security vulnerabilities responsibly via GitHub Security Advisories.

---

## Documentation

### Master Documents

| Document | Description |
|----------|-------------|
| [Architecture.md](Documentation/Architecture.md) | V2.3 Hybrid Architecture Specification |
| [System.md](Documentation/System.md) | Subsystem Definitions & Algorithms |
| [IPC-MATRIX.md](Documentation/IPC-MATRIX.md) | Inter-Process Communication Rules |

### Specifications

Each subsystem has a detailed specification in `SPECS/`:

- `SPEC-01-PEER-DISCOVERY.md` - Kademlia DHT
- `SPEC-02-BLOCK-STORAGE.md` - Storage engine
- `SPEC-08-CONSENSUS.md` - PoS/PBFT validation
- `SPEC-09-FINALITY.md` - Casper FFG
- `SPEC-10-SIGNATURE-VERIFICATION.md` - ECDSA/BLS

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
- **Rust Ecosystem:** k256, blst, subtle, zeroize

---

**Version:** 0.3.0 | **Architecture:** V2.3 | **Last Updated:** 2025-12-04

**Status:** ✅ Production Ready (651 tests passing)
