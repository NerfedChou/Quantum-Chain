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
#[cfg(not(feature = "rocksdb"))]
use qc_02_block_storage::ports::outbound::{InMemoryKVStore, MockFileSystemAdapter};
use qc_02_block_storage::{
    ports::outbound::{
        BincodeBlockSerializer, DefaultChecksumProvider, SystemTimeSource as StorageTimeSource,
    },
    AssemblyConfig, BlockAssemblyBuffer, BlockStorageService,
};

use qc_03_transaction_indexing::{IndexConfig, TransactionIndex};
use qc_04_state_management::PatriciaMerkleTrie;
use qc_06_mempool::TransactionPool;

// Consensus service and adapters
use crate::adapters::ports::consensus::{
    ConsensusEventBusAdapter, ConsensusMempoolAdapter, ConsensusSignatureAdapter,
    ConsensusValidatorSetAdapter,
};
use qc_08_consensus::{ConsensusConfig, ConsensusService};

// Finality service and adapters (used in handlers)
#[allow(unused_imports)]
use crate::adapters::ports::finality::{
    ConcreteFinalityBlockStorageAdapter, FinalityAttestationAdapter, FinalityValidatorSetAdapter,
};
#[allow(unused_imports)]
use qc_09_finality::service::{FinalityConfig, FinalityService};

// Block Production service (Subsystem 17)
use qc_17_block_production::ConcreteBlockProducer;

// RocksDB imports (when feature is enabled)
#[cfg(feature = "rocksdb")]
use crate::adapters::storage::{
    ProductionFileSystemAdapter, RocksDbConfig, RocksDbStore, RocksDbTrieDatabase,
};

#[cfg(feature = "rocksdb")]
use std::sync::Arc as StdArc;

/// Concrete type for Block Storage Service with in-memory backends (default).
#[cfg(not(feature = "rocksdb"))]
pub type ConcreteBlockStorageService = BlockStorageService<
    InMemoryKVStore,
    MockFileSystemAdapter,
    DefaultChecksumProvider,
    StorageTimeSource,
    BincodeBlockSerializer,
>;

/// Concrete type for Block Storage Service with RocksDB (production).
#[cfg(feature = "rocksdb")]
pub type ConcreteBlockStorageService = BlockStorageService<
    RocksDbStore,
    ProductionFileSystemAdapter,
    DefaultChecksumProvider,
    StorageTimeSource,
    BincodeBlockSerializer,
>;

/// Concrete type for Consensus Service with all adapters wired.
pub type ConcreteConsensusService = ConsensusService<
    ConsensusEventBusAdapter,
    ConsensusMempoolAdapter,
    ConsensusSignatureAdapter,
    ConsensusValidatorSetAdapter,
>;

/// Concrete type for Finality Service with all adapters wired (in-memory backends).
#[cfg(not(feature = "rocksdb"))]
pub type ConcreteFinalityService = FinalityService<
    ConcreteFinalityBlockStorageAdapter,
    FinalityAttestationAdapter,
    FinalityValidatorSetAdapter,
>;

/// Central container holding all subsystem instances.
///
/// This is the main integration point where all subsystems are wired together
/// with their adapters implementing the required ports.
///
/// ## V2.3 Choreography Pattern
///
/// All inter-subsystem communication flows through the Event Bus:
/// - Consensus (8) publishes `BlockValidated`
/// - Transaction Indexing (3) subscribes, publishes `MerkleRootComputed`
/// - State Management (4) subscribes, publishes `StateRootComputed`
/// - Block Storage (2) assembles all three components atomically
pub struct SubsystemContainer {
    // =========================================================================
    // LEVEL 0: No Dependencies
    // =========================================================================
    // Signature Verification (10) is stateless - no instance needed in container

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
    // LEVEL 2: Depends on Level 0-1
    // =========================================================================
    /// Transaction Indexing (Subsystem 3)
    /// Computes Merkle roots and provides proofs.
    pub transaction_index: Arc<RwLock<TransactionIndex>>,

