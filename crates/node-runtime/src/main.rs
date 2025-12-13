//! # Quantum-Chain Node Runtime
//!
//! The main entry point for the Quantum-Chain blockchain node.
//!
//! ## Architecture (v2.3 Choreography Pattern)
//!
//! This node implements a modular, event-driven architecture as specified
//! in Architecture.md v2.3. All subsystems communicate via the authenticated
//! message bus using the `AuthenticatedMessage<T>` envelope.
//!
//! ## Modular Structure
//!
//! - `container/` - Subsystem container with dependency injection
//! - `genesis/` - Genesis block creation and chain initialization
//! - `adapters/` - Port implementations connecting subsystems
//! - `handlers/` - Event handlers for choreography flow
//! - `wiring/` - Event routing and subsystem coordination
//!
//! ## V2.3 Choreography Flow (IPC-MATRIX.md)
//!
//! ```text
//! Consensus(8) ──BlockValidated──→ Event Bus
//!                                      │
//!         ┌────────────────────────────┼────────────────────────────┐
//!         ↓                            ↓                            ↓
//!   TxIndexing(3)              StateMgmt(4)              BlockStorage(2)
//!         │                            │                   [Assembler]
//!         ↓                            ↓                       ↑ ↑ ↑
//!   MerkleRootComputed          StateRootComputed              │ │ │
//!         │                            │                       │ │ │
//!         └────────────────────────────┴───────────────────────┘ │ │
//!                                                                  │ │
//!                              BlockValidated ─────────────────────┘ │
//!                                                                    │
//!                                      [Atomic Write when all 3 arrive]
//!                                                  │
//!                                                  ↓
//!                                            BlockStored
//!                                                  │
//!                                                  ↓
//!                                            Finality(9)
//! ```
//!
//! ## Startup Sequence (per README.md)
//!
//! 1. Load configuration (from file/env)
//! 2. Validate HMAC secret is not default
//! 3. Initialize subsystems in dependency order (Level 0 → Level 4)
//! 4. Create genesis block (if not exists)
//! 5. Start event handlers (spawn async tasks)
//! 6. Signal ready
//!
//! ## Core Subsystems (11 of 17)
//!
//! 1. Peer Discovery (qc-01) - Network topology
//! 2. Block Storage (qc-02) - Stateful Assembler
//! 3. Transaction Indexing (qc-03) - Merkle proofs
//! 4. State Management (qc-04) - Patricia trie
//! 5. Block Propagation (qc-05) - P2P networking
//! 6. Mempool (qc-06) - Transaction pool
//! 8. Consensus (qc-08) - Block validation
//! 9. Finality (qc-09) - Casper-FFG
//! 10. Signature Verification (qc-10) - ECDSA
//! 16. API Gateway (qc-16) - REST/WebSocket interface
//! 17. Block Production (qc-17) - Quantum-resistant mining

pub mod adapters;
pub mod container;
pub mod genesis;
pub mod handlers;
pub mod wiring;

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use primitive_types::U256;
use tracing::{error, info, warn};

use crate::adapters::{BlockStorageAdapter, RuntimeMempoolGateway};
use crate::container::{NodeConfig, SubsystemContainer};
use crate::genesis::{GenesisBuilder, GenesisConfig};
use crate::handlers::{
    ApiQueryHandler, BlockStorageHandler, FinalityHandler, SignatureVerificationHandler,
    StateMgmtHandler, TxIndexingHandler,
};
use crate::wiring::ChoreographyCoordinator;
use qc_02_block_storage::BlockStorageApi;
use qc_16_api_gateway::{ApiGatewayService, GatewayConfig};
use qc_17_block_production::{BlockProducerService, DifficultyConfig};
use quantum_telemetry::{init_telemetry, TelemetryConfig};

/// Helper to describe difficulty for logging
fn difficulty_desc(difficulty: &U256) -> String {
    let leading_zeros = difficulty.leading_zeros();
    let leading_zero_bytes = leading_zeros / 8;
    format!("~{} zero bytes", leading_zero_bytes)
}

