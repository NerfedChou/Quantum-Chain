# Running Quantum-Chain Locally

This guide explains how to run the Quantum-Chain node on your local system.

## Prerequisites

- Docker & Docker Compose (v2.0+)
- 4GB RAM minimum (8GB recommended)
- 10GB disk space

## Database Architecture

Quantum-Chain uses **RocksDB** (not PostgreSQL or Redis) for all persistent storage:

```
./data/rocksdb/        ← Block storage (headers, bodies, tx locations)
./data/state_db/       ← Account state (balances, nonces, contract storage)
```

### Why RocksDB?

- **Embedded**: No network overhead, runs in-process
- **Fast**: 10-100x faster than SQL databases for key-value operations
- **Atomic**: Batch writes are all-or-nothing (critical for blockchain)
- **Battle-tested**: Used by Ethereum (geth), Solana, Cosmos, etc.

### What Gets Stored

| Column Family | Contents |
|---------------|----------|
| `blocks` | Block headers and bodies |
| `state` | Account balances, nonces (Patricia Merkle Trie) |
| `tx_index` | Transaction → Block location mapping |
| `metadata` | Chain height, finalized block, etc. |

**Account balances are stored in the state database** and managed by the Patricia Merkle Trie. Each account has:
- `balance` (u128) - The account's coin balance
- `nonce` (u64) - Transaction counter
- `code_hash` - For smart contracts
- `storage_root` - Contract storage trie root

## Quick Start

### Option 1: Monolithic Node (Production-like)

```bash
# Build the Docker image
cd docker
docker compose build quantum-chain

# Run the node
docker compose up quantum-chain
```

This starts a single container with all 15 subsystems compiled into one binary.

**Ports:**
- `30303/tcp+udp` - P2P Discovery & Block Propagation
- `8545/tcp` - JSON-RPC API
- `8546/tcp` - WebSocket API

### Option 2: Development Stack with Redis Event Bus

```bash
# Start with Redis event bus for subsystem isolation testing
cd docker
docker compose --profile dev up
```

This starts:
- Redis (event bus for choreography)
- Individual subsystem containers (for debugging)

### Option 3: Full LGTM Monitoring Stack

```bash
# Start with full observability (Loki + Grafana + Tempo + Prometheus)
cd docker
docker compose --profile monitoring up
```

This starts the **LGTM Stack**:

| Service | Port | Purpose | URL |
|---------|------|---------|-----|
| **Grafana** | 3000 | Dashboards | http://localhost:3000 (admin/quantum) |
| **Prometheus** | 9090 | Metrics | http://localhost:9090 |
| **Loki** | 3100 | Logs | (query via Grafana) |
| **Tempo** | 3200 | Traces | (query via Grafana) |

#### What Each Tool Does

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         LGTM STACK OVERVIEW                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  LOKI (Logs)         When Subsystem #9 crashes, Loki tells you WHY         │
│  ─────────────       {"level":"ERROR","subsystem":"finality",               │
│                       "msg":"Circuit breaker triggered"}                    │
│                                                                             │
│  GRAFANA (Graphics)  The unified dashboard where you see everything         │
│  ─────────────────   CPU, memory, block height, errors - all in one place  │
│                                                                             │
│  TEMPO (Traces)      Track a transaction across ALL 15 subsystems          │
│  ─────────────       RPC → Sig(10) → Mempool(6) → Consensus(8) → ...       │
│                      See exactly where time is spent (bottleneck finder)   │
│                                                                             │
│  PROMETHEUS (Metrics) Numbers over time: peers_connected, blocks_total     │
│  ───────────────────  "How many transactions per second last hour?"        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### Example Queries

**Loki (Logs):**
```logql
# Find all errors in finality subsystem
{subsystem="finality"} |= "ERROR"

# Find circuit breaker events
{job="quantum-chain"} |~ "circuit.?breaker"
```

