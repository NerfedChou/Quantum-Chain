//! # Subsystem Container
//!
//! Holds all core subsystem instances and manages their lifecycle.
//!
//! ## Plug-and-Play Architecture (v2.4)
//!
//! Subsystems are optional and can be enabled/disabled via Cargo features:
//!
//! ```bash
//! # Minimal node (only required subsystems)
//! cargo build --features "qc-02,qc-08,qc-10"
//!
//! # Full node (all subsystems)
//! cargo build --features "full"
//! ```
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
use tracing::{info, instrument, warn};

use shared_bus::{InMemoryEventBus, TimeBoundedNonceCache};
use shared_types::SubsystemRegistry;

#[cfg(feature = "qc-01")]
use crate::adapters::peer_discovery::{RuntimeVerificationPublisher, SharedPeerDiscovery};
#[cfg(feature = "qc-01")]
use qc_01_peer_discovery::adapters::BootstrapHandler;

use crate::container::config::NodeConfig;

// =============================================================================
// CONDITIONAL IMPORTS - Only import enabled subsystems
// =============================================================================

#[cfg(feature = "qc-01")]
use qc_01_peer_discovery::PeerDiscoveryService;

#[cfg(feature = "qc-02")]
use qc_02_block_storage::{
    ports::outbound::{
        BincodeBlockSerializer, DefaultChecksumProvider, SystemTimeSource as StorageTimeSource,
    },
    AssemblyConfig, BlockAssemblyBuffer, BlockStorageService,
};

#[cfg(all(feature = "qc-02", not(feature = "rocksdb")))]
use qc_02_block_storage::ports::outbound::{FileBackedKVStore, MockFileSystemAdapter};

#[cfg(feature = "qc-03")]
use qc_03_transaction_indexing::{IndexConfig, TransactionIndex};

#[cfg(feature = "qc-04")]
use qc_04_state_management::PatriciaMerkleTrie;

#[cfg(feature = "qc-06")]
use qc_06_mempool::TransactionPool;

#[cfg(feature = "qc-08")]
use crate::adapters::ports::consensus::{
    ConsensusEventBusAdapter, ConsensusMempoolAdapter, ConsensusSignatureAdapter,
    ConsensusValidatorSetAdapter,
};
#[cfg(feature = "qc-08")]
use qc_08_consensus::{ConsensusConfig, ConsensusService};

#[cfg(feature = "qc-09")]
use crate::adapters::ports::finality::{
    ConcreteFinalityBlockStorageAdapter, FinalityAttestationAdapter, FinalityValidatorSetAdapter,
};
#[cfg(feature = "qc-09")]
use qc_09_finality::service::{FinalityConfig, FinalityService};

#[cfg(feature = "qc-17")]
use qc_17_block_production::ConcreteBlockProducer;

// RocksDB imports (when feature is enabled)
#[cfg(feature = "rocksdb")]
use crate::adapters::storage::{
    ProductionFileSystemAdapter, RocksDbConfig, RocksDbStore, RocksDbTrieDatabase,
};

#[cfg(feature = "rocksdb")]
use std::sync::Arc as StdArc;

// =============================================================================
// TYPE ALIASES - Conditional based on features
// =============================================================================

/// Concrete type for Block Storage Service with file-backed storage (default).
#[cfg(all(feature = "qc-02", not(feature = "rocksdb")))]
pub type ConcreteBlockStorageService = BlockStorageService<
    FileBackedKVStore,
    MockFileSystemAdapter,
    DefaultChecksumProvider,
    StorageTimeSource,
    BincodeBlockSerializer,
>;

/// Concrete type for Block Storage Service with RocksDB (production).
#[cfg(all(feature = "qc-02", feature = "rocksdb"))]
pub type ConcreteBlockStorageService = BlockStorageService<
    RocksDbStore,
    ProductionFileSystemAdapter,
    DefaultChecksumProvider,
    StorageTimeSource,
    BincodeBlockSerializer,
>;

/// Concrete type for Consensus Service with all adapters wired.
#[cfg(feature = "qc-08")]
pub type ConcreteConsensusService = ConsensusService<
    ConsensusEventBusAdapter,
    ConsensusMempoolAdapter,
    ConsensusSignatureAdapter,
    ConsensusValidatorSetAdapter,