    /// State Management (Subsystem 4)
    /// Patricia Merkle Trie for account state.
    pub state_trie: Arc<RwLock<PatriciaMerkleTrie>>,

    // =========================================================================
    // LEVEL 3: Depends on Level 0-2
    // =========================================================================
    /// Consensus (Subsystem 8)
    /// Block validation with PoS/PBFT.
    pub consensus: Arc<ConcreteConsensusService>,

    // =========================================================================
    // LEVEL 4: Depends on Level 0-3
    // =========================================================================
    /// Block Storage (Subsystem 2)
    /// Stateful Assembler for V2.3 choreography.
    pub block_storage: Arc<RwLock<ConcreteBlockStorageService>>,

    /// Block Assembly Buffer (for choreography)
    pub assembly_buffer: Arc<RwLock<BlockAssemblyBuffer>>,

    /// Finality (Subsystem 9)
    /// Casper FFG finalization gadget.
    #[cfg(not(feature = "rocksdb"))]
    pub finality: Arc<ConcreteFinalityService>,

    // =========================================================================
    // LEVEL 5: Advanced Subsystems
    // =========================================================================
    /// Block Production (Subsystem 17)
    /// Multi-threaded miner with ASIC-resistant PoW.
    pub block_producer: Arc<ConcreteBlockProducer>,

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
    /// 2. Initialize Level 0 subsystems (Signature Verification - stateless)
    /// 3. Initialize Level 1 subsystems (Peer Discovery, Mempool)
    /// 4. Initialize Level 2 subsystems (Transaction Indexing, State Management)
    /// 5. Initialize Level 4 subsystems (Block Storage, Finality)
    /// 6. Wire event subscriptions for V2.3 Choreography
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
        info!(
            "  [6] Mempool initialized (max {} txs)",
            config.mempool.max_transactions
        );

        // =====================================================================
        // PHASE 4: Level 2 - Transaction Indexing & State Management
        // =====================================================================
        info!("Phase 4: Initializing Level 2 subsystems");

        let transaction_index = Self::init_transaction_indexing();
        info!("  [3] Transaction Indexing initialized");

        let state_trie = Self::init_state_management(&config);
        info!("  [4] State Management initialized");

        // =====================================================================
        // PHASE 5: Level 3 - Consensus
        // =====================================================================
        info!("Phase 5: Initializing Level 3 subsystems");

        let consensus = Self::init_consensus(Arc::clone(&event_bus), Arc::clone(&mempool));
        info!("  [8] Consensus initialized (PoS/PBFT)");

        // =====================================================================
        // PHASE 6: Level 4 - Block Storage & Finality
        // =====================================================================
        info!("Phase 6: Initializing Level 4 subsystems");

        let (block_storage, assembly_buffer) = Self::init_block_storage(&config);
        info!(
            "  [2] Block Storage initialized (timeout={}s, max_pending={})",
            config.storage.assembly_timeout_secs, config.storage.max_pending_assemblies
        );

        #[cfg(not(feature = "rocksdb"))]
        let finality = Self::init_finality(Arc::clone(&block_storage));
        #[cfg(not(feature = "rocksdb"))]
        info!("  [9] Finality initialized (Casper FFG)");

        // =====================================================================
        // PHASE 7: Level 5 - Advanced Subsystems
        // =====================================================================
        info!("Phase 7: Initializing Level 5 advanced subsystems");

        let block_producer = Self::init_block_producer(Arc::clone(&event_bus), &config);
        info!("  [17] Block Production initialized (mining threads={})", config.mining.worker_threads);

        info!("All subsystems initialized successfully");
        info!("Choreography ready: Consensus→TxIndexing→StateManagement→BlockStorage→Finality");

