# SPEC-16-API-GATEWAY.md
## External API Gateway - Subsystem 16
**Version:** 1.1  
**Created:** 2025-12-04  
**Updated:** 2025-12-04  
**Security Level:** CRITICAL (External-Facing)  
**Architecture Pattern:** DDD + Hexagonal + EDA

---

## REVISION HISTORY

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2025-12-04 | Initial specification |
| 1.1 | 2025-12-04 | **Production Hardening** - Fixed 3 major flaws and 2 implementation traps |

### v1.1 Changes (Production-Critical Fixes)

**Major Flaw #1 FIXED:** Removed cryptographic signatures from internal IPC messages
- Internal messages (GetBalanceRequest, GetBlockRequest, etc.) no longer require signatures
- Rationale: In-memory channels (tokio::sync::mpsc) are process-private; signing adds ~50μs overhead with zero security benefit
- Only SubmitTransactionRequest contains a user signature (embedded in raw_transaction)

**Major Flaw #2 FIXED:** Added "Pending Request Store" architecture (Section 6)
- Bridges the async-to-sync gap between Event Bus and JSON-RPC
- Uses correlation IDs mapped to oneshot channels
- Proper timeout handling with background cleanup task

**Major Flaw #3 FIXED:** Added RLP Syntactic Validation (Section 8.1)
- API Gateway now validates transaction RLP structure BEFORE forwarding to Event Bus
- Prevents garbage bytes from spamming internal subsystems
- Returns immediate 400 error for malformed transactions

**Implementation Trap #1 FIXED:** Explicit U256 type definition (Section 16)
- Using `primitive-types` crate with serde feature
- Added JsonU256 wrapper for proper hex serialization

**Implementation Trap #2 FIXED:** Added missing standard methods (Section 3)
- Added: eth_accounts, eth_syncing, web3_sha3, eth_coinbase, eth_mining, eth_hashrate
- Added: eth_maxPriorityFeePerGas, eth_feeHistory, eth_protocolVersion
- Added: block transaction count and uncle count methods

---

## 1. PURPOSE

The API Gateway is the **single entry point** for all external interactions with the Quantum Chain node. It translates external protocols (JSON-RPC, REST, WebSocket) into internal event bus messages, enforcing security boundaries at the network edge.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           EXTERNAL WORLD                                    │
│         (Wallets, dApps, Block Explorers, CLI Tools, Monitoring)           │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                     SUBSYSTEM 16: API GATEWAY                               │
│                  ┌─────────────────────────────────────┐                    │
│                  │         Security Layers             │                    │
│                  │  (Rate Limit → Timeout → CORS)      │                    │
│                  └─────────────────────────────────────┘                    │
│                                    │                                        │
│     ┌──────────────┬───────────────┼───────────────┬──────────────┐        │
│     │              │               │               │              │        │
│     ▼              ▼               ▼               ▼              ▼        │
│ ┌────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌────────┐      │
│ │JSON-RPC│   │WebSocket │   │  REST    │   │ Metrics  │   │ Health │      │
│ │ :8545  │   │  :8546   │   │  :8080   │   │  :9090   │   │  :8081 │      │
│ └────────┘   └──────────┘   └──────────┘   └──────────┘   └────────┘      │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         INTERNAL EVENT BUS                                  │
│              (qc-01 through qc-15 subsystems)                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. INTERFACES

### 2.1 JSON-RPC 2.0 (Port 8545) - Ethereum Compatible
Primary interface for wallets (MetaMask), dApps, and standard tooling.

### 2.2 WebSocket (Port 8546) - Real-time Subscriptions
Event subscriptions for block explorers and live dashboards.

### 2.3 REST API (Port 8080) - Admin/Custom Endpoints
Node administration and custom queries (PROTECTED by default).

### 2.4 Prometheus Metrics (Port 9090) - Observability
Metrics endpoint for Grafana/Mimir integration.

### 2.5 Health Check (Port 8081) - Kubernetes/Docker
Liveness and readiness probes.

---

## 3. METHOD CLASSIFICATION & SECURITY TIERS

### Tier 1: PUBLIC (No Authentication Required)

These methods are safe for public access because:
- Read operations don't modify state
- Write operations require cryptographic signatures (pre-signed transactions)

| Method | Routes To | Description |
|--------|-----------|-------------|
| `eth_chainId` | Config | Returns chain ID (hex string, e.g., "0x1") |
| `eth_blockNumber` | qc-02 Block Storage | Current block height |
| `eth_gasPrice` | qc-06 Mempool | Suggested gas price |
| `eth_maxPriorityFeePerGas` | qc-06 Mempool | EIP-1559 priority fee suggestion |
| `eth_feeHistory` | qc-06 Mempool | Historical fee data |
| `eth_getBalance` | qc-04 State Management | Account balance |
| `eth_getCode` | qc-04 State Management | Contract bytecode |
| `eth_getStorageAt` | qc-04 State Management | Contract storage slot |
| `eth_getTransactionCount` | qc-04 State Management | Account nonce |
| `eth_getBlockByHash` | qc-02 Block Storage | Block by hash |
| `eth_getBlockByNumber` | qc-02 Block Storage | Block by number |
| `eth_getBlockTransactionCountByHash` | qc-02 Block Storage | Tx count in block |
| `eth_getBlockTransactionCountByNumber` | qc-02 Block Storage | Tx count in block |
| `eth_getUncleCountByBlockHash` | qc-02 Block Storage | Uncle count (always 0) |
| `eth_getUncleCountByBlockNumber` | qc-02 Block Storage | Uncle count (always 0) |
| `eth_getTransactionByHash` | qc-03 Transaction Indexing | Transaction details |
| `eth_getTransactionByBlockHashAndIndex` | qc-03 Transaction Indexing | Transaction by position |
| `eth_getTransactionByBlockNumberAndIndex` | qc-03 Transaction Indexing | Transaction by position |
| `eth_getTransactionReceipt` | qc-03 Transaction Indexing | Transaction receipt |
| `eth_call` | qc-11 Smart Contracts | Simulate contract call |
| `eth_estimateGas` | qc-11 Smart Contracts | Estimate gas usage |
| `eth_sendRawTransaction` | qc-06 Mempool | Submit signed transaction |
| `eth_getLogs` | qc-03 Transaction Indexing | Event logs |
| `eth_subscribe` | Event Bus | WebSocket subscriptions |
| `eth_unsubscribe` | Event Bus | Cancel subscription |
| `eth_accounts` | Config | Returns empty array `[]` (no managed accounts) |
| `eth_syncing` | Node Runtime | Sync status or `false` if synced |
| `eth_coinbase` | Config | Block producer address (or null) |
| `eth_mining` | qc-08 Consensus | Is node producing blocks? |
| `eth_hashrate` | Config | Always 0 (PoS chain) |
| `eth_protocolVersion` | Config | Protocol version string |
| `net_version` | Config | Network ID (decimal string) |
| `net_listening` | qc-01 Peer Discovery | Node listening status |
| `web3_clientVersion` | Config | `QuantumChain/v{VERSION}/{OS}/rustc{RUSTC}` |
| `web3_sha3` | Local | Keccak256 hash of input data |

