<div align="center">

# ⚛️ Quantum-Chain

### A Modular Blockchain Built from First Principles

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-Unlicense-blue?style=flat-square)](LICENSE)
[![Tests](https://img.shields.io/badge/Tests-1000+-brightgreen?style=flat-square)](#-test-results)
[![Architecture](https://img.shields.io/badge/Architecture-V2.4-purple?style=flat-square)](Documentation/Architecture.md)

**Event-Driven • Hexagonal Architecture • Zero-Trust Security • RocksDB Persistence**

[Getting Started](#-quick-start) •
[Architecture](#-architecture) •
[Subsystems](#-subsystems) •
[Docker](#-docker-deployment) •
[Monitoring](#-monitoring)

</div>

---

## 📋 Table of Contents

- [Overview](#-overview)
- [Key Features](#-key-features)
- [Architecture](#-architecture)
- [Subsystems](#-subsystems)
- [Quick Start](#-quick-start)
- [Docker Deployment](#-docker-deployment)
- [Monitoring](#-monitoring)
- [Data Persistence](#-data-persistence)
- [Event Flow](#-event-flow)
- [API Reference](#-api-reference)
- [Development](#-development)
- [Testing](#-testing)
- [Security](#-security)
- [Documentation](#-documentation)
- [License](#-license)

---

## 🌟 Overview

Quantum-Chain is a **ground-up blockchain implementation** written in Rust. It's not a fork—every line of code was written to understand and demonstrate how blockchains actually work.

```
┌─────────────────────────────────────────────────────────────────┐
│                    QUANTUM-CHAIN NODE                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   ┌─────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐        │
│   │  Block  │   │Consensus│   │ Mempool │   │  State  │        │
│   │Producer │──▶│  (PoW)  │──▶│         │──▶│ Manager │        │
│   │ (QC-17) │   │ (QC-08) │   │ (QC-06) │   │ (QC-04) │        │
│   └─────────┘   └─────────┘   └─────────┘   └─────────┘        │
│        │             │             │             │              │
│        └─────────────┴─────────────┴─────────────┘              │
│                          │                                      │
│                    ┌─────▼─────┐                                │
│                    │ Event Bus │  ◀── HMAC Authenticated        │
│                    └─────┬─────┘                                │
│                          │                                      │
│   ┌─────────┐   ┌────────▼──┐   ┌─────────┐   ┌─────────┐      │
│   │  Block  │   │   Block   │   │   Tx    │   │ Finality│      │
│   │ Storage │◀──│  Indexing │   │  Index  │   │ (QC-09) │      │
│   │ (QC-02) │   │  (QC-03)  │   │ (QC-03) │   └─────────┘      │
│   └────┬────┘   └───────────┘   └─────────┘                    │
│        │                                                        │
│   ┌────▼────┐                                                   │
│   │ RocksDB │  ◀── Persistent Storage                          │
│   └─────────┘                                                   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## ✨ Key Features

| Feature | Description |
|---------|-------------|
| **🧱 Modular Architecture** | 12 independent subsystems communicating via event bus |
| **⛏️ Proof of Work Mining** | SHA-256 based mining with adjustable difficulty |
| **💾 RocksDB Persistence** | Production-grade storage that survives restarts |
| **🔐 Zero-Trust Security** | HMAC-authenticated IPC, replay prevention |
| **📊 Real-Time Monitoring** | Grafana dashboards, Prometheus metrics, Loki logs |
| **🐳 Docker Ready** | One command deployment with persistence |
| **🔍 Event Flow Logging** | See exactly how blocks flow through the system |

---

## 🏗 Architecture

### Design Principles

```rust
// The Four Laws of Quantum-Chain
RULE #1: Subsystems have ZERO knowledge of each other
RULE #2: Direct subsystem-to-subsystem calls are FORBIDDEN  
RULE #3: ALL communication goes through the Event Bus
RULE #4: Every message is HMAC-authenticated
```

### Hexagonal Architecture

Each subsystem follows the **Ports & Adapters** pattern:

```
┌──────────────────────────────────────────────────────────┐
│                    SUBSYSTEM (e.g., QC-08)               │
├──────────────────────────────────────────────────────────┤
│                                                          │
│   ┌────────────────────────────────────────────────┐    │
│   │              DOMAIN (Pure Logic)               │    │
│   │  ┌──────────┐  ┌───────────┐  ┌────────────┐  │    │
│   │  │ Entities │  │ Services  │  │   Errors   │  │    │
│   │  └──────────┘  └───────────┘  └────────────┘  │    │
│   └────────────────────────────────────────────────┘    │
│                          │                               │
│   ┌──────────────────────┴───────────────────────┐      │
│   │              PORTS (Interfaces)               │      │
│   │  ┌─────────────────┐  ┌───────────────────┐  │      │
│   │  │  Inbound Port   │  │  Outbound Port    │  │      │
│   │  │ (what I offer)  │  │ (what I need)     │  │      │
│   │  └─────────────────┘  └───────────────────┘  │      │
│   └──────────────────────────────────────────────┘      │
│                          │                               │
│   ┌──────────────────────┴───────────────────────┐      │
│   │             ADAPTERS (Implementation)         │      │
│   │  ┌─────────────────┐  ┌───────────────────┐  │      │
│   │  │  IPC Adapter    │  │  Event Adapter    │  │      │
│   │  │ (handles msgs)  │  │ (publishes events)│  │      │
│   │  └─────────────────┘  └───────────────────┘  │      │
│   └──────────────────────────────────────────────┘      │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

---

## 🔧 Subsystems

### Active Subsystems

| ID | Name | Purpose | Status |
|----|------|---------|--------|
| **QC-01** | Peer Discovery | Kademlia DHT, node discovery | ✅ Active |
| **QC-02** | Block Storage | RocksDB persistence, atomic writes | ✅ Active |
| **QC-03** | Transaction Indexing | Merkle trees, tx lookups | ✅ Active |
| **QC-04** | State Management | Account balances, state root | ✅ Active |
| **QC-05** | Block Propagation | Gossip protocol | ✅ Active |
| **QC-06** | Mempool | Transaction pool, priority queue | ✅ Active |
| **QC-07** | Bloom Filters | SPV support, fast filtering | ✅ Active |
| **QC-08** | Consensus | PoW validation, block verification | ✅ Active |
| **QC-09** | Finality | Block finalization, checkpoints | ✅ Active |
| **QC-10** | Signature Verification | ECDSA/BLS, batch verification | ✅ Active |
| **QC-16** | API Gateway | JSON-RPC, REST, WebSocket | ✅ Active |
| **QC-17** | Block Production | PoW mining, coinbase creation | ✅ Active |

### Subsystem Communication Flow

```
Block Lifecycle:

  ┌─────────────────────────────────────────────────────────────┐
  │                                                             │
  │   QC-17 ──▶ QC-08 ──▶ QC-03 ──▶ QC-04 ──▶ QC-02 ──▶ QC-09  │
  │   Mine      Validate   Index     State     Store    Finalize│
  │                                                             │
  └─────────────────────────────────────────────────────────────┘

  [17:32:01] 🔨 QC-17 BlockProduced     | block:#123 | hash:0x8a2c...
  [17:32:01] ✅ QC-08 BlockValidated    | block:#123 | valid:true
  [17:32:01] 🌳 QC-03 MerkleComputed    | block:#123 | root:0x7f3e...
  [17:32:01] 💾 QC-04 StateUpdated      | block:#123 | accounts:42
  [17:32:01] 📦 QC-02 BlockStored       | block:#123 | size:2.4KB
  [17:32:01] 🔒 QC-09 BlockFinalized    | block:#123 | checkpoint:true
```

---

## 🚀 Quick Start

### Prerequisites

- **Rust** 1.75+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- **Docker** & Docker Compose (for containerized deployment)
- **RocksDB** dependencies (auto-installed with Docker)

### Option 1: Run with Docker (Recommended)

```bash
# Clone the repository
git clone https://github.com/NerfedChou/Quantum-Chain.git
cd Quantum-Chain

# Start the node (production mode with RocksDB)
docker compose up --build

# Watch the event flow
./tools/event-flow-logger.sh
```

### Option 2: Build from Source

```bash
# Clone and build
git clone https://github.com/NerfedChou/Quantum-Chain.git
cd Quantum-Chain

# Build with RocksDB support
cargo build --release --features rocksdb

# Run the node
./target/release/node-runtime --data-dir ./data
```

### Option 3: Development Mode

```bash
# Build and run with hot reload
cargo run --bin node-runtime

# In another terminal, watch the logs
./tools/event-flow-logger.sh
```

---

## 🐳 Docker Deployment

### Production Deployment

```bash
# Build production image
docker build -t quantum-chain:latest .

# Run with persistent storage
docker compose up -d

# View logs
docker logs -f quantum-chain-node
```

### Development Deployment

```bash
# Run with local code mounted (for development)
docker compose -f docker-compose.yml -f docker/docker-compose.dev.yml up
```

### Docker Compose Configuration

```yaml
# docker-compose.yml
services:
  quantum-chain:
    build: .
    ports:
      - "8545:8545"   # JSON-RPC
      - "8546:8546"   # WebSocket
      - "30303:30303" # P2P
      - "9090:9090"   # Prometheus metrics
    volumes:
      - quantum-chain-data:/var/quantum-chain/data
    environment:
      - RUST_LOG=info
      - QC_MINING_ENABLED=true

volumes:
  quantum-chain-data:
```

---

## 📊 Monitoring

### Event Flow Logger

See exactly what's happening in your blockchain:

```bash
./tools/event-flow-logger.sh
```

**Output:**
```
═══════════════════════════════════════════════════════════════════════
   🔗 QUANTUM-CHAIN EVENT FLOW LOGGER
═══════════════════════════════════════════════════════════════════════

[18:32:01.234] 🔨 [QC-17] BlockProduced | block:#123 | hash:0x8a2c3f...
   └─ Nonce: 1847592 | Difficulty: 0x1d00ffff | Reward: 50 QC

[18:32:01.289] ✅ [QC-08] BlockValidated | block:#123 | 45ms
   └─ PoW: valid | Merkle: valid | Signatures: 0

[18:32:01.301] 🌳 [QC-03] MerkleComputed | block:#123 | 12ms
   └─ Transactions: 0 | Root: 0x7f3e4d2...

[18:32:01.390] 💾 [QC-04] StateUpdated | block:#123 | 89ms
   └─ Accounts modified: 1 | New balance: 50 QC

[18:32:01.546] 📦 [QC-02] BlockStored | block:#123 | 156ms
   └─ RocksDB write: success | Size: 847 bytes

[18:32:01.548] 🔒 [QC-09] BlockFinalized | block:#123
   └─ Checkpoint: #123 | Finality depth: 6
───────────────────────────────────────────────────────────────────────
Stats: Mining ⛏️  | Height: 123 | Hashrate: 1.2 KH/s | Peers: 0
```

### Grafana Dashboards

```bash
# Start with monitoring stack
docker compose --profile monitoring up -d

# Access dashboards:
# - Grafana:    http://localhost:3000 (admin/admin)
# - Prometheus: http://localhost:9090
# - Loki:       http://localhost:3100
```

### Available Metrics

| Metric | Description |
|--------|-------------|
| `qc_blocks_mined_total` | Total blocks mined |
| `qc_block_height` | Current chain height |
| `qc_mempool_size` | Pending transactions |
| `qc_peer_count` | Connected peers |
| `qc_mining_hashrate` | Current hashrate |

---

## 💾 Data Persistence

### How It Works

Quantum-Chain uses **RocksDB** for persistent storage:

```
/var/quantum-chain/data/
├── rocksdb/           # Block data, headers, indices
│   ├── 000051.sst     # Sorted String Tables
│   ├── MANIFEST-*     # Database manifest
│   └── CURRENT        # Current manifest pointer
└── state_db/          # Account state, balances
    ├── 000040.log     # Write-ahead log
    └── MANIFEST-*     # State manifest
```

### Persistence Behavior

| Scenario | Behavior |
|----------|----------|
| `docker compose down` | Data **persists** in Docker volume |
| `docker compose down -v` | Data **deleted** (removes volumes) |
| Container restart | Chain **resumes** from last block |
| Fresh start (no data) | Creates **genesis block** |

### Check Your Data

```bash
# See what's stored
sudo ls -la /var/lib/docker/volumes/quantum-chain-data/_data/

# Output:
# rocksdb/   <- Block storage
# state_db/  <- Account state
```

### Backup & Restore

```bash
# Backup
docker run --rm -v quantum-chain-data:/data -v $(pwd):/backup \
  alpine tar czf /backup/qc-backup.tar.gz /data

# Restore
docker run --rm -v quantum-chain-data:/data -v $(pwd):/backup \
  alpine tar xzf /backup/qc-backup.tar.gz -C /
```

---

## 🔌 API Reference

### JSON-RPC Endpoints (Port 8545)

```bash
# Get current block height
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'

# Get block by number
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_getBlockByNumber","params":["0x1", true],"id":1}'

# Get account balance
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_getBalance","params":["0x742d35Cc6634C0532925a3b844Bc9e7595f5bA21","latest"],"id":1}'
```

### Supported Methods

| Method | Description |
|--------|-------------|
| `eth_blockNumber` | Current block height |
| `eth_getBlockByNumber` | Get block by height |
| `eth_getBlockByHash` | Get block by hash |
| `eth_getBalance` | Account balance |
| `eth_sendRawTransaction` | Submit transaction |
| `eth_getTransactionByHash` | Get transaction |
| `qc_getMiningStatus` | Mining statistics |
| `qc_getSubsystemStatus` | Subsystem health |

---

## 🛠 Development

### Project Structure

```
Quantum-Chain/
├── Cargo.toml              # Workspace manifest
├── Dockerfile              # Production image
├── docker-compose.yml      # Docker orchestration
│
├── crates/                 # Rust crates
│   ├── node-runtime/       # Main binary
│   ├── shared-types/       # Common types
│   ├── shared-bus/         # Event bus
│   ├── qc-01-*/            # Subsystem implementations
│   ├── qc-02-*/
│   └── ...
│
├── Documentation/          # Architecture docs
│   ├── Architecture.md     # System design
│   ├── System.md           # Subsystem specs
│   └── IPC-MATRIX.md       # Communication rules
│
├── tools/                  # Utilities
│   └── event-flow-logger.sh
│
└── docker/                 # Docker configs
    └── monitoring/         # Grafana/Prometheus
```

### Adding a New Subsystem

1. Create the crate:
```bash
cargo new --lib crates/qc-XX-my-subsystem
```

2. Follow the hexagonal structure:
```
crates/qc-XX-my-subsystem/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── domain/
    │   ├── entities.rs
    │   ├── services.rs
    │   └── errors.rs
    ├── ports/
    │   ├── inbound.rs
    │   └── outbound.rs
    └── adapters/
        └── ipc.rs
```

3. Register in `Cargo.toml` workspace
4. Wire in `node-runtime`

---

## 🧪 Testing

### Run All Tests

```bash
# Full test suite (~1000 tests)
cargo test --all

# With output
cargo test --all -- --nocapture

# Specific subsystem
cargo test -p qc-08-consensus
```

### Test Results

```
┌────────────────────────────────────────────────────────────┐
│                  TEST RESULTS SUMMARY                      │
├────────────────────────────────────────────────────────────┤
│  integration-tests ..................... 281 tests ✅     │
│  qc-16-api-gateway ..................... 110 tests ✅     │
│  qc-06-mempool .......................... 91 tests ✅     │
│  qc-01-peer-discovery ................... 80 tests ✅     │
│  qc-02-block-storage .................... 66 tests ✅     │
│  qc-10-signature-verification ........... 60 tests ✅     │
│  qc-07-bloom-filters .................... 56 tests ✅     │
│  qc-17-block-production ................. 46 tests ✅     │
│  qc-03-transaction-indexing ............. 40 tests ✅     │
│  qc-05-block-propagation ................ 37 tests ✅     │
│  qc-09-finality ......................... 32 tests ✅     │
│  qc-08-consensus ........................ 29 tests ✅     │
│  qc-04-state-management ................. 22 tests ✅     │
│  node-runtime ........................... 37 tests ✅     │
│  shared-bus ............................. 13 tests ✅     │
│  shared-types ........................... 11 tests ✅     │
├────────────────────────────────────────────────────────────┤
│  TOTAL: 1000+ tests passing                               │
└────────────────────────────────────────────────────────────┘
```

---

## 🔐 Security

### Security Model

| Layer | Protection |
|-------|------------|
| **IPC** | HMAC-SHA256 authentication |
| **Replay** | Time-bounded nonce cache (120s) |
| **Crypto** | Constant-time operations (`subtle`) |
| **Memory** | Zeroization of secrets (`zeroize`) |
| **API** | Rate limiting, method whitelists |

### Threat Mitigations

| Threat | Mitigation |
|--------|------------|
| Replay attacks | Nonce cache with 120s TTL |
| Side-channel | Constant-time comparisons |
| Memory leaks | Automatic zeroization |
| DoS | Per-subsystem rate limits |
| Signature malleability | EIP-2 low-S enforcement |

---

## 📚 Documentation

| Document | Description |
|----------|-------------|
| [Architecture.md](Documentation/Architecture.md) | System design & patterns |
| [System.md](Documentation/System.md) | Subsystem specifications |
| [IPC-MATRIX.md](Documentation/IPC-MATRIX.md) | Event bus communication |
| [DATA-ARCHITECTURE.md](Documentation/DATA-ARCHITECTURE.md) | Storage design |
| [TELEMETRY.md](Documentation/TELEMETRY.md) | Monitoring setup |

---

## 📄 License

This project is released into the **public domain** under the [Unlicense](LICENSE).

You are free to copy, modify, publish, use, compile, sell, or distribute this software for any purpose, commercial or non-commercial.

---

<div align="center">

**Built with ❤️ and Rust**

[⬆ Back to Top](#-quantum-chain)

</div>