>;

/// Concrete type for Finality Service with all adapters wired (in-memory backends).
#[cfg(all(feature = "qc-09", not(feature = "rocksdb")))]
pub type ConcreteFinalityService = FinalityService<
    ConcreteFinalityBlockStorageAdapter,
    FinalityAttestationAdapter,
    FinalityValidatorSetAdapter,
>;

/// Concrete type for Finality Service with RocksDB backend (production).
#[cfg(all(feature = "qc-09", feature = "rocksdb"))]
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
/// ## V2.4 Plug-and-Play Pattern
///
/// Subsystems are conditionally compiled based on Cargo features.
/// Missing subsystems are represented as `Option::None`.
pub struct SubsystemContainer {
    // =========================================================================
    // LEVEL 0: No Dependencies
    // =========================================================================
    // Signature Verification (10) is stateless - no instance needed in container

    // =========================================================================
    // LEVEL 1: Depends on Level 0
    // =========================================================================
    /// Peer Discovery (Subsystem 1) - Optional
    #[cfg(feature = "qc-01")]
    pub peer_discovery: Arc<RwLock<PeerDiscoveryService>>,
    #[cfg(feature = "qc-01")]
    pub bootstrap_handler:
        Arc<RwLock<BootstrapHandler<SharedPeerDiscovery, RuntimeVerificationPublisher>>>,

    /// Mempool (Subsystem 6) - Optional
    #[cfg(feature = "qc-06")]
    pub mempool: Arc<RwLock<TransactionPool>>,

    // =========================================================================
    // LEVEL 2: Depends on Level 0-1
    // =========================================================================
    /// Transaction Indexing (Subsystem 3) - Optional
    #[cfg(feature = "qc-03")]
    pub transaction_index: Arc<RwLock<TransactionIndex>>,

    /// State Management (Subsystem 4) - Optional
    #[cfg(feature = "qc-04")]
    pub state_trie: Arc<RwLock<PatriciaMerkleTrie>>,

    // =========================================================================
    // LEVEL 3: Depends on Level 0-2
    // =========================================================================
    /// Consensus (Subsystem 8) - Required for block validation
    #[cfg(feature = "qc-08")]
    pub consensus: Arc<ConcreteConsensusService>,

    // =========================================================================
    // LEVEL 4: Depends on Level 0-3
    // =========================================================================
    /// Block Storage (Subsystem 2) - Required for persistence
    #[cfg(feature = "qc-02")]
    pub block_storage: Arc<RwLock<ConcreteBlockStorageService>>,

    /// Block Assembly Buffer (for choreography)
    #[cfg(feature = "qc-02")]
    pub assembly_buffer: Arc<RwLock<BlockAssemblyBuffer>>,

    /// Finality (Subsystem 9) - Optional
    #[cfg(feature = "qc-09")]
    pub finality: Arc<ConcreteFinalityService>,

    // =========================================================================
    // LEVEL 5: Advanced Subsystems
    // =========================================================================
    /// Block Production (Subsystem 17) - Optional
    #[cfg(feature = "qc-17")]
    pub block_producer: Arc<ConcreteBlockProducer>,

    // =========================================================================
    // SHARED INFRASTRUCTURE (Always available)
    // =========================================================================
    /// Event Bus for inter-subsystem communication.
    pub event_bus: Arc<InMemoryEventBus>,

    /// Time-bounded nonce cache for replay prevention.
    pub nonce_cache: Arc<RwLock<TimeBoundedNonceCache>>,

    /// Subsystem registry for plug-and-play management.
    pub registry: Arc<RwLock<SubsystemRegistry>>,

    /// Node configuration (immutable after initialization).
    pub config: NodeConfig,
}

