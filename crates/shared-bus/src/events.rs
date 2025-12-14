//! # Blockchain Events
//!
//! Defines all event types that flow through the shared bus.
//! These correspond to IPC payloads in `shared-types/src/ipc.rs`.

use serde::{Deserialize, Serialize};
use shared_types::entities::{Hash, PeerId, PeerInfo, ValidatedBlock, ValidatedTransaction};
use shared_types::ipc::{VerifyNodeIdentityPayload, VerifyNodeIdentityResponse};

/// All events that can be published to the event bus.
///
/// Per Architecture.md Section 5, these are the choreography events
/// that drive the V2.3 decentralized block processing flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlockchainEvent {
    // =========================================================================
    // SUBSYSTEM 1: PEER DISCOVERY
    // =========================================================================
    /// A new peer was discovered and verified.
    PeerDiscovered(PeerInfo),

    /// A peer disconnected or was evicted.
    PeerDisconnected(PeerId),

    /// Request to verify node identity.
    /// Source: Subsystem 1 | Target: Subsystem 10
    VerifyNodeIdentity {
        correlation_id: String,
        payload: VerifyNodeIdentityPayload,
    },

    /// Result of node identity verification.
    /// Source: Subsystem 10 | Target: Subsystem 1
    NodeIdentityVerified {
        correlation_id: String,
        payload: VerifyNodeIdentityResponse,
    },

    // =========================================================================
    // SUBSYSTEM 17: BLOCK PRODUCTION (EDA Choreography Start)
    // =========================================================================
    /// A new block was produced/mined and is ready for consensus validation.
    /// **V2.3 CHOREOGRAPHY:** This triggers Consensus (8) to validate the block.
    /// Source: Subsystem 17 | Target: Subsystem 8
    BlockProduced {
        /// The produced block's height.
        block_height: u64,
        /// The produced block's hash.
        block_hash: Hash,
        /// Difficulty target used for PoW.
        difficulty: [u8; 32],
        /// Nonce that solved the PoW.
        nonce: u64,
        /// Block timestamp.
        timestamp: u64,
        /// Parent block hash.
        parent_hash: Hash,
    },

    // =========================================================================
    // SUBSYSTEM 8: CONSENSUS (Choreography Trigger)
    // =========================================================================
    /// A block was validated by consensus.
    /// **V2.3 CHOREOGRAPHY:** This is the PRIMARY trigger that starts
    /// parallel computation by Subsystems 3 (Merkle) and 4 (State).
    BlockValidated(ValidatedBlock),

    /// A block was rejected by consensus.
    BlockRejected {
        /// The rejected block's hash.
        hash: Hash,
        /// Reason for rejection.
        reason: String,
    },

    // =========================================================================
    // SUBSYSTEM 3: TRANSACTION INDEXING (Choreography Response)
    // =========================================================================
    /// Merkle root was computed for a validated block.
    /// **V2.3 CHOREOGRAPHY:** Consumed by Block Storage (2) for assembly.
    MerkleRootComputed {
        /// The block hash this root applies to.
        block_hash: Hash,
        /// The computed Merkle root.
        merkle_root: Hash,
    },

    // =========================================================================
    // SUBSYSTEM 4: STATE MANAGEMENT (Choreography Response)
    // =========================================================================
    /// State root was computed for a validated block.
    /// **V2.3 CHOREOGRAPHY:** Consumed by Block Storage (2) for assembly.
    StateRootComputed {
        /// The block hash this root applies to.
        block_hash: Hash,
        /// The computed state root.
        state_root: Hash,
    },

    // =========================================================================
    // SUBSYSTEM 2: BLOCK STORAGE (Choreography Completion)
    // =========================================================================
    /// A block was fully assembled and stored.
    /// **V2.3 CHOREOGRAPHY:** This signals completion of the block flow.
    BlockStored {
        /// The stored block's height.
        block_height: u64,
        /// The stored block's hash.
        block_hash: Hash,
    },

    /// Genesis block was initialized and stored.
    /// **V2.3 CHOREOGRAPHY:** This is a special bootstrap event that signals
    /// the chain has been initialized. Subsystems can use this to initialize
    /// their own genesis state.
    /// Source: Runtime | Target: All subsystems
    GenesisInitialized {
        /// Hash of the genesis block.
        block_hash: Hash,
        /// Genesis block height (always 0).
        height: u64,
        /// Genesis timestamp.
        timestamp: u64,
    },

    // =========================================================================
    // SUBSYSTEM 10: SIGNATURE VERIFICATION
    // =========================================================================
    /// A transaction signature was verified.
    TransactionVerified(ValidatedTransaction),

    /// A transaction signature was invalid.
    TransactionInvalid {
        /// The transaction hash.
        hash: Hash,
        /// Reason for invalidation.
        reason: String,
    },

    // =========================================================================
    // SUBSYSTEM 9: FINALITY
    // =========================================================================
    /// A block reached finality.
    BlockFinalized {
        /// The finalized block height.
        block_height: u64,
        /// The finalized block hash.
        block_hash: Hash,
        /// The epoch in which finality was reached.
        finalized_epoch: u64,
    },

    // =========================================================================
    // CRITICAL EVENTS (DLQ)
    // =========================================================================
    /// Critical error requiring operator attention.
    CriticalError {
        /// The subsystem that encountered the error.
        subsystem_id: u8,
        /// Error description.
        error: String,
    },

    // =========================================================================
    // API GATEWAY QUERIES (qc-16)
    // =========================================================================
    /// Query from API Gateway to a subsystem.
    /// The target subsystem should respond with ApiQueryResponse.
    ApiQuery {
        /// Unique correlation ID to match request/response.
        correlation_id: String,
        /// Target subsystem (e.g., "qc-02-block-storage").
        target: String,
        /// Query method name (e.g., "get_block_number").
        method: String,
        /// Query parameters as JSON.
        params: serde_json::Value,
    },

    /// Response from a subsystem to an API Gateway query.
    ApiQueryResponse {
        /// Correlation ID matching the original query.
        correlation_id: String,
        /// Source subsystem ID.
        source: u8,
        /// Result (Ok data or Err with code/message).
        result: Result<serde_json::Value, ApiQueryError>,
    },
}