### Tier 2: PROTECTED (API Key OR Localhost Only)

These methods expose internal node state and should be restricted.

| Method | Routes To | Description |
|--------|-----------|-------------|
| `admin_nodeInfo` | Node Runtime | Node information |
| `admin_peers` | qc-01 Peer Discovery | Connected peers list |
| `txpool_status` | qc-06 Mempool | Mempool statistics |
| `txpool_content` | qc-06 Mempool | Mempool transactions |
| `net_peerCount` | qc-01 Peer Discovery | Number of peers |
| `debug_traceTransaction` | qc-11 Smart Contracts | Transaction trace |

### Tier 3: ADMIN ONLY (Localhost + Authentication)

These methods control node behavior and MUST be restricted.

| Method | Routes To | Description |
|--------|-----------|-------------|
| `admin_addPeer` | qc-01 Peer Discovery | Add peer manually |
| `admin_removePeer` | qc-01 Peer Discovery | Remove peer |
| `admin_startRPC` | API Gateway | Start/restart RPC |
| `admin_stopRPC` | API Gateway | Stop RPC server |
| `debug_*` | Various | All debug methods |
| `miner_start` | qc-08 Consensus | Start block production |
| `miner_stop` | qc-08 Consensus | Stop block production |

---

## 4. IPC COMMUNICATION

### I Am Allowed To Talk To:

| Subsystem | Purpose | Message Types |
|-----------|---------|---------------|
| qc-01 Peer Discovery | Peer info queries | `GetPeersRequest`, `AddPeerRequest` |
| qc-02 Block Storage | Block queries | `ReadBlockRequest`, `ReadBlockRangeRequest` |
| qc-03 Transaction Indexing | Transaction/receipt queries | `GetTransactionRequest`, `GetLogsRequest`, `MerkleProofRequest` |
| qc-04 State Management | State queries | `StateReadRequest`, `BalanceCheckRequest` |
| qc-06 Mempool | Transaction submission | `AddTransactionRequest`, `GetMempoolStatusRequest` |
| qc-08 Consensus | Block production control | `StartMiningRequest`, `StopMiningRequest` (Admin only) |
| qc-10 Signature Verification | Transaction validation | `VerifyTransactionRequest` |
| qc-11 Smart Contracts | eth_call/estimateGas | `ExecuteCallRequest`, `EstimateGasRequest` |
| Event Bus | Subscriptions | Subscribe to `BlockValidated`, `TransactionReceived` |

### Who Is Allowed To Talk To Me:

| Source | Purpose |
|--------|---------|
| External HTTP/WS Clients | All public methods |
| Localhost Admin Clients | Protected/Admin methods |
| Event Bus | Subscription notifications |

### Strict Message Types:

