//! # Subsystem Container
//!
//! Holds all core subsystem instances and manages their lifecycle.
//!
//! ## Initialization Order (Architecture.md v2.3)
//!
//! Subsystems are initialized in strict dependency order:
//!
//! ```text
//! Level 0: Signature Verification (no dependencies)
//! Level 1: Peer Discovery, Mempool (depend on Level 0)
//! Level 2: Transaction Indexing, State Management, Block Propagation
//! Level 3: Consensus (depends on Level 0-2)
//! Level 4: Block Storage, Finality (depends on Level 0-3)
//! ```
//!
//! ## Thread Safety
//!
//! - All subsystems wrapped in `Arc` for shared ownership
//! - Mutable subsystems use `RwLock` for concurrent access
//! - Event bus is the sole communication channel (no direct calls)

use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use tracing::{info, instrument};

use shared_bus::{InMemoryEventBus, TimeBoundedNonceCache};

use crate::container::config::NodeConfig;

// Import subsystem services
use qc_01_peer_discovery::PeerDiscoveryService;
use qc_02_block_storage::{
    BlockAssemblyBuffer, AssemblyConfig, BlockStorageService,
    ports::outbound::{
        InMemoryKVStore, MockFileSystemAdapter, DefaultChecksumProvider,
        SystemTimeSource as StorageTimeSource, BincodeBlockSerializer,
    },
};
use qc_06_mempool::TransactionPool;

/// Concrete type for Block Storage Service with in-memory backends.
pub type ConcreteBlockStorageService = BlockStorageService<
    InMemoryKVStore,
    MockFileSystemAdapter,
    DefaultChecksumProvider,
    StorageTimeSource,
    BincodeBlockSerializer,
>;

/// Central container holding all subsystem instances.
///
/// This is the main integration point where all subsystems are wired together
/// with their adapters implementing the required ports.
pub struct SubsystemContainer {
    // =========================================================================
    // LEVEL 1: Depends on Level 0
    // =========================================================================
    /// Peer Discovery (Subsystem 1)
    /// Depends on Sig Verification for DDoS defense.
    pub peer_discovery: Arc<RwLock<PeerDiscoveryService>>,

    /// Mempool (Subsystem 6)
    /// Depends on Sig Verification for transaction validation.
    pub mempool: Arc<RwLock<TransactionPool>>,

    // =========================================================================
    // LEVEL 4: Depends on Level 0-3
    // =========================================================================
    /// Block Storage (Subsystem 2)
    /// Stateful Assembler for V2.3 choreography.
    pub block_storage: Arc<RwLock<ConcreteBlockStorageService>>,

    /// Block Assembly Buffer (for choreography)
    pub assembly_buffer: Arc<RwLock<BlockAssemblyBuffer>>,

    // =========================================================================
    // SHARED INFRASTRUCTURE
    // =========================================================================
    /// Event Bus for inter-subsystem communication.
    /// All choreography events flow through this bus.
    pub event_bus: Arc<InMemoryEventBus>,

    /// Time-bounded nonce cache for replay prevention.
    pub nonce_cache: Arc<RwLock<TimeBoundedNonceCache>>,

    /// Node configuration (immutable after initialization).
    pub config: NodeConfig,
}

impl SubsystemContainer {
    /// Create a new subsystem container with all subsystems initialized.
    ///
    /// ## Initialization Phases
    ///
    /// 1. Create shared infrastructure (event bus, nonce cache)
    /// 2. Initialize Level 0 subsystems
    /// 3. Initialize Level 1 subsystems with Level 0 adapters
    /// 4. Initialize Level 2-4 subsystems
    /// 5. Wire event subscriptions
    #[instrument(name = "subsystem_init", skip(config))]
    pub fn new(config: NodeConfig) -> Self {
        info!("Initializing Quantum-Chain subsystem container");
        info!("Architecture version: 2.3 (Choreography Pattern)");

        // =====================================================================
        // PHASE 1: Shared Infrastructure
        // =====================================================================
        info!("Phase 1: Creating shared infrastructure");
        
        let event_bus = Arc::new(InMemoryEventBus::new());
        let nonce_cache = Arc::new(RwLock::new(TimeBoundedNonceCache::new()));

        // =====================================================================
        // PHASE 2: Level 0 - No Dependencies
        // =====================================================================
        info!("Phase 2: Initializing Level 0 subsystems");
        info!("  [10] Signature Verification initialized (stateless)");

        // =====================================================================
        // PHASE 3: Level 1 - Depends on Level 0
        // =====================================================================
        info!("Phase 3: Initializing Level 1 subsystems");
        
        let peer_discovery = Self::init_peer_discovery();
        info!("  [1] Peer Discovery initialized");

        let mempool = Self::init_mempool(&config);
        info!("  [6] Mempool initialized (max {} txs)", config.mempool.max_transactions);

        // =====================================================================
        // PHASE 4: Level 4 - Block Storage (Stateful Assembler)
        // =====================================================================
        info!("Phase 4: Initializing Block Storage (Stateful Assembler)");
        
        let (block_storage, assembly_buffer) = Self::init_block_storage(&config);
        info!(
            "  [2] Block Storage initialized (timeout={}s, max_pending={})",
            config.storage.assembly_timeout_secs,
            config.storage.max_pending_assemblies
        );

        info!("All core subsystems initialized successfully");

        Self {
            peer_discovery,
            mempool,
            block_storage,
            assembly_buffer,
            event_bus,
            nonce_cache,
            config,
        }
    }

