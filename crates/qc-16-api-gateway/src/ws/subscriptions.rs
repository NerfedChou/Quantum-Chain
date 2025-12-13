//! WebSocket subscription manager per SPEC-16 Section 5.

use crate::domain::correlation::CorrelationId;
use crate::domain::types::{Filter, Hash};
use crate::SubscriptionType;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::broadcast;
use tracing::debug;

/// Subscription ID (hex string)
pub type SubscriptionId = String;

/// Subscription info
#[derive(Debug, Clone)]
pub struct Subscription {
    /// Unique subscription ID
    pub id: SubscriptionId,
    /// Subscription type
    pub sub_type: SubscriptionType,
    /// Filter for logs subscriptions
    pub filter: Option<Filter>,
    /// Connection ID this subscription belongs to
    pub connection_id: CorrelationId,
}

/// Subscription notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: SubscriptionParams,
}

/// Subscription params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionParams {
    pub subscription: SubscriptionId,
    pub result: serde_json::Value,
}

impl SubscriptionNotification {
    pub fn new(subscription_id: SubscriptionId, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: "eth_subscription".to_string(),
            params: SubscriptionParams {
                subscription: subscription_id,
                result,
            },
        }
    }
}

/// Subscription manager
pub struct SubscriptionManager {
    /// All active subscriptions by ID
    subscriptions: DashMap<SubscriptionId, Subscription>,
    /// Subscriptions by connection ID
    by_connection: DashMap<CorrelationId, Vec<SubscriptionId>>,
    /// Counter for generating subscription IDs
    id_counter: AtomicU64,
    /// Broadcast channel for new blocks (newHeads)
    new_heads_tx: broadcast::Sender<serde_json::Value>,
    /// Broadcast channel for pending transactions
    pending_tx_tx: broadcast::Sender<Hash>,
    /// Max subscriptions per connection
    max_per_connection: u32,
}

impl SubscriptionManager {
    pub fn new(max_per_connection: u32) -> Self {
        let (new_heads_tx, _) = broadcast::channel(1024);
        let (pending_tx_tx, _) = broadcast::channel(4096);

        Self {
            subscriptions: DashMap::new(),
            by_connection: DashMap::new(),
            id_counter: AtomicU64::new(1),
            new_heads_tx,
            pending_tx_tx,
            max_per_connection,
        }
    }

    /// Subscribe to a topic
    pub fn subscribe(
        &self,
        connection_id: CorrelationId,
        sub_type: SubscriptionType,
        filter: Option<Filter>,
    ) -> Result<SubscriptionId, SubscribeError> {
        // Check connection limit
        let mut conn_subs = self.by_connection.entry(connection_id).or_default();
        if conn_subs.len() as u32 >= self.max_per_connection {
            return Err(SubscribeError::TooManySubscriptions);
        }

        // Generate subscription ID
        let id_num = self.id_counter.fetch_add(1, Ordering::SeqCst);
        let sub_id = format!("0x{:x}", id_num);

        // Create subscription
        let subscription = Subscription {
            id: sub_id.clone(),
            sub_type,
            filter,
            connection_id,
        };

        // Store subscription
        self.subscriptions.insert(sub_id.clone(), subscription);
        conn_subs.push(sub_id.clone());

        debug!(
            subscription_id = %sub_id,
            connection_id = %connection_id,
            sub_type = ?sub_type,
            "Created subscription"
        );

        Ok(sub_id)
    }

    /// Unsubscribe from a topic
    pub fn unsubscribe(&self, subscription_id: &str) -> bool {
        if let Some((_, sub)) = self.subscriptions.remove(subscription_id) {
            // Remove from connection tracking
            if let Some(mut conn_subs) = self.by_connection.get_mut(&sub.connection_id) {
                conn_subs.retain(|id| id != subscription_id);
            }

            debug!(
                subscription_id = %subscription_id,
                "Removed subscription"
            );
            true
        } else {
            false
        }
    }

    /// Remove all subscriptions for a connection
    pub fn remove_connection(&self, connection_id: &CorrelationId) {
        if let Some((_, sub_ids)) = self.by_connection.remove(connection_id) {
            for sub_id in sub_ids {
                self.subscriptions.remove(&sub_id);
            }
            debug!(
                connection_id = %connection_id,
                "Removed all subscriptions for connection"
            );
        }
    }

    /// Get subscription by ID
    pub fn get(&self, subscription_id: &str) -> Option<Subscription> {
        self.subscriptions.get(subscription_id).map(|r| r.clone())
    }

