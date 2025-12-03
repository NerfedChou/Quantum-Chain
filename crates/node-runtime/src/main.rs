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
//! - `wiring/` - Subsystem initialization and event routing
//! - `adapters/` - Port implementations connecting subsystems
//! - `handlers/` - Event handlers for choreography flow
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
pub mod handlers;
pub mod wiring;

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use crate::adapters::BlockStorageAdapter;
use crate::handlers::{BlockStorageHandler, FinalityHandler, StateMgmtHandler, TxIndexingHandler};
use crate::wiring::{ChoreographyCoordinator, CoreSubsystemConfig, SubsystemRegistry};

/// Node configuration.
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// Network port for P2P communication.
    pub p2p_port: u16,
    /// Network port for RPC API.
    pub rpc_port: u16,
    /// Data directory for block storage.
    pub data_dir: String,
    /// Core subsystem configuration.
    pub subsystems: CoreSubsystemConfig,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            p2p_port: 30303,
            rpc_port: 8545,
            data_dir: "./data".to_string(),
            subsystems: CoreSubsystemConfig::default(),
        }
    }
}

/// The main node runtime orchestrating all subsystems.
pub struct NodeRuntime {
    /// Node configuration.
    config: NodeConfig,
    /// Choreography coordinator.
    choreography: ChoreographyCoordinator,
    /// Subsystem registry.
    registry: SubsystemRegistry,
    /// Shutdown signal.
    shutdown: tokio::sync::watch::Sender<bool>,
}

impl NodeRuntime {
    /// Create a new node runtime.
    pub fn new(config: NodeConfig) -> Self {
        let choreography = ChoreographyCoordinator::new();
        let registry = SubsystemRegistry::new(config.subsystems.clone());
        let (shutdown_tx, _) = tokio::sync::watch::channel(false);

        Self {
            config,
            choreography,
            registry,
            shutdown: shutdown_tx,
        }
    }

    /// Start the node runtime.
    pub async fn start(&self) -> Result<()> {
        info!("Starting Quantum-Chain node runtime...");
        info!("Architecture version: 2.3 (Choreography Pattern)");
        info!("Protocol version: 1");

        // Initialize subsystem registry
        self.registry.initialize_all().await?;

        // Start choreography coordinator
        self.choreography.start_monitoring().await;

        // Start event handlers for choreography
        self.start_choreography_handlers().await?;

        info!("All core subsystems initialized and running");
        info!("P2P Port: {}", self.config.p2p_port);
        info!("RPC Port: {}", self.config.rpc_port);
        info!("Data Dir: {}", self.config.data_dir);

        Ok(())
    }

    /// Start the choreography event handlers.
    async fn start_choreography_handlers(&self) -> Result<()> {
        let router = self.choreography.router();

        // Create Block Storage adapter
        let block_storage_adapter = Arc::new(BlockStorageAdapter::new(
            Arc::clone(&router),
            Duration::from_secs(self.config.subsystems.assembly_timeout_secs),
            self.config.subsystems.max_pending_assemblies,
        ));

        // Start Transaction Indexing handler
        let tx_indexing_handler = TxIndexingHandler::new(router.subscribe());
        let tx_router = Arc::clone(&router);
        tokio::spawn(async move {
            tx_indexing_handler.run(tx_router).await;
        });

        // Start State Management handler
        let state_mgmt_handler = StateMgmtHandler::new(router.subscribe());
        let state_router = Arc::clone(&router);
        tokio::spawn(async move {
            state_mgmt_handler.run(state_router).await;
        });

        // Start Block Storage handler
        let block_storage_handler =
            BlockStorageHandler::new(Arc::clone(&block_storage_adapter), router.subscribe());
        tokio::spawn(async move {
            block_storage_handler.run().await;
        });

        // Start Finality handler
        let finality_handler = FinalityHandler::new(router.subscribe());
        let finality_router = Arc::clone(&router);
        tokio::spawn(async move {
            finality_handler.run(finality_router).await;
        });

        info!("Choreography handlers started");
        Ok(())
    }

    /// Shutdown the node gracefully.
    pub fn shutdown(&self) {
        info!("Initiating graceful shutdown...");
        let _ = self.shutdown.send(true);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("===========================================");
    info!("  Quantum-Chain Node Runtime v0.1.0");
    info!("  Architecture: V2.3 Choreography");
    info!("===========================================");

    // Load configuration (would come from config file/env in production)
    let config = NodeConfig::default();

    // Create and start the node runtime
    let runtime = NodeRuntime::new(config);
    runtime.start().await?;

    // Keep the node running
    info!("Node is running. Press Ctrl+C to stop.");
    tokio::signal::ctrl_c().await?;

    // Graceful shutdown
    runtime.shutdown();
    info!("Shutdown complete.");

    Ok(())
}
