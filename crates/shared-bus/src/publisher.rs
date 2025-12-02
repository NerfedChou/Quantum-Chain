//! # Event Publisher
//!
//! Defines the publishing side of the event bus.

use crate::events::{BlockchainEvent, EventFilter};
use crate::nonce_cache::TimeBoundedNonceCache;
use crate::subscriber::{EventStream, Subscription};
use crate::DEFAULT_CHANNEL_CAPACITY;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;
use tracing::{debug, warn};

/// Trait for publishing events to the bus.
///
/// Per Architecture.md Section 5, this is the interface subsystems use
/// to emit events for consumption by other subsystems.
#[async_trait]
pub trait EventPublisher: Send + Sync {
    /// Publish an event to the bus.
    ///
    /// # Arguments
    ///
    /// * `event` - The event to publish
    ///
    /// # Returns
    ///
    /// The number of active subscribers that received the event.
    async fn publish(&self, event: BlockchainEvent) -> usize;

    /// Get the total number of events published.
    fn events_published(&self) -> u64;
}

/// In-memory implementation of the event bus.
///
/// Uses `tokio::sync::broadcast` for multi-producer, multi-consumer semantics.
/// Suitable for single-node operation; distributed deployments would use
/// a different implementation (e.g., Redis, Kafka).
pub struct InMemoryEventBus {
    /// Broadcast sender for events.
    sender: broadcast::Sender<BlockchainEvent>,

    /// Nonce cache for replay prevention.
    nonce_cache: Arc<RwLock<TimeBoundedNonceCache>>,

    /// Active subscription count by topic.
    subscriptions: Arc<RwLock<HashMap<String, usize>>>,

    /// Total events published.
    events_published: AtomicU64,

    /// Channel capacity.
    capacity: usize,
}

impl InMemoryEventBus {
    /// Create a new in-memory event bus with default capacity.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CHANNEL_CAPACITY)
    }

    /// Create a new in-memory event bus with specified capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            nonce_cache: Arc::new(RwLock::new(TimeBoundedNonceCache::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            events_published: AtomicU64::new(0),
            capacity,
        }
    }

    /// Subscribe to events matching a filter.
    ///
    /// Returns a `Subscription` handle that can be used to receive events.
    #[must_use]
    pub fn subscribe(&self, filter: EventFilter) -> Subscription {
        let receiver = self.sender.subscribe();
        let topic_key = format!("{:?}", filter.topics);

        // Track subscription
        {
            if let Ok(mut subs) = self.subscriptions.write() {
                *subs.entry(topic_key.clone()).or_insert(0) += 1;
            }
        }

        debug!(topics = ?filter.topics, "New subscription created");

        Subscription::new(receiver, filter, self.subscriptions.clone(), topic_key)
    }

    /// Get a stream of events matching a filter.
    ///
    /// This is a convenience method that returns an `EventStream`.
    #[must_use]
    pub fn event_stream(&self, filter: EventFilter) -> EventStream {
        EventStream::new(self.subscribe(filter))
    }

    /// Get the number of active subscribers.
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Get the channel capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get access to the nonce cache for message validation.
    pub fn nonce_cache(&self) -> Arc<RwLock<TimeBoundedNonceCache>> {
        self.nonce_cache.clone()
    }
}

impl Default for InMemoryEventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventPublisher for InMemoryEventBus {
    async fn publish(&self, event: BlockchainEvent) -> usize {
        let topic = event.topic();
        let source = event.source_subsystem();

        // Always increment counter (event was attempted)
        self.events_published.fetch_add(1, Ordering::Relaxed);

        match self.sender.send(event) {
            Ok(receiver_count) => {
                debug!(
                    topic = ?topic,
                    source = source,
                    receivers = receiver_count,
                    "Event published"
                );
                receiver_count
            }
            Err(e) => {
                // No receivers - event is dropped
                warn!(
                    topic = ?topic,
                    source = source,
                    error = %e,
                    "Event dropped (no receivers)"
                );
                0
            }
        }
    }

    fn events_published(&self) -> u64 {
        self.events_published.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventTopic;
    use shared_types::entities::ValidatedBlock;

    #[tokio::test]
    async fn test_publish_no_subscribers() {
        let bus = InMemoryEventBus::new();
        let event = BlockchainEvent::BlockValidated(ValidatedBlock::default());

        let receivers = bus.publish(event).await;
        assert_eq!(receivers, 0);
        assert_eq!(bus.events_published(), 1);
    }

    #[tokio::test]
    async fn test_publish_with_subscriber() {
        let bus = InMemoryEventBus::new();

        // Create subscriber BEFORE publishing
        let _sub = bus.subscribe(EventFilter::all());

        let event = BlockchainEvent::BlockValidated(ValidatedBlock::default());
        let receivers = bus.publish(event).await;

        assert_eq!(receivers, 1);
        assert_eq!(bus.subscriber_count(), 1);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let bus = InMemoryEventBus::new();

        let _sub1 = bus.subscribe(EventFilter::all());
        let _sub2 = bus.subscribe(EventFilter::all());
        let _sub3 = bus.subscribe(EventFilter::topics(vec![EventTopic::Consensus]));

        let event = BlockchainEvent::BlockValidated(ValidatedBlock::default());
        let receivers = bus.publish(event).await;

        assert_eq!(receivers, 3);
        assert_eq!(bus.subscriber_count(), 3);
    }

    #[tokio::test]
    async fn test_custom_capacity() {
        let bus = InMemoryEventBus::with_capacity(100);
        assert_eq!(bus.capacity(), 100);
    }

    #[test]
    fn test_default_bus() {
        let bus = InMemoryEventBus::default();
        assert_eq!(bus.capacity(), DEFAULT_CHANNEL_CAPACITY);
        assert_eq!(bus.subscriber_count(), 0);
        assert_eq!(bus.events_published(), 0);
    }
}