/// Error type for API query responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiQueryError {
    /// JSON-RPC error code.
    pub code: i32,
    /// Error message.
    pub message: String,
}

impl BlockchainEvent {
    /// Get the topic for this event (for filtering).
    #[must_use]
    pub fn topic(&self) -> EventTopic {
        match self {
            Self::PeerDiscovered(_)
            | Self::PeerDisconnected(_)
            | Self::VerifyNodeIdentity { .. }
            | Self::NodeIdentityVerified { .. } => EventTopic::PeerDiscovery,
            Self::BlockProduced { .. } => EventTopic::BlockProduction,
            Self::BlockValidated(_) | Self::BlockRejected { .. } => EventTopic::Consensus,
            Self::MerkleRootComputed { .. } => EventTopic::TransactionIndexing,
            Self::StateRootComputed { .. } => EventTopic::StateManagement,
            Self::BlockStored { .. } | Self::GenesisInitialized { .. } => EventTopic::BlockStorage,
            Self::TransactionVerified(_) | Self::TransactionInvalid { .. } => {
                EventTopic::SignatureVerification
            }
            Self::BlockFinalized { .. } => EventTopic::Finality,
            Self::CriticalError { .. } => EventTopic::DeadLetterQueue,
            Self::ApiQuery { .. } | Self::ApiQueryResponse { .. } => EventTopic::ApiGateway,
        }
    }

    /// Get the originating subsystem ID.
    #[must_use]
    pub fn source_subsystem(&self) -> u8 {
        match self {
            Self::PeerDiscovered(_)
            | Self::PeerDisconnected(_)
            | Self::VerifyNodeIdentity { .. } => 1,
            Self::NodeIdentityVerified { .. } => 10,
            Self::BlockStored { .. } | Self::GenesisInitialized { .. } => 2,
            Self::MerkleRootComputed { .. } => 3,
            Self::StateRootComputed { .. } => 4,
            Self::BlockProduced { .. } => 17,
            Self::BlockValidated(_) | Self::BlockRejected { .. } => 8,
            Self::BlockFinalized { .. } => 9,
            Self::TransactionVerified(_) | Self::TransactionInvalid { .. } => 10,
            Self::CriticalError { subsystem_id, .. } => *subsystem_id,
            Self::ApiQuery { .. } => 16,
            Self::ApiQueryResponse { source, .. } => *source,
        }
    }
}