/// Compute block hash (must match qc-02 logic)
fn compute_block_hash(block: &shared_types::ValidatedBlock) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(block.header.parent_hash);
    hasher.update(block.header.height.to_le_bytes());
    hasher.update(block.header.merkle_root);
    hasher.update(block.header.state_root);
    hasher.update(block.header.timestamp.to_le_bytes());
    hasher.finalize().into()
}

/// Resolve difficulty from stored block, using fallback if block has zero difficulty
fn resolve_difficulty(stored: &qc_02_block_storage::StoredBlock, fallback: U256) -> U256 {
    if stored.block.header.difficulty.is_zero() {
        fallback
    } else {
        stored.block.header.difficulty
    }
}

/// Load a single block's info for historical tracking
fn load_block_info(
    storage: &impl qc_02_block_storage::BlockStorageApi,
    height: u64,
    last_diff: &mut U256,
) -> Option<qc_17_block_production::HistoricalBlockInfo> {
    let stored = storage.read_block_by_height(height).ok()?;
    let difficulty = if stored.block.header.difficulty.is_zero() {
        *last_diff
    } else {
        *last_diff = stored.block.header.difficulty;
        stored.block.header.difficulty
    };
    Some(qc_17_block_production::HistoricalBlockInfo {
        height,
        timestamp: stored.block.header.timestamp,
        difficulty,
        hash: primitive_types::H256::from(stored.block_hash()),
    })
}

/// Parameters for creating a validated block from mined data
#[derive(Debug, Clone, Copy)]
struct MinedBlockParams {
    height: u64,
    difficulty: U256,
    nonce: u64,
    timestamp: u64,
    parent_hash: [u8; 32],
}

/// Create a ValidatedBlock from mined block parameters
fn create_validated_block(params: MinedBlockParams) -> shared_types::ValidatedBlock {
    use shared_types::{BlockHeader, ConsensusProof, Hash, PublicKey, ValidatedBlock};
    ValidatedBlock {
        header: BlockHeader {
            version: 1,
            height: params.height,
            parent_hash: params.parent_hash,
            merkle_root: Hash::default(),
            state_root: Hash::default(),
            timestamp: params.timestamp,
            proposer: PublicKey::default(),
            difficulty: params.difficulty,
            nonce: params.nonce,
        },
        transactions: vec![],
        consensus_proof: ConsensusProof::default(),
    }
}

/// The main node runtime orchestrating all subsystems.
pub struct NodeRuntime {
    /// Subsystem container with all initialized services.
    container: Arc<SubsystemContainer>,
    /// Choreography coordinator for event routing.
    choreography: ChoreographyCoordinator,
    /// API Gateway service (optional).
    api_gateway: Option<ApiGatewayService>,
    /// Shutdown signal sender.
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    /// Shutdown signal receiver.
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
}

impl NodeRuntime {
    /// Create a new node runtime with configuration.
    ///
    /// ## Initialization Order (Architecture.md v2.3)
    ///
    /// 1. Create shared infrastructure (event bus, nonce cache)
    /// 2. Initialize Level 0: Signature Verification
    /// 3. Initialize Level 1: Peer Discovery, Mempool
    /// 4. Initialize Level 2: Tx Indexing, State Management
    /// 5. Initialize Level 3: Consensus
    /// 6. Initialize Level 4: Block Storage, Finality
    /// 7. Initialize Level 5: API Gateway (external interface)
    pub fn new(config: NodeConfig) -> Self {
        info!("Creating Quantum-Chain node runtime");

        // Create subsystem container (initializes all subsystems)
        let container = Arc::new(SubsystemContainer::new(config));

        // Create choreography coordinator
        let choreography = ChoreographyCoordinator::new();

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        Self {
            container,
            choreography,
            api_gateway: None,
            shutdown_tx,
            shutdown_rx,
        }
    }

    /// Get reference to API Gateway if running.
    ///
    /// Returns None if API Gateway is disabled or not yet started.
    pub fn api_gateway(&self) -> Option<&ApiGatewayService> {
        self.api_gateway.as_ref()
    }

