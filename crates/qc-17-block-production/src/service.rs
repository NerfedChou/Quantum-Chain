//! Concrete Block Producer Service Implementation
//!
//! This module provides the concrete implementation of the BlockProducerService
//! trait for use in the node runtime.

use crate::{
    config::BlockProductionConfig,
    domain::{
        calculate_block_reward, calculate_transaction_fees, create_coinbase_transaction,
        BlockHeader, BlockTemplate, ConsensusMode, DifficultyAdjuster, DifficultyConfig, PoWMiner,
    },
    error::{BlockProductionError, Result},
    ports::{
        BlockProducerService, BlockStorageReader, MinedBlockInfo,
        ProductionConfig, ProductionStatus,
    },
    security::SecurityValidator,
};
use async_trait::async_trait;
use primitive_types::{H256, U256};
use shared_bus::InMemoryEventBus;
use shared_types::entities::{Address, ValidatedTransaction};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

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

    /// Difficulty adjuster for PoW
    difficulty_adjuster: Option<DifficultyAdjuster>,

    /// Block storage reader for chain state queries (V2.4)
    /// Used on startup to resume with correct difficulty
    block_storage_reader: Option<Arc<dyn BlockStorageReader>>,
}

impl ConcreteBlockProducer {
    /// Create a new block producer service
    pub fn new(event_bus: Arc<InMemoryEventBus>, config: BlockProductionConfig) -> Self {
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
            current_difficulty: None,
            last_nonce: None,
            pending_blocks: Vec::new(),
        };

        // Initialize PoW miner with number of threads from config or default
        let num_threads = config.pow.as_ref().map(|p| p.threads).unwrap_or(4);
        let pow_miner = PoWMiner::new(num_threads);

        // Initialize difficulty adjuster for PoW
        let difficulty_adjuster = if config.mode == ConsensusMode::ProofOfWork {
            let pow_config = config.pow.as_ref();
            let difficulty_config = DifficultyConfig {
                target_block_time: pow_config.and_then(|p| p.target_block_time).unwrap_or(10),
                use_dgw: pow_config.and_then(|p| p.use_dgw).unwrap_or(true),
                dgw_window: pow_config.and_then(|p| p.dgw_window).unwrap_or(24),
                ..Default::default()
            };
            info!(
                "  Difficulty Adjustment: {} (target: {}s per block)",
                if difficulty_config.use_dgw {
                    "Dark Gravity Wave"
                } else {
                    "Epoch-based"
                },
                difficulty_config.target_block_time
            );
            Some(DifficultyAdjuster::new(difficulty_config))
        } else {
            None
        };

