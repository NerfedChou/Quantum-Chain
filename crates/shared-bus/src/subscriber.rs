//! # Event Subscriber
//!
//! Defines the subscription side of the event bus.

use crate::events::{BlockchainEvent, EventFilter};
use async_trait::async_trait;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
use thiserror::Error;
use tokio::sync::broadcast;
use tokio_stream::Stream;
use tracing::debug;

/// Errors from subscription operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SubscriptionError {
    /// The event bus was closed.
    #[error("Event bus closed")]
    Closed,
}

/// Trait for subscribing to events from the bus.
#[async_trait]
pub trait EventSubscriber: Send + Sync {
    /// Subscribe to events matching a filter.
    fn subscribe(&self, filter: EventFilter) -> Subscription;
}

/// A subscription handle for receiving events.
///
/// When dropped, the subscription is automatically cleaned up.
pub struct Subscription {
    /// The broadcast receiver.
    receiver: broadcast::Receiver<BlockchainEvent>,

    /// Filter for this subscription.
    filter: EventFilter,

    /// Reference to subscription tracking (for cleanup).
    subscriptions: Arc<RwLock<HashMap<String, usize>>>,

    /// Topic key for this subscription.
    topic_key: String,
}

impl Subscription {
    /// Create a new subscription.
    pub(crate) fn new(
        receiver: broadcast::Receiver<BlockchainEvent>,
        filter: EventFilter,
        subscriptions: Arc<RwLock<HashMap<String, usize>>>,
        topic_key: String,
    ) -> Self {
        Self {
            receiver,
            filter,
            subscriptions,
            topic_key,
        }
    }

    /// Receive the next event that matches the filter.
    ///
    /// # Returns
    ///
    /// - `Some(event)` - The next matching event
    /// - `None` - The channel was closed (bus dropped)
    pub async fn recv(&mut self) -> Option<BlockchainEvent> {
        loop {
            let event = match self.receiver.recv().await {
                Ok(e) => e,
                Err(broadcast::error::RecvError::Closed) => return None,
                Err(broadcast::error::RecvError::Lagged(count)) => {
                    debug!(lagged = count, "Subscriber lagged, some events dropped");
                    continue;
                }
            };

            if self.filter.matches(&event) {
                return Some(event);
            }
            // Event doesn't match filter, continue waiting
        }
    }

    /// Try to receive the next event without blocking.
    ///
    /// # Returns
    ///
    /// - `Ok(Some(event))` - An event was available and matched
    /// - `Ok(None)` - No event available (would block)
    /// - `Err(SubscriptionError::Closed)` - The channel was closed
    pub fn try_recv(&mut self) -> Result<Option<BlockchainEvent>, SubscriptionError> {
        loop {
            let event = match self.receiver.try_recv() {
                Ok(e) => e,
                Err(broadcast::error::TryRecvError::Empty) => return Ok(None),
                Err(broadcast::error::TryRecvError::Closed) => {
                    return Err(SubscriptionError::Closed)
                }
                Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
            };

            if self.filter.matches(&event) {
                return Ok(Some(event));
            }
            // Event doesn't match filter, try again
        }
    }

    /// Get the filter for this subscription.
    #[must_use]
    pub fn filter(&self) -> &EventFilter {
        &self.filter
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        // Decrement subscription count
        let Ok(mut subs) = self.subscriptions.write() else {
            return;
        };
        let Some(count) = subs.get_mut(&self.topic_key) else {
            debug!(topic = %self.topic_key, "Subscription dropped");
            return;
        };

        *count = count.saturating_sub(1);
        if *count == 0 {
            subs.remove(&self.topic_key);
        }
        debug!(topic = %self.topic_key, "Subscription dropped");
    }
}

/// A stream wrapper for subscriptions.
///
/// Implements `tokio_stream::Stream` for use with stream combinators.
pub struct EventStream {
    subscription: Subscription,
}

impl EventStream {
    /// Create a new event stream from a subscription.
    #[must_use]
    pub fn new(subscription: Subscription) -> Self {
        Self { subscription }
    }

    /// Get the filter for this stream.
    #[must_use]
    pub fn filter(&self) -> &EventFilter {
        self.subscription.filter()
    }
}

impl Stream for EventStream {
    type Item = BlockchainEvent;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Use try_recv for non-blocking check
        match self.subscription.try_recv() {
            Ok(Some(event)) => Poll::Ready(Some(event)),
            Ok(None) => {
                // No event ready, need to wait
                // Register waker and return pending
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            Err(SubscriptionError::Closed) => Poll::Ready(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventTopic;
    use crate::publisher::InMemoryEventBus;
    use crate::EventPublisher;
    use shared_types::entities::{Hash, ValidatedBlock};
    use std::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_subscription_recv() {
        let bus = InMemoryEventBus::new();
        let mut sub = bus.subscribe(EventFilter::all());

        // Publish event
        let event = BlockchainEvent::BlockValidated(ValidatedBlock::default());
        bus.publish(event.clone()).await;

        // Receive event
        let received = timeout(Duration::from_millis(100), sub.recv())
            .await
            .expect("timeout")
            .expect("event");

        assert!(matches!(received, BlockchainEvent::BlockValidated(_)));
    }

    #[tokio::test]
    async fn test_subscription_filter() {
        let bus = InMemoryEventBus::new();

        // Subscribe only to consensus events
        let mut sub = bus.subscribe(EventFilter::topics(vec![EventTopic::Consensus]));

        // Publish storage event (should be filtered)
        let storage_event = BlockchainEvent::BlockStored {
            block_height: 1,
            block_hash: Hash::default(),
        };
        bus.publish(storage_event).await;

        // Publish consensus event (should be received)
        let consensus_event = BlockchainEvent::BlockValidated(ValidatedBlock::default());
        bus.publish(consensus_event).await;

        // Should receive only the consensus event
        let received = timeout(Duration::from_millis(100), sub.recv())
            .await
            .expect("timeout")
            .expect("event");

        assert!(matches!(received, BlockchainEvent::BlockValidated(_)));
    }

    #[tokio::test]
    async fn test_subscription_drop_cleanup() {
        let bus = InMemoryEventBus::new();

        {
            let _sub1 = bus.subscribe(EventFilter::all());
            let _sub2 = bus.subscribe(EventFilter::all());
            assert_eq!(bus.subscriber_count(), 2);
        }

        // After drop, count should be 0
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[tokio::test]
    async fn test_try_recv_empty() {
        let bus = InMemoryEventBus::new();
        let mut sub = bus.subscribe(EventFilter::all());

        // No events published yet
        let result = sub.try_recv();
        assert!(matches!(result, Ok(None)));
    }

    #[tokio::test]
    async fn test_try_recv_event() {
        let bus = InMemoryEventBus::new();
        let mut sub = bus.subscribe(EventFilter::all());

        // Publish event
        let event = BlockchainEvent::BlockValidated(ValidatedBlock::default());
        bus.publish(event).await;

        // Should receive immediately
        let result = sub.try_recv();
        assert!(matches!(
            result,
            Ok(Some(BlockchainEvent::BlockValidated(_)))
        ));
    }

    #[test]
    fn test_event_stream_filter() {
        let bus = InMemoryEventBus::new();
        let filter = EventFilter::topics(vec![EventTopic::Consensus]);
        let stream = bus.event_stream(filter);

        assert_eq!(stream.filter().topics.len(), 1);
        assert_eq!(stream.filter().topics[0], EventTopic::Consensus);
    }
}
