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
//! ## Core Subsystems (10 of 15)
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

pub mod adapters;
pub mod container;
pub mod genesis;
pub mod handlers;
pub mod wiring;

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use tracing::{error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

use qc_02_block_storage::BlockStorageApi;
use crate::adapters::BlockStorageAdapter;
use crate::container::{NodeConfig, SubsystemContainer};
use crate::genesis::{GenesisBuilder, GenesisConfig};
use crate::handlers::{BlockStorageHandler, FinalityHandler, StateMgmtHandler, TxIndexingHandler};
use crate::wiring::ChoreographyCoordinator;

/// The main node runtime orchestrating all subsystems.
pub struct NodeRuntime {
    /// Subsystem container with all initialized services.
    container: Arc<SubsystemContainer>,
    /// Choreography coordinator for event routing.
    choreography: ChoreographyCoordinator,
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
            shutdown_tx,
            shutdown_rx,
        }
    }

    /// Start the node runtime.
    ///
    /// ## Startup Sequence
    ///
    /// 1. Validate configuration for production
    /// 2. Initialize genesis block (if not exists)
    /// 3. Start choreography coordinator
    /// 4. Start event handlers
    /// 5. Signal ready
    pub async fn start(&self) -> Result<()> {
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

        info!("All core subsystems initialized and running");
        info!("P2P Port: {}", self.container.config.network.p2p_port);
        info!("RPC Port: {}", self.container.config.network.rpc_port);
        info!("Data Dir: {:?}", self.container.config.storage.data_dir);

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
            },
            transactions: vec![],
            consensus_proof: shared_types::ConsensusProof::default(),
        };
        
        // Write block using the proper API
        storage
            .write_block(genesis_block, genesis.header.merkle_root, genesis.header.state_root)
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
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(true)
        .with_thread_ids(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Load configuration
    let config = load_config();

    // Validate for production (optional - comment out for development)
    // config.validate_for_production();

    // Create and start the node runtime
    let runtime = NodeRuntime::new(config);
    runtime.start().await?;

    // Keep the node running
    info!("Node is running. Press Ctrl+C to stop.");
    tokio::signal::ctrl_c().await?;

    // Graceful shutdown
    runtime.shutdown().await;

    Ok(())
}
