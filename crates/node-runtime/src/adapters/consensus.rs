//! # Consensus Adapter
//!
//! Adapter for Consensus (qc-08) subsystem.
//!
//! ## V2.3 Choreography
//!
//! - Subscribes to: BlockProduced (from Block Production 17), BlockStored (for height tracking)
//! - Publishes: BlockValidated (triggers TxIndexing 3, StateMgmt 4, BlockStorage 2)
//!
//! ## Architecture
//!
//! This adapter is a thin wrapper around the domain `BlockValidator`.
//! All validation logic is in the domain layer (pure, no I/O).
//! The adapter handles:
//! - State management (validated blocks cache)
//! - Event publishing
//! - Time source (system clock)
//! - Chain height tracking (event-sourced from BlockStored)

use crate::adapters::EventBusAdapter;
use crate::wiring::{ChoreographyEvent, EventRouter};
use parking_lot::RwLock;
use qc_08_consensus::{BlockValidationConfig, BlockValidationParams, BlockValidator};
use shared_types::SubsystemId;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Parameters for block validation from BlockProduced event.
#[derive(Debug, Clone)]
pub struct BlockProducedParams {
    /// Block hash.
    pub block_hash: [u8; 32],
    /// Block height.
    pub block_height: u64,
    /// Difficulty target.
    pub difficulty: [u8; 32],
    /// PoW nonce.
    pub nonce: u64,
    /// Block timestamp.
    pub timestamp: u64,
    /// Parent block hash.
    pub parent_hash: [u8; 32],
}

/// Consensus adapter - validates blocks and publishes BlockValidated events.
///
/// This adapter wraps the domain `BlockValidator` and handles:
/// - External state (validated blocks cache)
/// - Event publishing
/// - System time access
/// - Chain height (event-sourced from BlockStored events)
///
/// ## Event-Sourced Chain Height
///
/// Chain height is updated only when `on_block_stored` is called, which happens
/// when the adapter receives a `BlockStored` event. This ensures the height
/// reflects the actual stored chain state, not just validated blocks.
///
/// Reference: SPEC-08 Section 4 (Event Schema)
pub struct ConsensusAdapter {
    event_bus: EventBusAdapter,
    /// Domain validator (pure logic, no I/O).
    validator: BlockValidator,
    /// Validated block hashes to prevent duplicates.
    validated_blocks: RwLock<HashSet<[u8; 32]>>,
    /// Current chain height (event-sourced from BlockStored).
    /// Uses AtomicU64 for lock-free reads during validation.
    chain_height: AtomicU64,
}

impl ConsensusAdapter {
    /// Create a new consensus adapter.
    pub fn new(router: Arc<EventRouter>) -> Self {
        let event_bus = EventBusAdapter::new(router, SubsystemId::Consensus);
        let validator = BlockValidator::new(BlockValidationConfig::default());
        Self {
            event_bus,
            validator,
            validated_blocks: RwLock::new(HashSet::new()),
            chain_height: AtomicU64::new(0),
        }
    }

    /// Set the initial chain height (from storage on startup).
    /// Called once during initialization before event handlers start.
    pub fn set_initial_chain_height(&self, height: u64) {
        self.chain_height.store(height, Ordering::SeqCst);
        info!("[qc-08] Initial chain height set to {}", height);
    }

    /// Get the current chain height.
    pub fn chain_height(&self) -> u64 {
        self.chain_height.load(Ordering::SeqCst)
    }

    /// Handle BlockStored event - update chain height (event sourcing).
    ///
    /// This is the ONLY place chain height is updated after initialization.
    /// Chain height reflects stored blocks, not just validated blocks.
    pub fn on_block_stored(&self, block_height: u64, block_hash: &[u8; 32]) {
        let current = self.chain_height.load(Ordering::SeqCst);
        if block_height > current {
            self.chain_height.store(block_height, Ordering::SeqCst);
            debug!(
                "[qc-08] Chain height updated: {} -> {} (BlockStored {:02x}{:02x}...)",
                current, block_height, block_hash[0], block_hash[1]
            );
        }
    }

    /// Get the event bus adapter for publishing.
    pub fn event_bus(&self) -> &EventBusAdapter {
        &self.event_bus
    }