impl SubsystemContainer {
    /// Create a new subsystem container with all enabled subsystems initialized.
    #[instrument(name = "subsystem_init", skip(config))]
    pub fn new(config: NodeConfig) -> Self {
        info!("Initializing Quantum-Chain subsystem container");
        info!("Architecture version: 2.4 (Plug-and-Play Pattern)");

        // Log which subsystems are enabled
        Self::log_enabled_subsystems();

        // =====================================================================
        // PHASE 1: Shared Infrastructure
        // =====================================================================
        info!("Phase 1: Creating shared infrastructure");

        let event_bus = Arc::new(InMemoryEventBus::new());
        let nonce_cache = Arc::new(RwLock::new(TimeBoundedNonceCache::new()));
        let registry = Arc::new(RwLock::new(SubsystemRegistry::new()));

        // =====================================================================
        // PHASE 2: Level 0 - No Dependencies
        // =====================================================================
        info!("Phase 2: Initializing Level 0 subsystems");
        #[cfg(feature = "qc-10")]
        info!("  [10] Signature Verification initialized (stateless)");
        #[cfg(not(feature = "qc-10"))]
        warn!("  [10] Signature Verification DISABLED");

        // =====================================================================
        // PHASE 3: Level 1 - Depends on Level 0
        // =====================================================================
        info!("Phase 3: Initializing Level 1 subsystems");

        #[cfg(feature = "qc-01")]
        let (peer_discovery, bootstrap_handler) = {
            let (pd, bh) = Self::init_peer_discovery(Arc::clone(&event_bus), &config);
            info!("  [1] Peer Discovery & DDoS Defense initialized");
            (pd, bh)
        };

        #[cfg(feature = "qc-06")]
        let mempool = {
            let mp = Self::init_mempool(&config);
            info!(
                "  [6] Mempool initialized (max {} txs)",
                config.mempool.max_transactions
            );
            mp
        };

        #[cfg(not(feature = "qc-01"))]
        warn!("  [1] Peer Discovery DISABLED");
        #[cfg(not(feature = "qc-06"))]
        warn!("  [6] Mempool DISABLED");

        // =====================================================================
        // PHASE 4: Level 2 - Transaction Indexing & State Management
        // =====================================================================
        info!("Phase 4: Initializing Level 2 subsystems");

        #[cfg(feature = "qc-03")]
        let transaction_index = {
            let ti = Self::init_transaction_indexing();
            info!("  [3] Transaction Indexing initialized");
            ti
        };

        #[cfg(feature = "qc-04")]
        let state_trie = {
            let st = Self::init_state_management(&config);
            info!("  [4] State Management initialized");
            st
        };

        #[cfg(not(feature = "qc-03"))]
        warn!("  [3] Transaction Indexing DISABLED");
        #[cfg(not(feature = "qc-04"))]
        warn!("  [4] State Management DISABLED");

        // =====================================================================
        // PHASE 5: Level 3 - Consensus
        // =====================================================================
        info!("Phase 5: Initializing Level 3 subsystems");

        #[cfg(feature = "qc-08")]
        let consensus = {
            #[cfg(feature = "qc-06")]
            let cs =
                Self::init_consensus_with_mempool(Arc::clone(&event_bus), Arc::clone(&mempool));
            #[cfg(not(feature = "qc-06"))]
            let cs = Self::init_consensus_standalone(Arc::clone(&event_bus));

            info!("  [8] Consensus initialized (PoS/PBFT)");
            cs
        };

        #[cfg(not(feature = "qc-08"))]
        warn!("  [8] Consensus DISABLED - blocks will not be validated!");

        // =====================================================================
        // PHASE 6: Level 4 - Block Storage & Finality
        // =====================================================================
        info!("Phase 6: Initializing Level 4 subsystems");

        #[cfg(feature = "qc-02")]
        let (block_storage, assembly_buffer) = {
            let (bs, ab) = Self::init_block_storage(&config);
            info!(
                "  [2] Block Storage initialized (timeout={}s, max_pending={})",
                config.storage.assembly_timeout_secs, config.storage.max_pending_assemblies
            );
            (bs, ab)
        };

        #[cfg(feature = "qc-09")]
        let finality = {
            #[cfg(feature = "qc-02")]
            let fin = Self::init_finality(Arc::clone(&block_storage));
            #[cfg(not(feature = "qc-02"))]
            compile_error!("qc-09 (Finality) requires qc-02 (Block Storage)");

            info!("  [9] Finality initialized (Casper FFG)");
            fin
        };

        #[cfg(not(feature = "qc-02"))]
        warn!("  [2] Block Storage DISABLED - blocks will not be persisted!");
        #[cfg(not(feature = "qc-09"))]
        warn!("  [9] Finality DISABLED");

        // =====================================================================
        // PHASE 7: Level 5 - Advanced Subsystems
        // =====================================================================
        info!("Phase 7: Initializing Level 5 advanced subsystems");

        #[cfg(feature = "qc-17")]
        let block_producer = {
            let bp = Self::init_block_producer(Arc::clone(&event_bus), &config);
            info!(
                "  [17] Block Production initialized (mining threads={})",
                config.mining.worker_threads
            );
            bp
        };

        #[cfg(not(feature = "qc-17"))]
        warn!("  [17] Block Production DISABLED - node will not mine blocks");

        info!("All enabled subsystems initialized successfully");

        Self {
            #[cfg(feature = "qc-01")]
            peer_discovery,
            #[cfg(feature = "qc-01")]
            bootstrap_handler,
            #[cfg(feature = "qc-06")]
            mempool,
            #[cfg(feature = "qc-03")]
            transaction_index,
            #[cfg(feature = "qc-04")]
            state_trie,
            #[cfg(feature = "qc-08")]
            consensus,
            #[cfg(feature = "qc-02")]
            block_storage,
            #[cfg(feature = "qc-02")]
            assembly_buffer,
            #[cfg(feature = "qc-09")]
            finality,
            #[cfg(feature = "qc-17")]
            block_producer,
            event_bus,
            nonce_cache,
            registry,
            config,
        }
    }

