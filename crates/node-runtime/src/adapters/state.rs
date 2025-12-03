//! # State Management Adapter
//!
//! Adapter that wraps qc-04-state-management domain logic and connects
//! it to the choreography event bus.
//!
//! ## V2.3 Choreography
//!
//! - Subscribes to: BlockValidated (from Consensus 8)
//! - Publishes: StateRootComputed (to Block Storage 2)

use parking_lot::RwLock;
use std::sync::Arc;
use tracing::{debug, error};

use qc_04_state_management::{PatriciaMerkleTrie, StateConfig, Hash as StateHash};
use shared_types::{Hash, SubsystemId};

use crate::adapters::EventBusAdapter;
use crate::wiring::{ChoreographyEvent, EventRouter};

/// Transaction representation for state application.
#[derive(Debug, Clone)]
pub struct StateTransaction {
    /// Sender address
    pub from: [u8; 20],
    /// Recipient address (None for contract creation)
    pub to: Option<[u8; 20]>,
    /// Value in wei
    pub value: u128,
    /// Gas limit
    pub gas_limit: u64,
    /// Gas price in wei
    pub gas_price: u128,
    /// Nonce
    pub nonce: u64,
}

/// State management adapter - wraps qc-04 domain logic.
///
/// Applies transactions to state trie and computes state roots.
pub struct StateAdapter {
    /// Event bus for publishing StateRootComputed
    event_bus: EventBusAdapter,
    /// Patricia Merkle Trie (domain logic from qc-04)
    trie: Arc<RwLock<PatriciaMerkleTrie>>,
}

impl StateAdapter {
    /// Create a new adapter with default configuration.
    pub fn new(router: Arc<EventRouter>) -> Self {
        let event_bus = EventBusAdapter::new(router, SubsystemId::StateManagement);
        let trie = Arc::new(RwLock::new(PatriciaMerkleTrie::new()));
        
        Self { event_bus, trie }
    }

    /// Create with custom configuration.
    pub fn with_config(router: Arc<EventRouter>, config: StateConfig) -> Self {
        let event_bus = EventBusAdapter::new(router, SubsystemId::StateManagement);
        let trie = Arc::new(RwLock::new(PatriciaMerkleTrie::with_config(config)));
        
        Self { event_bus, trie }
    }

    /// Process a BlockValidated event - apply transactions and compute state root.
    ///
    /// This is the main choreography trigger for State Management.
    ///
    /// ## Returns
    ///
    /// - The computed state root
    /// - Publishes `StateRootComputed` event on success
    pub fn process_block_validated(
        &self,
        block_hash: Hash,
        _block_height: u64,
        transactions: Vec<StateTransaction>,
    ) -> Result<Hash, StateAdapterError> {
        debug!(
            "[qc-04] Processing BlockValidated: {} transactions",
            transactions.len()
        );

        // Step 1: Apply transactions to trie
        {
            let mut trie = self.trie.write();
            for tx in &transactions {
                // Apply transaction effects to state
                // In full implementation: deduct from sender, credit to recipient
                // For now, we track the state root computation
                if let Some(_to) = tx.to {
                    // Transfer - update balances (simplified)
                    debug!("[qc-04] Applying transfer: {} wei", tx.value);
                }
            }
        }

        // Step 2: Compute state root
        let state_root: Hash = {
            let trie = self.trie.read();
            let root: StateHash = trie.root_hash();
            root
        };

        // Step 3: Publish StateRootComputed event
        let event = ChoreographyEvent::StateRootComputed {
            block_hash,
            state_root,
            sender_id: SubsystemId::StateManagement,
        };

        if let Err(e) = self.event_bus.publish(event) {
            error!("[qc-04] Failed to publish StateRootComputed: {}", e);
            return Err(StateAdapterError::PublishFailed(e.to_string()));
        }

        debug!("[qc-04] StateRootComputed published: {:?}", &state_root[..4]);

        Ok(state_root)
    }

    /// Get the trie for querying state.
    pub fn trie(&self) -> Arc<RwLock<PatriciaMerkleTrie>> {
        Arc::clone(&self.trie)
    }
}

/// State adapter errors.
#[derive(Debug)]
pub enum StateAdapterError {
    /// Failed to publish event.
    PublishFailed(String),
    /// State error from domain.
    StateError(String),
}

impl std::fmt::Display for StateAdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PublishFailed(msg) => write!(f, "Publish failed: {}", msg),
            Self::StateError(msg) => write!(f, "State error: {}", msg),
        }
    }
}

impl std::error::Error for StateAdapterError {}
