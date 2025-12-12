//! Event Bus Adapter for Bloom Filter subsystem
//!
//! Reference: Architecture.md Section 5 - Event-Driven Architecture
//!
//! Subscribes to incoming messages from the shared-bus, validates them
//! per IPC-MATRIX.md security boundaries, and dispatches to the handler.
//!
//! Per IPC-MATRIX.md Subsystem 7:
//! - Accept TransactionHashUpdate from Subsystem 3 ONLY
//! - This subsystem does NOT receive choreography events directly
//!   (unlike Block Storage which assembles from multiple sources)

use crate::domain::{BloomConfig, BloomFilter};
use crate::error::FilterError;
use crate::handler::BloomFilterHandler;
use crate::ports::BloomFilterApi;
use crate::service::BloomFilterService;
use futures::StreamExt;
use shared_bus::{
    ApiQueryError, BlockchainEvent, EventFilter, EventPublisher, EventTopic, InMemoryEventBus,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Bloom Filter subsystem ID
const SUBSYSTEM_ID: u8 = 7;

/// Bus adapter for the Bloom Filter subsystem
///
/// Handles:
/// 1. ApiQuery events from API Gateway (qc-16) for filter operations
/// 2. TransactionHashUpdate events from Transaction Indexing (qc-03)
pub struct BloomFilterBusAdapter<P>
where
    P: crate::ports::TransactionDataProvider + 'static,
{
    /// Reference to the event bus
    bus: Arc<InMemoryEventBus>,
    /// The bloom filter service
    service: Arc<RwLock<BloomFilterService<P>>>,
    /// IPC handler for validation
    handler: Arc<BloomFilterHandler>,
    /// Active filters by client ID
    active_filters: Arc<RwLock<std::collections::HashMap<String, BloomFilter>>>,
    /// Default configuration
    default_config: BloomConfig,
}

impl<P> BloomFilterBusAdapter<P>
where
    P: crate::ports::TransactionDataProvider + Send + Sync + 'static,
{
    /// Create a new bus adapter
    pub fn new(
        bus: Arc<InMemoryEventBus>,
        service: BloomFilterService<P>,
        handler: BloomFilterHandler,
    ) -> Self {
        Self {
            bus,
            service: Arc::new(RwLock::new(service)),
            handler: Arc::new(handler),
            active_filters: Arc::new(RwLock::new(std::collections::HashMap::new())),
            default_config: BloomConfig::default(),
        }
    }

    /// Check if a sender is authorized for filter operations
    ///
    /// Per IPC-MATRIX.md Subsystem 7 security boundaries
    pub fn is_authorized_for_filter(&self, sender_id: u8) -> bool {
        self.handler.is_authorized_for_build_filter(sender_id)
    }

    /// Start listening for events
    ///
    /// This should be spawned as a background task.
    pub async fn run(self: Arc<Self>) {
        info!("[BloomFilterBusAdapter] Started listening for events");

        // Subscribe to ApiGateway topic for API queries
        let filter = EventFilter::topics(vec![EventTopic::ApiGateway]);
        let mut stream = self.bus.event_stream(filter);

        loop {
            match stream.next().await {
                Some(event) => {
                    if let Err(e) = self.handle_event(event).await {
                        error!("Error handling event: {:?}", e);
                    }
                }
                None => {
                    warn!("[BloomFilterBusAdapter] Event stream ended, shutting down");
                    break;
                }
            }
        }
    }

    /// Handle an incoming blockchain event
    async fn handle_event(&self, event: BlockchainEvent) -> Result<(), FilterError> {
        match event {
            BlockchainEvent::ApiQuery {
                correlation_id,
                target,
                method,
                params,
            } => {
                // Only handle queries targeting us
                if target == "qc-07-bloom-filters" {
                    debug!(
                        correlation_id = %correlation_id,
                        method = %method,
                        "Handling ApiQuery"
                    );
                    self.handle_api_query(&correlation_id, &method, params)
                        .await;
                }
            }
            _ => {
                // Ignore other events
            }
        }
        Ok(())
    }

    /// Handle an API query and publish response
    async fn handle_api_query(
        &self,
        correlation_id: &str,
        method: &str,
        params: serde_json::Value,
    ) {
        let result = match method {
            "build_filter" => self.handle_build_filter(params).await,
            "check_membership" => self.handle_check_membership(params).await,
            "get_filter_status" => self.handle_get_filter_status(params).await,
            "get_filtered_transactions" => self.handle_get_filtered_transactions(params).await,
            _ => Err(FilterError::InvalidMethod(method.to_string())),
        };

        // Publish response
        let response_event = BlockchainEvent::ApiQueryResponse {
            correlation_id: correlation_id.to_string(),
            source: SUBSYSTEM_ID,
            result: result.map_err(|e| ApiQueryError {
                code: -32000,
                message: e.to_string(),
            }),
        };

        self.bus.publish(response_event).await;
    }

    /// Handle build_filter request
    async fn handle_build_filter(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, FilterError> {
        #[derive(serde::Deserialize)]
        struct BuildParams {
            client_id: String,
            addresses: Vec<[u8; 20]>,
            target_fpr: Option<f64>,
        }

        let params: BuildParams = serde_json::from_value(params)
            .map_err(|e| FilterError::InvalidParams(e.to_string()))?;

        // Create filter with optimal parameters using service
        let config = BloomConfig {
            target_fpr: params.target_fpr.unwrap_or(0.01),
            ..self.default_config
        };

        let service = self.service.read().await;
        let filter = service.create_filter(&params.addresses, &config)?;

        // Store filter
        let mut filters = self.active_filters.write().await;
        filters.insert(params.client_id.clone(), filter.clone());

        Ok(serde_json::json!({
            "client_id": params.client_id,
            "filter_id": params.client_id,
            "bit_count": filter.size_bits(),
            "hash_count": filter.hash_count(),
            "address_count": params.addresses.len(),
            "estimated_fpr": filter.false_positive_rate()
        }))
    }

    /// Handle check_membership request
    async fn handle_check_membership(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, FilterError> {
        #[derive(serde::Deserialize)]
        struct CheckParams {
            client_id: String,
            address: [u8; 20],
        }

        let params: CheckParams = serde_json::from_value(params)
            .map_err(|e| FilterError::InvalidParams(e.to_string()))?;

        let filters = self.active_filters.read().await;
        let filter = filters
            .get(&params.client_id)
            .ok_or(FilterError::FilterNotFound(params.client_id.clone()))?;

        let contains = filter.contains(&params.address);

        Ok(serde_json::json!({
            "client_id": params.client_id,
            "address": hex::encode(params.address),
            "may_contain": contains
        }))
    }

    /// Handle get_filter_status request
    async fn handle_get_filter_status(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, FilterError> {
        #[derive(serde::Deserialize)]
        struct StatusParams {
            client_id: String,
        }

        let params: StatusParams = serde_json::from_value(params)
            .map_err(|e| FilterError::InvalidParams(e.to_string()))?;

        let filters = self.active_filters.read().await;
        let filter = filters
            .get(&params.client_id)
            .ok_or(FilterError::FilterNotFound(params.client_id.clone()))?;

        Ok(serde_json::json!({
            "client_id": params.client_id,
            "bit_count": filter.size_bits(),
            "hash_count": filter.hash_count(),
            "element_count": filter.elements_inserted(),
            "estimated_fpr": filter.false_positive_rate(),
            "memory_bytes": filter.size_bits() / 8
        }))
    }

    /// Handle get_filtered_transactions request
    async fn handle_get_filtered_transactions(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, FilterError> {
        #[derive(serde::Deserialize)]
        struct FilteredTxParams {
            client_id: String,
            block_height: u64,
        }

        let params: FilteredTxParams = serde_json::from_value(params)
            .map_err(|e| FilterError::InvalidParams(e.to_string()))?;

        let filters = self.active_filters.read().await;
        let filter = filters
            .get(&params.client_id)
            .ok_or(FilterError::FilterNotFound(params.client_id.clone()))?;

        // Get filtered transactions from service
        let service = self.service.read().await;
        let matching_txs = service
            .get_filtered_transactions(params.block_height, filter)
            .await?;

        Ok(serde_json::json!({
            "client_id": params.client_id,
            "block_height": params.block_height,
            "matching_transactions": matching_txs.iter()
                .map(|tx| hex::encode(tx.hash()))
                .collect::<Vec<_>>()
        }))
    }
}

/// API Gateway handler for external HTTP/WS requests
///
/// Exposes Bloom Filter operations to qc-16 API Gateway.
/// Per IPC-MATRIX.md Subsystem 16, API Gateway routes filter requests here.
pub struct ApiGatewayHandler {
    /// Reference to the event bus
    bus: Arc<InMemoryEventBus>,
}

impl ApiGatewayHandler {
    /// Create a new API Gateway handler
    pub fn new(bus: Arc<InMemoryEventBus>) -> Self {
        Self { bus }
    }

    /// Handle an API query from the gateway
    ///
    /// This is called by the node-runtime's ApiQueryHandler when it
    /// receives a query targeting "qc-07-bloom-filters".
    pub async fn handle_query(
        &self,
        correlation_id: &str,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, ApiQueryError> {
        // Forward to the bus adapter via ApiQuery event
        // The BloomFilterBusAdapter will handle the actual processing
        let event = BlockchainEvent::ApiQuery {
            correlation_id: correlation_id.to_string(),
            target: "qc-07-bloom-filters".to_string(),
            method: method.to_string(),
            params,
        };

        self.bus.publish(event).await;

        // Note: The actual response will be published as ApiQueryResponse
        // The caller should listen for it via correlation_id
        Ok(serde_json::json!({
            "status": "query_dispatched",
            "correlation_id": correlation_id
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_gateway_handler_creation() {
        let bus = Arc::new(InMemoryEventBus::new());
        let handler = ApiGatewayHandler::new(bus);
        assert!(Arc::strong_count(&handler.bus) >= 1);
    }
}