    /// Log which subsystems are enabled at compile time.
    fn log_enabled_subsystems() {
        info!("Enabled subsystems:");

        #[cfg(feature = "qc-01")]
        info!("  ✓ qc-01: Peer Discovery");
        #[cfg(feature = "qc-02")]
        info!("  ✓ qc-02: Block Storage");
        #[cfg(feature = "qc-03")]
        info!("  ✓ qc-03: Transaction Indexing");
        #[cfg(feature = "qc-04")]
        info!("  ✓ qc-04: State Management");
        #[cfg(feature = "qc-05")]
        info!("  ✓ qc-05: Block Propagation");
        #[cfg(feature = "qc-06")]
        info!("  ✓ qc-06: Mempool");
        #[cfg(feature = "qc-08")]
        info!("  ✓ qc-08: Consensus");
        #[cfg(feature = "qc-09")]
        info!("  ✓ qc-09: Finality");
        #[cfg(feature = "qc-10")]
        info!("  ✓ qc-10: Signature Verification");
        #[cfg(feature = "qc-16")]
        info!("  ✓ qc-16: API Gateway");
        #[cfg(feature = "qc-17")]
        info!("  ✓ qc-17: Block Production");

        #[cfg(feature = "rocksdb")]
        info!("  ✓ rocksdb: Production storage backend");
        #[cfg(not(feature = "rocksdb"))]
        info!("  ○ rocksdb: Using file-backed storage");
    }

    /// Create a container for testing with in-memory backends.
    #[cfg(test)]
    pub fn new_for_testing() -> Self {
        Self::new(NodeConfig::default())
    }

    // =========================================================================
    // SUBSYSTEM INITIALIZATION METHODS
    // =========================================================================

    #[cfg(feature = "qc-01")]
    fn init_peer_discovery(
        event_bus: Arc<InMemoryEventBus>,
        _config: &NodeConfig,
    ) -> (
        Arc<RwLock<PeerDiscoveryService>>,
        Arc<RwLock<BootstrapHandler<SharedPeerDiscovery, RuntimeVerificationPublisher>>>,
    ) {
        use qc_01_peer_discovery::{
            adapters::network::ProofOfWorkValidator, KademliaConfig, NodeId, SystemTimeSource,
            TimeSource,
        };

        let local_node_id = NodeId::new(rand::random());
        let kademlia_config = KademliaConfig::default();
        let time_source: Box<dyn TimeSource> = Box::new(SystemTimeSource);

        let service = Arc::new(RwLock::new(PeerDiscoveryService::new(
            local_node_id,
            kademlia_config,
            Box::new(SystemTimeSource), // Separate instance
        )));

        let shared_service = SharedPeerDiscovery {
            inner: service.clone(),
        };

        let verification_publisher = RuntimeVerificationPublisher::new(event_bus);
        let node_id_validator = ProofOfWorkValidator::new(16); // 16 bits = 2 zero bytes

        // Instantiate additional time source for handler
        let handler_time_source: Box<dyn TimeSource> = Box::new(SystemTimeSource);

        let bootstrap_handler = Arc::new(RwLock::new(BootstrapHandler::new(
            shared_service,
            verification_publisher,
            Box::new(node_id_validator),
            handler_time_source,
        )));

        (service, bootstrap_handler)
    }

