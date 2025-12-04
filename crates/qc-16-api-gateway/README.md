# QC-16 API Gateway

> **Subsystem 16** — External interface for JSON-RPC, WebSocket, and REST APIs

The API Gateway is the **single entry point** for all external interactions with the Quantum Chain blockchain. It exposes the standard Ethereum JSON-RPC API, WebSocket subscriptions for real-time events, and an admin API for node management.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           API GATEWAY (qc-16)                                │
├─────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                         │
│  │   HTTP/RPC  │  │  WebSocket  │  │    Admin    │                         │
│  │  Port 8545  │  │  Port 8546  │  │  Port 8080  │                         │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘                         │
│         │                │                │                                 │
│  ┌──────┴────────────────┴────────────────┴──────┐                         │
│  │              Tower Middleware Stack           │                         │
│  │  IpProtection → RateLimit → Validation →     │                         │
│  │  Auth → Timeout → Tracing                    │                         │
│  └────────────────────┬───────────────────────────┘                         │
│                       │                                                     │
│  ┌────────────────────┴───────────────────────┐                            │
│  │           Pending Request Store            │                            │
│  │     (Async-to-Sync Bridge via oneshot)     │                            │
│  └────────────────────┬───────────────────────┘                            │
│                       │                                                     │
│  ┌────────────────────┴───────────────────────┐                            │
│  │              IPC Handler                    │                            │
│  │        (Event Bus Integration)             │                            │
│  └────────────────────┬───────────────────────┘                            │
└───────────────────────┼─────────────────────────────────────────────────────┘
                        │
                   Event Bus
                        │
    ┌───────────────────┼───────────────────────┐
    ▼                   ▼                       ▼
qc-04-state      qc-02-block           qc-06-mempool
```

## Features

### Security Features

- **RLP Pre-Validation**: Validates transaction structure BEFORE sending to mempool (rejects garbage at the gate)
- **Per-IP Rate Limiting**: Token bucket algorithm with configurable limits
- **Method Tier Enforcement**: Public/Protected/Admin method classification
- **Request Size Limits**: Protects against payload DoS attacks
- **Batch Size Limits**: Maximum 100 requests per batch
- **WebSocket Message Limits**: 1MB max message size, rate limiting per connection
- **X-Forwarded-For Protection**: Only trusts configured proxy IPs
- **CORS Configuration**: Granular origin control

### API Tiers

| Tier | Authentication | Methods |
|------|----------------|---------|
| **Tier 1 (Public)** | None | `eth_*`, `web3_*`, `net_*` |
| **Tier 2 (Protected)** | API Key OR localhost | `txpool_*`, `admin_peers`, `admin_nodeInfo` |
| **Tier 3 (Admin)** | Localhost AND API Key | `admin_addPeer`, `admin_removePeer`, `debug_*` |

### Supported JSON-RPC Methods

#### Ethereum Methods (eth_*)
- `eth_chainId`, `eth_blockNumber`, `eth_gasPrice`
- `eth_getBalance`, `eth_getCode`, `eth_getStorageAt`
- `eth_getBlockByHash`, `eth_getBlockByNumber`
- `eth_getTransactionByHash`, `eth_getTransactionReceipt`
- `eth_getTransactionCount`, `eth_accounts`
- `eth_call`, `eth_estimateGas`, `eth_sendRawTransaction`
- `eth_getLogs`, `eth_getBlockReceipts`, `eth_feeHistory`
- `eth_syncing`, `eth_maxPriorityFeePerGas`
- `eth_subscribe`, `eth_unsubscribe` (WebSocket only)

#### Web3 Methods
- `web3_clientVersion`, `web3_sha3`

#### Net Methods
- `net_version`, `net_listening`, `net_peerCount`

#### TxPool Methods (Protected)
- `txpool_status`, `txpool_content`

#### Admin Methods (Admin)
- `admin_peers`, `admin_nodeInfo`, `admin_addPeer`, `admin_removePeer`

### WebSocket Subscriptions

```javascript
// Subscribe to new block headers
{"jsonrpc":"2.0","method":"eth_subscribe","params":["newHeads"],"id":1}

// Subscribe to pending transactions
{"jsonrpc":"2.0","method":"eth_subscribe","params":["newPendingTransactions"],"id":2}

// Subscribe to logs with filter
{"jsonrpc":"2.0","method":"eth_subscribe","params":["logs",{"address":"0x..."}],"id":3}