        Self {
            peer_discovery,
            mempool,
            transaction_index,
            state_trie,
            consensus,
            block_storage,
            assembly_buffer,
            #[cfg(not(feature = "rocksdb"))]
            finality,
            block_producer,
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
        use qc_01_peer_discovery::{KademliaConfig, NodeId, TimeSource, Timestamp};

        // Create a simple time source (no panics)
        struct SimpleTimeSource;
        impl TimeSource for SimpleTimeSource {
            fn now(&self) -> Timestamp {
                Timestamp::new(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0), // Fallback to epoch if system time is before UNIX_EPOCH
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

    fn init_transaction_indexing() -> Arc<RwLock<TransactionIndex>> {
        let index_config = IndexConfig::default();
        Arc::new(RwLock::new(TransactionIndex::new(index_config)))
    }

    #[cfg(not(feature = "rocksdb"))]
    fn init_state_management(_config: &NodeConfig) -> Arc<RwLock<PatriciaMerkleTrie>> {
        info!("Initializing State Management with in-memory backend (testing mode)");
        Arc::new(RwLock::new(PatriciaMerkleTrie::new()))
    }

    #[cfg(feature = "rocksdb")]
    fn init_state_management(config: &NodeConfig) -> Arc<RwLock<PatriciaMerkleTrie>> {
        info!("Initializing State Management with RocksDB persistence");
        let db_path = config.storage.data_dir.join("state_db");
        let rocks_config = RocksDbConfig {
            path: db_path.to_string_lossy().to_string(),
            ..RocksDbConfig::default()
        };

        // Open RocksDB for state
        let store = RocksDbStore::open(rocks_config).expect("Failed to open RocksDB for state");
        let trie_db = RocksDbTrieDatabase::new(StdArc::new(store));

        // Try to load existing state, or create new
        let trie = match PatriciaMerkleTrie::load_from_db(&trie_db) {
            Ok(t) => {
                info!("Loaded existing state from RocksDB");
                t
            }
            Err(_) => {
                info!("No existing state found, starting fresh");
                PatriciaMerkleTrie::new()
            }
        };

        Arc::new(RwLock::new(trie))
    }

    fn init_block_storage(
        config: &NodeConfig,
    ) -> (
        Arc<RwLock<ConcreteBlockStorageService>>,
        Arc<RwLock<BlockAssemblyBuffer>>,
    ) {
        // Create assembly buffer for choreography
        let assembly_config = AssemblyConfig {
            assembly_timeout_secs: config.storage.assembly_timeout_secs,
            max_pending_assemblies: config.storage.max_pending_assemblies,
        };
        let assembly_buffer = Arc::new(RwLock::new(BlockAssemblyBuffer::new(assembly_config)));

        let checksum = DefaultChecksumProvider;
        let time_source = StorageTimeSource;
        let serializer = BincodeBlockSerializer;
        let storage_config = qc_02_block_storage::StorageConfig::default();

        // Use RocksDB for production, in-memory for testing
        #[cfg(feature = "rocksdb")]
        let service = {
            info!("Initializing Block Storage with RocksDB backend");
            let db_path = config.storage.data_dir.join("rocksdb");
            let rocks_config = RocksDbConfig {
                path: db_path.to_string_lossy().to_string(),
                ..RocksDbConfig::default()
            };
            let kv_store = RocksDbStore::open(rocks_config).expect("Failed to open RocksDB");
            let fs_adapter = ProductionFileSystemAdapter::new(
                config.storage.data_dir.to_string_lossy().to_string(),
            );

            BlockStorageService::new(
                kv_store,
                fs_adapter,
                checksum,
                time_source,
                serializer,
                storage_config,
            )
        };

        #[cfg(not(feature = "rocksdb"))]
        let service = {
            info!("Initializing Block Storage with in-memory backend (testing mode)");
            let kv_store = InMemoryKVStore::new();
            let fs_adapter = MockFileSystemAdapter::new(50); // 50% disk available

            BlockStorageService::new(
                kv_store,
                fs_adapter,
                checksum,
                time_source,
                serializer,
                storage_config,
            )
        };

        (Arc::new(RwLock::new(service)), assembly_buffer)
    }

    /// Initialize Consensus service with all port adapters.
    fn init_consensus(
        event_bus: Arc<InMemoryEventBus>,
        mempool: Arc<RwLock<TransactionPool>>,
    ) -> Arc<ConcreteConsensusService> {
        // Create port adapters
        let event_bus_adapter = Arc::new(ConsensusEventBusAdapter::new(event_bus));
        let mempool_adapter = Arc::new(ConsensusMempoolAdapter::new(mempool));
        let sig_adapter = Arc::new(ConsensusSignatureAdapter::new());
        let validator_adapter = Arc::new(ConsensusValidatorSetAdapter::new());

        // Create consensus service with default config
        let consensus_config = ConsensusConfig::default();

        Arc::new(ConsensusService::new(
            event_bus_adapter,
            mempool_adapter,
            sig_adapter,
            validator_adapter,
            consensus_config,
        ))
    }

    /// Initialize Finality service with all port adapters.
    #[cfg(not(feature = "rocksdb"))]
    fn init_finality(
        block_storage: Arc<RwLock<ConcreteBlockStorageService>>,
    ) -> Arc<ConcreteFinalityService> {
        // Create port adapters
        let storage_adapter = Arc::new(ConcreteFinalityBlockStorageAdapter::new(block_storage));
        let attestation_adapter = Arc::new(FinalityAttestationAdapter::new());
        let validator_adapter = Arc::new(FinalityValidatorSetAdapter::new());

        // Create finality service with default config
        let finality_config = FinalityConfig::default();

        Arc::new(FinalityService::new(
            finality_config,
            storage_adapter,
            attestation_adapter,
            validator_adapter,
        ))
    }

    /// Initialize Block Producer service (Subsystem 17).
    fn init_block_producer(
        event_bus: Arc<InMemoryEventBus>,
        config: &NodeConfig,
    ) -> Arc<ConcreteBlockProducer> {
        use qc_17_block_production::{BlockProductionConfig, ConsensusMode};
        use primitive_types::U256;

        // Map node config to block production config
        let mut block_config = BlockProductionConfig::default();
        block_config.mode = ConsensusMode::ProofOfStake; // Default to PoS
        block_config.gas_limit = 30_000_000; // 30M gas
        block_config.min_gas_price = U256::from(1_000_000_000u64); // 1 gwei
        block_config.fair_ordering = true;

        // Configure PoW if mining is enabled
        if config.mining.enabled {
            block_config.mode = ConsensusMode::ProofOfWork;
            block_config.pow = Some(qc_17_block_production::PoWConfig {
                threads: config.mining.worker_threads as u8,
                algorithm: qc_17_block_production::HashAlgorithm::Keccak256,
            });
        }

        Arc::new(ConcreteBlockProducer::new(event_bus, block_config))
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

    /// Get transaction index for Merkle operations.
    pub fn transaction_index(&self) -> Arc<RwLock<TransactionIndex>> {
        Arc::clone(&self.transaction_index)
    }

    /// Get state trie for account state operations.
    pub fn state_trie(&self) -> Arc<RwLock<PatriciaMerkleTrie>> {
        Arc::clone(&self.state_trie)
    }

    /// Get consensus service for block validation.
    pub fn consensus(&self) -> Arc<ConcreteConsensusService> {
        Arc::clone(&self.consensus)
    }

    /// Get finality service for Casper FFG operations.
    #[cfg(not(feature = "rocksdb"))]
    pub fn finality(&self) -> Arc<ConcreteFinalityService> {
        Arc::clone(&self.finality)
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
    fn test_transaction_indexing_initialized() {
        let container = SubsystemContainer::new_for_testing();

        // Verify transaction index is accessible
        let _index = container.transaction_index();
    }

    #[test]
    fn test_state_management_initialized() {
        let container = SubsystemContainer::new_for_testing();

        // Verify state trie is accessible
        let trie = container.state_trie();
        // Just verify we can read the root hash (value depends on empty trie implementation)
        let _root = trie.read().root_hash();
    }

    #[test]
    fn test_consensus_initialized() {
        let container = SubsystemContainer::new_for_testing();
        // Verify consensus is accessible
        let _consensus = container.consensus();
    }

    #[cfg(not(feature = "rocksdb"))]
    #[test]
    fn test_finality_initialized() {
        let container = SubsystemContainer::new_for_testing();
        // Verify finality is accessible
        let _finality = container.finality();
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
