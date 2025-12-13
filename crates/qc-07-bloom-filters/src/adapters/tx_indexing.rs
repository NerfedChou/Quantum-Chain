//! Transaction Indexing Adapter
//!
//! Reference: SPEC-07 Section 4.1 - Connects to qc-03-transaction-indexing
//!
//! Per Architecture.md v2.3: Communicates via shared-bus ApiQuery events,
//! NOT direct function calls.

use async_trait::async_trait;
use shared_bus::{BlockchainEvent, EventFilter, EventPublisher, EventTopic, InMemoryEventBus};
use shared_types::{Hash, SignedTransaction};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::error::DataError;
use crate::ports::{TransactionAddresses, TransactionDataProvider};

/// Default timeout for IPC queries
const IPC_TIMEOUT: Duration = Duration::from_secs(30);

/// Adapter for Transaction Indexing subsystem (qc-03)
///
/// Connects to qc-03-transaction-indexing via shared-bus ApiQuery events.
/// Per Architecture.md Section 5, direct subsystem calls are FORBIDDEN.
pub struct TxIndexingAdapter {
    /// Reference to the event bus
    bus: Arc<InMemoryEventBus>,
    /// Cache for recent transaction hashes (block_height -> hashes)
    tx_cache: RwLock<lru::LruCache<u64, Vec<Hash>>>,
}

impl TxIndexingAdapter {
    /// Create a new adapter connected to the event bus
    pub fn new(bus: Arc<InMemoryEventBus>) -> Self {
        Self {
            bus,
            tx_cache: RwLock::new(lru::LruCache::new(
                std::num::NonZeroUsize::new(100).unwrap(),
            )),
        }
    }

    /// Send an ApiQuery to qc-03 and wait for response
    async fn query_tx_indexing(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, DataError> {
        let correlation_id = Uuid::new_v4().to_string();

        // Publish ApiQuery to the bus
        let event = BlockchainEvent::ApiQuery {
            correlation_id: correlation_id.clone(),
            target: "qc-03-transaction-indexing".to_string(),
            method: method.to_string(),
            params,
        };

        // Check if there are any ApiGateway subscribers (our query handler)
        // by checking if the bus has any subscribers for ApiGateway topic
        // Note: We need at least one subscriber (the ApiQueryHandler in node-runtime)
        // to process our query and return a response.
        let subscribers = self.bus.subscriber_count();
        if subscribers == 0 {
            warn!(
                method = method,
                "No subscribers for ApiQuery to qc-03 (is node-runtime running?)"
            );
            return Err(DataError::ConnectionError(
                "No subscribers for qc-03 queries".to_string(),
            ));
        }

        // Subscribe to ApiGateway topic for response AFTER checking
        let filter = EventFilter::topics(vec![EventTopic::ApiGateway]);
        let mut stream = self.bus.event_stream(filter);

        let receivers = self.bus.publish(event).await;
        debug!(
            correlation_id = %correlation_id,
            method = method,
            receivers = receivers,
            "ApiQuery sent to qc-03, waiting for response"
        );

        // Wait for response with timeout
        let response = timeout(IPC_TIMEOUT, async {
            use futures::StreamExt;
            while let Some(event) = stream.next().await {
                if let BlockchainEvent::ApiQueryResponse {
                    correlation_id: resp_id,
                    result,
                    ..
                } = event
                {
                    if resp_id == correlation_id {
                        return Some(result);
                    }
                }
            }
            None
        })
        .await
        .map_err(|_| DataError::Timeout)?
        .ok_or_else(|| DataError::ConnectionError("Response stream ended".to_string()))?;

        // Convert result
        response.map_err(|e| DataError::QueryError(e.message))
    }
}

#[async_trait]
impl TransactionDataProvider for TxIndexingAdapter {
    async fn get_transaction_hashes(&self, block_height: u64) -> Result<Vec<Hash>, DataError> {
        // Check cache first
        {
            let cache = self.tx_cache.read().await;
            if let Some(hashes) = cache.peek(&block_height) {
                debug!(block_height = block_height, "Cache hit for tx hashes");
                return Ok(hashes.clone());
            }
        }

        // Query qc-03 via shared-bus
        let params = serde_json::json!({
            "block_height": block_height
        });

        let result = self
            .query_tx_indexing("get_transaction_hashes", params)
            .await?;

        // Parse response - Hash is a type alias for [u8; 32]
        let hashes: Vec<Hash> =
            serde_json::from_value(result).map_err(|e| DataError::ParseError(e.to_string()))?;

        // Update cache
        {
            let mut cache = self.tx_cache.write().await;
            cache.put(block_height, hashes.clone());
        }

        Ok(hashes)
    }

    async fn get_transactions(
        &self,
        block_height: u64,
    ) -> Result<Vec<SignedTransaction>, DataError> {
        // Query qc-03 via shared-bus
        let params = serde_json::json!({
            "block_height": block_height
        });

        let result = self.query_tx_indexing("get_transactions", params).await?;

        // Parse response
        let transactions: Vec<SignedTransaction> =
            serde_json::from_value(result).map_err(|e| DataError::ParseError(e.to_string()))?;

        Ok(transactions)
    }

    async fn get_transaction_addresses(
        &self,
        block_height: u64,
    ) -> Result<Vec<TransactionAddresses>, DataError> {
        // Query qc-03 via shared-bus
        let params = serde_json::json!({
            "block_height": block_height
        });

        let result = self
            .query_tx_indexing("get_transaction_addresses", params)
            .await?;

        // Parse response - expecting array of address data
        #[derive(serde::Deserialize)]
        struct AddressData {
            tx_hash: [u8; 32],
            sender: [u8; 20],
            recipient: Option<[u8; 20]>,
            created_contract: Option<[u8; 20]>,
            log_addresses: Vec<[u8; 20]>,
        }

        let addresses: Vec<AddressData> =
            serde_json::from_value(result).map_err(|e| DataError::ParseError(e.to_string()))?;

        Ok(addresses
            .into_iter()
            .map(|a| TransactionAddresses {
                tx_hash: Hash::from(a.tx_hash),
                sender: a.sender,
                recipient: a.recipient,
                created_contract: a.created_contract,
                log_addresses: a.log_addresses,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adapter_creation() {
        let bus = Arc::new(InMemoryEventBus::new());
        let _adapter = TxIndexingAdapter::new(bus);
        // Adapter created successfully
    }

    #[tokio::test]
    async fn test_no_subscribers_returns_error() {
        let bus = Arc::new(InMemoryEventBus::new());
        let adapter = TxIndexingAdapter::new(bus);

        // No subscribers, should return connection error
        let result = adapter.get_transaction_hashes(100).await;
        assert!(matches!(result, Err(DataError::ConnectionError(_))));
    }
}
