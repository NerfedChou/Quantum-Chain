//! # Event Routing for V2.3 Choreography
//!
//! This module sets up the event routing between subsystems according to
//! the choreography pattern defined in Architecture.md v2.3.
//!
//! ## Event Flow (from IPC-MATRIX.md)
//!
//! ```text
//! CONSENSUS (8)
//!     │
//!     ├──BlockValidated──────────────────────────────────────────┐
//!     │                                                          │
//!     │              ┌───────────────────────────────────────────┤
//!     │              │                                           │
//!     ▼              ▼                                           ▼
//! TX INDEXING (3)  STATE MGMT (4)                          BLOCK STORAGE (2)
//!     │              │                                      [Assembler]
//!     │              │                                           │
//!     ▼              ▼                                           │
//! MerkleRootComputed StateRootComputed                           │
//!     │              │                                           │
//!     └──────────────┴───────────────────────────────────────────┘
//!                                 │
//!                                 ▼
//!                    [ATOMIC WRITE when all 3 arrive]
//!                                 │
//!                                 ▼
//!                           BlockStored
//!                                 │
//!                                 ▼
//!                          FINALITY (9)
//!                                 │
//!                                 ▼
//!                          BlockFinalized
//! ```
//!
//! ## Security (from Architecture.md Section 3.2)
//!
//! All messages MUST use `AuthenticatedMessage<T>` envelope with:
//! - `version`: Protocol version
//! - `sender_id`: Subsystem ID (verified by HMAC)
//! - `recipient_id`: Target subsystem
//! - `correlation_id`: Request/response tracking
//! - `timestamp`: Replay prevention
//! - `nonce`: Unique per message
//! - `signature`: HMAC-SHA256

use std::sync::Arc;

use anyhow::Result;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

use shared_types::SubsystemId;

/// Event types that flow between subsystems.
#[derive(Debug, Clone)]
pub enum ChoreographyEvent {
    /// Block validated by Consensus (8) - triggers choreography.
    BlockValidated {
        block_hash: [u8; 32],
        block_height: u64,
        sender_id: SubsystemId,
    },

    /// Merkle root computed by Transaction Indexing (3).
    MerkleRootComputed {
        block_hash: [u8; 32],
        merkle_root: [u8; 32],
        sender_id: SubsystemId,
    },

    /// State root computed by State Management (4).
    StateRootComputed {
        block_hash: [u8; 32],
        state_root: [u8; 32],
        sender_id: SubsystemId,
    },

    /// Block stored atomically by Block Storage (2).
    BlockStored {
        block_hash: [u8; 32],
        block_height: u64,
        merkle_root: [u8; 32],
        state_root: [u8; 32],
        sender_id: SubsystemId,
    },

    /// Block finalized by Finality (9).
    BlockFinalized {
        block_hash: [u8; 32],
        block_height: u64,
        finality_proof: Vec<u8>,
        sender_id: SubsystemId,
    },

    /// Assembly timeout - incomplete block dropped.
    AssemblyTimeout {
        block_hash: [u8; 32],
        missing_components: Vec<&'static str>,
        sender_id: SubsystemId,
    },
}

/// Authorization rules per IPC-MATRIX.md.
pub struct AuthorizationRules;

impl AuthorizationRules {
    /// Validate that the sender is authorized to emit this event type.
    pub fn validate_sender(event: &ChoreographyEvent) -> Result<(), AuthorizationError> {
        match event {
            ChoreographyEvent::BlockValidated { sender_id, .. } => {
                if *sender_id != SubsystemId::Consensus {
                    return Err(AuthorizationError::UnauthorizedSender {
                        event_type: "BlockValidated",
                        expected: SubsystemId::Consensus,
                        actual: *sender_id,
                    });
                }
            }
            ChoreographyEvent::MerkleRootComputed { sender_id, .. } => {
                if *sender_id != SubsystemId::TransactionIndexing {
                    return Err(AuthorizationError::UnauthorizedSender {
                        event_type: "MerkleRootComputed",
                        expected: SubsystemId::TransactionIndexing,
                        actual: *sender_id,
                    });
                }
            }
            ChoreographyEvent::StateRootComputed { sender_id, .. } => {
                if *sender_id != SubsystemId::StateManagement {
                    return Err(AuthorizationError::UnauthorizedSender {
                        event_type: "StateRootComputed",
                        expected: SubsystemId::StateManagement,
                        actual: *sender_id,
                    });
                }
            }
            ChoreographyEvent::BlockStored { sender_id, .. } => {
                if *sender_id != SubsystemId::BlockStorage {
                    return Err(AuthorizationError::UnauthorizedSender {
                        event_type: "BlockStored",
                        expected: SubsystemId::BlockStorage,
                        actual: *sender_id,
                    });
                }
            }
            ChoreographyEvent::BlockFinalized { sender_id, .. } => {
                if *sender_id != SubsystemId::Finality {
                    return Err(AuthorizationError::UnauthorizedSender {
                        event_type: "BlockFinalized",
                        expected: SubsystemId::Finality,
                        actual: *sender_id,
                    });
                }
            }
            ChoreographyEvent::AssemblyTimeout { sender_id, .. } => {
                if *sender_id != SubsystemId::BlockStorage {
                    return Err(AuthorizationError::UnauthorizedSender {
                        event_type: "AssemblyTimeout",
                        expected: SubsystemId::BlockStorage,
                        actual: *sender_id,
                    });
                }
            }
        }
        Ok(())
    }
}

/// Authorization error.
#[derive(Debug)]
pub enum AuthorizationError {
    UnauthorizedSender {
        event_type: &'static str,
        expected: SubsystemId,
        actual: SubsystemId,
    },
}

