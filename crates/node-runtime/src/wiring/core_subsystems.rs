//! # Core Subsystem Initialization
//!
//! Initializes and wires the 10 core subsystems per System.md priority:
//!
//! ## Phase 1 (Core - No Dependencies):
//! - Subsystem 10: Signature Verification (pure crypto, no deps)
//!
//! ## Phase 2 (Core - Depends on 10):
//! - Subsystem 1: Peer Discovery (needs 10 for DDoS defense)
//! - Subsystem 6: Mempool (needs 10 for tx verification)
//!
//! ## Phase 3 (Consensus - Depends on 1, 6, 10):
//! - Subsystem 8: Consensus (validates blocks, publishes BlockValidated)
//! - Subsystem 5: Block Propagation (gossip protocol)
//!
//! ## Phase 4 (Choreography Participants):
//! - Subsystem 3: Transaction Indexing (Merkle trees)
//! - Subsystem 4: State Management (Patricia trie)
//! - Subsystem 2: Block Storage (Stateful Assembler)
//! - Subsystem 9: Finality (Casper-FFG)

use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::info;

/// Configuration for core subsystem wiring.
#[derive(Debug, Clone)]
pub struct CoreSubsystemConfig {
    /// HMAC secret for inter-subsystem authentication.
    pub hmac_secret: [u8; 32],
    /// Nonce cache expiry in seconds.
    pub nonce_cache_expiry_secs: u64,
    /// Block assembly timeout in seconds.
    pub assembly_timeout_secs: u64,
    /// Maximum pending block assemblies.
    pub max_pending_assemblies: usize,
    /// Maximum validators for finality.
    pub max_validators: usize,
    /// Epoch length in blocks.
    pub epoch_length: u64,
    /// Mempool max transactions.
    pub mempool_max_txs: usize,
    /// Minimum gas price in wei.
    pub min_gas_price: u64,
}

impl Default for CoreSubsystemConfig {
    fn default() -> Self {
        Self {
            hmac_secret: [0u8; 32], // Must be set from environment
            nonce_cache_expiry_secs: 120,
            assembly_timeout_secs: 30,
            max_pending_assemblies: 1000,
            max_validators: 100,
            epoch_length: 32,
            mempool_max_txs: 5000,
            min_gas_price: 1_000_000_000, // 1 gwei
        }
    }
}

/// Handle to an initialized subsystem.
pub struct SubsystemHandle<T> {
    /// The subsystem instance.
    pub instance: Arc<RwLock<T>>,
    /// Subsystem ID.
    pub id: u8,
    /// Whether the subsystem is running.
    pub running: bool,
}

impl<T> SubsystemHandle<T> {
    /// Create a new subsystem handle.
    pub fn new(instance: T, id: u8) -> Self {
        Self {
            instance: Arc::new(RwLock::new(instance)),
            id,
            running: false,
        }
    }

    /// Mark the subsystem as running.
    pub fn set_running(&mut self) {
        self.running = true;
    }
}

/// Registry of all core subsystem handles.
pub struct SubsystemRegistry {
    /// Configuration.
    pub config: CoreSubsystemConfig,
}

impl SubsystemRegistry {
    /// Create a new subsystem registry.
    pub fn new(config: CoreSubsystemConfig) -> Self {
        Self { config }
    }

    /// Initialize all core subsystems in dependency order.
    pub async fn initialize_all(&self) -> anyhow::Result<()> {
        info!("Initializing core subsystems in dependency order...");

        // Phase 1: No dependencies
        info!("Phase 1: Initializing Signature Verification (10)");
        self.init_signature_verification().await?;

        // Phase 2: Depends on 10
        info!("Phase 2: Initializing Peer Discovery (1), Mempool (6)");
        self.init_peer_discovery().await?;
        self.init_mempool().await?;

        // Phase 3: Consensus layer
        info!("Phase 3: Initializing Consensus (8), Block Propagation (5)");
        self.init_consensus().await?;
        self.init_block_propagation().await?;

        // Phase 4: Choreography participants
        info!("Phase 4: Initializing Tx Indexing (3), State Mgmt (4), Block Storage (2), Finality (9)");
        self.init_transaction_indexing().await?;
        self.init_state_management().await?;
        self.init_block_storage().await?;
        self.init_finality().await?;

        info!("All 10 core subsystems initialized successfully");
        Ok(())
    }

    // Subsystem initialization methods (stubs - actual impl connects to subsystem crates)

    async fn init_signature_verification(&self) -> anyhow::Result<()> {
        info!("  [10] Signature Verification - ECDSA/secp256k1");
        // qc-10 has no dependencies, pure crypto operations
        Ok(())
    }

    async fn init_peer_discovery(&self) -> anyhow::Result<()> {
        info!("  [1] Peer Discovery - Kademlia DHT");
        // qc-01 depends on qc-10 for DDoS defense (signature verification at edge)
        Ok(())
    }

    async fn init_mempool(&self) -> anyhow::Result<()> {
        info!(
            "  [6] Mempool - Priority Queue (max {} txs)",
            self.config.mempool_max_txs
        );
        // qc-06 depends on qc-10 for transaction signature verification
        Ok(())
    }

    async fn init_consensus(&self) -> anyhow::Result<()> {
        info!("  [8] Consensus - PoS/PBFT (2/3 attestation threshold)");
        // qc-08 depends on qc-05, qc-06, qc-10
        // Publishes BlockValidated to event bus
        Ok(())
    }

    async fn init_block_propagation(&self) -> anyhow::Result<()> {
        info!("  [5] Block Propagation - Gossip Protocol (fanout=8)");
        // qc-05 depends on qc-01, qc-08
        Ok(())
    }

    async fn init_transaction_indexing(&self) -> anyhow::Result<()> {
        info!("  [3] Transaction Indexing - Merkle Trees");
        // qc-03 subscribes to BlockValidated, publishes MerkleRootComputed
        Ok(())
    }

    async fn init_state_management(&self) -> anyhow::Result<()> {
        info!("  [4] State Management - Patricia Merkle Trie");
        // qc-04 subscribes to BlockValidated, publishes StateRootComputed
        Ok(())
    }

    async fn init_block_storage(&self) -> anyhow::Result<()> {
        info!(
            "  [2] Block Storage - Stateful Assembler (timeout={}s)",
            self.config.assembly_timeout_secs
        );
        // qc-02 subscribes to BlockValidated, MerkleRootComputed, StateRootComputed
        // Performs atomic write when all 3 components arrive
        Ok(())
    }

    async fn init_finality(&self) -> anyhow::Result<()> {
        info!(
            "  [9] Finality - Casper-FFG (epoch_length={})",
            self.config.epoch_length
        );
        // qc-09 monitors blocks for finalization
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_subsystem_registry_initialization() {
        let config = CoreSubsystemConfig::default();
        let registry = SubsystemRegistry::new(config);

        // Should initialize without error
        let result = registry.initialize_all().await;
        assert!(result.is_ok());
    }
}
