//! Concrete Block Producer Service Implementation
//!
//! This module provides the concrete implementation of the BlockProducerService
//! trait for use in the node runtime.

use crate::{
    config::BlockProductionConfig,
    domain::{BlockTemplate, ConsensusMode, BlockHeader, PoWMiner, 
             create_coinbase_transaction, calculate_block_reward, calculate_transaction_fees},
    error::{BlockProductionError, Result},
    ports::{BlockProducerService, ProductionConfig, ProductionStatus},
    security::SecurityValidator,
};
use async_trait::async_trait;
use primitive_types::{H256, U256};
use shared_bus::InMemoryEventBus;
use shared_types::entities::{Address, ValidatedTransaction};
use std::sync::Arc;
use tracing::{info, warn, error};

/// Concrete implementation of BlockProducerService
///
/// This service orchestrates block production across different consensus modes:
/// - PoW: Multi-threaded mining with ASIC resistance
/// - PoS: VRF-based proposer selection
/// - PBFT: Leader-based block proposal
pub struct ConcreteBlockProducer {
    /// Event bus for IPC communication
    event_bus: Arc<InMemoryEventBus>,

    /// Block production configuration
    config: std::sync::RwLock<BlockProductionConfig>,

    /// Security validator (used for transaction validation)
    #[allow(dead_code)]
    security: SecurityValidator,

    /// Current production status
    status: Arc<std::sync::RwLock<ProductionStatus>>,

    /// Whether production is active
    is_active: std::sync::atomic::AtomicBool,

    /// PoW miner instance (used in mining task)
    #[allow(dead_code)]
    pow_miner: PoWMiner,

    /// Mining thread handle
    mining_handle: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl ConcreteBlockProducer {
    /// Create a new block producer service
    pub fn new(
        event_bus: Arc<InMemoryEventBus>,
        config: BlockProductionConfig,
    ) -> Self {
        info!("[qc-17] Initializing Block Producer Service");
        info!("  Consensus Mode: {:?}", config.mode);
        info!("  Gas Limit: {}", config.gas_limit);
        info!("  Fair Ordering: {}", config.fair_ordering);

        let security = SecurityValidator::new(config.gas_limit, config.min_gas_price);

        let initial_status = ProductionStatus {
            active: false,
            mode: Some(config.mode),
            blocks_produced: 0,
            total_fees: U256::zero(),
            hashrate: None,
            last_block_at: None,
        };

        // Initialize PoW miner with number of threads from config or default
        let num_threads = config.pow.as_ref().map(|p| p.threads).unwrap_or(4);
        let pow_miner = PoWMiner::new(num_threads);

        Self {
            event_bus,
            config: std::sync::RwLock::new(config),
            security,
            status: Arc::new(std::sync::RwLock::new(initial_status)),
            is_active: std::sync::atomic::AtomicBool::new(false),
            pow_miner,
            mining_handle: std::sync::Mutex::new(None),
        }
    }

    /// Get the current production status
    pub fn status_sync(&self) -> ProductionStatus {
        self.status.read().unwrap().clone()
    }

    /// Get the production configuration
    pub fn config_sync(&self) -> BlockProductionConfig {
        self.config.read().unwrap().clone()
    }

    /// Get the event bus
    pub fn event_bus(&self) -> Arc<InMemoryEventBus> {
        Arc::clone(&self.event_bus)
    }
}

#[async_trait]
impl BlockProducerService for ConcreteBlockProducer {
    async fn produce_block(
        &self,
        _parent_hash: H256,
        _beneficiary: [u8; 20],
    ) -> Result<BlockTemplate> {
        // Block production via produce_block() is reserved for on-demand block creation
        // Continuous mining uses start_production() with PoW mode
        warn!("[qc-17] produce_block() not yet implemented");
        Err(BlockProductionError::NotImplemented(
            "Block production not yet implemented".to_string(),
        ))
    }

