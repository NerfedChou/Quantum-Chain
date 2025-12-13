//! WebSocket handler for real-time subscriptions per SPEC-16 Section 5.
//!
//! Security features:
//! - Message size limits (default 1MB)
//! - Connection-level subscription limits
//! - Rate limiting per connection

use crate::domain::correlation::CorrelationId;
use crate::SubscriptionType;
use crate::domain::types::Filter;
use crate::ws::subscriptions::{SubscriptionManager, SubscriptionNotification};
use axum::extract::ws::{Message, WebSocket};
use futures::StreamExt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Default maximum message size (1MB)
pub const DEFAULT_MAX_MESSAGE_SIZE: usize = 1024 * 1024;

/// Default rate limit (100 messages per second)
pub const DEFAULT_RATE_LIMIT: u32 = 100;

/// WebSocket configuration
#[derive(Debug, Clone)]
pub struct WebSocketConfig {
    /// Maximum message size in bytes
    pub max_message_size: usize,
    /// Rate limit (messages per second per connection)
    pub rate_limit: u32,
    /// Ping interval
    pub ping_interval: Duration,
    /// Idle timeout (disconnect if no activity)
    pub idle_timeout: Duration,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            max_message_size: DEFAULT_MAX_MESSAGE_SIZE,
            rate_limit: DEFAULT_RATE_LIMIT,
            ping_interval: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(300),
        }
    }
}

/// WebSocket connection handler
pub struct WebSocketHandler {
    subscription_manager: Arc<SubscriptionManager>,
    connection_id: CorrelationId,
    config: WebSocketConfig,
    /// Message counter for rate limiting
    message_count: u32,
    /// Rate limit window start
    rate_limit_window: Instant,
}

impl WebSocketHandler {
    pub fn new(subscription_manager: Arc<SubscriptionManager>) -> Self {
        Self::with_config(subscription_manager, WebSocketConfig::default())
    }

    pub fn with_config(
        subscription_manager: Arc<SubscriptionManager>,
        config: WebSocketConfig,
    ) -> Self {
        Self {
            subscription_manager,
            connection_id: CorrelationId::new(),
            config,
            message_count: 0,
            rate_limit_window: Instant::now(),
        }
    }

    /// Check rate limit, returns true if request is allowed
    fn check_rate_limit(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.rate_limit_window);

        // Reset window every second
        if elapsed >= Duration::from_secs(1) {
            self.rate_limit_window = now;
            self.message_count = 0;
        }

