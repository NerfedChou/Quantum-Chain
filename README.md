# Quantum-Chain

**A Modular Blockchain System with Quantum-Inspired Architecture**

[![Rust](https://img.shields.io/badge/rust-stable%20(1.85%2B)-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-Unlicense-blue.svg)](LICENSE)
[![Architecture](https://img.shields.io/badge/architecture-v2.3-green.svg)](Documentation/Architecture.md)
[![CI](https://github.com/NerfedChou/Quantum-Chain/actions/workflows/rust.yml/badge.svg)](https://github.com/NerfedChou/Quantum-Chain/actions/workflows/rust.yml)
[![Docker](https://github.com/NerfedChou/Quantum-Chain/actions/workflows/docker-publish.yml/badge.svg)](https://github.com/NerfedChou/Quantum-Chain/actions/workflows/docker-publish.yml)

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Subsystems](#subsystems)
4. [Quick Start](#quick-start)
5. [Development](#development)
6. [DevOps & Deployment](#devops--deployment)
7. [Documentation](#documentation)
8. [Security](#security)
9. [Contributing](#contributing)

---

## Overview

Quantum-Chain is a **modular blockchain system** built with Rust, implementing a hybrid architecture that combines:

- **Domain-Driven Design (DDD)** - Business logic as first-class citizens
- **Hexagonal Architecture** - Dependency inversion via Ports & Adapters
- **Event-Driven Architecture (EDA)** - Asynchronous, decoupled communication
- **Test-Driven Development (TDD)** - Design validated by tests first

### Key Design Principles

```
RULE #1: Libraries have ZERO knowledge of the binary/CLI/Docker
RULE #2: Direct subsystem-to-subsystem calls are FORBIDDEN
RULE #3: Implementation code CANNOT be written without tests first
RULE #4: All inter-subsystem communication via Shared Bus ONLY
```

---

## Architecture

### System Topology

Quantum-Chain is architected as a **fortress of isolated subsystems**, each representing a distinct business capability (Bounded Context). The system achieves:

- **Modularity:** Each subsystem is a standalone Rust library crate
- **Security:** Compartmentalized design prevents cascade failures
- **Maintainability:** Pure domain logic separated from infrastructure
- **Testability:** Test-driven development enforced at every layer

### Communication Pattern (V2.3 Choreography)

The system uses **event-driven choreography**, NOT centralized orchestration:

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
│                                    [Atomic Write]                           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Security Mandates (V2.3)

| Mandate | Description |
|---------|-------------|
| **Envelope-Only Identity** | Payloads MUST NOT contain identity fields; `sender_id` in envelope is sole truth |
| **Choreography Pattern** | No single subsystem "orchestrates" others |
| **Time-Bounded Nonce** | Replay prevention with bounded memory (120s window) |
| **Zero-Trust Verification** | Critical signatures re-verified independently |

---

## Subsystems

### Core Subsystems (Required)

| ID | Crate | Bounded Context | Status | Security |
|----|-------|-----------------|--------|----------|
| 1 | `qc-01-peer-discovery` | Network Topology | 🟢 Implemented | ✅ Shared IPC Security |
| 2 | `qc-02-block-storage` | Persistence | 🟢 Implemented | ✅ Stateful Assembler |
| 3 | `qc-03-transaction-indexing` | Data Retrieval | 🟢 Implemented | ✅ Merkle Proofs |
| 4 | `qc-04-state-management` | Account State | 🟢 Implemented | ✅ Patricia Trie |
| 5 | `qc-05-block-propagation` | Network Broadcast | 🟢 Implemented | ✅ Gossip Protocol |
| 6 | `qc-06-mempool` | Transaction Queue | 🟢 Implemented | ✅ Two-Phase Commit |
| 8 | `qc-08-consensus` | Agreement | 🟢 Implemented | ✅ Zero-Trust Sigs |
| 9 | `qc-09-finality` | Economic Security | 🟢 Implemented | ✅ Circuit Breaker |
| 10 | `qc-10-signature-verification` | Cryptography | 🟢 Implemented | ✅ ECDSA/BLS |

### Optional Subsystems (Advanced Features)

| ID | Crate | Bounded Context | Status |
|----|-------|-----------------|--------|
| 7 | `qc-07-bloom-filters` | Light Client Support | 🔴 Not Started |
| 11 | `qc-11-smart-contracts` | Programmability | 🔴 Not Started |
| 12 | `qc-12-transaction-ordering` | Parallel Execution | 🔴 Not Started |
| 13 | `qc-13-light-client-sync` | Resource Efficiency | 🔴 Not Started |
| 14 | `qc-14-sharding` | Horizontal Scaling | 🔴 Not Started |
| 15 | `qc-15-cross-chain` | Interoperability | 🔴 Not Started |

### Infrastructure Crates

| Crate | Purpose | Status |
|-------|---------|--------|
| `shared-types` | Common types (Hash, Address, Signature) | 🟢 Implemented |
| `shared-bus` | Event-driven communication (Choreography) | 🟢 Implemented |
| `node-runtime` | Application binary that wires everything together | 🟡 In Progress |

---

## Quick Start

### Prerequisites

- **Rust** stable toolchain (1.85+, required for edition2024 dependencies)
- **Cargo** (comes with Rust)
- **Docker** (optional, for containerized deployment)

> **Note:** This project runs on **stable Rust** (1.85+). The `edition2024` feature used by some dependencies (e.g., `base64ct`) requires Rust 1.85 or later. CI/CD pipelines use the `stable` toolchain.

### Build from Source

```bash
# Clone the repository
git clone https://github.com/NerfedChou/Quantum-Chain.git
cd Quantum-Chain

# Build all crates
cargo build --release

# Run tests
cargo test --all

# Run the node
cargo run --release --bin node-runtime
```

### Docker Deployment

```bash
# Build the Docker image
docker build -t quantum-chain:latest .

# Run the node
docker run -p 30303:30303 quantum-chain:latest
```

---

## Development

### Project Structure

```
Quantum-Chain/
├── Cargo.toml                    # Workspace root
├── Dockerfile                    # Production container
├── Documentation/                # Master architecture documents
│   ├── Architecture.md          # V2.3 - Hybrid Architecture Spec
│   ├── System.md                # V2.3 - Subsystem Definitions
│   └── IPC-MATRIX.md            # V2.3 - Inter-Process Communication
├── SPECS/                        # Micro-level specifications
│   ├── SPEC-01-PEER-DISCOVERY.md
│   ├── SPEC-02-BLOCK-STORAGE.md
│   └── ...
└── crates/                       # Rust library crates
    ├── node-runtime/            # Main binary
    ├── shared-types/            # Common types
    ├── qc-01-peer-discovery/    # Subsystem 1
    ├── qc-02-block-storage/     # Subsystem 2
    └── ...
```

### Crate Structure Template

Each subsystem follows this hexagonal architecture:

```
crates/qc-XX-subsystem-name/
├── Cargo.toml
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

### TDD Workflow

**ENFORCEMENT:** No implementation code without a failing test first.

```
Phase 1: RED    → Write a test that fails
Phase 2: GREEN  → Write MINIMUM code to pass the test
Phase 3: REFACTOR → Clean up while keeping tests green
```

### Running Tests

```bash
# Run all tests
cargo test --all

# Run tests for a specific subsystem
cargo test -p qc-01-peer-discovery

# Run tests with output
cargo test --all -- --nocapture

# Run clippy lints
cargo clippy --all -- -D warnings

# Check formatting
cargo fmt -- --check
```

---

## DevOps & Deployment

### CI/CD Pipeline

The project uses GitHub Actions for continuous integration:

| Workflow | Trigger | Actions |
|----------|---------|---------|
| `rust.yml` | Push/PR to main | Format, Build, Clippy, Test (unit + subsystem isolation), Docs |
| `docker-publish.yml` | Push/Tag/Schedule | Build Monolithic + Per-Subsystem, Push to GHCR, Sign with Cosign |

### Hybrid Container Architecture

Quantum-Chain supports **two deployment modes** to balance production efficiency with development flexibility:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    HYBRID DOCKER ARCHITECTURE (V2.3)                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                    MODE 1: MONOLITHIC (Production)                   │    │
│  │                                                                      │    │
│  │   ┌──────────────────────────────────────────────────────────────┐  │    │
│  │   │                quantum-chain:latest                          │  │    │
│  │   │  ┌────────┬────────┬────────┬────────┬────────┬────────────┐ │  │    │
│  │   │  │ SS-01  │ SS-02  │ SS-03  │  ...   │ SS-14  │   SS-15    │ │  │    │
│  │   │  └────────┴────────┴────────┴────────┴────────┴────────────┘ │  │    │
│  │   │               Single Binary (~50MB image)                    │  │    │
│  │   └──────────────────────────────────────────────────────────────┘  │    │
│  │   Use: Production nodes, validators                                 │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                 MODE 2: PER-SUBSYSTEM (Development)                  │    │
│  │                                                                      │    │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐       ┌──────────┐        │    │
│  │  │ qc-01-*  │  │ qc-02-*  │  │ qc-03-*  │  ...  │ qc-15-*  │        │    │
│  │  │ :dev     │  │ :dev     │  │ :dev     │       │ :dev     │        │    │
│  │  └────┬─────┘  └────┬─────┘  └────┬─────┘       └────┬─────┘        │    │
│  │       │             │             │                  │              │    │
│  │       └─────────────┴─────────────┴──────────────────┘              │    │
│  │                            │                                        │    │
│  │                    ┌───────▼───────┐                                │    │
│  │                    │   Event Bus   │ (Redis Streams)                │    │
│  │                    │   IPC Layer   │ (Unix Domain Sockets)          │    │
│  │                    └───────────────┘                                │    │
│  │   Use: Isolation testing, debugging, microservice deployment        │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Docker Deployment Commands

```bash
# ============================================================================
# MODE 1: MONOLITHIC (Production)
# ============================================================================

# Build the production image
docker build -t quantum-chain:latest .

# Run the node
docker run -d \
  --name quantum-chain \
  -p 30303:30303 \
  -p 8545:8545 \
  -v qc-data:/var/quantum-chain/data \
  quantum-chain:latest

# ============================================================================
# MODE 2: DOCKER COMPOSE (Development)
# ============================================================================

# Start monolithic node only
docker compose -f docker/docker-compose.yml up quantum-chain

# Start with development profile (individual subsystem containers)
docker compose -f docker/docker-compose.yml --profile dev up

# Start with monitoring (Prometheus + Grafana)
docker compose -f docker/docker-compose.yml --profile monitoring up

# Build specific subsystem for testing
docker build -f docker/Dockerfile.subsystem \
  --build-arg SUBSYSTEM_ID=08 \
  --build-arg SUBSYSTEM_NAME=consensus \
  -t quantum-chain/qc-08-consensus:dev .
```

### IPC Architecture (Per-Subsystem Mode)

When running in per-subsystem mode, inter-container communication follows **IPC-MATRIX.md**:

| Channel | Technology | Use Case |
|---------|-----------|----------|
| Event Bus | Redis Streams | Async events (BlockValidated, MerkleRootComputed) |
| Request/Response | gRPC | Sync calls with `correlation_id` pattern |
| Shared Memory | Unix Domain Sockets | High-performance local IPC |

```yaml
# docker/docker-compose.yml excerpt
services:
  event-bus:
    image: redis:7-alpine
    # All subsystems publish/subscribe events here
    
  qc-08-consensus:
    environment:
      QC_EVENT_BUS_URL: redis://event-bus:6379
      QC_EVENT_PUBLICATIONS: "BlockValidated"
      
  qc-02-block-storage:
    environment:
      QC_EVENT_SUBSCRIPTIONS: "BlockValidated,MerkleRootComputed,StateRootComputed"
```

### Why Hybrid Architecture?

| Aspect | Monolithic | Per-Subsystem |
|--------|-----------|---------------|
| **Latency** | ✅ In-process calls | ❌ Network overhead |
| **Debugging** | ❌ Harder to isolate | ✅ Test one component |
| **Deployment** | ✅ Single artifact | ❌ 15+ containers |
| **Resource Usage** | ✅ Shared memory | ❌ Per-container overhead |
| **Fault Isolation** | ❌ Process crash = all down | ✅ One container fails |
| **Development** | ❌ Full rebuild | ✅ Hot-reload one crate |

**Production:** Use monolithic for performance and simplicity.
**Development:** Use per-subsystem for isolation testing and debugging.

### Configuration

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

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `QC_LOG_LEVEL` | Logging verbosity | `info` |
| `QC_DATA_DIR` | Data directory | `/var/quantum-chain` |
| `QC_P2P_PORT` | P2P listening port | `30303` |
| `QC_RPC_PORT` | RPC API port | `8545` |

---

## Documentation

### Master Documents (Architecture)

| Document | Version | Description |
|----------|---------|-------------|
| [Architecture.md](Documentation/Architecture.md) | V2.3 | Hybrid Architecture Specification |
| [System.md](Documentation/System.md) | V2.3 | Subsystem Definitions & Algorithms |
| [IPC-MATRIX.md](Documentation/IPC-MATRIX.md) | V2.3 | Inter-Process Communication Rules |

### Micro Specifications (SPECS)

Each subsystem has a detailed specification in the `SPECS/` directory:

- `SPEC-01-PEER-DISCOVERY.md` - Kademlia DHT implementation
- `SPEC-02-BLOCK-STORAGE.md` - LSM Tree storage engine
- `SPEC-03-TRANSACTION-INDEXING.md` - Merkle tree proofs
- ... (see SPECS/ directory for complete list)

### Document Hierarchy

```
                    ┌─────────────────────┐
                    │   Architecture.md   │ ← Constitution
                    │      (V2.3)         │
                    └──────────┬──────────┘
                               │
          ┌────────────────────┼────────────────────┐
          ↓                    ↓                    ↓
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│   System.md     │  │ IPC-MATRIX.md   │  │  Data-Arch.md   │
│  (Subsystems)   │  │ (Firewall Rules)│  │ (Data Flows)    │
└────────┬────────┘  └────────┬────────┘  └────────┬────────┘
         │                    │                    │
         └────────────────────┼────────────────────┘
                              ↓
              ┌──────────────────────────────────┐
              │     SPEC-XX Documents            │
              │  (Micro-level Implementation)    │
              └──────────────────────────────────┘
```

---

## Security

### Defense in Depth (8 Layers)

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

### Key Security Features

| Feature | Implementation |
|---------|----------------|
| **Compartmentalization** | Each subsystem is isolated; breach cannot spread |
| **Zero-Trust** | Consensus/Finality re-verify all signatures |
| **Replay Prevention** | Time-bounded nonce cache (120s window) |
| **DDoS Defense** | Signature verification at network edge |
| **Finality Safety** | Circuit breaker prevents livelock |

### Reporting Vulnerabilities

Please report security vulnerabilities responsibly. See [SECURITY.md](SECURITY.md) for details.

---

## Contributing

### Getting Started

1. Read the [Architecture.md](Documentation/Architecture.md) document
2. Review the [IPC-MATRIX.md](Documentation/IPC-MATRIX.md) for communication rules
3. Pick a subsystem (start with #10 Signature Verification - no dependencies)
4. Read its SPEC document (or create one if missing)
5. Write tests first (TDD Phase 1: Red)
6. Implement domain logic (TDD Phase 2: Green)
7. Refactor (TDD Phase 3: Clean)

### Pull Request Process

1. Ensure all tests pass: `cargo test --all`
2. Run lints: `cargo clippy --all -- -D warnings`
3. Format code: `cargo fmt`
4. Update relevant documentation
5. Submit PR with clear description

### Code Style

- Follow Rust idioms and conventions
- Use meaningful names matching domain language
- Only comment code that needs clarification
- Keep functions small and focused

---

## License

This project is licensed under the [Unlicense](LICENSE) - see the LICENSE file for details.

---

## Acknowledgments

- **Domain-Driven Design:** Eric Evans
- **Hexagonal Architecture:** Alistair Cockburn
- **Event-Driven Architecture:** Martin Fowler
- **Rust Patterns:** The Rust community

---

**Version:** 0.2.0 | **Architecture:** V2.3 | **Last Updated:** 2024-12-03
