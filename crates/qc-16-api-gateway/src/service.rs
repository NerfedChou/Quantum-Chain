//! API Gateway service - main entry point per SPEC-16 Section 2.
//!
//! Provides HTTP (JSON-RPC), WebSocket, and Admin API servers.

use crate::domain::config::GatewayConfig;
use crate::domain::error::GatewayError;
use crate::domain::pending::PendingRequestStore;
use crate::ipc::handler::{IpcHandler, IpcSender};
use crate::middleware::{
    create_cors_layer, GatewayMetrics, RateLimitLayer, TimeoutLayer, TracingLayer, ValidationLayer,
};
use crate::rpc::RpcHandlers;
use crate::ws::{SubscriptionManager, WebSocketHandler};
use axum::{
    extract::{ws::WebSocketUpgrade, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tower::ServiceBuilder;
use tracing::{error, info};

/// API Gateway service state
pub struct ApiGatewayService {
    config: GatewayConfig,
    rpc_handlers: Arc<RpcHandlers>,
    subscription_manager: Arc<SubscriptionManager>,
    pending_store: Arc<PendingRequestStore>,
    metrics: Arc<GatewayMetrics>,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl ApiGatewayService {
    /// Create a new API Gateway service
    pub fn new(
        config: GatewayConfig,
        ipc_sender: Arc<dyn IpcSender>,
        data_dir: PathBuf,
    ) -> Result<Self, GatewayError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| GatewayError::Config(e.to_string()))?;

        // Create pending request store
        let pending_store = Arc::new(PendingRequestStore::new(config.timeouts.default));

        // Create IPC handler
        let ipc_handler = Arc::new(IpcHandler::new(
            Arc::clone(&pending_store),
            ipc_sender,
            config.timeouts.default,
        ));

        // Create RPC handlers
        let rpc_handlers = Arc::new(RpcHandlers::new(&config, ipc_handler, data_dir));

        // Create subscription manager
        let subscription_manager = Arc::new(SubscriptionManager::new(
            config.websocket.max_subscriptions_per_connection,
        ));

        // Create metrics
        let metrics = Arc::new(GatewayMetrics::new());

        Ok(Self {
            config,
            rpc_handlers,
            subscription_manager,
            pending_store,
            metrics,
            shutdown_tx: None,
        })
    }

    /// Start the API Gateway servers
    pub async fn start(&mut self) -> Result<(), GatewayError> {
        info!("Starting API Gateway...");

        // Create shutdown channel
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        // Start cleanup tasks
        self.start_cleanup_tasks();

        // Build routers
        let http_router = self.build_http_router();
        let ws_router = self.build_ws_router();
        let admin_router = self.build_admin_router();

        // Start HTTP server
        let http_addr = self.config.http_addr();
        let http_handle = if self.config.http.enabled {
            info!(addr = %http_addr, "Starting HTTP server");
            let router = http_router;
            Some(tokio::spawn(async move {
                let listener = tokio::net::TcpListener::bind(http_addr).await?;
                axum::serve(listener, router).await
            }))
        } else {
            None
        };

        // Start WebSocket server
        let ws_addr = self.config.ws_addr();
        let _ws_handle = if self.config.websocket.enabled {
            info!(addr = %ws_addr, "Starting WebSocket server");
            let router = ws_router;
            Some(tokio::spawn(async move {
                let listener = tokio::net::TcpListener::bind(ws_addr).await?;
                axum::serve(listener, router).await
            }))
        } else {
            None
        };

        // Start Admin server
        let admin_addr = self.config.admin_addr();
        let _admin_handle = if self.config.admin.enabled {
            info!(addr = %admin_addr, "Starting Admin server");
            let router = admin_router;
            Some(tokio::spawn(async move {
                let listener = tokio::net::TcpListener::bind(admin_addr).await?;
                axum::serve(listener, router).await
            }))
        } else {
            None
        };

        info!("API Gateway started successfully");

        // Wait for shutdown signal or server error
        tokio::select! {
            _ = &mut shutdown_rx => {
                info!("Received shutdown signal");
            }
            result = async {
                if let Some(h) = http_handle {
                    h.await
                } else {
                    Ok(Ok(()))
                }
            } => {
                if let Err(e) = result {
                    error!(error = %e, "HTTP server error");
                }
            }
        }

        info!("API Gateway stopped");
        Ok(())
    }

    /// Trigger graceful shutdown
    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }

    /// Get metrics
    pub fn metrics(&self) -> Arc<GatewayMetrics> {
        Arc::clone(&self.metrics)
    }

    /// Build HTTP router for JSON-RPC
    fn build_http_router(&self) -> Router {
        let state = AppState {
            rpc_handlers: Arc::clone(&self.rpc_handlers),
            metrics: Arc::clone(&self.metrics),
        };

        // Build middleware stack
        let middleware = ServiceBuilder::new()
            .layer(create_cors_layer(&self.config.cors))
            .layer(TracingLayer::new())
            .layer(TimeoutLayer::new(self.config.timeouts.clone()))
            .layer(ValidationLayer::new(self.config.limits.clone()))
            .layer(RateLimitLayer::new(self.config.rate_limit.clone()));

        Router::new()
            .route("/", post(handle_json_rpc))
            .route("/health", get(health_check))
            .layer(middleware)
            .with_state(state)
    }

    /// Build WebSocket router
    fn build_ws_router(&self) -> Router {
        let subscription_manager = Arc::clone(&self.subscription_manager);

        Router::new().route(
            "/",
            get(move |ws: WebSocketUpgrade| async move {
                ws.on_upgrade(move |socket| async move {
                    let handler = WebSocketHandler::new(subscription_manager);
                    handler.handle(socket).await;
                })
            }),
        )
    }

    /// Build Admin router
    fn build_admin_router(&self) -> Router {
        let metrics = Arc::clone(&self.metrics);
        let pending_store = Arc::clone(&self.pending_store);

        Router::new()
            .route("/health", get(health_check))
            .route(
                "/metrics",
                get(move || {
                    let metrics = Arc::clone(&metrics);
                    async move { Json(metrics.to_json()) }
                }),
            )
            .route(
                "/pending",
                get(move || {
                    let pending = Arc::clone(&pending_store);
                    async move {
                        Json(serde_json::json!({
                            "count": pending.pending_count(),
                            "stats": {
                                "registered": pending.stats().total_registered.load(std::sync::atomic::Ordering::Relaxed),
                                "completed": pending.stats().total_completed.load(std::sync::atomic::Ordering::Relaxed),
                                "timeouts": pending.stats().total_timeouts.load(std::sync::atomic::Ordering::Relaxed),
                            }
                        }))
                    }
                }),
            )
    }

    /// Start background cleanup tasks
    fn start_cleanup_tasks(&self) {
        // Pending request cleanup
        let pending_store = Arc::clone(&self.pending_store);
        tokio::spawn(async move {
            crate::domain::pending::cleanup_task(pending_store, Duration::from_secs(10)).await;
        });

        // Rate limit bucket cleanup would go here
    }
}