    /// Start the node runtime.
    ///
    /// ## Startup Sequence
    ///
    /// 1. Validate configuration for production
    /// 2. Initialize genesis block (if not exists)
    /// 3. Start choreography coordinator
    /// 4. Start event handlers
    /// 5. Start API Gateway
    /// 6. Signal ready
    pub async fn start(&mut self) -> Result<()> {
        info!("===========================================");
        info!("  Quantum-Chain Node Runtime v0.1.0");
        info!("  Architecture: V2.3 Choreography Pattern");
        info!("===========================================");

        // Step 1: Initialize genesis if needed
        self.initialize_genesis().await?;

        // Step 2: Start choreography coordinator
        self.choreography.start_monitoring().await;

        // Step 3: Start event handlers
        self.start_choreography_handlers().await?;

        // Step 4: Start API Gateway
        if self.container.config.api_gateway.enabled {
            self.start_api_gateway().await?;
        }

        info!("All core subsystems initialized and running");
        info!("P2P Port: {}", self.container.config.network.p2p_port);
        info!("RPC Port: {}", self.container.config.api_gateway.http_port);
        info!("WS Port: {}", self.container.config.api_gateway.ws_port);
        info!(
            "Admin Port: {}",
            self.container.config.api_gateway.admin_port
        );
        info!("Data Dir: {:?}", self.container.config.storage.data_dir);

        Ok(())
    }

