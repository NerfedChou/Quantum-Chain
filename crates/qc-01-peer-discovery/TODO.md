# TODO: Subsystem 01 - Peer Discovery & Routing

**Specification:** `SPECS/SPEC-01-PEER-DISCOVERY.md` v2.4  
**Crate:** `crates/qc-01-peer-discovery`  
**Created:** 2025-12-02  
**Last Updated:** 2025-12-02  
**Status:** 🟢 COMPLETE (Phase 1-7 Done, Phase 8 Deferred)

---

## CURRENT PHASE

```
[x] Phase 1: RED       - Domain tests (XOR distance, K-Bucket, IP Diversity, Ban, Staging, Eviction)
[x] Phase 2: GREEN     - Domain implementation
[x] Phase 3: PORTS     - Port trait definitions (PeerDiscoveryApi, NetworkSocket, TimeSource, etc.)
[x] Phase 4: SERVICE   - PeerDiscoveryService implementing PeerDiscoveryApi
[x] Phase 5: IPC       - Security boundaries & authorization per IPC-MATRIX.md
[x] Phase 6: DOCS      - Rustdoc examples & README
[x] Phase 7: BUS       - Event bus adapter for V2.3 choreography
[ ] Phase 8: RUNTIME   - Wire to node runtime (deferred until node-runtime is ready)
```

**Test Results:** 80 tests passing
- 3 entity tests
- 3 value object tests
- 10 XOR distance/service tests
- 21 routing table tests (k-bucket, ban, staging, eviction)
- 2 port tests
- 5 service tests
- 1 doc test
- 14 IPC security tests
- 12 IPC handler tests
- 7 adapter/publisher tests
- 6 adapter/subscriber tests
- ✅ Clippy clean with `-D warnings`

---

## COMPLIANCE AUDIT

### SPEC-01 Compliance ✅

| Section | Requirement | Status |
|---------|-------------|--------|
| 2.1 | Core Entities (NodeId, PeerInfo, SocketAddr) | ✅ Implemented |
| 2.2 | Routing Table, KBucket, PendingPeer, BannedPeers | ✅ Implemented |
| 2.3 | Value Objects (Distance, KademliaConfig, SubnetMask) | ✅ Implemented |
| 2.4 | INVARIANT-1 (bucket size ≤ K) | ✅ Tested |
| 2.4 | INVARIANT-2 (local_node_id immutable) | ✅ Enforced |
| 2.4 | INVARIANT-3 (IP diversity) | ✅ Tested |
| 2.4 | INVARIANT-4 (banned peer exclusion) | ✅ Tested |
| 2.4 | INVARIANT-5 (self-exclusion) | ✅ Tested |
| 2.4 | INVARIANT-6 (distance ordering) | ✅ Tested |
| 2.4 | INVARIANT-7 (pending verification staging) | ✅ Tested |
| 2.4 | INVARIANT-8 (verification timeout) | ✅ Tested |
| 2.4 | INVARIANT-9 (bounded staging - Memory Bomb Defense) | ✅ Tested |
| 2.4 | INVARIANT-10 (Eviction-on-Failure - Eclipse Defense) | ✅ Tested |
| 3.1 | PeerDiscoveryApi trait (Driving Port) | ✅ Implemented |
| 3.2 | NetworkSocket, TimeSource, ConfigProvider, NodeIdValidator (Driven Ports) | ✅ Implemented |
| 4.1 | PeerDiscoveryEventPayload enum | ✅ Implemented |
| 4.2 | PeerDiscoveryRequestPayload enum | ✅ Implemented |
| 4.3 | Request/Response flow with correlation ID | ✅ Implemented |
| 5.1 | TDD Test Groups 1-8 | ✅ All implemented |
| 6.1 | Sybil Attack Resistance (IP diversity) | ✅ Implemented |
| 6.2 | Eclipse Attack Defense | ✅ Implemented |
| 6.5 | Memory Bomb Defense | ✅ Implemented |
| 6.6 | No Panic Policy (.get() over indexing) | ✅ Verified |

### Architecture.md Compliance ✅

| Principle | Requirement | Status |
|-----------|-------------|--------|
| DDD - Bounded Context | Isolated crate with pure domain logic | ✅ |
| Hexagonal - Ports/Adapters | Domain + Ports + Service + Adapters complete | ✅ |
| TDD - Tests First | All 80 tests pass | ✅ |
| Zero direct subsystem calls | Via IPC/Event Bus ONLY | ✅ |
| V2.3 Choreography | EventBusAdapter for events | ✅ |
| IPC-MATRIX Authorization | Sender validation per matrix | ✅ |