/// Event topics for subscription filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventTopic {
    /// Subsystem 1 events.
    PeerDiscovery,
    /// Subsystem 2 events.
    BlockStorage,
    /// Subsystem 3 events.
    TransactionIndexing,
    /// Subsystem 4 events.
    StateManagement,
    /// Subsystem 5 events.
    BlockPropagation,
    /// Subsystem 6 events.
    Mempool,
    /// Subsystem 17 events (Block Production).
    BlockProduction,
    /// Subsystem 8 events.
    Consensus,
    /// Subsystem 9 events.
    Finality,
    /// Subsystem 10 events.
    SignatureVerification,
    /// Subsystem 16 events (API Gateway queries).
    ApiGateway,
    /// Dead Letter Queue for critical errors.
    DeadLetterQueue,
    /// All events (no filtering).
    All,
}

/// Filter for subscribing to specific events.
#[derive(Debug, Clone, Default)]
pub struct EventFilter {
    /// Topics to include. Empty means all topics.
    pub topics: Vec<EventTopic>,
    /// Source subsystems to include. Empty means all sources.
    pub source_subsystems: Vec<u8>,
}

impl EventFilter {
    /// Create a filter that accepts all events.
    #[must_use]
    pub fn all() -> Self {
        Self::default()
    }

    /// Create a filter for specific topics.
    #[must_use]
    pub fn topics(topics: Vec<EventTopic>) -> Self {
        Self {
            topics,
            source_subsystems: Vec::new(),
        }
    }

    /// Create a filter for events from specific subsystems.
    #[must_use]
    pub fn from_subsystems(subsystems: Vec<u8>) -> Self {
        Self {
            topics: Vec::new(),
            source_subsystems: subsystems,
        }
    }

    /// Check if an event matches this filter.
    #[must_use]
    pub fn matches(&self, event: &BlockchainEvent) -> bool {
        let topic_match = self.topics.is_empty()
            || self.topics.contains(&EventTopic::All)
            || self.topics.contains(&event.topic());

        let source_match = self.source_subsystems.is_empty()
            || self.source_subsystems.contains(&event.source_subsystem());

        topic_match && source_match
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_topic_mapping() {
        let event = BlockchainEvent::BlockValidated(ValidatedBlock::default());
        assert_eq!(event.topic(), EventTopic::Consensus);
        assert_eq!(event.source_subsystem(), 8);
    }

    #[test]
    fn test_filter_all() {
        let filter = EventFilter::all();
        let event = BlockchainEvent::BlockValidated(ValidatedBlock::default());
        assert!(filter.matches(&event));
    }

    #[test]
    fn test_filter_by_topic() {
        let filter = EventFilter::topics(vec![EventTopic::Consensus]);

        let consensus_event = BlockchainEvent::BlockValidated(ValidatedBlock::default());
        assert!(filter.matches(&consensus_event));

        let storage_event = BlockchainEvent::BlockStored {
            block_height: 1,
            block_hash: Hash::default(),
        };
        assert!(!filter.matches(&storage_event));
    }

    #[test]
    fn test_filter_by_subsystem() {
        let filter = EventFilter::from_subsystems(vec![8, 10]);

        let consensus_event = BlockchainEvent::BlockValidated(ValidatedBlock::default());
        assert!(filter.matches(&consensus_event)); // subsystem 8

        let storage_event = BlockchainEvent::BlockStored {
            block_height: 1,
            block_hash: Hash::default(),
        };
        assert!(!filter.matches(&storage_event)); // subsystem 2
    }

    #[test]
    fn test_merkle_root_event() {
        let event = BlockchainEvent::MerkleRootComputed {
            block_hash: Hash::default(),
            merkle_root: Hash::default(),
        };
        assert_eq!(event.topic(), EventTopic::TransactionIndexing);
        assert_eq!(event.source_subsystem(), 3);
    }

    #[test]
    fn test_state_root_event() {
        let event = BlockchainEvent::StateRootComputed {
            block_hash: Hash::default(),
            state_root: Hash::default(),
        };
        assert_eq!(event.topic(), EventTopic::StateManagement);
        assert_eq!(event.source_subsystem(), 4);
    }
}