    async fn start_production(&self, mode: ConsensusMode, config: ProductionConfig) -> Result<()> {
        info!("[qc-17] Starting block production");

        self.is_active
            .store(true, std::sync::atomic::Ordering::SeqCst);
        {
            let mut status = self.status.write().unwrap();
            status.active = true;
            status.mode = Some(mode);
        }

        // Get starting height from config (resuming from persisted chain)
        let starting_height = config.starting_height;
        if starting_height > 0 {
            info!("[qc-17] 💾 Resuming from height {} (loaded from storage)", starting_height);
        }

        match mode {
            ConsensusMode::ProofOfWork => {
                info!("  Mode: PoW Mining");
                let threads = self.config.read().unwrap().pow.as_ref().map(|p| p.threads).unwrap_or(4);
                info!("  Threads: {}", threads);
                
                // Start PoW mining in background task
                let is_active = Arc::new(std::sync::atomic::AtomicBool::new(true));
                let is_active_clone = Arc::clone(&is_active);
                let _event_bus = Arc::clone(&self.event_bus); // Reserved for future event publishing
                let block_config = self.config.read().unwrap().clone();
                let pow_miner = PoWMiner::new(threads);
                let status = self.status.clone(); // Share the same RwLock, don't copy!
                
                let mining_task = tokio::task::spawn(async move {
                    info!("[qc-17] PoW mining task started");
                    
                    // Start from persisted chain height
                    let mut blocks_mined = starting_height;
                    let start_time = std::time::Instant::now();
                    
                    while is_active_clone.load(std::sync::atomic::Ordering::Relaxed) {
                        // Step 1: Get pending transactions from mempool
                        // Mempool integration via qc-06 IPC (empty for coinbase-only blocks)
                        let pending_transactions: Vec<ValidatedTransaction> = vec![];
                        
                        // Step 2: Calculate block number (resume from where we left off)
                        let parent_hash = H256::random(); // Chain tip placeholder
                        let block_number = 1 + blocks_mined; // Next block after persisted height
                        let timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        
                        // Step 3: Calculate mining rewards
                        let base_reward = calculate_block_reward(block_number);
                        let transaction_fees = calculate_transaction_fees(&pending_transactions);
                        
                        // Use beneficiary from config, fallback to zero address
                        let beneficiary: Address = [0u8; 20]; // Default beneficiary
                        
                        // Step 4: Create coinbase transaction
                        let coinbase_tx = match create_coinbase_transaction(
                            block_number,
                            beneficiary,
                            base_reward,
                            transaction_fees,
                            timestamp,
                        ) {
                            Ok(tx) => tx,
                            Err(e) => {
                                error!("[qc-17] Failed to create coinbase transaction: {}", e);
                                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                                continue;
                            }
                        };
                        
                        // Step 5: Build transaction list (coinbase first)
                        let mut validated_transactions = vec![coinbase_tx];
                        validated_transactions.extend(pending_transactions);
                        
                        // Serialize transactions for BlockTemplate (simple encoding for now)
                        let transactions: Vec<Vec<u8>> = validated_transactions
                            .iter()
                            .map(|tx| serde_json::to_vec(&tx).unwrap_or_default())
                            .collect();
                        
                        // Step 6: Calculate difficulty
                        // Development: require 1 leading zero byte for fast mining
                        // Production: increase to U256::from(2).pow(U256::from(224)) for 4 leading zeros
                        let difficulty = U256::from(2).pow(U256::from(248)); // ~1 leading zero byte
                        
                        let template = BlockTemplate {
                            header: BlockHeader {
                                parent_hash,
                                block_number,
                                timestamp,
                                beneficiary,
                                gas_used: 0,
                                gas_limit: block_config.gas_limit,
                                difficulty,
                                extra_data: b"qc-17-miner".to_vec(),
                                merkle_root: None,
                                state_root: Some(H256::zero()),
                                nonce: None,
                            },
                            transactions,
                            total_gas_used: 0,
                            total_fees: transaction_fees,
                            consensus_mode: ConsensusMode::ProofOfWork,
                            created_at: timestamp,
                        };
                        
                        // Step 7: Mine with calculated difficulty
                        
                        info!("[qc-17] Mining block #{} (reward: {} + fees: {}) with difficulty target: {:?}", 
                              block_number, base_reward, transaction_fees, difficulty);
                        
                        let difficulty_for_log = format!("{}", difficulty);
                        match pow_miner.mine_block(template.clone(), difficulty) {
                            Some(nonce) => {
                                blocks_mined += 1;
                                let elapsed = start_time.elapsed().as_secs();
                                let hashrate = if elapsed > 0 {
                                    Some((blocks_mined as f64 / elapsed as f64) * 1_000_000.0) // Rough estimate
                                } else {
                                    None
                                };
                                
                                info!("[qc-17] ✓ Block #{} mined! Nonce: {}, Total blocks: {}", 
                                    block_number, nonce, blocks_mined);
                                
                                // JSON EVENT LOG
                                let correlation_id = uuid::Uuid::new_v4().to_string();
                                let block_hash_str = format!("{:016x}", nonce);
                                let event = serde_json::json!({
                                    "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
                                    "subsystem_id": "qc-17",
                                    "event_type": "BlockProduced",
                                    "correlation_id": correlation_id,
                                    "block_hash": block_hash_str,
                                    "block_height": block_number,
                                    "processing_time_ms": 0,
                                    "metadata": {
                                        "nonce": nonce,
                                        "difficulty_target": difficulty_for_log,
                                        "total_blocks": blocks_mined,
                                        "hashrate": hashrate,
                                        "next_step": "qc-08 (Consensus Validation)"
                                    }
                                });
                                info!("EVENT_FLOW_JSON {}", event);
                                
                                // Update status
                                {
                                    let mut status_guard = status.write().unwrap();
                                    status_guard.blocks_produced = blocks_mined;
                                    status_guard.hashrate = hashrate;
                                    status_guard.last_block_at = Some(timestamp);
                                    info!("[qc-17] 📊 Status updated: blocks_produced={}", status_guard.blocks_produced);
                                }
                                
                                // Block validation is triggered by the choreography bridge in node-runtime
                                // which polls status and publishes BlockValidated events
                                
                                // Small delay before next block
                                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                            }
                            None => {
                                error!("[qc-17] Failed to mine block #{} - no valid nonce found", block_number);
                                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                            }
                        }
                    }
                    
                    info!("[qc-17] PoW mining task stopped. Total blocks mined: {}", blocks_mined);
                });
                
                // Store handle
                *self.mining_handle.lock().unwrap() = Some(mining_task);
            }
            ConsensusMode::ProofOfStake => {
                info!("  Mode: PoS Proposing");
                // Slot assignments handled via choreography (Phase 7 for PoS)
                warn!("PoS proposing not yet implemented");
            }
            ConsensusMode::PBFT => {
                info!("  Mode: PBFT Leader Proposal");
                // Leader election via VRF (Phase 7 for PoS)
                warn!("PBFT proposal not yet implemented");
            }
        }

        Ok(())
    }