    /// Get current system time in seconds.
    fn current_time(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Process a BlockProduced event - validate and publish BlockValidated.
    ///
    /// This is the primary choreography handler for Consensus.
    /// All validation logic is delegated to the domain `BlockValidator`.
    ///
    /// ## Validation Steps (delegated to domain)
    ///
    /// 1. Check for duplicate blocks
    /// 2. Validate block height is sequential
    /// 3. Verify PoW difficulty
    /// 4. Check timestamp bounds
    ///
    /// ## Note on Chain Height
    ///
    /// Chain height is read from event-sourced state (BlockStored events).
    /// This ensures validation uses the actual stored chain height, not
    /// an optimistic height from pending validations.
    pub fn process_block_produced(
        &self,
        params: &BlockProducedParams,
    ) -> Result<(), ConsensusAdapterError> {
        debug!(
            "[qc-08] Validating BlockProduced #{} (nonce: {})",
            params.block_height, params.nonce
        );

        // Convert to domain parameters
        let validation_params = BlockValidationParams {
            block_hash: params.block_hash,
            block_height: params.block_height,
            difficulty: params.difficulty,
            nonce: params.nonce,
            timestamp: params.timestamp,
            parent_hash: params.parent_hash,
        };

        // Get current state (chain height is atomic, no lock needed)
        let current_height = self.chain_height.load(Ordering::SeqCst);
        let validated_blocks = self.validated_blocks.read();
        let current_time = self.current_time();

        // Delegate validation to domain service
        match self
            .validator
            .validate_block(&validation_params, current_height, current_time, &validated_blocks)
        {
            Ok(result) => {
                drop(validated_blocks); // Release read lock before write

                // Log any warnings
                for warning in &result.warnings {
                    warn!("[qc-08] Validation warning: {:?}", warning);
                }

                // Update validated blocks cache only (NOT chain height)
                // Chain height is updated via on_block_stored()
                self.update_validated_blocks_cache(&params.block_hash);

                info!(
                    "[qc-08] ✓ Block #{} validated (hash: {:02x}{:02x}...)",
                    params.block_height, params.block_hash[0], params.block_hash[1]
                );

                // Publish BlockValidated event
                self.publish_block_validated(params)?;

                Ok(())
            }
            Err(e) => {
                // Map domain error to adapter error
                Err(ConsensusAdapterError::from_domain_error(e))
            }
        }
    }

    /// Update validated blocks cache after successful validation.
    /// Note: Chain height is NOT updated here - it's event-sourced from BlockStored.
    fn update_validated_blocks_cache(&self, block_hash: &[u8; 32]) {
        let mut validated = self.validated_blocks.write();
        validated.insert(*block_hash);

        // Check if cache needs eviction
        if self.validator.should_evict_cache(validated.len()) {
            validated.clear();
        }
    }

    /// Publish BlockValidated event to choreography.
    fn publish_block_validated(
        &self,
        params: &BlockProducedParams,
    ) -> Result<(), ConsensusAdapterError> {
        let event = ChoreographyEvent::BlockValidated {
            block_hash: params.block_hash,
            block_height: params.block_height,
            sender_id: SubsystemId::Consensus,
        };

        if let Err(e) = self.event_bus.publish(event) {
            error!("[qc-08] Failed to publish BlockValidated: {}", e);
            return Err(ConsensusAdapterError::PublishFailed(e.to_string()));
        }

        info!(
            "[qc-08] 📤 Published BlockValidated #{} to choreography",
            params.block_height
        );

        Ok(())
    }
}

/// Consensus adapter errors.
#[derive(Debug)]
pub enum ConsensusAdapterError {
    /// Block already validated.
    DuplicateBlock,
    /// Invalid difficulty.
    InvalidDifficulty,
    /// Invalid block height.
    InvalidHeight { expected: u64, got: u64 },
    /// Invalid timestamp.
    InvalidTimestamp,
    /// Failed to publish event.
    PublishFailed(String),
}

impl ConsensusAdapterError {
    /// Convert from domain validation error.
    fn from_domain_error(err: qc_08_consensus::BlockValidationError) -> Self {
        use qc_08_consensus::BlockValidationError;
        match err {
            BlockValidationError::DuplicateBlock { .. } => Self::DuplicateBlock,
            BlockValidationError::NonSequentialHeight { expected, got } => {
                Self::InvalidHeight { expected, got }
            }
            BlockValidationError::ZeroDifficulty => Self::InvalidDifficulty,
            BlockValidationError::FutureTimestamp { .. } => Self::InvalidTimestamp,
        }
    }
}

impl std::fmt::Display for ConsensusAdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateBlock => write!(f, "Block already validated"),
            Self::InvalidDifficulty => write!(f, "Invalid difficulty"),
            Self::InvalidHeight { expected, got } => {
                write!(f, "Invalid height: expected {}, got {}", expected, got)
            }
            Self::InvalidTimestamp => write!(f, "Invalid timestamp"),
            Self::PublishFailed(msg) => write!(f, "Failed to publish: {}", msg),
        }
    }
}

impl std::error::Error for ConsensusAdapterError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_adapter() -> ConsensusAdapter {
        let router = Arc::new(EventRouter::default());
        ConsensusAdapter::new(router)
    }

    #[test]
    fn test_initial_chain_height() {
        let adapter = create_test_adapter();
        assert_eq!(adapter.chain_height(), 0);

        adapter.set_initial_chain_height(100);
        assert_eq!(adapter.chain_height(), 100);
    }

    #[test]
    fn test_on_block_stored_updates_height() {
        let adapter = create_test_adapter();
        adapter.set_initial_chain_height(5);

        // BlockStored at height 6 should update
        adapter.on_block_stored(6, &[0u8; 32]);
        assert_eq!(adapter.chain_height(), 6);

        // BlockStored at height 5 (lower) should NOT update
        adapter.on_block_stored(5, &[1u8; 32]);
        assert_eq!(adapter.chain_height(), 6);

        // BlockStored at height 10 should update
        adapter.on_block_stored(10, &[2u8; 32]);
        assert_eq!(adapter.chain_height(), 10);
    }

    #[test]
    fn test_validated_blocks_cache() {
        let adapter = create_test_adapter();
        let hash = [42u8; 32];

        // Cache should start empty
        assert!(!adapter.validated_blocks.read().contains(&hash));

        // After update, cache should contain hash
        adapter.update_validated_blocks_cache(&hash);
        assert!(adapter.validated_blocks.read().contains(&hash));
    }
}