**Tempo (Traces):**
```
# Find slow transactions (>500ms)
{ duration > 500ms }

# Find traces through consensus
{ resource.service.name = "qc-08-consensus" }
```

**Prometheus (Metrics):**
```promql
# Blocks per minute
rate(qc_consensus_blocks_validated_total[1m]) * 60

# Mempool size
qc_mempool_transactions_pending
```

## Running Without Docker

If you want to run directly on your system:

```bash
# Build release binary with RocksDB support
cargo build --release -p node-runtime --features rocksdb

# Run the node
./target/release/node-runtime

# Or with custom config
QC_P2P_PORT=30303 QC_RPC_PORT=8545 ./target/release/node-runtime
```

### Required System Dependencies (for RocksDB)

**Ubuntu/Debian:**
```bash
sudo apt-get install -y librocksdb-dev libsnappy-dev
```

**macOS:**
```bash
brew install rocksdb snappy
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `QC_P2P_PORT` | 30303 | P2P listening port |
| `QC_RPC_PORT` | 8545 | JSON-RPC port |
| `QC_WS_PORT` | 8546 | WebSocket port |
| `QC_DATA_DIR` | /var/quantum-chain/data | Data directory |
| `QC_LOG_LEVEL` | info | Log level (debug, info, warn, error) |
| `QC_HMAC_SECRET` | (random) | 32-byte hex-encoded HMAC secret |
| `QC_NETWORK` | testnet | Network name |

### Telemetry Environment Variables (LGTM Stack)

| Variable | Default | Description |
|----------|---------|-------------|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | http://tempo:4317 | OpenTelemetry collector (traces) |
| `OTEL_SERVICE_NAME` | quantum-chain | Service name in traces |
| `LOKI_ENDPOINT` | http://loki:3100 | Loki log aggregator |
| `QC_METRICS_PORT` | 9100 | Prometheus metrics port |

## CLI Commands

```bash
# Show version
./node-runtime --version

# Show help
./node-runtime --help

# Health check
./node-runtime health
```

## Architecture

The node implements the **V2.3 Choreography Pattern**:

```
Consensus(8) ──BlockValidated──→ Event Bus
                                     │
        ┌────────────────────────────┼────────────────────────────┐
        ↓                            ↓                            ↓
  TxIndexing(3)              StateMgmt(4)              BlockStorage(2)
        │                            │                    [Assembler]
        ↓                            ↓                        ↑
  MerkleRootComputed          StateRootComputed               │
        │                            │                        │
        └──────────────→ Event Bus ←──────────────────────────┘
                              │
                              ↓
                        BlockStored → Finality(9) → BlockFinalized
```

## Verification

After starting, verify the node is running:

```bash
# Check health
curl http://localhost:8545/health

# Check version (via CLI)
docker exec quantum-chain-node quantum-chain --version
```

## Data Persistence

Data is stored in Docker volumes:
- `quantum-chain-data` - Blockchain data (blocks, state)
- `quantum-chain-config` - Configuration files

To reset all data:
```bash
docker compose down -v
```

## Troubleshooting

**Container won't start:**
```bash
docker compose logs quantum-chain
```

**Check disk space:**
```bash
df -h /var/lib/docker
```

**Memory issues:**
```bash
# Increase Docker memory limit in Docker Desktop settings
# Or limit container memory:
docker compose up -d --scale quantum-chain=1 --memory=4g
```

## Test Coverage

Before running in production, verify all tests pass:

```bash
cargo test
# Expected: 731 tests passing
```

## Security Notes

The Docker image:
- Runs as non-root user (quantum:1000)
- Supports read-only root filesystem
- Drops all capabilities
- Has no shell (minimal attack surface)

For production, run with:
```bash
docker run -p 30303:30303 -p 8545:8545 \
  --security-opt=no-new-privileges:true \
  --cap-drop=ALL \
  --read-only \
  --tmpfs /tmp:rw,noexec,nosuid,size=64m \
  quantum-chain:latest
```