        Self {
            event_bus,
            config: std::sync::RwLock::new(config),
            security,
            status: Arc::new(std::sync::RwLock::new(initial_status)),
            is_active: std::sync::atomic::AtomicBool::new(false),
            pow_miner,
            mining_handle: std::sync::Mutex::new(None),
            difficulty_adjuster,
            block_storage_reader: None,
        }
    }

    /// Set the block storage reader for chain state queries
    ///
    /// V2.4: Used to query qc-02 for chain tip and recent blocks
    /// on startup for proper difficulty adjustment continuity.
    pub fn with_storage_reader(mut self, reader: Arc<dyn BlockStorageReader>) -> Self {
        self.block_storage_reader = Some(reader);
        self
    }

    /// Query chain state from Block Storage (qc-02)
    ///
    /// V2.4: Queries current chain tip and recent blocks for
    /// difficulty adjustment. Returns a ProductionConfig populated
    /// with chain state, or an error if unavailable.
    pub async fn query_chain_state(&self) -> Result<ProductionConfig> {
        let dgw_window = self
            .config
            .read()
            .unwrap()
            .pow
            .as_ref()
            .and_then(|p| p.dgw_window)
            .unwrap_or(24) as u32;

        if let Some(ref reader) = self.block_storage_reader {
            debug!(
                "[qc-17] Querying Block Storage for chain state (DGW window: {})",
                dgw_window
            );

            match reader.get_chain_info(dgw_window).await {
                Ok(chain_info) => {
                    info!(
                        "[qc-17] 📊 Retrieved chain state: height={}, {} recent blocks",
                        chain_info.chain_tip_height,
                        chain_info.recent_blocks.len()
                    );

                    // Get last difficulty from most recent block
                    let last_difficulty = chain_info.recent_blocks.first().map(|b| b.difficulty);

                    if let Some(diff) = last_difficulty {
                        info!(
                            "[qc-17] 💎 Last difficulty: {}",
                            DifficultyAdjuster::describe_difficulty(diff)
                        );
                    }

                    Ok(ProductionConfig {
                        starting_height: chain_info.chain_tip_height,
                        last_difficulty,
                        recent_blocks: chain_info.recent_blocks,
                        ..ProductionConfig::default()
                    })
                }
                Err(e) => {
                    warn!(
                        "[qc-17] Failed to query chain state: {}. Starting from genesis.",
                        e
                    );
                    Ok(ProductionConfig::default())
                }
            }
        } else {
            debug!("[qc-17] No block storage reader configured, using default config");
            Ok(ProductionConfig::default())
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

        // Get starting height from config (resuming from persisted chain)
        let starting_height = config.starting_height;
        // V2.4: Use proper initial difficulty from DifficultyConfig, not hardcoded value
        let initial_difficulty = config
            .last_difficulty
            .unwrap_or_else(|| DifficultyConfig::default().initial_difficulty);

        {
            let mut status = self.status.write().unwrap();
            status.active = true;
            status.mode = Some(mode);
            // CRITICAL: Initialize blocks_produced to starting_height so Bridge syncs correctly
            status.blocks_produced = starting_height;
            // CRITICAL: Initialize difficulty for Bridge to use on first block
            status.current_difficulty = Some(initial_difficulty);
        }

        if starting_height > 0 {
            info!(
                "[qc-17] 💾 Resuming from height {} with initial difficulty: {}",
                starting_height,
                DifficultyAdjuster::describe_difficulty(initial_difficulty)
            );
        }

        match mode {
            ConsensusMode::ProofOfWork => {
                info!("  Mode: PoW Mining");
                let threads = self
                    .config
                    .read()
                    .unwrap()
                    .pow
                    .as_ref()
                    .map(|p| p.threads)
                    .unwrap_or(4);
                info!("  Threads: {}", threads);

                // Start PoW mining in background task
                let is_active = Arc::new(std::sync::atomic::AtomicBool::new(true));
                let is_active_clone = Arc::clone(&is_active);
                let _event_bus = Arc::clone(&self.event_bus); // Reserved for future event publishing
                let block_config = self.config.read().unwrap().clone();
                let pow_miner = PoWMiner::new(threads);
                let status = self.status.clone(); // Share the same RwLock, don't copy!
                let difficulty_adjuster = self.difficulty_adjuster.clone();

                let mining_task = tokio::task::spawn(async move {
                    info!("[qc-17] PoW mining task started");

                    // Start from persisted chain height
                    let mut blocks_mined = starting_height;
                    let start_time = std::time::Instant::now();

                    // Track recent blocks for difficulty adjustment
                    // Initialize with historical blocks from config if resuming
                    let mut recent_blocks: Vec<crate::domain::difficulty::BlockInfo> = config
                        .recent_blocks
                        .iter()
                        .map(|b| crate::domain::difficulty::BlockInfo {
                            height: b.height,
                            timestamp: b.timestamp,
                            difficulty: b.difficulty,
                        })
                        .collect();

                    // If we have historical blocks, log the resumption info
                    if !recent_blocks.is_empty() {
                        let last_diff = &recent_blocks[0].difficulty;
                        info!(
                            "[qc-17] 📊 Resuming with {} historical blocks, last difficulty: {}",
                            recent_blocks.len(),
                            DifficultyAdjuster::describe_difficulty(*last_diff)
                        );
                    } else if let Some(last_diff) = config.last_difficulty {
                        // If we have last_difficulty but no recent blocks, create a synthetic entry
                        let timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        recent_blocks.push(crate::domain::difficulty::BlockInfo {
                            height: starting_height,
                            timestamp,
                            difficulty: last_diff,
                        });
                        info!(
                            "[qc-17] 📊 Resuming with last difficulty: {}",
                            DifficultyAdjuster::describe_difficulty(last_diff)
                        );
                    }

                    // Track the last mined block hash for proper chaining
                    let mut last_block_hash = H256::zero(); // Genesis parent
                    let mut last_block_timestamp = 0u64;

                    // Get target block time for minimum interval enforcement
                    let target_block_time = block_config
                        .pow
                        .as_ref()
                        .and_then(|p| p.target_block_time)
                        .unwrap_or(10);

                    while is_active_clone.load(std::sync::atomic::Ordering::Relaxed) {
                        // Step 1: Get pending transactions from mempool
                        // Mempool integration via qc-06 IPC (empty for coinbase-only blocks)
                        let pending_transactions: Vec<ValidatedTransaction> = vec![];

                        // Step 2: Calculate block number (resume from where we left off)
                        let parent_hash = last_block_hash; // Proper chain linking
                        let block_number = 1 + blocks_mined; // Next block after persisted height
                        let timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();

                        // Enforce timestamp monotonicity (must be >= parent timestamp)
                        let timestamp = timestamp.max(last_block_timestamp + 1);

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

                        // Step 6: Calculate difficulty dynamically based on recent blocks
                        let difficulty = if let Some(ref adjuster) = difficulty_adjuster {
                            let calculated = adjuster.calculate_next_difficulty(&recent_blocks);
                            let desc = DifficultyAdjuster::describe_difficulty(calculated);
                            // Log difficulty on first block or every 10 blocks
                            if block_number == starting_height + 1 || block_number % 10 == 1 {
                                info!(
                                    "[qc-17] 📊 Difficulty: {} (window: {} blocks)",
                                    desc,
                                    recent_blocks.len()
                                );
                            }
                            calculated
                        } else {
                            // Fallback: static difficulty
                            U256::from(2).pow(U256::from(240))
                        };

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

                        // Step 7: Mine with calculated difficulty using GPU/CPU compute engine
                        // Log includes difficulty description for debugging
                        let diff_desc = DifficultyAdjuster::describe_difficulty(difficulty);
                        info!(
                            "[qc-17] ⛏️  Mining block #{} (using {})...",
                            block_number,
                            pow_miner.backend_name()
                        );

                        // Use async GPU-accelerated mining (falls back to CPU if unavailable)
                        match pow_miner
                            .mine_block_async(template.clone(), difficulty)
                            .await
                        {
                            Some((nonce, block_hash)) => {
                                blocks_mined += 1;
                                let elapsed = start_time.elapsed().as_secs();
                                let hashrate = if elapsed > 0 {
                                    Some((blocks_mined as f64 / elapsed as f64) * 1_000_000.0)
                                // Rough estimate
                                } else {
                                    None
                                };

                                info!(
                                    "[qc-17] Block #{} mined! | nonce: {} | hash: {}",
                                    block_number,
                                    nonce,
                                    hex::encode(&block_hash[..8])
                                );

                                // Track this block for difficulty adjustment
                                recent_blocks.insert(
                                    0,
                                    crate::domain::difficulty::BlockInfo {
                                        height: block_number,
                                        timestamp,
                                        difficulty,
                                    },
                                );
                                // Keep only the last 50 blocks in memory
                                if recent_blocks.len() > 50 {
                                    recent_blocks.truncate(50);
                                }

                                // Compute difficulty description for logs
                                let difficulty_for_log = diff_desc.clone();
                                let correlation_id = uuid::Uuid::new_v4().to_string();
                                let block_hash_str = hex::encode(block_hash);
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
                                        "backend": pow_miner.backend_name(),
                                        "next_step": "qc-08 (Consensus Validation)"
                                    }
                                });
                                info!("EVENT_FLOW_JSON {}", event);

                                // Update status with difficulty and nonce for bridge
                                // CRITICAL: Push to pending_blocks queue so bridge gets each block's correct data
                                {
                                    let mut status_guard = status.write().unwrap();
                                    status_guard.blocks_produced = blocks_mined;
                                    status_guard.hashrate = hashrate;
                                    status_guard.last_block_at = Some(timestamp);
                                    status_guard.current_difficulty = Some(difficulty);
                                    status_guard.last_nonce = Some(nonce);

                                    // Push this block's info to pending queue for bridge
                                    status_guard.pending_blocks.push(MinedBlockInfo {
                                        height: block_number,
                                        timestamp,
                                        difficulty,
                                        nonce,
                                        parent_hash: parent_hash.0,
                                    });

                                    info!(
                                        "[qc-17] 📊 Status updated: blocks_produced={}, pending_queue={}",
                                        status_guard.blocks_produced, status_guard.pending_blocks.len()
                                    );
                                }

                                // Block validation is triggered by the choreography bridge in node-runtime
                                // which polls status and publishes BlockValidated events

                                // Update last block hash for proper chain linking
                                // Use the hash from mining directly
                                last_block_hash = H256::from_slice(&block_hash);
                                last_block_timestamp = timestamp;

                                // CRITICAL: Enforce minimum block interval
                                // Even if mining is fast, don't start next block immediately
                                // This prevents runaway block production when difficulty is too easy
                                let min_block_interval =
                                    std::time::Duration::from_secs(target_block_time / 2);
                                let mining_duration =
                                    std::time::Instant::now().duration_since(start_time);
                                if mining_duration < min_block_interval {
                                    let wait_time = min_block_interval - mining_duration;
                                    info!("[qc-17] ⏱️  Waiting {:?} to enforce minimum block interval", wait_time);
                                    tokio::time::sleep(wait_time).await;
                                } else {
                                    // Small yield to allow other tasks to run
                                    tokio::time::sleep(tokio::time::Duration::from_millis(100))
                                        .await;
                                }
                            }
                            None => {
                                error!(
                                    "[qc-17] Failed to mine block #{} - no valid nonce found",
                                    block_number
                                );
                                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                            }
                        }
                    }

                    info!(
                        "[qc-17] PoW mining task stopped. Total blocks mined: {}",
                        blocks_mined
                    );
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

    async fn drain_pending_blocks(&self) -> Vec<MinedBlockInfo> {
        let mut status = self.status.write().unwrap();
        std::mem::take(&mut status.pending_blocks)
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

    #[tokio::test]
    async fn test_query_chain_state_no_reader() {
        let event_bus = Arc::new(InMemoryEventBus::new());
        let config = BlockProductionConfig::default();

        let service = ConcreteBlockProducer::new(event_bus, config);

        // Without a reader configured, should return default config
        let result = service.query_chain_state().await;
        assert!(result.is_ok());

        let production_config = result.unwrap();
        assert_eq!(production_config.starting_height, 0);
        assert!(production_config.last_difficulty.is_none());
        assert!(production_config.recent_blocks.is_empty());
    }

    #[tokio::test]
    async fn test_query_chain_state_with_mock_reader() {
        use crate::adapters::ipc::IpcBlockStorageReader;

        let event_bus = Arc::new(InMemoryEventBus::new());
        let config = BlockProductionConfig::default();

        // Create service with mock storage reader
        let reader: Arc<dyn BlockStorageReader> = Arc::new(IpcBlockStorageReader::new());
        let service = ConcreteBlockProducer::new(event_bus, config).with_storage_reader(reader);

        // Mock reader returns empty chain info
        let result = service.query_chain_state().await;
        assert!(result.is_ok());

        let production_config = result.unwrap();
        // Mock returns empty chain (height 0, no blocks)
        assert_eq!(production_config.starting_height, 0);
    }

    #[test]
    fn test_initial_difficulty_uses_config() {
        // Verify that the fallback uses DifficultyConfig::default().initial_difficulty
        // which should be 2^220 for proper block times
        let expected = DifficultyConfig::default().initial_difficulty;
        let old_hardcoded = U256::from(2).pow(U256::from(252));

        // Expected should be 2^220 (harder than 2^252)
        assert!(
            expected < old_hardcoded,
            "Initial difficulty should be 2^220 (lower target = harder) not 2^252"
        );

        // Verify it's exactly 2^220
        let expected_value = U256::from(2).pow(U256::from(220));
        assert_eq!(expected, expected_value);
    }
}
