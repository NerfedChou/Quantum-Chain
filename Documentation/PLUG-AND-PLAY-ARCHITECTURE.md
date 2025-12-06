# Quantum-Chain Architecture: True Plug-and-Play Subsystems

## Overview

Quantum-Chain implements a **modular blockchain architecture** using four key patterns:

1. **EDA (Event-Driven Architecture)** - Subsystems communicate via Event Bus ONLY
2. **DDD (Domain-Driven Design)** - Each subsystem owns its bounded context
3. **Hexagonal Architecture** - Ports define contracts, Adapters implement them
4. **TDD (Test-Driven Development)** - Tests drive the design

## The Problem We Solved

Previously, subsystems were **hard-coded** in `SubsystemContainer`:

```rust
// ❌ OLD: Hard-coded coupling
pub struct SubsystemContainer {
    pub peer_discovery: Arc<RwLock<PeerDiscoveryService>>,  // REQUIRED
    pub mempool: Arc<RwLock<TransactionPool>>,              // REQUIRED
    pub consensus: Arc<ConcreteConsensusService>,           // REQUIRED
    // ... every field is required!
}
```

This violated plug-and-play because:
- Removing ANY subsystem = compilation failure
- No way to disable subsystems at runtime
- Direct coupling between components

## The Solution: Subsystem Registry

The new architecture uses a **registry pattern**:

```rust
// ✅ NEW: Plug-and-play registry
pub struct SubsystemRegistry {
    subsystems: HashMap<SubsystemId, Arc<dyn Subsystem>>,
    config: SubsystemConfig,
    event_bus: Arc<InMemoryEventBus>,
}
```

### How It Works

```
┌─────────────────────────────────────────────────────────────────┐
│                     SubsystemRegistry                          │
│                                                                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐       │
│  │  qc-01   │  │  qc-02   │  │  qc-03   │  │   ...    │       │
│  │ ENABLED  │  │ ENABLED  │  │ DISABLED │  │          │       │
│  └────┬─────┘  └────┬─────┘  └──────────┘  └──────────┘       │
│       │             │                                          │
│       └─────────────┴──────────────┐                          │
│                                    ▼                          │
│                          ┌─────────────────┐                  │
│                          │   Event Bus     │                  │
│                          │ (shared-bus)    │                  │
│                          └─────────────────┘                  │
└─────────────────────────────────────────────────────────────────┘
```

### Configuration

Enable/disable subsystems via environment or config file:

```bash
# Environment variables
export QC_SUBSYSTEM_QC_01_PEER_DISCOVERY=true
export QC_SUBSYSTEM_QC_05_BLOCK_PROPAGATION=false
export QC_SUBSYSTEM_QC_07_BLOOM_FILTERS=false
```

```toml
# config.toml
[subsystems]
qc-01-peer-discovery = true
qc-02-block-storage = true
qc-03-transaction-indexing = true
qc-04-state-management = true
qc-05-block-propagation = false  # Not implemented yet
qc-06-mempool = true
qc-07-bloom-filters = false      # Optional optimization
qc-08-consensus = true
qc-09-finality = true
qc-10-signature-verification = true
qc-16-api-gateway = true
qc-17-block-production = true
```

## Implementing a New Subsystem

### Step 1: Implement the Subsystem Trait

```rust
use node_runtime::{Subsystem, SubsystemId, SubsystemStatus, SubsystemError};

pub struct MySubsystem {
    event_bus: Arc<InMemoryEventBus>,
    status: RwLock<SubsystemStatus>,
}

#[async_trait::async_trait]
impl Subsystem for MySubsystem {
    fn id(&self) -> SubsystemId {
        SubsystemId::MySubsystem
    }

    async fn init(&self) -> Result<(), SubsystemError> {
        // Initialize resources
        Ok(())
    }

    async fn start(&self) -> Result<(), SubsystemError> {
        // Subscribe to events and start processing
        let subscription = self.event_bus.subscribe(EventFilter::topics(vec![
            EventTopic::Consensus,
        ]));
        
        // Spawn event handler task
        tokio::spawn(async move {
            while let Some(event) = subscription.recv().await {
                // Process event
            }
        });

        *self.status.write() = SubsystemStatus::Running;
        Ok(())
    }

    async fn stop(&self) -> Result<(), SubsystemError> {
        *self.status.write() = SubsystemStatus::Stopped;
        Ok(())
    }

    fn status(&self) -> SubsystemStatus {
        *self.status.read()
    }
}
```

### Step 2: Register with the Registry