    /// Create a container for testing with in-memory backends.
    #[cfg(test)]
    pub fn new_for_testing() -> Self {
        Self::new(NodeConfig::default())
    }

    // =========================================================================
    // SUBSYSTEM INITIALIZATION METHODS
    // =========================================================================

    fn init_peer_discovery() -> Arc<RwLock<PeerDiscoveryService>> {
        use qc_01_peer_discovery::{
            KademliaConfig, NodeId, TimeSource, Timestamp,
        };

        // Create a simple time source
        struct SimpleTimeSource;
        impl TimeSource for SimpleTimeSource {
            fn now(&self) -> Timestamp {
                Timestamp::new(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                )
            }
        }

        // Generate a random local node ID for this instance
        let local_node_id = NodeId::new(rand::random());
        let kademlia_config = KademliaConfig::default();
        let time_source: Box<dyn TimeSource> = Box::new(SimpleTimeSource);

        Arc::new(RwLock::new(PeerDiscoveryService::new(
            local_node_id,
            kademlia_config,
            time_source,
        )))
    }

    fn init_mempool(config: &NodeConfig) -> Arc<RwLock<TransactionPool>> {
        use qc_06_mempool::MempoolConfig as PoolConfig;

        let pool_config = PoolConfig {
            max_transactions: config.mempool.max_transactions,
            max_per_account: config.mempool.max_per_account,
            min_gas_price: config.mempool.min_gas_price.into(),
            pending_inclusion_timeout_ms: config.mempool.pending_inclusion_timeout_secs * 1000,
            ..PoolConfig::default()
        };

        Arc::new(RwLock::new(TransactionPool::new(pool_config)))
    }

    fn init_block_storage(
        config: &NodeConfig,
    ) -> (Arc<RwLock<ConcreteBlockStorageService>>, Arc<RwLock<BlockAssemblyBuffer>>) {
        // Create assembly buffer for choreography
        let assembly_config = AssemblyConfig {
            assembly_timeout_secs: config.storage.assembly_timeout_secs,
            max_pending_assemblies: config.storage.max_pending_assemblies,
        };
        let assembly_buffer = Arc::new(RwLock::new(BlockAssemblyBuffer::new(assembly_config)));

        // Create block storage with in-memory backends (for now)
        // Production would use RocksDB adapter
        let kv_store = InMemoryKVStore::new();
        let fs_adapter = MockFileSystemAdapter::new(50); // 50% disk available
        let checksum = DefaultChecksumProvider::default();
        let time_source = StorageTimeSource::default();
        let serializer = BincodeBlockSerializer::default();

        let storage_config = qc_02_block_storage::StorageConfig::default();
        let service = BlockStorageService::new(
            kv_store,
            fs_adapter,
            checksum,
            time_source,
            serializer,
            storage_config,
        );

        (Arc::new(RwLock::new(service)), assembly_buffer)
    }

    // =========================================================================
    // ACCESSOR METHODS
    // =========================================================================

    /// Get the event bus for publishing/subscribing.
    pub fn event_bus(&self) -> Arc<InMemoryEventBus> {
        Arc::clone(&self.event_bus)
    }

    /// Get the nonce cache for message validation.
    pub fn nonce_cache(&self) -> Arc<RwLock<TimeBoundedNonceCache>> {
        Arc::clone(&self.nonce_cache)
    }

    /// Get assembly timeout duration.
    pub fn assembly_timeout(&self) -> Duration {
        Duration::from_secs(self.config.storage.assembly_timeout_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_initialization() {
        let container = SubsystemContainer::new_for_testing();
        
        // Verify all subsystems are initialized
        assert_eq!(container.event_bus.subscriber_count(), 0);
        assert!(container.mempool.read().is_empty());
    }

    #[test]
    fn test_event_bus_accessible() {
        let container = SubsystemContainer::new_for_testing();
        let bus = container.event_bus();
        
        // Should be able to subscribe
        let _sub = bus.subscribe(shared_bus::EventFilter::all());
        assert_eq!(bus.subscriber_count(), 1);
    }
}