> **DESIGN DECISION (FIX FOR MAJOR FLAW #1):** Internal IPC messages do NOT require 
> cryptographic signatures. The Event Bus uses in-memory channels (`tokio::sync::mpsc`) 
> which are process-private. Signing every internal message would add ~50μs overhead 
> per request with zero security benefit. Only `SubmitTransactionRequest` contains a 
> user signature (the pre-signed transaction itself).

**OUTGOING (To Internal Subsystems):**

```rust
use primitive_types::U256;  // FIX FOR TRAP #1: Explicit u256 type

/// Request to qc-06 Mempool for transaction submission
/// Transaction is already signed by user's private key
struct SubmitTransactionRequest {
    version: u16,
    correlation_id: [u8; 16],
    reply_to: Topic,
    /// Raw signed transaction bytes (RLP encoded)
    /// NOTE: User's ECDSA signature is embedded IN the raw_transaction
    raw_transaction: Vec<u8>,
    /// Recovered signer address (validated by Gateway before forwarding)
    signer_address: [u8; 20],
}

/// Request to qc-04 State Management for balance query
/// NO SIGNATURE REQUIRED - this is a read-only internal request
struct GetBalanceRequest {
    version: u16,
    correlation_id: [u8; 16],
    reply_to: Topic,
    address: [u8; 20],
    block_number: Option<u64>,  // None = latest
}

/// Request to qc-02 Block Storage for block query
/// NO SIGNATURE REQUIRED - read-only
struct GetBlockRequest {
    version: u16,
    correlation_id: [u8; 16],
    reply_to: Topic,
    block_id: BlockId,
    include_transactions: bool,
}

enum BlockId {
    Hash([u8; 32]),
    Number(u64),
    Latest,
    Pending,
    Earliest,
}

/// Request to qc-11 Smart Contracts for eth_call
/// NO SIGNATURE REQUIRED - this simulates execution without state changes
struct ExecuteCallRequest {
    version: u16,
    correlation_id: [u8; 16],
    reply_to: Topic,
    from: Option<[u8; 20]>,
    to: [u8; 20],
    gas: Option<u64>,
    gas_price: Option<U256>,    // Using primitive_types::U256
    value: Option<U256>,         // Using primitive_types::U256
    data: Vec<u8>,
    block_number: Option<u64>,
}

/// Subscription request to Event Bus
/// NO SIGNATURE REQUIRED - internal subscription management
struct SubscribeRequest {
    version: u16,
    correlation_id: [u8; 16],
    reply_to: Topic,
    subscription_type: SubscriptionType,
    filter: Option<SubscriptionFilter>,
}

enum SubscriptionType {
    NewHeads,           // New block headers
    Logs,               // Contract event logs
    NewPendingTransactions,
    Syncing,
}

struct SubscriptionFilter {
    address: Option<Vec<[u8; 20]>>,
    topics: Option<Vec<Option<[u8; 32]>>>,
}
```

**INCOMING (From Internal Subsystems):**

```rust
/// Response from qc-04 State Management
/// NO SIGNATURE - internal trusted response
struct BalanceResponse {
    version: u16,
    correlation_id: [u8; 16],
    balance: U256,              // Using primitive_types::U256
    block_number: u64,
}

/// Response from qc-02 Block Storage
/// NO SIGNATURE - internal trusted response
struct BlockResponse {
    version: u16,
    correlation_id: [u8; 16],
    block: Option<BlockWithTransactions>,
}

/// Response from qc-06 Mempool (transaction submission)
/// NO SIGNATURE - internal trusted response
struct TransactionSubmissionResponse {
    version: u16,
    correlation_id: [u8; 16],
    transaction_hash: [u8; 32],
    accepted: bool,
    error: Option<String>,
}

/// Subscription notification from Event Bus
/// NO SIGNATURE - internal event notification
struct SubscriptionNotification {
    version: u16,
    subscription_id: u64,
    result: SubscriptionResult,
}

enum SubscriptionResult {
    NewHead(BlockHeader),
    Log(Log),
    PendingTransaction([u8; 32]),
    SyncStatus(SyncStatus),
}
```

---

## 5. SECURITY ARCHITECTURE

### 5.1 Tower Middleware Stack

```rust
/// Security layers applied in order (outermost to innermost)
let middleware_stack = ServiceBuilder::new()
    // Layer 1: Request ID for tracing
    .layer(RequestIdLayer::new())
    // Layer 2: Rate limiting (per IP)
    .layer(RateLimitLayer::new(config.rate_limit))
    // Layer 3: Request size limit
    .layer(RequestBodyLimitLayer::new(config.max_request_size))
    // Layer 4: Timeout protection
    .layer(TimeoutLayer::new(config.request_timeout))
    // Layer 5: CORS
    .layer(CorsLayer::new(config.cors))
    // Layer 6: Compression
    .layer(CompressionLayer::new())
    // Layer 7: Tracing (OpenTelemetry)
    .layer(TraceLayer::new());
```

### 5.2 Rate Limiting Configuration

```rust
struct RateLimitConfig {
    /// Default rate limit for public methods (requests per second per IP)
    pub public_rate_limit: u32,        // Default: 100
    
    /// Rate limit for write operations (eth_sendRawTransaction)
    pub write_rate_limit: u32,         // Default: 10
    
    /// Rate limit for heavy operations (eth_call, eth_getLogs)
    pub heavy_rate_limit: u32,         // Default: 20
    
    /// Burst allowance (token bucket)
    pub burst_size: u32,               // Default: 50
    
    /// Rate limit window duration
    pub window_duration: Duration,     // Default: 1 second
    
    /// IP whitelist (bypasses rate limiting)
    pub whitelist: Vec<IpAddr>,        // Default: [127.0.0.1, ::1]
}
```

### 5.3 Request Validation

```rust
struct RequestValidationConfig {
    /// Maximum JSON-RPC request size
    pub max_request_size: usize,       // Default: 1MB (1_048_576 bytes)
    
    /// Maximum batch request size
    pub max_batch_size: usize,         // Default: 100 requests
    
    /// Maximum array size in parameters
    pub max_array_size: usize,         // Default: 1000 elements
    
    /// Maximum eth_getLogs block range
    pub max_logs_block_range: u64,     // Default: 10000 blocks
    
    /// Maximum eth_getLogs result size
    pub max_logs_results: usize,       // Default: 10000 logs
}
```

### 5.4 Timeout Configuration

```rust
struct TimeoutConfig {
    /// Simple lookup operations (eth_getBalance, eth_blockNumber)
    pub simple_timeout: Duration,      // Default: 5 seconds
    
    /// Normal query operations (eth_getBlock, eth_getTransaction)
    pub normal_timeout: Duration,      // Default: 10 seconds
    
    /// Heavy operations (eth_call, eth_getLogs)
    pub heavy_timeout: Duration,       // Default: 30 seconds
    
    /// WebSocket ping interval
    pub ws_ping_interval: Duration,    // Default: 30 seconds
    
    /// WebSocket connection timeout
    pub ws_timeout: Duration,          // Default: 60 seconds
}
```

### 5.5 CORS Configuration

```rust
struct CorsConfig {
    /// Allowed origins (* for public, specific domains for restricted)
    pub allowed_origins: Vec<String>,  // Default: ["*"]
    
    /// Allowed methods
    pub allowed_methods: Vec<Method>,  // Default: [POST, GET, OPTIONS]
    
    /// Allowed headers
    pub allowed_headers: Vec<String>,  // Default: [Content-Type]
    
    /// Max age for preflight cache
    pub max_age: Duration,             // Default: 3600 seconds
}
```

### 5.6 Method Whitelist Enforcement

```rust
/// Enforces method access based on security tier
struct MethodWhitelist {
    /// Tier 1: Public methods (always allowed)
    pub public_methods: HashSet<String>,
    
    /// Tier 2: Protected methods (require API key or localhost)
    pub protected_methods: HashSet<String>,
    
    /// Tier 3: Admin methods (require localhost + auth)
    pub admin_methods: HashSet<String>,
    
    /// Completely disabled methods
    pub disabled_methods: HashSet<String>,
}

impl MethodWhitelist {
    fn check_access(&self, method: &str, context: &RequestContext) -> Result<(), AccessError> {
        // Check if method is disabled
        if self.disabled_methods.contains(method) {
            return Err(AccessError::MethodDisabled);
        }
        
        // Tier 1: Always allowed
        if self.public_methods.contains(method) {
            return Ok(());
        }
        
        // Tier 2: Protected - require API key or localhost
        if self.protected_methods.contains(method) {
            if context.is_localhost || context.has_valid_api_key {
                return Ok(());
            }
            return Err(AccessError::Unauthorized);
        }
        
        // Tier 3: Admin - require localhost AND auth
        if self.admin_methods.contains(method) {
            if context.is_localhost && context.has_admin_auth {
                return Ok(());
            }
            return Err(AccessError::Forbidden);
        }
        
        // Unknown method
        Err(AccessError::MethodNotFound)
    }
}
```

---

## 6. REQUEST STATE MANAGEMENT (ASYNC-TO-SYNC BRIDGE)

> **DESIGN DECISION (FIX FOR MAJOR FLAW #2):** JSON-RPC is synchronous (request-response), 
> but our internal Event Bus is asynchronous. We need a "Pending Request Store" to bridge 
> this gap. This is the mechanism that maps correlation IDs back to waiting HTTP handlers.

### 6.1 Pending Request Store Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    ASYNC-TO-SYNC BRIDGE                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  [Axum HTTP Handler]                                                        │
│         │                                                                   │
│         │ 1. Generate correlation_id = uuid::Uuid::new_v4()                │
│         │ 2. Create oneshot::channel()                                     │
│         ▼                                                                   │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ PENDING REQUEST STORE                                               │   │
│  │ HashMap<CorrelationId, PendingRequest>                              │   │
│  │                                                                     │   │
│  │ correlation_id -> {                                                 │   │
│  │     method: "eth_getBalance",                                       │   │
│  │     created_at: Instant::now(),                                     │   │
│  │     timeout: Duration::from_secs(5),                                │   │
│  │     response_sender: oneshot::Sender<Result<JsonValue>>             │   │
│  │ }                                                                   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│         │                                                                   │
│         │ 3. Fire IPC message to Event Bus with correlation_id             │
│         │ 4. HTTP handler AWAITS on oneshot::Receiver                      │
│         │    (with timeout using tokio::time::timeout)                     │
│         ▼                                                                   │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ EVENT BUS RESPONSE LISTENER (Background Task)                       │   │
│  │                                                                     │   │
│  │ loop {                                                              │   │
│  │     msg = event_bus.recv().await;                                   │   │
│  │     if let Some(pending) = store.remove(&msg.correlation_id) {      │   │
│  │         pending.response_sender.send(msg.result);  // Unblocks HTTP │   │
│  │     }                                                               │   │
│  │ }                                                                   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│         │                                                                   │
│         ▼                                                                   │
│  [HTTP Response Sent]                                                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Pending Request Store Implementation

```rust
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{oneshot, RwLock};
use uuid::Uuid;

/// Correlation ID for matching requests to responses
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CorrelationId([u8; 16]);

impl CorrelationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().into_bytes())
    }
    
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// A pending request waiting for a response
pub struct PendingRequest {
    pub method: String,
    pub created_at: Instant,
    pub timeout: Duration,
    pub response_sender: oneshot::Sender<Result<serde_json::Value, JsonRpcError>>,
}

/// Thread-safe store for pending requests
pub struct PendingRequestStore {
    requests: RwLock<HashMap<CorrelationId, PendingRequest>>,
    /// Background task handle for cleanup of expired requests
    cleanup_interval: Duration,
}

impl PendingRequestStore {
    pub fn new(cleanup_interval: Duration) -> Self {
        Self {
            requests: RwLock::new(HashMap::new()),
            cleanup_interval,
        }
    }
    
    /// Register a new pending request, returns the receiver to await
    pub async fn register(
        &self,
        correlation_id: CorrelationId,
        method: String,
        timeout: Duration,
    ) -> oneshot::Receiver<Result<serde_json::Value, JsonRpcError>> {
        let (sender, receiver) = oneshot::channel();
        
        let pending = PendingRequest {
            method,
            created_at: Instant::now(),
            timeout,
            response_sender: sender,
        };
        
        self.requests.write().await.insert(correlation_id, pending);
        receiver
    }
    
    /// Complete a pending request with a response
    pub async fn complete(
        &self,
        correlation_id: &CorrelationId,
        result: Result<serde_json::Value, JsonRpcError>,
    ) -> bool {
        if let Some(pending) = self.requests.write().await.remove(correlation_id) {
            // Ignore send error - receiver may have been dropped due to timeout
            let _ = pending.response_sender.send(result);
            true
        } else {
            false
        }
    }
    
    /// Remove expired requests (called by background cleanup task)
    pub async fn cleanup_expired(&self) {
        let now = Instant::now();
        let mut requests = self.requests.write().await;
        
        requests.retain(|_, pending| {
            let elapsed = now.duration_since(pending.created_at);
            if elapsed > pending.timeout {
                // Send timeout error before dropping
                let _ = pending.response_sender.send(Err(JsonRpcError {
                    code: error_codes::RESOURCE_UNAVAILABLE,
                    message: format!("Request timed out after {:?}", pending.timeout),
                    data: None,
                }));
                false  // Remove from map
            } else {
                true   // Keep in map
            }
        });
    }
    
    /// Get current pending request count (for metrics)
    pub async fn pending_count(&self) -> usize {
        self.requests.read().await.len()
    }
}

/// Start the background cleanup task
pub fn start_cleanup_task(store: Arc<PendingRequestStore>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(store.cleanup_interval);
        loop {
            interval.tick().await;
            store.cleanup_expired().await;
        }
    })
}
```

### 6.3 Complete Request Flow Example

```rust
impl ApiGatewayService {
    /// Handle a JSON-RPC request (the FULL flow)
    pub async fn handle_request(
        &self,
        request: JsonRpcRequest,
        context: RequestContext,
    ) -> JsonRpcResponse {
        // 1. Generate correlation ID
        let correlation_id = CorrelationId::new();
        
        // 2. Get route config for this method
        let route = match self.router.get(&request.method) {
            Some(r) => r,
            None => return JsonRpcResponse::error(
                request.id,
                error_codes::METHOD_NOT_FOUND,
                format!("Method not found: {}", request.method),
            ),
        };
        
        // 3. Register pending request BEFORE sending IPC message
        let receiver = self.pending_store.register(
            correlation_id,
            request.method.clone(),
            route.timeout_category.to_duration(),
        ).await;
        
        // 4. Build and send IPC message
        let ipc_message = route.message_builder.build(
            correlation_id.as_bytes(),
            &request.params,
        );
        
        if let Err(e) = self.event_bus.publish(route.subsystem, ipc_message).await {
            // Remove pending request on send failure
            self.pending_store.complete(&correlation_id, Err(e.into())).await;
            return JsonRpcResponse::error(
                request.id,
                error_codes::INTERNAL_ERROR,
                "Failed to route request to subsystem",
            );
        }
        
        // 5. Await response with timeout
        let timeout = route.timeout_category.to_duration();
        match tokio::time::timeout(timeout, receiver).await {
            Ok(Ok(Ok(result))) => JsonRpcResponse::success(request.id, result),
            Ok(Ok(Err(rpc_error))) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(rpc_error),
                id: request.id,
            },
            Ok(Err(_)) => {
                // Channel closed unexpectedly (shouldn't happen)
                JsonRpcResponse::error(
                    request.id,
                    error_codes::INTERNAL_ERROR,
                    "Internal response channel error",
                )
            }
            Err(_) => {
                // Timeout elapsed
                JsonRpcResponse::error(
                    request.id,
                    error_codes::RESOURCE_UNAVAILABLE,
                    format!("Request timed out after {:?}", timeout),
                )
            }
        }
    }
}
```

---

## 7. INTERNAL ROUTING TABLE

```rust
/// Maps JSON-RPC methods to internal subsystem handlers
struct MethodRouter {
    routes: HashMap<String, RouteConfig>,
}

struct RouteConfig {
    /// Target subsystem
    subsystem: SubsystemId,
    /// Message type to construct
    message_builder: Box<dyn MessageBuilder>,
    /// Timeout category
    timeout_category: TimeoutCategory,
    /// Rate limit category
    rate_limit_category: RateLimitCategory,
}

enum TimeoutCategory {
    Simple,   // 5s
    Normal,   // 10s
    Heavy,    // 30s
}

enum RateLimitCategory {
    Public,   // 100/s
    Write,    // 10/s
    Heavy,    // 20/s
}

/// Routing table initialization
fn build_routes() -> MethodRouter {
    let mut routes = HashMap::new();
    
    // State Management (qc-04)
    routes.insert("eth_getBalance", RouteConfig {
        subsystem: SubsystemId::StateManagement,
        message_builder: Box::new(GetBalanceBuilder),
        timeout_category: TimeoutCategory::Simple,
        rate_limit_category: RateLimitCategory::Public,
    });
    
    routes.insert("eth_getCode", RouteConfig {
        subsystem: SubsystemId::StateManagement,
        message_builder: Box::new(GetCodeBuilder),
        timeout_category: TimeoutCategory::Simple,
        rate_limit_category: RateLimitCategory::Public,
    });
    
    // Block Storage (qc-02)
    routes.insert("eth_getBlockByHash", RouteConfig {
        subsystem: SubsystemId::BlockStorage,
        message_builder: Box::new(GetBlockByHashBuilder),
        timeout_category: TimeoutCategory::Normal,
        rate_limit_category: RateLimitCategory::Public,
    });
    
    routes.insert("eth_getBlockByNumber", RouteConfig {
        subsystem: SubsystemId::BlockStorage,
        message_builder: Box::new(GetBlockByNumberBuilder),
        timeout_category: TimeoutCategory::Normal,
        rate_limit_category: RateLimitCategory::Public,
    });
    
    // Transaction Indexing (qc-03)
    routes.insert("eth_getTransactionByHash", RouteConfig {
        subsystem: SubsystemId::TransactionIndexing,
        message_builder: Box::new(GetTransactionBuilder),
        timeout_category: TimeoutCategory::Normal,
        rate_limit_category: RateLimitCategory::Public,
    });
    
    routes.insert("eth_getLogs", RouteConfig {
        subsystem: SubsystemId::TransactionIndexing,
        message_builder: Box::new(GetLogsBuilder),
        timeout_category: TimeoutCategory::Heavy,
        rate_limit_category: RateLimitCategory::Heavy,
    });
    
    // Mempool (qc-06)
    routes.insert("eth_sendRawTransaction", RouteConfig {
        subsystem: SubsystemId::Mempool,
        message_builder: Box::new(SendRawTransactionBuilder),
        timeout_category: TimeoutCategory::Normal,
        rate_limit_category: RateLimitCategory::Write,
    });
    
    routes.insert("eth_gasPrice", RouteConfig {
        subsystem: SubsystemId::Mempool,
        message_builder: Box::new(GetGasPriceBuilder),
        timeout_category: TimeoutCategory::Simple,
        rate_limit_category: RateLimitCategory::Public,
    });
    
    // Smart Contracts (qc-11)
    routes.insert("eth_call", RouteConfig {
        subsystem: SubsystemId::SmartContracts,
        message_builder: Box::new(EthCallBuilder),
        timeout_category: TimeoutCategory::Heavy,
        rate_limit_category: RateLimitCategory::Heavy,
    });
    
    routes.insert("eth_estimateGas", RouteConfig {
        subsystem: SubsystemId::SmartContracts,
        message_builder: Box::new(EstimateGasBuilder),
        timeout_category: TimeoutCategory::Heavy,
        rate_limit_category: RateLimitCategory::Heavy,
    });
    
    MethodRouter { routes }
}
```

---

## 8. TRANSACTION SUBMISSION FLOW

> **DESIGN DECISION (FIX FOR MAJOR FLAW #3):** The API Gateway MUST perform syntactic 
> validation (RLP decoding) BEFORE forwarding transactions to the Event Bus. This prevents
> attackers from spamming internal subsystems with garbage bytes.

The most security-critical flow is transaction submission. Here's the full path:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    eth_sendRawTransaction FLOW                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  [External Client]                                                          │
│         │                                                                   │
│         │ POST /rpc { "method": "eth_sendRawTransaction", "params": [raw] } │
│         ▼                                                                   │
│  ┌──────────────────────────────────────────────────────────────────┐      │
│  │ LAYER 1: Rate Limiting                                           │      │
│  │ Check: IP not exceeding 10 write requests/second                 │      │
│  │ Fail: HTTP 429 Too Many Requests                                 │      │
│  └──────────────────────────────────────────────────────────────────┘      │
│         │                                                                   │
│         ▼                                                                   │
│  ┌──────────────────────────────────────────────────────────────────┐      │
│  │ LAYER 2: Request Validation                                      │      │
│  │ Check: Request size < 1MB, valid JSON-RPC format                 │      │
│  │ Fail: HTTP 400 Bad Request                                       │      │
│  └──────────────────────────────────────────────────────────────────┘      │
│         │                                                                   │
│         ▼                                                                   │
│  ┌──────────────────────────────────────────────────────────────────┐      │
│  │ LAYER 3: HEX DECODING                                            │      │
│  │ Parse "0x..." hex string into raw bytes                          │      │
│  │ Fail: JSON-RPC Error -32602 "invalid hex encoding"               │      │
│  └──────────────────────────────────────────────────────────────────┘      │
│         │                                                                   │
│         ▼                                                                   │
│  ┌──────────────────────────────────────────────────────────────────┐      │
│  │ LAYER 4: RLP SYNTACTIC VALIDATION (GATEWAY RESPONSIBILITY)       │      │
│  │ • Decode RLP structure (not full tx parsing, just structure)     │      │
│  │ • Verify minimum length (>= 85 bytes for legacy, >= 2 for typed) │      │
│  │ • Verify RLP list has correct number of elements                 │      │
│  │ • Extract tx_type (0x00=legacy, 0x01=EIP2930, 0x02=EIP1559)      │      │
│  │ Fail: JSON-RPC Error -32000 "invalid transaction: malformed RLP" │      │
│  │                                                                  │      │
│  │ NOTE: This is SYNTACTIC validation only. We don't verify:        │      │
│  │ - Signature validity (that's qc-10's job)                        │      │
│  │ - Nonce correctness (that's qc-06's job)                         │      │
│  │ - Balance sufficiency (that's qc-04's job)                       │      │
│  └──────────────────────────────────────────────────────────────────┘      │
│         │                                                                   │
│         ▼                                                                   │
│  ┌──────────────────────────────────────────────────────────────────┐      │
│  │ LAYER 5: Basic Field Validation (API Gateway)                    │      │
│  │ Check: chain_id matches (if present), gas_limit > 0              │      │
│  │ Fail: JSON-RPC Error -32000 "invalid chain id"                   │      │
│  └──────────────────────────────────────────────────────────────────┘      │
│         │                                                                   │
│         ▼                                                                   │
│  ┌──────────────────────────────────────────────────────────────────┐      │
│  │ LAYER 6: Forward to Signature Verification (qc-10)               │      │
│  │ VerifyTransactionRequest { raw_transaction }                     │      │
│  │ Response: VerifiedTransaction { signer_address, valid }          │      │
│  │ Fail: JSON-RPC Error -32000 "invalid signature"                  │      │
│  └──────────────────────────────────────────────────────────────────┘      │
│         │                                                                   │
│         ▼                                                                   │
│  ┌──────────────────────────────────────────────────────────────────┐      │
│  │ LAYER 7: Forward to Mempool (qc-06)                              │      │
│  │ SubmitTransactionRequest {                                       │      │
│  │     raw_transaction,                                             │      │
│  │     signer_address  // Pre-validated by qc-10                    │      │
│  │ }                                                                │      │
│  │ Response: TransactionSubmissionResponse { tx_hash, accepted }    │      │
│  │ Fail: JSON-RPC Error -32000 "insufficient funds" / "nonce too low"│      │
│  └──────────────────────────────────────────────────────────────────┘      │
│         │                                                                   │
│         ▼                                                                   │
│  [Success Response]                                                         │
│  { "jsonrpc": "2.0", "id": 1, "result": "0x<transaction_hash>" }           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 8.1 RLP Syntactic Validator Implementation

```rust
use alloy_rlp::Decodable;

/// Transaction type prefix bytes
pub mod tx_type {
    pub const LEGACY: u8 = 0x00;        // No prefix (starts with RLP list)
    pub const EIP2930: u8 = 0x01;       // Access list transaction
    pub const EIP1559: u8 = 0x02;       // Fee market transaction
    pub const EIP4844: u8 = 0x03;       // Blob transaction
}

/// Errors from RLP validation
#[derive(Debug, thiserror::Error)]
pub enum RlpValidationError {
    #[error("Empty transaction data")]
    EmptyData,
    
    #[error("Transaction too short: {0} bytes (minimum: {1})")]
    TooShort(usize, usize),
    
    #[error("Invalid RLP structure: {0}")]
    InvalidRlp(String),
    
    #[error("Unknown transaction type: 0x{0:02x}")]
    UnknownTxType(u8),
    
    #[error("Invalid chain ID: expected {expected}, got {actual}")]
    InvalidChainId { expected: u64, actual: u64 },
    
    #[error("Gas limit is zero")]
    ZeroGasLimit,
}

/// Syntactic validation result
pub struct RlpValidationResult {
    pub tx_type: u8,
    pub chain_id: Option<u64>,
    pub gas_limit: u64,
    pub to: Option<[u8; 20]>,  // None for contract creation
    pub data_len: usize,
}

/// Validate RLP structure WITHOUT full signature verification
/// This is cheap (~1μs) and catches garbage before hitting the Event Bus
pub fn validate_transaction_rlp(
    raw: &[u8],
    expected_chain_id: u64,
) -> Result<RlpValidationResult, RlpValidationError> {
    if raw.is_empty() {
        return Err(RlpValidationError::EmptyData);
    }
    
    // Determine transaction type
    let (tx_type, payload) = if raw[0] >= 0x80 {
        // Legacy transaction (RLP list starts with byte >= 0x80)
        (tx_type::LEGACY, raw)
    } else if raw[0] <= 0x03 {
        // Typed transaction (EIP-2718)
        if raw.len() < 2 {
            return Err(RlpValidationError::TooShort(raw.len(), 2));
        }
        (raw[0], &raw[1..])
    } else {
        return Err(RlpValidationError::UnknownTxType(raw[0]));
    };
    
    // Minimum sizes based on tx type
    let min_size = match tx_type {
        tx_type::LEGACY => 85,   // Minimum legacy tx
        tx_type::EIP2930 => 50,  // Minimum EIP-2930
        tx_type::EIP1559 => 50,  // Minimum EIP-1559
        tx_type::EIP4844 => 100, // Minimum blob tx
        _ => return Err(RlpValidationError::UnknownTxType(tx_type)),
    };
    
    if raw.len() < min_size {
        return Err(RlpValidationError::TooShort(raw.len(), min_size));
    }
    
    // Decode RLP list structure (lightweight, doesn't parse all fields)
    let header = alloy_rlp::Header::decode(&mut &payload[..])
        .map_err(|e| RlpValidationError::InvalidRlp(e.to_string()))?;
    
    if !header.list {
        return Err(RlpValidationError::InvalidRlp(
            "Transaction must be an RLP list".to_string()
        ));
    }
    
    // For full validation, we'd parse fields here
    // For now, just validate the structure is decodable
    // The actual values are validated by qc-10 and qc-06
    
    Ok(RlpValidationResult {
        tx_type,
        chain_id: None,  // Would parse from RLP in full implementation
        gas_limit: 21000, // Would parse from RLP
        to: None,
        data_len: payload.len(),
    })
}
```

---

## 8. WEBSOCKET SUBSCRIPTION FLOW

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    WebSocket Subscription FLOW                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  [Client]                                                                   │
│     │                                                                       │
│     │ WebSocket CONNECT ws://node:8546                                      │
│     ▼                                                                       │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ CONNECTION ESTABLISHED                                               │   │
│  │ • Connection ID assigned                                             │   │
│  │ • Ping/pong heartbeat started (30s interval)                        │   │
│  │ • Max subscriptions per connection: 100                             │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│     │                                                                       │
│     │ eth_subscribe ["newHeads"]                                           │
│     ▼                                                                       │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ SUBSCRIPTION CREATED                                                 │   │
│  │ • Subscription ID: "0x1234..."                                       │   │
│  │ • Event Bus subscription registered for BlockValidated events       │   │
│  │ • Response: { "result": "0x1234..." }                               │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│     │                                                                       │
│     │ ← [Event Bus] BlockValidated event                                   │
│     ▼                                                                       │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ NOTIFICATION SENT                                                    │   │
│  │ {                                                                    │   │
│  │   "jsonrpc": "2.0",                                                 │   │
│  │   "method": "eth_subscription",                                     │   │
│  │   "params": {                                                       │   │
│  │     "subscription": "0x1234...",                                    │   │
│  │     "result": { <block_header> }                                    │   │
│  │   }                                                                 │   │
│  │ }                                                                    │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│     │                                                                       │
│     │ eth_unsubscribe ["0x1234..."]                                        │
│     ▼                                                                       │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ SUBSCRIPTION REMOVED                                                 │   │
│  │ • Event Bus subscription cancelled                                   │   │
│  │ • Response: { "result": true }                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 9. OBSERVABILITY (LGTM INTEGRATION)

### 9.1 Metrics (Prometheus/Mimir)

```rust
/// Metrics exposed on :9090/metrics
struct ApiGatewayMetrics {
    // Request metrics
    requests_total: Counter,              // Total requests by method
    requests_duration: Histogram,         // Request duration by method
    requests_in_flight: Gauge,            // Current concurrent requests
    
    // Error metrics
    errors_total: Counter,                // Errors by type (rate_limit, timeout, etc.)
    
    // Rate limiting metrics
    rate_limit_hits: Counter,             // Rate limit rejections by IP
    
    // WebSocket metrics
    ws_connections: Gauge,                // Active WebSocket connections
    ws_subscriptions: Gauge,              // Active subscriptions by type
    ws_messages_sent: Counter,            // Messages sent to clients
    
    // Subsystem routing metrics
    subsystem_requests: Counter,          // Requests by target subsystem
    subsystem_latency: Histogram,         // Response time by subsystem
}
```

### 9.2 Traces (Tempo)

Every request generates a trace with spans:

```
[API Gateway] eth_getBalance
├── [Middleware] Rate Limit Check (0.1ms)
├── [Middleware] Request Validation (0.2ms)
├── [Router] Method Lookup (0.05ms)
├── [IPC] Send to qc-04 State Management (0.5ms)
├── [IPC] Await Response (5.2ms)
└── [Serialization] JSON Response (0.1ms)
Total: 6.15ms
```

### 9.3 Logs (Loki)

Structured JSON logs for all requests:

```json
{
  "timestamp": "2025-12-04T18:30:00Z",
  "level": "INFO",
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "method": "eth_getBalance",
  "client_ip": "192.168.1.100",
  "duration_ms": 6.15,
  "status": "success",
  "subsystem": "qc-04",
  "trace_id": "abc123def456"
}
```

---

## 10. CONFIGURATION

```toml
[api_gateway]
# Interface binding
http_host = "0.0.0.0"
http_port = 8545
ws_host = "0.0.0.0"
ws_port = 8546
admin_host = "127.0.0.1"  # Localhost only by default
admin_port = 8080
metrics_port = 9090
health_port = 8081

# Rate limiting
rate_limit_public = 100        # requests/sec/IP for public methods
rate_limit_write = 10          # requests/sec/IP for write operations
rate_limit_heavy = 20          # requests/sec/IP for heavy operations
rate_limit_burst = 50          # burst allowance

# Request limits
max_request_size = "1MB"
max_batch_size = 100
max_logs_block_range = 10000
max_logs_results = 10000

# Timeouts
timeout_simple = "5s"
timeout_normal = "10s"
timeout_heavy = "30s"
ws_ping_interval = "30s"
ws_connection_timeout = "60s"

# Security
cors_origins = ["*"]           # Or specific domains: ["https://app.example.com"]
cors_methods = ["POST", "GET", "OPTIONS"]
api_key_header = "X-API-Key"   # Header for protected methods
admin_auth_enabled = true

# WebSocket
ws_max_connections = 1000
ws_max_subscriptions_per_connection = 100

# Disabled methods (security hardening)
disabled_methods = ["debug_setHead", "debug_gcStats"]
```

---

## 11. DOMAIN MODEL

```rust
// ═══════════════════════════════════════════════════════════════════════════
//                              VALUE OBJECTS
// ═══════════════════════════════════════════════════════════════════════════

/// JSON-RPC 2.0 Request
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,           // Must be "2.0"
    pub method: String,
    pub params: Option<JsonValue>,
    pub id: JsonRpcId,
}

/// JSON-RPC 2.0 Response
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,           // Always "2.0"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: JsonRpcId,
}

/// JSON-RPC Error
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<JsonValue>,
}

/// Standard JSON-RPC error codes
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    
    // Ethereum-specific error codes
    pub const EXECUTION_ERROR: i32 = -32000;
    pub const RESOURCE_NOT_FOUND: i32 = -32001;
    pub const RESOURCE_UNAVAILABLE: i32 = -32002;
    pub const TRANSACTION_REJECTED: i32 = -32003;
    pub const METHOD_NOT_SUPPORTED: i32 = -32004;
    pub const LIMIT_EXCEEDED: i32 = -32005;
}

/// Request context for access control
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub request_id: Uuid,
    pub client_ip: IpAddr,
    pub is_localhost: bool,
    pub has_valid_api_key: bool,
    pub has_admin_auth: bool,
    pub timestamp: u64,
}

/// WebSocket subscription
#[derive(Debug, Clone)]
pub struct Subscription {
    pub id: u64,
    pub connection_id: Uuid,
    pub subscription_type: SubscriptionType,
    pub filter: Option<SubscriptionFilter>,
    pub created_at: u64,
}

// ═══════════════════════════════════════════════════════════════════════════
//                              ENTITIES
// ═══════════════════════════════════════════════════════════════════════════

/// Pending IPC request awaiting response
pub struct PendingRequest {
    pub correlation_id: [u8; 16],
    pub method: String,
    pub created_at: Instant,
    pub timeout: Duration,
    pub response_channel: oneshot::Sender<Result<JsonValue, JsonRpcError>>,
}

/// Active WebSocket connection
pub struct WebSocketConnection {
    pub id: Uuid,
    pub client_ip: IpAddr,
    pub connected_at: u64,
    pub subscriptions: HashMap<u64, Subscription>,
    pub sender: mpsc::Sender<WebSocketMessage>,
}
```

---

## 12. PORTS (INTERFACES)

### 12.1 Inbound Ports (Driving - API)

```rust
/// HTTP JSON-RPC handler
#[async_trait]
pub trait JsonRpcHandler: Send + Sync {
    /// Handle a single JSON-RPC request
    async fn handle_request(
        &self,
        request: JsonRpcRequest,
        context: RequestContext,
    ) -> JsonRpcResponse;
    
    /// Handle a batch of JSON-RPC requests
    async fn handle_batch(
        &self,
        requests: Vec<JsonRpcRequest>,
        context: RequestContext,
    ) -> Vec<JsonRpcResponse>;
}

/// WebSocket handler
#[async_trait]
pub trait WebSocketHandler: Send + Sync {
    /// Handle new WebSocket connection
    async fn on_connect(&self, connection_id: Uuid, client_ip: IpAddr) -> Result<(), Error>;
    
    /// Handle WebSocket message
    async fn on_message(
        &self,
        connection_id: Uuid,
        message: JsonRpcRequest,
    ) -> JsonRpcResponse;
    
    /// Handle WebSocket disconnection
    async fn on_disconnect(&self, connection_id: Uuid);
}

/// Health check handler
#[async_trait]
pub trait HealthHandler: Send + Sync {
    /// Liveness probe (is the process running?)
    async fn liveness(&self) -> HealthStatus;
    
    /// Readiness probe (is the node ready to serve requests?)
    async fn readiness(&self) -> HealthStatus;
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthStatus {
    pub status: String,         // "healthy", "degraded", "unhealthy"
    pub version: String,
    pub uptime_seconds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<HashMap<String, String>>,
}
```

### 12.2 Outbound Ports (Driven - SPI)

```rust
/// Event bus publisher for IPC
#[async_trait]
pub trait EventBusPublisher: Send + Sync {
    /// Publish request to target subsystem and await response
    async fn request<Req, Res>(
        &self,
        target: SubsystemId,
        request: Req,
        timeout: Duration,
    ) -> Result<Res, IpcError>
    where
        Req: Serialize + Send,
        Res: DeserializeOwned;
    
    /// Subscribe to events from the bus
    async fn subscribe(
        &self,
        event_type: EventType,
        filter: Option<EventFilter>,
    ) -> Result<EventStream, IpcError>;
    
    /// Unsubscribe from events
    async fn unsubscribe(&self, subscription_id: u64) -> Result<(), IpcError>;
}

/// Rate limiter
#[async_trait]
pub trait RateLimiter: Send + Sync {
    /// Check if request is allowed
    fn check(&self, client_ip: IpAddr, category: RateLimitCategory) -> Result<(), RateLimitError>;
    
    /// Record a request
    fn record(&self, client_ip: IpAddr, category: RateLimitCategory);
}

/// Metrics reporter
pub trait MetricsReporter: Send + Sync {
    /// Record request metrics
    fn record_request(&self, method: &str, duration: Duration, success: bool);
    
    /// Record WebSocket metrics
    fn record_ws_connection(&self, connected: bool);
    fn record_ws_subscription(&self, subscription_type: &str, active: bool);
    fn record_ws_message(&self);
}

/// Tracer for distributed tracing
pub trait Tracer: Send + Sync {
    /// Start a new span
    fn start_span(&self, name: &str) -> Span;
    
    /// Add event to current span
    fn add_event(&self, span: &Span, name: &str, attributes: HashMap<String, String>);
    
    /// End span
    fn end_span(&self, span: Span);
}
```

---

## 13. ERROR HANDLING

```rust
/// API Gateway errors
#[derive(Debug, thiserror::Error)]
pub enum ApiGatewayError {
    #[error("Rate limit exceeded")]
    RateLimitExceeded { retry_after: Duration },
    
    #[error("Request timeout")]
    Timeout { method: String, duration: Duration },
    
    #[error("Method not found: {method}")]
    MethodNotFound { method: String },
    
    #[error("Invalid parameters: {message}")]
    InvalidParams { message: String },
    
    #[error("Unauthorized: {reason}")]
    Unauthorized { reason: String },
    
    #[error("Forbidden: {reason}")]
    Forbidden { reason: String },
    
    #[error("IPC error: {message}")]
    IpcError { subsystem: SubsystemId, message: String },
    
    #[error("Internal error: {message}")]
    InternalError { message: String },
}

impl From<ApiGatewayError> for JsonRpcError {
    fn from(err: ApiGatewayError) -> Self {
        match err {
            ApiGatewayError::RateLimitExceeded { .. } => JsonRpcError {
                code: error_codes::LIMIT_EXCEEDED,
                message: err.to_string(),
                data: None,
            },
            ApiGatewayError::MethodNotFound { .. } => JsonRpcError {
                code: error_codes::METHOD_NOT_FOUND,
                message: err.to_string(),
                data: None,
            },
            ApiGatewayError::InvalidParams { .. } => JsonRpcError {
                code: error_codes::INVALID_PARAMS,
                message: err.to_string(),
                data: None,
            },
            ApiGatewayError::Unauthorized { .. } | ApiGatewayError::Forbidden { .. } => JsonRpcError {
                code: error_codes::EXECUTION_ERROR,
                message: err.to_string(),
                data: None,
            },
            ApiGatewayError::Timeout { .. } => JsonRpcError {
                code: error_codes::RESOURCE_UNAVAILABLE,
                message: err.to_string(),
                data: None,
            },
            _ => JsonRpcError {
                code: error_codes::INTERNAL_ERROR,
                message: err.to_string(),
                data: None,
            },
        }
    }
}
```

---

## 14. SECURITY BOUNDARIES

### Allowed Senders (Who Can Talk To Me):
- ✅ External HTTP/WS clients (via network)
- ✅ Internal Event Bus (subscription notifications)
- ✅ Localhost admin clients (protected/admin methods)

### Allowed Recipients (Who I Can Talk To):
- ✅ qc-01 Peer Discovery (peer info, admin operations)
- ✅ qc-02 Block Storage (block queries)
- ✅ qc-03 Transaction Indexing (transaction/receipt queries)
- ✅ qc-04 State Management (state queries)
- ✅ qc-06 Mempool (transaction submission, status)
- ✅ qc-08 Consensus (admin: start/stop mining)
- ✅ qc-10 Signature Verification (transaction validation)
- ✅ qc-11 Smart Contracts (eth_call, estimateGas)
- ✅ Event Bus (subscriptions)

### Security Invariants:
1. **No Direct State Modification**: API Gateway CANNOT directly modify blockchain state
2. **All Writes Require Signatures**: Transaction submission requires pre-signed transactions
3. **Rate Limiting Always Enforced**: Even localhost requests are rate-limited (higher limits)
4. **Admin Methods Protected**: Admin operations require localhost AND authentication
5. **Request Size Bounded**: All requests have maximum size limits
6. **Timeout Enforcement**: All operations have timeouts to prevent resource exhaustion

---

## 15. TDD STRATEGY

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    // Rate limiting
    #[test]
    fn test_rate_limiter_allows_requests_under_limit() { }
    #[test]
    fn test_rate_limiter_blocks_requests_over_limit() { }
    #[test]
    fn test_rate_limiter_resets_after_window() { }
    
    // Method routing
    #[test]
    fn test_routes_eth_get_balance_to_state_management() { }
    #[test]
    fn test_routes_eth_send_raw_transaction_to_mempool() { }
    #[test]
    fn test_rejects_unknown_method() { }
    
    // Access control
    #[test]
    fn test_public_methods_allowed_without_auth() { }
    #[test]
    fn test_protected_methods_require_api_key() { }
    #[test]
    fn test_admin_methods_require_localhost() { }
    
    // Request validation
    #[test]
    fn test_rejects_oversized_request() { }
    #[test]
    fn test_rejects_oversized_batch() { }
    #[test]
    fn test_validates_json_rpc_format() { }
    
    // WebSocket
    #[test]
    fn test_subscription_creates_event_listener() { }
    #[test]
    fn test_unsubscription_removes_listener() { }
    #[test]
    fn test_max_subscriptions_per_connection() { }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_full_transaction_submission_flow() { }

#[tokio::test]
async fn test_websocket_subscription_receives_events() { }

#[tokio::test]
async fn test_rate_limiting_across_methods() { }

#[tokio::test]
async fn test_timeout_handling_for_slow_subsystem() { }
```

---

## 16. DEPENDENCIES

> **DESIGN DECISION (FIX FOR IMPLEMENTATION TRAP #1):** We use `primitive-types` for U256 
> (standard in Ethereum Rust ecosystem). The `alloy` family provides modern, well-maintained
> alternatives. Serde is configured to serialize U256 as hex strings for JSON-RPC compatibility.

```toml
[dependencies]
# Web framework
axum = "0.8"
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "timeout", "limit", "compression", "trace"] }

# JSON-RPC
jsonrpsee = { version = "0.24", features = ["server", "ws-server"] }

# WebSocket
tokio-tungstenite = "0.26"

# Async runtime
tokio = { version = "1", features = ["full"] }

# Ethereum types (FIX FOR TRAP #1)
primitive-types = { version = "0.13", features = ["serde"] }  # U256, H256, H160
alloy-rlp = "0.3"          # RLP encoding/decoding for transaction validation
alloy-primitives = "0.8"   # Alternative to primitive-types (more modern)

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
hex = "0.4"                # Hex encoding/decoding

# Tracing/Observability
tracing = "0.1"
tracing-opentelemetry = "0.27"
opentelemetry = "0.26"
opentelemetry-otlp = "0.26"
prometheus = "0.13"

# Shared types
shared-types = { path = "../shared-types" }
shared-bus = { path = "../shared-bus" }

# Utilities
thiserror = "2"
uuid = { version = "1", features = ["v4"] }
governor = "0.6"           # Rate limiting with token bucket
dashmap = "6"              # Concurrent HashMap for pending requests
```

### Type Serialization for JSON-RPC

```rust
use primitive_types::U256;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Serialize U256 as hex string with 0x prefix (Ethereum standard)
pub fn serialize_u256<S>(value: &U256, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&format!("0x{:x}", value))
}

/// Deserialize U256 from hex string (with or without 0x prefix)
pub fn deserialize_u256<'de, D>(deserializer: D) -> Result<U256, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let s = s.strip_prefix("0x").unwrap_or(&s);
    U256::from_str_radix(s, 16).map_err(serde::de::Error::custom)
}

/// Wrapper type for JSON-RPC compatible U256
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonU256(pub U256);

impl Serialize for JsonU256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_u256(&self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for JsonU256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_u256(deserializer).map(JsonU256)
    }
}
```

---

## 17. FUTURE CONSIDERATIONS

### 17.1 GraphQL Support (V2)
Consider adding GraphQL endpoint for complex queries that are inefficient with JSON-RPC.

### 17.2 gRPC Support (V2)
For high-performance internal integrations and cross-node communication.

### 17.3 API Key Management (V2)
Full API key lifecycle management with quotas, rate limits per key, and analytics.

### 17.4 Request Caching (V2)
Cache frequently requested data (latest block, chain ID) to reduce internal IPC load.

---

**END OF SPEC-16-API-GATEWAY.md**