    #[cfg(feature = "qc-06")]
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

    #[cfg(feature = "qc-03")]
    fn init_transaction_indexing() -> Arc<RwLock<TransactionIndex>> {
        let index_config = IndexConfig::default();
        Arc::new(RwLock::new(TransactionIndex::new(index_config)))
    }

    #[cfg(all(feature = "qc-04", not(feature = "rocksdb")))]
    fn init_state_management(_config: &NodeConfig) -> Arc<RwLock<PatriciaMerkleTrie>> {
        info!("Initializing State Management with in-memory backend (testing mode)");
        Arc::new(RwLock::new(PatriciaMerkleTrie::new()))
    }

    #[cfg(all(feature = "qc-04", feature = "rocksdb"))]
    fn init_state_management(config: &NodeConfig) -> Arc<RwLock<PatriciaMerkleTrie>> {
        info!("Initializing State Management with RocksDB persistence");
        let db_path = config.storage.data_dir.join("state_db");
        let rocks_config = RocksDbConfig {
            path: db_path.to_string_lossy().to_string(),
            ..RocksDbConfig::default()
        };

        let store = RocksDbStore::open(rocks_config).expect("Failed to open RocksDB for state");
        let trie_db = RocksDbTrieDatabase::new(StdArc::new(store));

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

    #[cfg(feature = "qc-02")]
    fn init_block_storage(
        config: &NodeConfig,
    ) -> (
        Arc<RwLock<ConcreteBlockStorageService>>,
        Arc<RwLock<BlockAssemblyBuffer>>,
    ) {
        let assembly_config = AssemblyConfig {
            assembly_timeout_secs: config.storage.assembly_timeout_secs,
            max_pending_assemblies: config.storage.max_pending_assemblies,
        };
        let assembly_buffer = Arc::new(RwLock::new(BlockAssemblyBuffer::new(assembly_config)));

        let checksum = DefaultChecksumProvider;
        let time_source = StorageTimeSource;
        let serializer = BincodeBlockSerializer;
        let storage_config = qc_02_block_storage::StorageConfig::default();

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
            let data_dir = std::env::var("QC_DATA_DIR")
                .unwrap_or_else(|_| "/var/quantum-chain/data".to_string());
            let storage_path = std::path::PathBuf::from(&data_dir).join("blocks.db");
            info!(
                "Initializing Block Storage with file-backed persistence at {}",
                storage_path.display()
            );

            let kv_store = FileBackedKVStore::new(&storage_path);
            let fs_adapter = MockFileSystemAdapter::new(50);

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

    #[cfg(all(feature = "qc-08", feature = "qc-06"))]
    fn init_consensus_with_mempool(
        event_bus: Arc<InMemoryEventBus>,
        mempool: Arc<RwLock<TransactionPool>>,
    ) -> Arc<ConcreteConsensusService> {
        let event_bus_adapter = Arc::new(ConsensusEventBusAdapter::new(event_bus));
        let mempool_adapter = Arc::new(ConsensusMempoolAdapter::new(mempool));
        let sig_adapter = Arc::new(ConsensusSignatureAdapter::new());
        let validator_adapter = Arc::new(ConsensusValidatorSetAdapter::new());

        let consensus_config = ConsensusConfig::default();

        Arc::new(ConsensusService::new(
            event_bus_adapter,
            mempool_adapter,
            sig_adapter,
            validator_adapter,
            consensus_config,
        ))
    }

    #[cfg(all(feature = "qc-08", not(feature = "qc-06")))]
    fn init_consensus_standalone(
        event_bus: Arc<InMemoryEventBus>,
    ) -> Arc<ConcreteConsensusService> {
        let event_bus_adapter = Arc::new(ConsensusEventBusAdapter::new(event_bus));
        let mempool_adapter = Arc::new(ConsensusMempoolAdapter::new());
        let sig_adapter = Arc::new(ConsensusSignatureAdapter::new());
        let validator_adapter = Arc::new(ConsensusValidatorSetAdapter::new());

        let consensus_config = ConsensusConfig::default();

        Arc::new(ConsensusService::new(
            event_bus_adapter,
            mempool_adapter,
            sig_adapter,
            validator_adapter,
            consensus_config,
        ))
    }

    #[cfg(all(feature = "qc-09", feature = "qc-02"))]
    fn init_finality(
        block_storage: Arc<RwLock<ConcreteBlockStorageService>>,
    ) -> Arc<ConcreteFinalityService> {
        let storage_adapter = Arc::new(ConcreteFinalityBlockStorageAdapter::new(block_storage));
        let attestation_adapter = Arc::new(FinalityAttestationAdapter::new());
        let validator_adapter = Arc::new(FinalityValidatorSetAdapter::new());

        let finality_config = FinalityConfig::default();

        Arc::new(FinalityService::new(
            finality_config,
            storage_adapter,
            attestation_adapter,
            validator_adapter,
        ))
    }

    #[cfg(feature = "qc-17")]
    fn init_block_producer(
        event_bus: Arc<InMemoryEventBus>,
        config: &NodeConfig,
    ) -> Arc<ConcreteBlockProducer> {
        use primitive_types::U256;
        use qc_17_block_production::{BlockProductionConfig, ConsensusMode};

        let mut block_config = BlockProductionConfig::default();
        block_config.mode = ConsensusMode::ProofOfStake;
        block_config.gas_limit = 30_000_000;
        block_config.min_gas_price = U256::from(1_000_000_000u64);
        block_config.fair_ordering = true;

        if config.mining.enabled {
            block_config.mode = ConsensusMode::ProofOfWork;
            block_config.pow = Some(qc_17_block_production::PoWConfig {
                threads: config.mining.worker_threads as u8,
                algorithm: qc_17_block_production::HashAlgorithm::Keccak256,
                target_block_time: Some(10),
                use_dgw: Some(true),
                dgw_window: Some(24),
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

    /// Get transaction index for Merkle operations (if enabled).
    #[cfg(feature = "qc-03")]
    pub fn transaction_index(&self) -> Arc<RwLock<TransactionIndex>> {
        Arc::clone(&self.transaction_index)
    }

    /// Get state trie for account state operations (if enabled).
    #[cfg(feature = "qc-04")]
    pub fn state_trie(&self) -> Arc<RwLock<PatriciaMerkleTrie>> {
        Arc::clone(&self.state_trie)
    }

    /// Get consensus service for block validation (if enabled).
    #[cfg(feature = "qc-08")]
    pub fn consensus(&self) -> Arc<ConcreteConsensusService> {
        Arc::clone(&self.consensus)
    }

    /// Get finality service for Casper FFG operations (if enabled).
    #[cfg(feature = "qc-09")]
    pub fn finality(&self) -> Arc<ConcreteFinalityService> {
        Arc::clone(&self.finality)
    }

    /// Check if a subsystem is enabled.
    pub fn is_subsystem_enabled(id: u8) -> bool {
        match id {
            1 => cfg!(feature = "qc-01"),
            2 => cfg!(feature = "qc-02"),
            3 => cfg!(feature = "qc-03"),
            4 => cfg!(feature = "qc-04"),
            5 => cfg!(feature = "qc-05"),
            6 => cfg!(feature = "qc-06"),
            8 => cfg!(feature = "qc-08"),
            9 => cfg!(feature = "qc-09"),
            10 => cfg!(feature = "qc-10"),
            16 => cfg!(feature = "qc-16"),
            17 => cfg!(feature = "qc-17"),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_initialization() {
        let container = SubsystemContainer::new_for_testing();
        assert_eq!(container.event_bus.subscriber_count(), 0);
    }

    #[test]
    fn test_subsystem_enabled_check() {
        // These should reflect the features enabled in test builds
        #[cfg(feature = "qc-02")]
        assert!(SubsystemContainer::is_subsystem_enabled(2));

        // Non-existent subsystem
        assert!(!SubsystemContainer::is_subsystem_enabled(99));
    }
}