        self.message_count += 1;
        self.message_count <= self.config.rate_limit
    }

    /// Check message size, returns error response if too large
    fn check_message_size(&self, size: usize) -> Option<String> {
        if size > self.config.max_message_size {
            warn!(
                connection_id = %self.connection_id,
                size = size,
                max = self.config.max_message_size,
                "Message exceeds size limit"
            );
            Some(json_rpc_error(
                None,
                -32600,
                &format!(
                    "Message too large: {} bytes (max: {})",
                    size, self.config.max_message_size
                ),
            ))
        } else {
            None
        }
    }

    /// Handle a WebSocket connection
    pub async fn handle(mut self, mut socket: WebSocket) {
        info!(
            connection_id = %self.connection_id,
            "New WebSocket connection"
        );

        // Channel for sending notifications to client
        let (_notif_tx, _notif_rx) = mpsc::channel::<SubscriptionNotification>(256);

        // Spawn notification sender task
        let _send_handle = {
            let _conn_id = self.connection_id;
            tokio::spawn(async move {
                // This would be split from socket - simplified for now
            })
        };

        let mut last_activity = Instant::now();

        // Handle incoming messages
        while let Some(result) = socket.next().await {
            // Check idle timeout
            if last_activity.elapsed() > self.config.idle_timeout {
                info!(
                    connection_id = %self.connection_id,
                    "Closing idle WebSocket connection"
                );
                break;
            }

            last_activity = Instant::now();

            match result {
                Ok(Message::Text(text)) => {
                    // Check message size
                    if let Some(error_response) = self.check_message_size(text.len()) {
                        if let Err(e) = socket.send(Message::Text(error_response)).await {
                            error!(error = %e, "Failed to send error response");
                            break;
                        }
                        continue;
                    }

                    // Check rate limit
                    if !self.check_rate_limit() {
                        let error = json_rpc_error(None, -32005, "Rate limit exceeded");
                        if let Err(e) = socket.send(Message::Text(error)).await {
                            error!(error = %e, "Failed to send rate limit error");
                            break;
                        }
                        continue;
                    }

                    let response = self.handle_message(&text).await;
                    if let Err(e) = socket.send(Message::Text(response)).await {
                        error!(error = %e, "Failed to send WebSocket response");
                        break;
                    }
                }
                Ok(Message::Binary(data)) => {
                    // Check message size
                    if let Some(error_response) = self.check_message_size(data.len()) {
                        if let Err(e) = socket.send(Message::Text(error_response)).await {
                            error!(error = %e, "Failed to send error response");
                            break;
                        }
                        continue;
                    }

                    // Check rate limit
                    if !self.check_rate_limit() {
                        let error = json_rpc_error(None, -32005, "Rate limit exceeded");
                        if let Err(e) = socket.send(Message::Text(error)).await {
                            error!(error = %e, "Failed to send rate limit error");
                            break;
                        }
                        continue;
                    }

                    // Try to parse as JSON
                    if let Ok(text) = String::from_utf8(data) {
                        let response = self.handle_message(&text).await;
                        if let Err(e) = socket.send(Message::Text(response)).await {
                            error!(error = %e, "Failed to send WebSocket response");
                            break;
                        }
                    }
                }
                Ok(Message::Ping(data)) => {
                    if let Err(e) = socket.send(Message::Pong(data)).await {
                        error!(error = %e, "Failed to send pong");
                        break;
                    }
                }
                Ok(Message::Pong(_)) => {
                    // Ignore pongs
                }
                Ok(Message::Close(_)) => {
                    debug!(connection_id = %self.connection_id, "WebSocket close received");
                    break;
                }
                Err(e) => {
                    warn!(error = %e, "WebSocket error");
                    break;
                }
            }
        }

        // Cleanup subscriptions on disconnect
        self.subscription_manager
            .remove_connection(&self.connection_id);

        info!(
            connection_id = %self.connection_id,
            "WebSocket connection closed"
        );
    }

    /// Handle a single JSON-RPC message
    async fn handle_message(&self, text: &str) -> String {
        // Parse JSON-RPC request
        let request: serde_json::Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(e) => {
                return json_rpc_error(None, -32700, &format!("Parse error: {}", e));
            }
        };

        let id = request.get("id").cloned();
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = request.get("params");

        match method {
            "eth_subscribe" => self.handle_subscribe(id, params).await,
            "eth_unsubscribe" => self.handle_unsubscribe(id, params).await,
            _ => {
                // For other methods, they should go through HTTP
                // But we can handle some simple ones
                json_rpc_error(id, -32601, &format!("Method not found: {}", method))
            }
        }
    }

    /// Handle eth_subscribe
    async fn handle_subscribe(
        &self,
        id: Option<serde_json::Value>,
        params: Option<&serde_json::Value>,
    ) -> String {
        let params = match params {
            Some(serde_json::Value::Array(arr)) => arr,
            _ => {
                return json_rpc_error(id, -32602, "Invalid params: expected array");
            }
        };

        if params.is_empty() {
            return json_rpc_error(id, -32602, "Invalid params: missing subscription type");
        }

        let sub_type_str = match params[0].as_str() {
            Some(s) => s,
            None => {
                return json_rpc_error(
                    id,
                    -32602,
                    "Invalid params: subscription type must be string",
                );
            }
        };

        let sub_type = match SubscriptionType::from_str(sub_type_str) {
            Some(t) => t,
            None => {
                return json_rpc_error(
                    id,
                    -32602,
                    &format!("Invalid subscription type: {}", sub_type_str),
                );
            }
        };

        // Parse filter for logs subscription
        let filter = if sub_type == SubscriptionType::Logs {
            if params.len() > 1 {
                match serde_json::from_value::<Filter>(params[1].clone()) {
                    Ok(f) => Some(f),
                    Err(e) => {
                        return json_rpc_error(id, -32602, &format!("Invalid filter: {}", e));
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        // Create subscription
        match self
            .subscription_manager
            .subscribe(self.connection_id, sub_type, filter)
        {
            Ok(sub_id) => json_rpc_result(id, serde_json::json!(sub_id)),
            Err(e) => json_rpc_error(id, -32000, &e.to_string()),
        }
    }

    /// Handle eth_unsubscribe
    async fn handle_unsubscribe(
        &self,
        id: Option<serde_json::Value>,
        params: Option<&serde_json::Value>,
    ) -> String {
        let params = match params {
            Some(serde_json::Value::Array(arr)) => arr,
            _ => {
                return json_rpc_error(id, -32602, "Invalid params: expected array");
            }
        };

        if params.is_empty() {
            return json_rpc_error(id, -32602, "Invalid params: missing subscription ID");
        }

        let sub_id = match params[0].as_str() {
            Some(s) => s,
            None => {
                return json_rpc_error(
                    id,
                    -32602,
                    "Invalid params: subscription ID must be string",
                );
            }
        };

        let result = self.subscription_manager.unsubscribe(sub_id);
        json_rpc_result(id, serde_json::json!(result))
    }
}

/// Create JSON-RPC success response
fn json_rpc_result(id: Option<serde_json::Value>, result: serde_json::Value) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
    .to_string()
}

/// Create JSON-RPC error response
fn json_rpc_error(id: Option<serde_json::Value>, code: i32, message: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_rpc_result() {
        let result = json_rpc_result(Some(serde_json::json!(1)), serde_json::json!("0x1"));
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["result"], "0x1");
    }

    #[test]
    fn test_json_rpc_error() {
        let result = json_rpc_error(Some(serde_json::json!(1)), -32601, "Method not found");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"]["code"], -32601);
    }
}