// Unsubscribe
{"jsonrpc":"2.0","method":"eth_unsubscribe","params":["0x1"],"id":4}
```

## Usage

### Basic Usage

```rust
use qc_16_api_gateway::{ApiGatewayService, GatewayConfig};
use std::sync::Arc;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create configuration
    let config = GatewayConfig::default();
    
    // Create IPC sender (connects to event bus)
    let ipc_sender: Arc<dyn IpcSender> = /* ... */;
    
    // Create and start gateway
    let mut gateway = ApiGatewayService::new(
        config,
        ipc_sender,
        PathBuf::from("/data/quantum-chain"),
    )?;
    
    gateway.start().await?;
    Ok(())
}
```

### Configuration

```toml
[api_gateway]
# HTTP/JSON-RPC server
[api_gateway.http]
host = "0.0.0.0"
port = 8545
enabled = true

# WebSocket server
[api_gateway.websocket]
host = "0.0.0.0"
port = 8546
enabled = true
max_connections_per_ip = 10
max_subscriptions_per_connection = 100

# Admin server (localhost only by default)
[api_gateway.admin]
host = "127.0.0.1"
port = 8080
enabled = true
api_key = "your-secret-key"  # Optional
allow_external = false       # DANGER if true

# Rate limiting
[api_gateway.rate_limit]
requests_per_second = 100
writes_per_second = 10
burst_size = 200
enabled = true

# Request limits
[api_gateway.limits]
max_request_size = 1048576   # 1MB
max_batch_size = 100
max_response_size = 10485760 # 10MB
max_log_block_range = 10000
max_log_results = 10000

# Timeouts
[api_gateway.timeouts]
default = "10s"
eth_call = "30s"
simple = "5s"
get_logs = "60s"

# Chain info
[api_gateway.chain]
chain_id = 1
network_name = "quantum-chain"
```

## Internal Communication

The API Gateway communicates with other subsystems via the Event Bus:

| JSON-RPC Method | Target Subsystem |
|-----------------|------------------|
| `eth_getBalance`, `eth_getCode`, `eth_getStorageAt` | qc-04-state-management |
| `eth_getBlock*`, `eth_blockNumber` | qc-02-block-storage |
| `eth_getTransaction*`, `eth_getLogs` | qc-03-transaction-indexing |
| `eth_sendRawTransaction`, `eth_gasPrice` | qc-06-mempool |
| `eth_call`, `eth_estimateGas` | qc-11-smart-contracts |
| `admin_peers`, `net_*` | qc-07-network |

### Async-to-Sync Bridge

JSON-RPC is request-response, but the Event Bus is asynchronous. The gateway uses a **Pending Request Store** to bridge this:

1. Generate `correlation_id`
2. Create `oneshot::channel`
3. Store sender in `HashMap<CorrelationId, OneShotSender>`
4. Publish event to bus
5. Await response on receiver
6. Response listener matches `correlation_id` and completes

## Security Considerations

### Transaction Validation

`eth_sendRawTransaction` performs **syntactic validation** before forwarding:

1. **Size Check**: 85 bytes ≤ size ≤ 128KB
2. **RLP Structure**: Valid RLP list with correct field count
3. **Transaction Type**: Legacy (0xc0+), EIP-2930 (0x01), EIP-1559 (0x02)
4. **Signature Recovery**: Recover sender address from signature
5. **EIP-2 Compliance**: Verify `s` value is in lower half of curve order

This prevents garbage from reaching the mempool and wasting subsystem resources.

### Rate Limiting

Per-IP rate limiting with token bucket:
- **Read requests**: 100/sec (default)
- **Write requests**: 10/sec (sendRawTransaction)
- **Burst allowance**: 200 tokens

### Admin API

- **Localhost only by default** (binds to 127.0.0.1)
- Optional API key requirement
- `allow_external = true` requires explicit opt-in (not recommended)

## Observability

### Metrics (Prometheus)

```prometheus
# Request counters
api_gateway_requests_total{method="eth_getBalance",status="success"}
api_gateway_requests_total{method="eth_sendRawTransaction",status="error"}

# Latency histograms
api_gateway_request_duration_seconds{method="eth_call"}

# Connection gauges
api_gateway_websocket_connections
api_gateway_pending_requests
```

### Health Endpoints

- `GET /health` - HTTP health check
- `GET /health` (admin) - Admin health check
- `GET /metrics` (admin) - Prometheus metrics
- `GET /pending` (admin) - Pending request stats

## Testing

```bash
# Run unit tests
cargo test -p qc-16-api-gateway

# Run with all features
cargo test -p qc-16-api-gateway --all-features

# Test specific module
cargo test -p qc-16-api-gateway domain::pending
```

## SPEC Reference

See [SPEC-16-API-GATEWAY.md](../../SPECS/SPEC-16-API-GATEWAY.md) for full specification.

## IPC Matrix

Per [IPC-MATRIX.md](../../Documentation/IPC-MATRIX.md):
- **Subsystem ID**: 16
- **Isolation Level**: CRITICAL (exposed to public)
- **Allowed Communications**: qc-02, qc-03, qc-04, qc-06, qc-07, qc-11

## License

MIT