/// Application state shared across handlers
#[derive(Clone)]
struct AppState {
    rpc_handlers: Arc<RpcHandlers>,
    metrics: Arc<GatewayMetrics>,
}

/// Handle JSON-RPC request
async fn handle_json_rpc(State(state): State<AppState>, body: String) -> impl IntoResponse {
    // Parse request
    let request: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32700,
                        "message": format!("Parse error: {}", e)
                    },
                    "id": null
                })),
            );
        }
    };

    // Handle batch or single request
    let response = if request.is_array() {
        // Batch request
        let requests = request.as_array().unwrap();
        let mut responses = Vec::with_capacity(requests.len());

        for req in requests {
            let resp = process_single_request(&state, req).await;
            responses.push(resp);
        }

        serde_json::Value::Array(responses)
    } else {
        // Single request
        process_single_request(&state, &request).await
    };

    (StatusCode::OK, Json(response))
}

/// Process a single JSON-RPC request
async fn process_single_request(
    state: &AppState,
    request: &serde_json::Value,
) -> serde_json::Value {
    let id = request.get("id").cloned();

    // Validate request ID per JSON-RPC 2.0 spec
    // Null ID means notification (no response) - we reject this for security
    if let Some(ref id_val) = id {
        if id_val.is_null() {
            return serde_json::json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32600,
                    "message": "Invalid Request: null id (notifications not supported)"
                },
                "id": null
            });
        }

        // Validate string IDs are not too long (DoS protection)
        if let Some(s) = id_val.as_str() {
            if s.is_empty() {
                return serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32600,
                        "message": "Invalid Request: empty string id"
                    },
                    "id": null
                });
            }
            if s.len() > 256 {
                return serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32600,
                        "message": "Invalid Request: id string too long (max 256 chars)"
                    },
                    "id": null
                });
            }
        }

        // Reject non-standard ID types (must be string, number, or null)
        if !id_val.is_string() && !id_val.is_number() {
            return serde_json::json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32600,
                    "message": "Invalid Request: id must be string or number"
                },
                "id": null
            });
        }
    }

    let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let _params = request.get("params");

    // Route to appropriate handler
    // This is a simplified dispatcher - production would use method registry
    let result: Result<serde_json::Value, crate::domain::error::ApiError> = match method {
        "eth_chainId" => state
            .rpc_handlers
            .eth
            .chain_id()
            .await
            .map(|v| serde_json::to_value(v).unwrap()),
        "eth_accounts" => state
            .rpc_handlers
            .eth
            .accounts()
            .await
            .map(|v| serde_json::to_value(v).unwrap()),
        "web3_clientVersion" => state
            .rpc_handlers
            .web3
            .client_version()
            .await
            .map(|v| serde_json::json!(v)),
        "net_version" => state
            .rpc_handlers
            .net
            .version()
            .await
            .map(|v| serde_json::json!(v)),
        _ => {
            // Method not found or requires IPC (which needs full setup)
            Err(crate::domain::error::ApiError::method_not_found(method))
        }
    };

    match result {
        Ok(value) => {
            state.metrics.record_request(true, false, 0);
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": value
            })
        }
        Err(e) => {
            state.metrics.record_request(false, false, 0);
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": e.code,
                    "message": e.message
                }
            })
        }
    }
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "api-gateway",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let config = GatewayConfig::default();
        assert!(config.validate().is_ok());
    }
}