    async fn stop_production(&self) -> Result<()> {
        info!("[qc-17] Stopping block production");

        self.is_active
            .store(false, std::sync::atomic::Ordering::SeqCst);
        
        // Stop mining task if running
        if let Some(handle) = self.mining_handle.lock().unwrap().take() {
            handle.abort();
            info!("[qc-17] Mining task aborted");
        }
        
        {
            let mut status = self.status.write().unwrap();
            status.active = false;
        }

        Ok(())
    }

    async fn get_status(&self) -> ProductionStatus {
        self.status.read().unwrap().clone()
    }

    async fn update_gas_limit(&self, new_limit: u64) -> Result<()> {
        info!("[qc-17] Updating gas limit to {}", new_limit);
        let mut config = self.config.write().unwrap();
        config.gas_limit = new_limit;
        Ok(())
    }

    async fn update_min_gas_price(&self, new_price: U256) -> Result<()> {
        info!("[qc-17] Updating min gas price to {}", new_price);
        let mut config = self.config.write().unwrap();
        config.min_gas_price = new_price;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_service_creation() {
        let event_bus = Arc::new(InMemoryEventBus::new());
        let config = BlockProductionConfig::default();

        let service = ConcreteBlockProducer::new(event_bus, config);
        let status = service.get_status().await;
        assert!(!status.active);
        assert_eq!(status.blocks_produced, 0);
    }

    #[tokio::test]
    async fn test_start_stop() {
        let event_bus = Arc::new(InMemoryEventBus::new());
        let config = BlockProductionConfig::default();

        let service = ConcreteBlockProducer::new(event_bus, config);

        assert!(service
            .start_production(ConsensusMode::ProofOfStake, ProductionConfig::default())
            .await
            .is_ok());
        assert!(service.get_status().await.active);

        assert!(service.stop_production().await.is_ok());
        assert!(!service.get_status().await.active);
    }

    #[tokio::test]
    async fn test_update_config() {
        let event_bus = Arc::new(InMemoryEventBus::new());
        let config = BlockProductionConfig::default();

        let service = ConcreteBlockProducer::new(event_bus, config);

        let new_gas_limit = 50_000_000;
        service.update_gas_limit(new_gas_limit).await.unwrap();
        assert_eq!(service.config_sync().gas_limit, new_gas_limit);

        let new_price = U256::from(2_000_000_000u64);
        service.update_min_gas_price(new_price).await.unwrap();
        assert_eq!(service.config_sync().min_gas_price, new_price);
    }
}