```rust
let registry = SubsystemRegistry::new(config, event_bus);

// Register subsystems (order doesn't matter - dependencies are checked)
registry.register(Arc::new(MySubsystem::new(event_bus.clone())))?;
registry.register(Arc::new(AnotherSubsystem::new(event_bus.clone())))?;

// Initialize and start
registry.init_all().await?;
registry.start_all().await?;
```

### Step 3: Communicate via Events ONLY

```rust
// ✅ CORRECT: Publish event to bus
self.event_bus.publish(BlockchainEvent::MerkleRootComputed {
    block_hash,
    merkle_root,
});

// ❌ WRONG: Direct function call
other_subsystem.compute_merkle_root(block);  // FORBIDDEN!
```

## Subsystem Dependencies

Dependencies are declared in `SubsystemId::dependencies()`:

```rust
pub fn dependencies(&self) -> Vec<SubsystemId> {
    match self {
        // Level 0: No dependencies
        Self::SignatureVerification => vec![],
        
        // Level 1: Depends on Level 0
        Self::PeerDiscovery => vec![Self::SignatureVerification],
        Self::Mempool => vec![Self::SignatureVerification],
        
        // Level 2: No external deps, but logical ordering
        Self::TransactionIndexing => vec![],
        Self::StateManagement => vec![],
        
        // Level 3: Depends on signature verification
        Self::Consensus => vec![Self::SignatureVerification],
        
        // Level 4: Depends on consensus
        Self::Finality => vec![Self::BlockStorage, Self::Consensus],
        Self::BlockProduction => vec![Self::Consensus],
        
        // Optional: No hard dependencies
        Self::ApiGateway => vec![],
        Self::BloomFilters => vec![],
    }
}
```

The registry validates dependencies before starting:

```rust
// This will FAIL if BlockStorage is disabled
config.enable(SubsystemId::Finality);
config.disable(SubsystemId::BlockStorage);

let result = config.validate();
// Error: qc-09-finality requires qc-02-block-storage but it is disabled
```

## Core vs Optional Subsystems

**Core subsystems** (required for blockchain to function):
- qc-02-block-storage
- qc-03-transaction-indexing  
- qc-04-state-management
- qc-08-consensus
- qc-09-finality

**Optional subsystems** (can be disabled):
- qc-01-peer-discovery (needed for networking)
- qc-05-block-propagation (P2P)
- qc-06-mempool (transaction pool)
- qc-07-bloom-filters (optimization)
- qc-10-signature-verification (can use hardware)
- qc-16-api-gateway (external interface)
- qc-17-block-production (mining)

## Health Monitoring

The registry tracks subsystem health:

```rust
// Check overall health
if registry.is_healthy() {
    println!("All core subsystems running");
}

// Get individual status
let status = registry.get_status(SubsystemId::Consensus);
match status {
    SubsystemStatus::Running => println!("Consensus OK"),
    SubsystemStatus::Failed => println!("Consensus FAILED"),
    SubsystemStatus::Disabled => println!("Consensus DISABLED"),
    _ => {}
}

// Print all statuses
registry.print_status();
// Output:
// ===========================================
//   SUBSYSTEM REGISTRY STATUS
// ===========================================
//   ✅ qc-01-peer-discovery           Running
//   ✅ qc-02-block-storage             Running  [CORE]
//   ✅ qc-03-transaction-indexing      Running  [CORE]
//   ⏸️  qc-05-block-propagation         Disabled
//   ❌ qc-07-bloom-filters             Failed
// ===========================================
```

## Event Flow Example

When a block is produced, here's the event flow:

```
[qc-17] Block produced
    │
    ▼
Event Bus: BlockProduced { height: 100, ... }
    │
    ├──────────────────────────┐
    ▼                          ▼
[qc-08] Validates block    [Other subscribers...]
    │
    ▼
Event Bus: BlockValidated { height: 100, ... }
    │
    ├─────────────────────────────────────────┐
    ▼                          ▼              ▼
[qc-03] Merkle root       [qc-04] State    [qc-02] Assembly
    │                          │              (waits for 3)
    ▼                          ▼                   │
MerkleRootComputed        StateRootComputed        │
    │                          │                   │
    └──────────────────────────┴───────────────────┘
                               │
                               ▼
                    [qc-02] Atomic write
                               │
                               ▼
                    Event Bus: BlockStored
                               │
                               ▼
                    [qc-09] Finality check
```

## Summary

The plug-and-play architecture ensures:

1. **No compilation changes** to enable/disable subsystems
2. **Graceful degradation** - disabled subsystems don't crash the node
3. **Clear dependencies** - validated before startup
4. **Event-driven communication** - no direct coupling
5. **Health monitoring** - track each subsystem's status
6. **Easy testing** - mock any subsystem via the Event Bus

This is the foundation for a truly modular, scalable blockchain.
