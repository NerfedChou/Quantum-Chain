//! Event publisher adapter for the Mempool subsystem.
//!
//! Publishes events to the shared bus for inter-subsystem communication.

use crate::domain::Hash;
use crate::ipc::payloads::MempoolStatusPayload;
use uuid::Uuid;

/// Topics for mempool events.
pub mod topics {
    /// Topic for transaction batch proposals (to Consensus).
    pub const PROPOSE_TRANSACTION_BATCH: &str = "mempool.propose_batch";
    /// Topic for balance check requests (to State Management).
    pub const BALANCE_CHECK_REQUEST: &str = "state.balance_check";
    /// Topic for mempool status updates.
    pub const MEMPOOL_STATUS: &str = "mempool.status";
}

/// Event publisher trait for the Mempool.
///
/// Implementations connect to the actual event bus (shared-bus).
pub trait MempoolEventPublisher: Send + Sync {
    /// Publishes a transaction batch proposal to Consensus (Subsystem 8).
    fn publish_propose_batch(
        &self,
        correlation_id: Uuid,
        tx_hashes: Vec<Hash>,
        total_gas: u64,
        target_block_height: u64,
    ) -> Result<(), PublishError>;

    /// Publishes a balance check request to State Management (Subsystem 4).
    fn publish_balance_check(
        &self,
        correlation_id: Uuid,
        address: [u8; 32],
        required_balance: u128,
    ) -> Result<(), PublishError>;

    /// Publishes mempool status update.
    fn publish_status(&self, status: MempoolStatusPayload) -> Result<(), PublishError>;
}

/// Error type for publish operations.
#[derive(Debug, Clone)]
pub enum PublishError {
    /// The event bus is not connected.
    NotConnected,
    /// Failed to serialize the message.
    SerializationError(String),
    /// The topic does not exist.
    TopicNotFound(String),
    /// Internal error.
    Internal(String),
}

impl std::fmt::Display for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConnected => write!(f, "Event bus not connected"),
            Self::SerializationError(e) => write!(f, "Serialization error: {}", e),
            Self::TopicNotFound(t) => write!(f, "Topic not found: {}", t),
            Self::Internal(e) => write!(f, "Internal error: {}", e),
        }
    }
}

impl std::error::Error for PublishError {}

/// No-op publisher for testing without an event bus.
#[derive(Debug, Clone, Default)]
pub struct NoOpPublisher;

impl MempoolEventPublisher for NoOpPublisher {
    fn publish_propose_batch(
        &self,
        _correlation_id: Uuid,
        _tx_hashes: Vec<Hash>,
        _total_gas: u64,
        _target_block_height: u64,
    ) -> Result<(), PublishError> {
        Ok(())
    }

    fn publish_balance_check(
        &self,
        _correlation_id: Uuid,
        _address: [u8; 32],
        _required_balance: u128,
    ) -> Result<(), PublishError> {
        Ok(())
    }

    fn publish_status(&self, _status: MempoolStatusPayload) -> Result<(), PublishError> {
        Ok(())
    }
}

/// Recording publisher for testing.
#[cfg(test)]
pub struct RecordingPublisher {
    pub batches: std::sync::Mutex<Vec<(Uuid, Vec<Hash>, u64, u64)>>,
    pub balance_checks: std::sync::Mutex<Vec<(Uuid, [u8; 32], u128)>>,
    pub statuses: std::sync::Mutex<Vec<MempoolStatusPayload>>,
}

#[cfg(test)]
impl RecordingPublisher {
    pub fn new() -> Self {
        Self {
            batches: std::sync::Mutex::new(Vec::new()),
            balance_checks: std::sync::Mutex::new(Vec::new()),
            statuses: std::sync::Mutex::new(Vec::new()),
        }
    }
}

#[cfg(test)]
impl MempoolEventPublisher for RecordingPublisher {
    fn publish_propose_batch(
        &self,
        correlation_id: Uuid,
        tx_hashes: Vec<Hash>,
        total_gas: u64,
        target_block_height: u64,
    ) -> Result<(), PublishError> {
        self.batches
            .lock()
            .unwrap()
            .push((correlation_id, tx_hashes, total_gas, target_block_height));
        Ok(())
    }

    fn publish_balance_check(
        &self,
        correlation_id: Uuid,
        address: [u8; 32],
        required_balance: u128,
    ) -> Result<(), PublishError> {
        self.balance_checks
            .lock()
            .unwrap()
            .push((correlation_id, address, required_balance));
        Ok(())
    }

    fn publish_status(&self, status: MempoolStatusPayload) -> Result<(), PublishError> {
        self.statuses.lock().unwrap().push(status);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_op_publisher() {
        let publisher = NoOpPublisher;
        assert!(publisher
            .publish_propose_batch(Uuid::new_v4(), vec![], 0, 0)
            .is_ok());
        assert!(publisher
            .publish_balance_check(Uuid::new_v4(), [0; 32], 0)
            .is_ok());
    }

    #[test]
    fn test_recording_publisher() {
        let publisher = RecordingPublisher::new();
        let id = Uuid::new_v4();
        let hashes = vec![[0xAA; 32], [0xBB; 32]];

        publisher
            .publish_propose_batch(id, hashes.clone(), 42000, 1)
            .unwrap();

        let batches = publisher.batches.lock().unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].0, id);
        assert_eq!(batches[0].1, hashes);
        assert_eq!(batches[0].2, 42000);
        assert_eq!(batches[0].3, 1);
    }
}