    /// Get all subscriptions for a connection
    pub fn get_connection_subscriptions(&self, connection_id: &CorrelationId) -> Vec<Subscription> {
        self.by_connection
            .get(connection_id)
            .map(|sub_ids| {
                sub_ids
                    .iter()
                    .filter_map(|id| self.subscriptions.get(id).map(|r| r.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get new heads broadcast receiver
    pub fn subscribe_new_heads(&self) -> broadcast::Receiver<serde_json::Value> {
        self.new_heads_tx.subscribe()
    }

    /// Get pending transactions broadcast receiver
    pub fn subscribe_pending_txs(&self) -> broadcast::Receiver<Hash> {
        self.pending_tx_tx.subscribe()
    }

    /// Broadcast new block header
    pub fn broadcast_new_head(&self, header: serde_json::Value) {
        if self.new_heads_tx.receiver_count() > 0 {
            let _ = self.new_heads_tx.send(header);
        }
    }

    /// Broadcast new pending transaction
    pub fn broadcast_pending_tx(&self, tx_hash: Hash) {
        if self.pending_tx_tx.receiver_count() > 0 {
            let _ = self.pending_tx_tx.send(tx_hash);
        }
    }

    /// Get subscriptions matching a log filter
    pub fn get_matching_log_subscriptions(
        &self,
        log_address: &crate::domain::types::Address,
        log_topics: &[Hash],
    ) -> Vec<Subscription> {
        self.subscriptions
            .iter()
            .filter(|r| {
                if r.sub_type != SubscriptionType::Logs {
                    return false;
                }

                // Check filter match
                if let Some(filter) = &r.filter {
                    match_log_filter(filter, log_address, log_topics)
                } else {
                    true // No filter = all logs
                }
            })
            .map(|r| r.clone())
            .collect()
    }

    /// Get total subscription count
    pub fn total_subscriptions(&self) -> usize {
        self.subscriptions.len()
    }

    /// Get connection count
    pub fn connection_count(&self) -> usize {
        self.by_connection.len()
    }
}

/// Check if a log matches a filter
fn match_log_filter(
    filter: &Filter,
    log_address: &crate::domain::types::Address,
    log_topics: &[Hash],
) -> bool {
    // Check address filter
    if let Some(ref addr_filter) = filter.address {
        let matches = match addr_filter {
            crate::domain::types::FilterAddress::Single(addr) => addr == log_address,
            crate::domain::types::FilterAddress::Multiple(addrs) => addrs.contains(log_address),
        };
        if !matches {
            return false;
        }
    }

    // Check topics filter
    if let Some(ref topics) = filter.topics {
        for (i, topic_filter) in topics.iter().enumerate() {
            if let Some(filter) = topic_filter {
                if i >= log_topics.len() {
                    return false;
                }

                let matches = match filter {
                    crate::domain::types::FilterTopic::Single(t) => t == &log_topics[i],
                    crate::domain::types::FilterTopic::Multiple(ts) => ts.contains(&log_topics[i]),
                };

                if !matches {
                    return false;
                }
            }
        }
    }

    true
}

/// Subscribe error
#[derive(Debug, thiserror::Error)]
pub enum SubscribeError {
    #[error("too many subscriptions for this connection")]
    TooManySubscriptions,
    #[error("invalid subscription type")]
    InvalidType,
    #[error("invalid filter")]
    InvalidFilter,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscribe_unsubscribe() {
        let manager = SubscriptionManager::new(100);
        let conn_id = CorrelationId::new();

        let sub_id = manager
            .subscribe(conn_id, SubscriptionType::NewHeads, None)
            .unwrap();

        assert!(manager.get(&sub_id).is_some());
        assert_eq!(manager.total_subscriptions(), 1);

        assert!(manager.unsubscribe(&sub_id));
        assert!(manager.get(&sub_id).is_none());
        assert_eq!(manager.total_subscriptions(), 0);
    }

    #[test]
    fn test_connection_limit() {
        let manager = SubscriptionManager::new(2);
        let conn_id = CorrelationId::new();

        let _ = manager
            .subscribe(conn_id, SubscriptionType::NewHeads, None)
            .unwrap();
        let _ = manager
            .subscribe(conn_id, SubscriptionType::Logs, None)
            .unwrap();

        // Should fail - at limit
        let result = manager.subscribe(conn_id, SubscriptionType::NewPendingTransactions, None);
        assert!(matches!(result, Err(SubscribeError::TooManySubscriptions)));
    }

    #[test]
    fn test_remove_connection() {
        let manager = SubscriptionManager::new(100);
        let conn_id = CorrelationId::new();

        let _ = manager
            .subscribe(conn_id, SubscriptionType::NewHeads, None)
            .unwrap();
        let _ = manager
            .subscribe(conn_id, SubscriptionType::Logs, None)
            .unwrap();

        assert_eq!(manager.total_subscriptions(), 2);

        manager.remove_connection(&conn_id);

        assert_eq!(manager.total_subscriptions(), 0);
    }
}