impl std::fmt::Display for AuthorizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthorizationError::UnauthorizedSender {
                event_type,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Unauthorized sender for {}: expected {:?}, got {:?}",
                    event_type, expected, actual
                )
            }
        }
    }
}

impl std::error::Error for AuthorizationError {}

/// Event router for choreography pattern.
pub struct EventRouter {
    /// Broadcast channel for choreography events.
    sender: broadcast::Sender<ChoreographyEvent>,
    /// Channel capacity for diagnostics.
    capacity: usize,
}

impl EventRouter {
    /// Create a new event router with the specified capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender, capacity }
    }

    /// Subscribe to choreography events.
    pub fn subscribe(&self) -> broadcast::Receiver<ChoreographyEvent> {
        self.sender.subscribe()
    }

    /// Get the channel capacity.
    /// 
    /// Useful for diagnostics and monitoring.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Publish an event (with authorization check).
    pub fn publish(&self, event: ChoreographyEvent) -> Result<(), AuthorizationError> {
        // Validate sender authorization
        AuthorizationRules::validate_sender(&event)?;

        // Publish to all subscribers
        match &event {
            ChoreographyEvent::BlockValidated { block_height, .. } => {
                debug!("Publishing BlockValidated for height {}", block_height);
            }
            ChoreographyEvent::MerkleRootComputed { block_hash, .. } => {
                debug!("Publishing MerkleRootComputed for {:?}", &block_hash[..4]);
            }
            ChoreographyEvent::StateRootComputed { block_hash, .. } => {
                debug!("Publishing StateRootComputed for {:?}", &block_hash[..4]);
            }
            ChoreographyEvent::BlockStored { block_height, .. } => {
                info!("Block {} stored successfully", block_height);
            }
            ChoreographyEvent::BlockFinalized { block_height, .. } => {
                info!("Block {} finalized!", block_height);
            }
            ChoreographyEvent::AssemblyTimeout {
                block_hash,
                missing_components,
                ..
            } => {
                warn!(
                    "Assembly timeout for {:?}, missing: {:?}",
                    &block_hash[..4],
                    missing_components
                );
            }
        }

        // Ignore send errors (no subscribers)
        let _ = self.sender.send(event);
        Ok(())
    }

    /// Get the number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for EventRouter {
    fn default() -> Self {
        Self::new(1024)
    }
}

/// Choreography coordinator that manages event routing between subsystems.
pub struct ChoreographyCoordinator {
    /// Event router.
    pub router: Arc<EventRouter>,
}

impl ChoreographyCoordinator {
    /// Create a new choreography coordinator.
    pub fn new() -> Self {
        Self {
            router: Arc::new(EventRouter::default()),
        }
    }

    /// Get a reference to the router for subsystems to use.
    pub fn router(&self) -> Arc<EventRouter> {
        Arc::clone(&self.router)
    }

    /// Start the coordination loop (monitors for issues).
    pub async fn start_monitoring(&self) {
        info!("Choreography coordinator started");
        info!("  - BlockValidated → triggers TxIndexing, StateMgmt, BlockStorage");
        info!("  - MerkleRootComputed + StateRootComputed → BlockStorage assembles");
        info!("  - BlockStored → Finality checks for finalization");
    }
}

impl Default for ChoreographyCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authorization_rules_block_validated() {
        // Valid sender
        let event = ChoreographyEvent::BlockValidated {
            block_hash: [0u8; 32],
            block_height: 1,
            sender_id: SubsystemId::Consensus,
        };
        assert!(AuthorizationRules::validate_sender(&event).is_ok());

        // Invalid sender
        let event = ChoreographyEvent::BlockValidated {
            block_hash: [0u8; 32],
            block_height: 1,
            sender_id: SubsystemId::Mempool,
        };
        assert!(AuthorizationRules::validate_sender(&event).is_err());
    }

    #[test]
    fn test_authorization_rules_merkle_root() {
        // Valid sender
        let event = ChoreographyEvent::MerkleRootComputed {
            block_hash: [0u8; 32],
            merkle_root: [1u8; 32],
            sender_id: SubsystemId::TransactionIndexing,
        };
        assert!(AuthorizationRules::validate_sender(&event).is_ok());

        // Invalid sender
        let event = ChoreographyEvent::MerkleRootComputed {
            block_hash: [0u8; 32],
            merkle_root: [1u8; 32],
            sender_id: SubsystemId::Consensus,
        };
        assert!(AuthorizationRules::validate_sender(&event).is_err());
    }

    #[test]
    fn test_authorization_rules_state_root() {
        // Valid sender
        let event = ChoreographyEvent::StateRootComputed {
            block_hash: [0u8; 32],
            state_root: [2u8; 32],
            sender_id: SubsystemId::StateManagement,
        };
        assert!(AuthorizationRules::validate_sender(&event).is_ok());
    }

    #[tokio::test]
    async fn test_event_router_publish_subscribe() {
        let router = EventRouter::new(16);
        let mut receiver = router.subscribe();

        let event = ChoreographyEvent::BlockValidated {
            block_hash: [0u8; 32],
            block_height: 42,
            sender_id: SubsystemId::Consensus,
        };

        router.publish(event.clone()).unwrap();

        let received = receiver.recv().await.unwrap();
        match received {
            ChoreographyEvent::BlockValidated { block_height, .. } => {
                assert_eq!(block_height, 42);
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_event_router_rejects_unauthorized() {
        let router = EventRouter::new(16);

        // Mempool cannot send BlockValidated
        let event = ChoreographyEvent::BlockValidated {
            block_hash: [0u8; 32],
            block_height: 1,
            sender_id: SubsystemId::Mempool,
        };

        assert!(router.publish(event).is_err());
    }
}
