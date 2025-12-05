//! WebSocket client for real-time subscriptions to qc-16 API Gateway.

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Events received from WebSocket subscriptions.
#[derive(Debug, Clone)]
pub enum WsEvent {
    /// New block header received.
    NewHead(BlockHeader),
    /// New pending transaction hash.
    PendingTransaction(String),
    /// Connection established.
    Connected,
    /// Connection lost.
    Disconnected,
    /// Error occurred.
    Error(String),
}

/// Block header from newHeads subscription.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct BlockHeader {
    pub number: String,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: String,
    #[serde(default)]
    pub transactions: Vec<String>,
    pub gas_used: Option<String>,
    pub gas_limit: Option<String>,
}

impl BlockHeader {
    /// Get block number as u64.
    pub fn block_number(&self) -> u64 {
        parse_hex(&self.number).unwrap_or(0)
    }

    /// Get transaction count.
    pub fn tx_count(&self) -> usize {
        self.transactions.len()
    }

    /// Get short hash (first 10 chars).
    pub fn short_hash(&self) -> String {
        if self.hash.len() > 10 {
            format!("{}...", &self.hash[..10])
        } else {
            self.hash.clone()
        }
    }
}

/// JSON-RPC request for WebSocket.
#[derive(Debug, Serialize)]
struct WsRequest<T: Serialize> {
    jsonrpc: &'static str,
    method: &'static str,
    params: T,
    id: u64,
}

/// JSON-RPC subscription response.
#[derive(Debug, Deserialize)]
struct SubscriptionResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: Option<u64>,
    result: Option<String>,
    error: Option<RpcError>,
    method: Option<String>,
    params: Option<SubscriptionParams>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    #[allow(dead_code)]
    code: i64,
    message: String,
}

#[derive(Debug, Deserialize)]
struct SubscriptionParams {
    subscription: String,
    result: serde_json::Value,
}

/// Tracks WebSocket subscription state.
struct SubscriptionState {
    new_heads_id: u64,
    pending_tx_id: u64,
    new_heads_sub: Option<String>,
    pending_tx_sub: Option<String>,
}

impl SubscriptionState {
    fn new(new_heads_id: u64, pending_tx_id: u64) -> Self {
        Self {
            new_heads_id,
            pending_tx_id,
            new_heads_sub: None,
            pending_tx_sub: None,
        }
    }
    
    fn register_subscription(&mut self, id: Option<u64>, result: String) {
        if id == Some(self.new_heads_id) {
            self.new_heads_sub = Some(result);
        } else if id == Some(self.pending_tx_id) {
            self.pending_tx_sub = Some(result);
        }
    }
    
    async fn handle_notification(
        &self,
        params: SubscriptionParams,
        event_tx: &mpsc::Sender<WsEvent>,
    ) {
        if Some(&params.subscription) == self.new_heads_sub.as_ref() {
            if let Ok(header) = serde_json::from_value::<BlockHeader>(params.result) {
                let _ = event_tx.send(WsEvent::NewHead(header)).await;
            }
        } else if Some(&params.subscription) == self.pending_tx_sub.as_ref() {
            if let Some(tx_hash) = params.result.as_str() {
                let _ = event_tx.send(WsEvent::PendingTransaction(tx_hash.to_string())).await;
            }
        }
    }
}

/// Maximum WebSocket reconnection attempts before giving up.
const MAX_RECONNECT_ATTEMPTS: u32 = 10;

/// Base delay between reconnection attempts (exponential backoff).
const RECONNECT_BASE_DELAY_SECS: u64 = 2;

/// Maximum delay between reconnection attempts.
const MAX_RECONNECT_DELAY_SECS: u64 = 60;