---

## COMPLETED COMPONENTS

### Domain Layer ✅

| Component | File | Tests |
|-----------|------|-------|
| Core Entities | `domain/entities.rs` | 3 |
| Value Objects | `domain/value_objects.rs` | 3 |
| Domain Services | `domain/services.rs` | 10 |
| Routing Table | `domain/routing_table.rs` | 21 |
| Errors | `domain/errors.rs` | - |

### Ports Layer ✅

| Component | File | Tests |
|-----------|------|-------|
| Inbound Port (PeerDiscoveryApi) | `ports/inbound.rs` | - |
| Outbound Ports (NetworkSocket, TimeSource, etc.) | `ports/outbound.rs` | 2 |

### Service Layer ✅

| Component | File | Tests |
|-----------|------|-------|
| PeerDiscoveryService | `service.rs` | 5 |

### IPC Layer ✅

| Component | File | Tests |
|-----------|------|-------|
| Event/Request Payloads | `ipc/payloads.rs` | 6 |
| Security & Authorization | `ipc/security.rs` | 14 |
| IPC Handler | `ipc/handler.rs` | 12 |

### Adapters Layer ✅

| Component | File | Tests |
|-----------|------|-------|
| Event Publisher | `adapters/publisher.rs` | 7 |
| Event Subscriber | `adapters/subscriber.rs` | 6 |

### Documentation ✅

| Component | File | Status |
|-----------|------|--------|
| Crate README | `README.md` | ✅ |
| Module docs | `lib.rs` docstrings | ✅ |
| API examples | Rustdoc examples | ✅ |

### Security Tests Completed ✅

| Test | Description | Status |
|------|-------------|--------|
| `test_table_poisoning_attack_is_blocked` | Eclipse Attack Defense (V2.4) | ✅ |
| `test_staging_area_rejects_peer_when_at_capacity` | Memory Bomb Defense (V2.3) | ✅ |
| `test_staging_area_uses_tail_drop_not_eviction` | Tail Drop Strategy | ✅ |
| `test_bucket_prefers_stable_peers_over_new_peers` | Eviction-on-Failure | ✅ |
| `test_bucket_evicts_dead_peers_for_new_peers` | Dead Peer Replacement | ✅ |
| `test_peer_silently_dropped_on_identity_valid_false` | IP Spoofing Defense | ✅ |
| `test_rejects_third_peer_from_same_subnet` | Subnet Diversity | ✅ |
| `test_peer_list_authorization` | IPC-MATRIX sender check | ✅ |
| `test_validate_timestamp` | Time-bounded replay prevention | ✅ |
| `test_validate_reply_to` | Forwarding attack prevention | ✅ |

---

## REMAINING PHASE

### Phase 8: Runtime Integration (Deferred)

| Task | Description | Status |
|------|-------------|--------|
| 8.1 | Wire to node runtime | ⬜ Deferred |
| 8.2 | End-to-end integration tests | ⬜ Deferred |

**Note:** Phase 8 is deferred until `node-runtime` crate is ready for integration.

---

## DIRECTORY STRUCTURE

```
crates/qc-01-peer-discovery/
├── Cargo.toml
├── README.md                    # Crate documentation
├── TODO.md                      # This file
├── src/
│   ├── lib.rs                   # Public API exports
│   ├── domain/                  # Inner layer (pure logic)
│   │   ├── mod.rs
│   │   ├── entities.rs          # NodeId, PeerInfo, SocketAddr
│   │   ├── value_objects.rs     # Distance, KademliaConfig, SubnetMask
│   │   ├── services.rs          # XOR distance, subnet checks
│   │   ├── routing_table.rs     # KBucket, RoutingTable, BannedPeers
│   │   └── errors.rs            # PeerDiscoveryError
│   ├── ports/                   # Middle layer (traits)
│   │   ├── mod.rs
│   │   ├── inbound.rs           # PeerDiscoveryApi trait
│   │   └── outbound.rs          # NetworkSocket, TimeSource, etc.
│   ├── service.rs               # PeerDiscoveryService (implements API)
│   ├── ipc/                     # IPC layer (security boundaries)
│   │   ├── mod.rs
│   │   ├── payloads.rs          # Event/Request payloads
│   │   ├── security.rs          # Authorization rules per IPC-MATRIX
│   │   └── handler.rs           # IPC message handler
│   └── adapters/                # Outer layer (event bus)
│       ├── mod.rs
│       ├── publisher.rs         # Event publishing adapter
│       └── subscriber.rs        # Event subscription adapter
└── tests/                       # Integration tests (deferred to Phase 8)
```
