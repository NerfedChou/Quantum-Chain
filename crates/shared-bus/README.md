# Shared Bus - Event Bus for Inter-Subsystem Communication

**Crate:** `shared-bus`  
**Architecture Reference:** Architecture.md Section 5 (V2.3 Choreography Pattern)

---

## Purpose

The Shared Bus implements the **V2.3 Choreography Pattern** for decentralized event-driven communication between subsystems. It enforces **Architecture.md Rule #4:**

> "All inter-subsystem communication via Shared Bus ONLY"

Direct subsystem-to-subsystem calls are **FORBIDDEN**.

## Architecture

```
┌──────────────┐                    ┌──────────────┐
│ Subsystem A  │                    │ Subsystem B  │
│              │    publish()       │              │
│              │ ──────┐            │              │
└──────────────┘       │            └──────────────┘
                       ▼                    ↑
                 ┌──────────────┐          │
                 │  Event Bus   │          │
                 │              │ ─────────┘
                 └──────────────┘  subscribe()
```

## Quick Start

```rust
use shared_bus::{InMemoryEventBus, EventPublisher, EventFilter, BlockchainEvent};
use shared_types::entities::ValidatedBlock;

#[tokio::main]
async fn main() {
    // Create the bus
    let bus = InMemoryEventBus::new();
    
    // Subscribe to consensus events
    let mut sub = bus.subscribe(EventFilter::topics(vec![EventTopic::Consensus]));
    
    // Publish an event (from another task)
    let event = BlockchainEvent::BlockValidated(ValidatedBlock::default());
    bus.publish(event).await;
    
    // Receive the event
    if let Some(event) = sub.recv().await {
        println!("Received: {:?}", event);
    }
}
```

## Key Components

| Component | Description |
|-----------|-------------|
| `BlockchainEvent` | Enum of all events in the system |
| `EventFilter` | Filter for subscribing to specific events |
| `InMemoryEventBus` | In-memory implementation (single-node) |
| `TimeBoundedNonceCache` | Replay attack prevention |
| `Subscription` | Handle for receiving events |

## Security Features

### Time-Bounded Nonce Cache (v2.1)

Prevents replay attacks by:
1. Validating message timestamps (60s past to 10s future)
2. Rejecting duplicate nonces within the validity window
3. Garbage collecting expired nonces automatically

### Envelope-Only Identity

All events are wrapped in `AuthenticatedMessage<T>` envelopes. The `sender_id` in the envelope is the **sole source of truth** for sender identity.

## Event Types

| Event | Source | Description |
|-------|--------|-------------|
| `BlockValidated` | Subsystem 8 | Triggers choreography flow |
| `MerkleRootComputed` | Subsystem 3 | Assembly component |
| `StateRootComputed` | Subsystem 4 | Assembly component |
| `BlockStored` | Subsystem 2 | Choreography completion |
| `PeerDiscovered` | Subsystem 1 | Network events |
| `TransactionVerified` | Subsystem 10 | Signature events |
| `BlockFinalized` | Subsystem 9 | Finality events |

## Testing

```bash
cargo test -p shared-bus
```

**Test Coverage:** 26 tests
- Events: 6 tests
- Nonce Cache: 7 tests
- Publisher: 5 tests
- Subscriber: 6 tests
- Lib: 2 tests

## Related Documentation

- [Architecture.md](../../Documentation/Architecture.md) - Section 5: Event Bus
- [IPC-MATRIX.md](../../Documentation/IPC-MATRIX.md) - Security boundaries
- [System.md](../../Documentation/System.md) - Subsystem definitions