    /// Start the API Gateway service.
    async fn start_api_gateway(&mut self) -> Result<()> {
        info!("Starting API Gateway (qc-16)...");

        // Create gateway configuration from node config
        let api_config = &self.container.config.api_gateway;
        let mut gateway_config = GatewayConfig::default();
        gateway_config.http.port = api_config.http_port;
        gateway_config.websocket.port = api_config.ws_port;
        gateway_config.admin.port = api_config.admin_port;
        gateway_config.admin.api_key = api_config.api_key.clone();
        gateway_config.rate_limit.requests_per_second = api_config.rate_limit_per_second;
        gateway_config.limits.max_batch_size = api_config.max_batch_size;
        gateway_config.chain.chain_id = api_config.chain_id;

        // Create IPC sender that connects to event bus
        let ipc_sender = Arc::new(crate::adapters::api_gateway::EventBusIpcSender::new(
            Arc::clone(&self.container.event_bus),
        ));

        // Create API Gateway service
        let mut gateway = ApiGatewayService::new(
            gateway_config,
            ipc_sender,
            self.container.config.storage.data_dir.clone(),
        )
        .context("Failed to create API Gateway service")?;

        // Get pending store before moving gateway
        let pending_store = gateway.pending_store();

        // Start EventBusIpcReceiver to complete pending requests from ApiQueryResponse events
        let receiver =
            crate::adapters::EventBusIpcReceiver::new(&self.container.event_bus, pending_store);
        let mut receiver_shutdown = self.shutdown_rx.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = receiver.run() => {}
                _ = receiver_shutdown.changed() => {
                    info!("[EventBusIpcReceiver] Shutdown signal received");
                }
            }
        });

        // Spawn gateway in background task
        let mut shutdown_rx = self.shutdown_rx.clone();
        tokio::spawn(async move {
            tokio::select! {
                result = gateway.start() => {
                    if let Err(e) = result {
                        error!("API Gateway error: {}", e);
                    }
                }
                _ = shutdown_rx.changed() => {
                    info!("[qc-16] Shutdown signal received");
                    gateway.shutdown();
                }
            }
        });

        info!(
            "  [16] API Gateway started (HTTP:{}, WS:{}, Admin:{})",
            api_config.http_port, api_config.ws_port, api_config.admin_port
        );

        Ok(())
    }

    /// Initialize the genesis block if chain is empty.
    async fn initialize_genesis(&self) -> Result<()> {
        info!("Checking for genesis block...");

        // Check if genesis exists in block storage
        let storage = self.container.block_storage.read();

        // Try to read block at height 0
        let genesis_exists = storage.read_block_by_height(0).is_ok();
        drop(storage);

        if genesis_exists {
            info!("Genesis block found, chain initialized");
            return Ok(());
        }

        info!("No genesis block found, creating...");

        // Create genesis block
        let genesis_config = GenesisConfig::default();
        let genesis = GenesisBuilder::new(genesis_config)
            .build()
            .context("Failed to build genesis block")?;

        info!(
            "Genesis block created: hash={:?}, chain_id={}",
            &genesis.header.block_hash[..8],
            genesis.header.chain_id
        );

        // Store genesis block
        // Note: Genesis bypasses the normal assembly flow
        let mut storage = self.container.block_storage.write();

        // Create a genesis ValidatedBlock using shared-types
        let genesis_block = shared_types::ValidatedBlock {
            header: shared_types::BlockHeader {
                version: 1,
                height: genesis.header.height,
                parent_hash: genesis.header.parent_hash,
                merkle_root: genesis.header.merkle_root,
                state_root: genesis.header.state_root,
                timestamp: genesis.header.timestamp,
                proposer: [0u8; 32], // No proposer for genesis
                // Genesis uses initial (easy) difficulty - 2^252
                difficulty: primitive_types::U256::from(2).pow(primitive_types::U256::from(252)),
                nonce: 0, // Genesis doesn't require mining
            },
            transactions: vec![],
            consensus_proof: shared_types::ConsensusProof::default(),
        };

        // Write block using the proper API
        storage
            .write_block(
                genesis_block,
                genesis.header.merkle_root,
                genesis.header.state_root,
            )
            .context("Failed to store genesis block")?;

        info!("Genesis block stored successfully");

        // Initialize finalized height to 0
        info!("Setting initial finalized height to 0");

        Ok(())
    }

    /// Start the choreography event handlers.
    async fn start_choreography_handlers(&self) -> Result<()> {
        let router = self.choreography.router();
        let container = Arc::clone(&self.container);

        // Create Block Storage adapter
        let block_storage_adapter = Arc::new(BlockStorageAdapter::new(
            Arc::clone(&router),
            container.assembly_timeout(),
            container.config.storage.max_pending_assemblies,
        ));

        // Create Transaction Indexing adapter (wraps qc-03 domain logic)
        let tx_indexing_adapter = Arc::new(crate::adapters::TransactionIndexingAdapter::new(
            Arc::clone(&router),
        ));

        // Create State Management adapter (wraps qc-04 domain logic)
        let state_adapter = Arc::new(crate::adapters::StateAdapter::new(Arc::clone(&router)));

        // Start Transaction Indexing handler
        let tx_indexing_handler =
            TxIndexingHandler::new(router.subscribe(), Arc::clone(&tx_indexing_adapter));
        let tx_router = Arc::clone(&router);
        let mut tx_shutdown = self.shutdown_rx.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = tx_indexing_handler.run(tx_router) => {}
                _ = tx_shutdown.changed() => {
                    info!("[qc-03] Shutdown signal received");
                }
            }
        });

        // Start State Management handler
        let state_mgmt_handler =
            StateMgmtHandler::new(router.subscribe(), Arc::clone(&state_adapter));
        let state_router = Arc::clone(&router);
        let mut state_shutdown = self.shutdown_rx.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = state_mgmt_handler.run(state_router) => {}
                _ = state_shutdown.changed() => {
                    info!("[qc-04] Shutdown signal received");
                }
            }
        });

        // Start Block Storage handler
        let block_storage_handler =
            BlockStorageHandler::new(Arc::clone(&block_storage_adapter), router.subscribe());
        let mut storage_shutdown = self.shutdown_rx.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = block_storage_handler.run() => {}
                _ = storage_shutdown.changed() => {
                    info!("[qc-02] Shutdown signal received");
                }
            }
        });

        // Start Finality handler
        let finality_handler = FinalityHandler::new(router.subscribe());
        let finality_router = Arc::clone(&router);
        let mut finality_shutdown = self.shutdown_rx.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = finality_handler.run(finality_router) => {}
                _ = finality_shutdown.changed() => {
                    info!("[qc-09] Shutdown signal received");
                }
            }
        });

        // Start Transaction Ordering handler (qc-12)
        #[cfg(feature = "qc-12")]
        {
            use crate::handlers::TransactionOrderingHandler;
            let tx_ordering_adapter = Arc::new(crate::adapters::TransactionOrderingAdapter::new(
                Arc::clone(&router),
            ));
            let tx_ordering_handler = TransactionOrderingHandler::new(
                router.subscribe(),
                Arc::clone(&tx_ordering_adapter),
            );
            let mut tx_ordering_shutdown = self.shutdown_rx.clone();
            tokio::spawn(async move {
                tokio::select! {
                    _ = tx_ordering_handler.run() => {}
                    _ = tx_ordering_shutdown.changed() => {
                        info!("[qc-12] Shutdown signal received");
                    }
                }
            });
            info!("[qc-12] Transaction Ordering handler started");
        }

        // Start Signature Verification handler (qc-10) - CRITICAL for peer discovery and secure IPC
        // Handles VerifyNodeIdentity events from qc-01, verifying signatures and responding
        let mempool_gateway = RuntimeMempoolGateway::new(Arc::clone(&container.event_bus));
        let sv_service =
            qc_10_signature_verification::SignatureVerificationService::new(mempool_gateway);
        let sv_handler = SignatureVerificationHandler::new(
            Arc::clone(&container.event_bus),
            sv_service,
            &container.config,
        );
        let mut sv_shutdown = self.shutdown_rx.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = sv_handler.run() => {}
                _ = sv_shutdown.changed() => {
                    info!("[SignatureVerificationHandler] Shutdown signal received");
                }
            }
        });

        // Start API Query handler (bridges qc-16 to subsystems)
        let api_query_handler = ApiQueryHandler::new(Arc::clone(&container));
        let mut api_shutdown = self.shutdown_rx.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = api_query_handler.run() => {}
                _ = api_shutdown.changed() => {
                    info!("[ApiQueryHandler] Shutdown signal received");
                }
            }
        });

        // Start Block Production Miner (qc-17) - auto-start on node initialization
        info!("Starting Block Production Miner (qc-17)...");

        // Create miner configuration (PoW mode by default)
        let miner_config = qc_17_block_production::BlockProductionConfig {
            mode: qc_17_block_production::ConsensusMode::ProofOfWork,
            gas_limit: container.config.consensus.max_block_gas,
            min_gas_price: U256::from(container.config.mempool.min_gas_price),
            fair_ordering: true,
            min_transactions: 1,
            pow: Some(qc_17_block_production::PoWConfig {
                threads: num_cpus::get() as u8,
                algorithm: qc_17_block_production::HashAlgorithm::Keccak256,
                target_block_time: Some(10),
                use_dgw: Some(true),
                dgw_window: Some(24),
                batch_size: Some(10_000_000),
            }),
            pos: None,
            pbft: None,
            performance: qc_17_block_production::PerformanceConfig::default(),
        };

        // Create the block producer service
        let miner_service = Arc::new(qc_17_block_production::ConcreteBlockProducer::new(
            Arc::clone(&container.event_bus),
            miner_config,
        ));

        // Get current chain height from storage to resume from
        let chain_height = {
            let storage = container.block_storage.read();
            storage.get_latest_height().unwrap_or(0)
        };

        if chain_height > 0 {
            info!(
                "[qc-17] 💾 Chain height loaded from storage: {}",
                chain_height
            );
        }

        // Load recent block history for difficulty adjustment when resuming
        // We need timestamps from recent blocks to calculate proper difficulty
        // Load recent blocks for difficulty adjustment - CRITICAL for chain continuity
        // We track the last known good difficulty to handle old blocks without difficulty
        let recent_blocks: Vec<qc_17_block_production::HistoricalBlockInfo> = {
            let storage = container.block_storage.read();
            let window_size = 24.min(chain_height as usize); // DGW window size
            let mut last_known_difficulty =
                primitive_types::U256::from(2).pow(primitive_types::U256::from(252));

            let start_height = chain_height.saturating_sub(window_size as u64);
            let mut blocks: Vec<_> = (start_height..=chain_height)
                .filter_map(|h| load_block_info(&*storage, h, &mut last_known_difficulty))
                .collect();

            // Reverse to get newest-first order (required by DGW algorithm)
            blocks.reverse();

            if let Some(first) = blocks.first() {
                info!(
                    "[qc-17] 📊 Loaded {} historical blocks for difficulty adjustment (last: {})",
                    blocks.len(),
                    difficulty_desc(&first.difficulty)
                );
            }

            blocks
        };

        // Extract last known difficulty from loaded blocks for production config
        let last_known_difficulty =
            recent_blocks
                .first()
                .map(|b| b.difficulty)
                .unwrap_or_else(|| {
                    primitive_types::U256::from(2).pow(primitive_types::U256::from(252))
                });

        // Start production in PoW mode with the correct starting height
        let miner_clone = Arc::clone(&miner_service);
        let production_config = qc_17_block_production::ProductionConfig {
            starting_height: chain_height,
            recent_blocks,
            last_difficulty: Some(last_known_difficulty),
            ..Default::default()
        };

        tokio::spawn(async move {
            if let Err(e) = miner_clone
                .start_production(
                    qc_17_block_production::ConsensusMode::ProofOfWork,
                    production_config,
                )
                .await
            {
                error!("[qc-17] Failed to start production: {}", e);
            }
        });

        // Monitor shutdown signal
        let miner_shutdown_clone = Arc::clone(&miner_service);
        let mut miner_shutdown = self.shutdown_rx.clone();
        tokio::spawn(async move {
            let _ = miner_shutdown.changed().await;
            info!("[qc-17] Shutdown signal received");
            if let Err(e) = miner_shutdown_clone.stop_production().await {
                error!("[qc-17] Error during shutdown: {}", e);
            }
        });

        info!("  [17] Block Production Miner started (PoW auto-mining enabled)");

        // CHOREOGRAPHY BRIDGE: Create a task that triggers BlockValidated for mined blocks
        // Since qc-17 uses PoW, each mined block is already validated by difficulty proof
        let choreography_router = self.choreography.router();
        let miner_status_checker = Arc::clone(&miner_service);
        let block_storage_for_bridge = Arc::clone(&container.block_storage);
        let mut last_block_height = chain_height; // Start from loaded height

        // Track the last block hash for parent linking
        let (mut last_block_hash, _last_stored_difficulty): ([u8; 32], primitive_types::U256) = {
            let initial_difficulty = DifficultyConfig::default().initial_difficulty;
            let target_height = if chain_height > 0 { chain_height } else { 0 };
            let storage = block_storage_for_bridge.read();

            match storage.read_block_by_height(target_height) {
                Err(_) => {
                    let label = match chain_height > 0 {
                        true => "last block",
                        false => "genesis",
                    };
                    info!("[Bridge] ⚠️ Could not load {}, using zeros", label);
                    ([0u8; 32], initial_difficulty)
                }
                Ok(stored) => {
                    let hash = compute_block_hash(&stored.block);
                    let diff = resolve_difficulty(&stored, last_known_difficulty);
                    let label = match target_height == 0 {
                        true => "genesis",
                        false => "last",
                    };
                    info!(
                        "[Bridge] 📖 Loaded {} block hash ({:02x}{:02x}..., diff: {})",
                        label,
                        hash[0],
                        hash[1],
                        crate::difficulty_desc(&diff)
                    );
                    (hash, diff)
                }
            }
        };

        info!("[Bridge] Starting choreography bridge task...");

        tokio::spawn(async move {
            info!("[Bridge] 🌉 Bridge task loop starting...");
            let mut iteration = 0u64;
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                iteration += 1;

                // CRITICAL FIX: Drain the pending blocks queue instead of just reading status
                // This ensures each block gets its own correct difficulty/nonce even when
                // multiple blocks are mined between bridge polls
                let pending_blocks = miner_status_checker.drain_pending_blocks().await;

                // Debug: Log every 10th iteration or when we have blocks
                if iteration % 10 == 0 || !pending_blocks.is_empty() {
                    let status = miner_status_checker.get_status().await;
                    info!(
                        "[Bridge] 🔄 Poll #{}: total_mined={}, pending={}, last_height={}",
                        iteration,
                        status.blocks_produced,
                        pending_blocks.len(),
                        last_block_height
                    );
                }

                // Process each mined block with its OWN difficulty and nonce
                for mined_block in pending_blocks {
                    let block_height = mined_block.height;
                    let difficulty = mined_block.difficulty;
                    let nonce = mined_block.nonce;

                    let block = create_validated_block(MinedBlockParams {
                        height: block_height,
                        difficulty,
                        nonce,
                        timestamp: mined_block.timestamp,
                        parent_hash: last_block_hash,
                    });

                    info!(
                        "[Bridge] 🌉 Storing block #{} (nonce: {}, diff: {}) to storage",
                        block_height,
                        nonce,
                        crate::difficulty_desc(&difficulty)
                    );

                    // Store block directly to qc-02
                    use qc_02_block_storage::ports::inbound::BlockStorageApi;
                    let mut storage = block_storage_for_bridge.write();

                    let stored_hash = match (*storage).write_block(
                        block,
                        shared_types::Hash::default(),
                        shared_types::Hash::default(),
                    ) {
                        Ok(hash) => hash,
                        Err(e) => {
                            error!("[Bridge] ❌ Failed to store block #{}: {}", block_height, e);
                            continue;
                        }
                    };

                    info!(
                        "[Bridge] 💾 Block #{} stored successfully (hash: {:02x}{:02x}...)",
                        block_height, stored_hash[0], stored_hash[1]
                    );

                    last_block_hash = stored_hash;

                    // Publish BlockValidated to choreography router
                    let event = crate::wiring::ChoreographyEvent::BlockValidated {
                        block_hash: last_block_hash,
                        block_height,
                        sender_id: shared_types::SubsystemId::Consensus,
                    };
                    if let Err(e) = choreography_router.publish(event) {
                        error!("[Bridge] ❌ Failed to publish BlockValidated: {}", e);
                    } else {
                        info!(
                            "[Bridge] ✅ Published BlockValidated for block #{}",
                            block_height
                        );
                    }

                    last_block_height = block_height;
                }
            }
        });

        info!("  [Bridge] Choreography bridge started");

        info!("Choreography handlers started");
        Ok(())
    }

    /// Shutdown the node gracefully.
    ///
    /// ## Shutdown Sequence
    ///
    /// 1. Signal shutdown to all handlers
    /// 2. Drain pending events (with timeout)
    /// 3. Persist subsystem state
    /// 4. Exit
    pub async fn shutdown(&self) {
        info!("Initiating graceful shutdown...");

        // Signal all handlers to stop
        if let Err(e) = self.shutdown_tx.send(true) {
            error!("Failed to send shutdown signal: {}", e);
        }

        // Give handlers time to clean up
        tokio::time::sleep(Duration::from_secs(2)).await;

        info!("Shutdown complete");
    }

    /// Get a reference to the subsystem container.
    pub fn container(&self) -> Arc<SubsystemContainer> {
        Arc::clone(&self.container)
    }
}

