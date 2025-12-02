//! Event subscriber adapter for the Mempool subsystem.
//!
//! Subscribes to events from the shared bus for Two-Phase Commit handling.

use crate::domain::Hash;
use crate::ipc::payloads::{BlockRejectedNotification, BlockStorageConfirmation};

/// Topics that the Mempool subscribes to.
pub mod topics {
    /// Topic for storage confirmations (from Block Storage).
    pub const BLOCK_STORAGE_CONFIRMATION: &str = "storage.block_confirmed";
    /// Topic for block rejection notifications.
    pub const BLOCK_REJECTED: &str = "consensus.block_rejected";
    /// Topic for add transaction requests (from Signature Verification).
    pub const ADD_TRANSACTION: &str = "mempool.add_transaction";
    /// Topic for get transactions requests (from Consensus).
    pub const GET_TRANSACTIONS: &str = "mempool.get_transactions";
}

/// Event subscriber trait for the Mempool.
///
/// Implementations connect to the actual event bus (shared-bus).
pub trait MempoolEventSubscriber: Send + Sync {
    /// Called when a block storage confirmation is received.
    ///
    /// This is Phase 2a of the Two-Phase Commit protocol.
    fn on_storage_confirmation(&mut self, confirmation: BlockStorageConfirmation);

    /// Called when a block rejection is received.
    ///
    /// This is Phase 2b of the Two-Phase Commit protocol.
    fn on_block_rejected(&mut self, notification: BlockRejectedNotification);
}

/// Subscription handle for managing event subscriptions.
pub struct SubscriptionHandle {
    /// Unique identifier for this subscription.
    pub id: uuid::Uuid,
    /// Topic being subscribed to.
    pub topic: String,
}

impl SubscriptionHandle {
    /// Creates a new subscription handle.
    pub fn new(topic: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            topic: topic.into(),
        }
    }
}

/// Event types that can be received by the Mempool.
#[derive(Debug, Clone)]
pub enum MempoolEvent {
    /// Storage confirmation received.
    StorageConfirmation(BlockStorageConfirmation),
    /// Block rejected notification received.
    BlockRejected(BlockRejectedNotification),
    /// Add transaction request received.
    AddTransaction {
        tx_hash: Hash,
        sender: [u8; 32],
        nonce: u64,
        gas_price: u128,
        gas_limit: u64,
        value: u128,
        data: Vec<u8>,
    },
    /// Get transactions request received.
    GetTransactions {
        max_count: u32,
        max_gas: u64,
        target_block_height: u64,
        reply_to: String,
    },
}

/// No-op subscriber for testing.
#[derive(Debug, Default)]
pub struct NoOpSubscriber;

impl MempoolEventSubscriber for NoOpSubscriber {
    fn on_storage_confirmation(&mut self, _confirmation: BlockStorageConfirmation) {}
    fn on_block_rejected(&mut self, _notification: BlockRejectedNotification) {}
}

/// Recording subscriber for testing.
#[cfg(test)]
pub struct RecordingSubscriber {
    pub confirmations: Vec<BlockStorageConfirmation>,
    pub rejections: Vec<BlockRejectedNotification>,
}

#[cfg(test)]
impl RecordingSubscriber {
    pub fn new() -> Self {
        Self {
            confirmations: Vec::new(),
            rejections: Vec::new(),
        }
    }
}

#[cfg(test)]
impl MempoolEventSubscriber for RecordingSubscriber {
    fn on_storage_confirmation(&mut self, confirmation: BlockStorageConfirmation) {
        self.confirmations.push(confirmation);
    }

    fn on_block_rejected(&mut self, notification: BlockRejectedNotification) {
        self.rejections.push(notification);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::payloads::BlockRejectionReason;
    use uuid::Uuid;

    #[test]
    fn test_subscription_handle() {
        let handle = SubscriptionHandle::new("test.topic");
        assert_eq!(handle.topic, "test.topic");
    }

    #[test]
    fn test_recording_subscriber() {
        let mut subscriber = RecordingSubscriber::new();

        let confirmation = BlockStorageConfirmation {
            correlation_id: Uuid::new_v4(),
            block_hash: [0xAA; 32],
            block_height: 1,
            included_transactions: vec![[0xBB; 32]],
            storage_timestamp: 1000,
        };

        subscriber.on_storage_confirmation(confirmation.clone());

        assert_eq!(subscriber.confirmations.len(), 1);
        assert_eq!(subscriber.confirmations[0].block_height, 1);
    }

    #[test]
    fn test_recording_subscriber_rejection() {
        let mut subscriber = RecordingSubscriber::new();

        let notification = BlockRejectedNotification {
            correlation_id: Uuid::new_v4(),
            block_hash: [0xAA; 32],
            block_height: 1,
            affected_transactions: vec![[0xBB; 32]],
            rejection_reason: BlockRejectionReason::ConsensusRejected,
        };

        subscriber.on_block_rejected(notification);

        assert_eq!(subscriber.rejections.len(), 1);
        assert_eq!(
            subscriber.rejections[0].rejection_reason,
            BlockRejectionReason::ConsensusRejected
        );
    }
}