/// WebSocket client for subscriptions.
pub struct WsClient {
    ws_url: String,
    #[allow(dead_code)]
    request_id: AtomicU64,
    event_tx: mpsc::Sender<WsEvent>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl WsClient {
    /// Create a new WebSocket client.
    pub fn new(ws_url: String, event_tx: mpsc::Sender<WsEvent>) -> Self {
        Self {
            ws_url,
            request_id: AtomicU64::new(1),
            event_tx,
            shutdown_tx: None,
        }
    }

    /// Start the WebSocket connection and subscriptions.
    pub async fn start(&mut self) -> Result<()> {
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        let ws_url = self.ws_url.clone();
        let event_tx = self.event_tx.clone();
        let request_id = Arc::new(AtomicU64::new(1));

        tokio::spawn(Self::connection_loop(ws_url, event_tx, request_id, shutdown_rx));

        Ok(())
    }
    
    /// Connection loop with reconnection logic.
    async fn connection_loop(
        ws_url: String,
        event_tx: mpsc::Sender<WsEvent>,
        request_id: Arc<AtomicU64>,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) {
        let mut reconnect_attempts = 0u32;
        
        loop {
            match Self::run_connection(&ws_url, event_tx.clone(), request_id.clone()).await {
                Ok(()) => break, // Clean disconnect
                Err(e) => {
                    reconnect_attempts += 1;
                    let _ = event_tx.send(WsEvent::Error(e.to_string())).await;
                    let _ = event_tx.send(WsEvent::Disconnected).await;

                    if !Self::should_retry(reconnect_attempts, &event_tx).await {
                        break;
                    }

                    let delay_secs = Self::calculate_backoff_delay(reconnect_attempts);

                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_secs(delay_secs)) => {}
                        _ = shutdown_rx.recv() => break,
                    }
                }
            }
        }
    }
    
    /// Check if we should retry connection.
    async fn should_retry(attempts: u32, event_tx: &mpsc::Sender<WsEvent>) -> bool {
        if attempts >= MAX_RECONNECT_ATTEMPTS {
            let msg = format!("WebSocket reconnection failed after {} attempts", MAX_RECONNECT_ATTEMPTS);
            let _ = event_tx.send(WsEvent::Error(msg)).await;
            return false;
        }
        true
    }
    
    /// Calculate exponential backoff delay.
    fn calculate_backoff_delay(attempts: u32) -> u64 {
        std::cmp::min(
            RECONNECT_BASE_DELAY_SECS.saturating_mul(1 << attempts.min(6)),
            MAX_RECONNECT_DELAY_SECS
        )
    }

    /// Run a single WebSocket connection.
    async fn run_connection(
        ws_url: &str,
        event_tx: mpsc::Sender<WsEvent>,
        request_id: Arc<AtomicU64>,
    ) -> Result<()> {
        // Connect to WebSocket
        let (ws_stream, _) = connect_async(ws_url)
            .await
            .context("Failed to connect to WebSocket")?;

        let _ = event_tx.send(WsEvent::Connected).await;

        let (mut write, mut read) = ws_stream.split();

        // Subscribe to newHeads
        let new_heads_id = request_id.fetch_add(1, Ordering::SeqCst);
        let new_heads_req = WsRequest {
            jsonrpc: "2.0",
            method: "eth_subscribe",
            params: ("newHeads",),
            id: new_heads_id,
        };
        let msg = Message::Text(serde_json::to_string(&new_heads_req)?.into());
        write.send(msg).await.context("Failed to send subscription")?;

        // Subscribe to newPendingTransactions
        let pending_tx_id = request_id.fetch_add(1, Ordering::SeqCst);
        let pending_tx_req = WsRequest {
            jsonrpc: "2.0",
            method: "eth_subscribe",
            params: ("newPendingTransactions",),
            id: pending_tx_id,
        };
        let msg = Message::Text(serde_json::to_string(&pending_tx_req)?.into());
        write.send(msg).await.context("Failed to send subscription")?;

        // Subscription state
        let mut state = SubscriptionState::new(new_heads_id, pending_tx_id);

        // Read messages
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    Self::handle_text_message(&text, &mut state, &event_tx).await;
                }
                Ok(Message::Close(_)) => {
                    let _ = event_tx.send(WsEvent::Disconnected).await;
                    break;
                }
                Ok(Message::Ping(data)) => {
                    let _ = write.send(Message::Pong(data)).await;
                }
                Err(e) => {
                    let _ = event_tx.send(WsEvent::Error(e.to_string())).await;
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }
    
    /// Handle a text message from WebSocket.
    async fn handle_text_message(
        text: &str,
        state: &mut SubscriptionState,
        event_tx: &mpsc::Sender<WsEvent>,
    ) {
        let Ok(response) = serde_json::from_str::<SubscriptionResponse>(text) else {
            return;
        };

        // Handle subscription confirmation
        if let Some(result) = response.result {
            state.register_subscription(response.id, result);
        }

        // Handle subscription notification
        if response.method.as_deref() == Some("eth_subscription") {
            if let Some(params) = response.params {
                state.handle_notification(params, event_tx).await;
            }
        }

        // Handle errors
        if let Some(error) = response.error {
            let _ = event_tx.send(WsEvent::Error(error.message)).await;
        }
    }

    /// Stop the WebSocket connection.
    pub async fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
    }
}

/// Parse hex string to u64.
fn parse_hex(s: &str) -> Result<u64> {
    let s = s.trim_start_matches("0x");
    u64::from_str_radix(s, 16).context("Failed to parse hex number")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_header_parsing() {
        let json = r#"{
            "number": "0x12d687",
            "hash": "0xabc123def456789",
            "parentHash": "0x000000",
            "timestamp": "0x60000000",
            "transactions": ["0x1", "0x2", "0x3"],
            "gasUsed": "0x5208",
            "gasLimit": "0x1c9c380"
        }"#;

        let header: BlockHeader = serde_json::from_str(json).unwrap();
        assert_eq!(header.block_number(), 1234567);
        assert_eq!(header.tx_count(), 3);
        assert_eq!(header.short_hash(), "0xabc123de...");
    }
}