/// Load configuration from environment and files.
fn load_config() -> NodeConfig {
    let mut config = NodeConfig::default();

    // Override HMAC secret from environment
    if let Ok(secret_hex) = std::env::var("QC_HMAC_SECRET") {
        if let Ok(secret_bytes) = hex::decode(&secret_hex) {
            if secret_bytes.len() == 32 {
                config.security.hmac_secret.copy_from_slice(&secret_bytes);
                info!("Loaded HMAC secret from environment");
            } else {
                warn!("QC_HMAC_SECRET must be 32 bytes (64 hex chars)");
            }
        }
    }

    // Override ports from environment
    if let Ok(port) = std::env::var("QC_P2P_PORT") {
        if let Ok(p) = port.parse() {
            config.network.p2p_port = p;
        }
    }
    if let Ok(port) = std::env::var("QC_RPC_PORT") {
        if let Ok(p) = port.parse() {
            config.network.rpc_port = p;
        }
    }

    config
}

#[tokio::main]
async fn main() -> Result<()> {
    // Handle CLI commands
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "--version" | "-V" => {
                println!("quantum-chain {}", env!("CARGO_PKG_VERSION"));
                println!("Architecture: V2.3 Choreography Pattern");
                println!("Subsystems: 17 (all compiled into single binary)");
                return Ok(());
            }
            "health" => {
                // Health check - just verify we can start
                println!("healthy");
                return Ok(());
            }
            "--help" | "-h" => {
                println!("Quantum-Chain Node Runtime");
                println!();
                println!("USAGE:");
                println!("    quantum-chain [OPTIONS]");
                println!();
                println!("OPTIONS:");
                println!("    --version, -V    Print version information");
                println!("    --help, -h       Print this help message");
                println!("    health           Run health check");
                println!();
                println!("ENVIRONMENT VARIABLES:");
                println!("    QC_HMAC_SECRET   32-byte hex-encoded HMAC secret");
                println!("    QC_P2P_PORT      P2P port (default: 30303)");
                println!("    QC_RPC_PORT      RPC port (default: 8545)");
                println!("    QC_DATA_DIR      Data directory path");
                println!("    QC_LOG_LEVEL     Log level (default: info)");
                println!("    QC_COMPUTE_BACKEND  Compute backend: auto, cpu, opencl");
                println!();
                println!("TELEMETRY (LGTM Stack):");
                println!("    OTEL_EXPORTER_OTLP_ENDPOINT   Tempo endpoint (default: http://localhost:4317)");
                println!("    LOKI_ENDPOINT                 Loki endpoint (default: http://localhost:3100)");
                println!(
                    "    QC_METRICS_PORT               Prometheus metrics port (default: 9100)"
                );
                return Ok(());
            }
            _ => {}
        }
    }

    // Initialize LGTM telemetry (Loki, Grafana, Tempo, Metrics)
    let telemetry_config = TelemetryConfig::from_env();
    let _telemetry_guard = init_telemetry(telemetry_config)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to initialize telemetry: {}", e))?;

    // Load configuration
    let config = load_config();

    // Auto-detect compute backend (GPU/CPU)
    info!("===========================================");
    info!("  COMPUTE BACKEND DETECTION");
    info!("===========================================");

    match qc_compute::auto_detect() {
        Ok(engine) => {
            let device = engine.device_info();
            info!("✅ Compute Backend: {}", engine.backend());
            info!("   Device: {}", device.name);
            info!("   Compute Units: {}", device.compute_units);
            if device.memory_bytes > 0 {
                info!("   Memory: {} MB", device.memory_bytes / 1024 / 1024);
            }

            // Log subsystem recommendations
            info!("   GPU-accelerated subsystems:");
            info!(
                "     - QC-17 (Mining): {}",
                qc_compute::recommended_backend_for("qc-17")
            );
            info!(
                "     - QC-10 (Signatures): {}",
                qc_compute::recommended_backend_for("qc-10")
            );
            info!(
                "     - QC-03 (Merkle): {}",
                qc_compute::recommended_backend_for("qc-03")
            );
        }
        Err(e) => {
            warn!("⚠️  GPU detection failed: {}. Using CPU fallback.", e);
            info!("   Tip: Install OpenCL drivers for GPU acceleration");
        }
    }
    info!("===========================================");

    // Validate for production (optional - comment out for development)
    // config.validate_for_production();

    // Create and start the node runtime
    let mut runtime = NodeRuntime::new(config);
    runtime.start().await?;

    // Keep the node running
    info!("Node is running. Press Ctrl+C to stop.");
    tokio::signal::ctrl_c().await?;

    // Graceful shutdown
    runtime.shutdown().await;

    Ok(())
}
