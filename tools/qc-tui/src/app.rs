//! Application state management.

use crate::rpc::{BlockInfo, NodeInfo, PeerInfo, RpcClient, SyncStatus, TxInfo, TxPoolStatus};
use crate::ws::{BlockHeader, WsEvent};
use anyhow::Result;
use futures_util::future::join_all;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Maximum number of events to keep in history.
const MAX_EVENTS: usize = 100;

/// Maximum number of recent blocks to display.
const MAX_RECENT_BLOCKS: usize = 10;

/// Maximum blocks to fetch for blocks view.
const MAX_BLOCKS_VIEW: usize = 50;

/// Active tab/view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Mempool,
    Blocks,
    Peers,
}

impl Tab {
    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        match self {
            Tab::Dashboard => "Dashboard",
            Tab::Mempool => "Mempool",
            Tab::Blocks => "Blocks",
            Tab::Peers => "Peers",
        }
    }
}

/// A live event from WebSocket subscriptions.
#[derive(Debug, Clone)]
pub struct LiveEvent {
    /// Timestamp when event was received.
    pub timestamp: Instant,
    /// Event type string.
    pub event_type: String,
    /// Event description.
    pub description: String,
}

/// Application state holding all dashboard data.
pub struct App {
    /// RPC client for API communication.
    rpc: RpcClient,

    /// Current active tab.
    pub active_tab: Tab,

    /// Current block height.
    pub block_height: u64,

    /// Chain ID.
    pub chain_id: u64,

    /// Current gas price in wei.
    pub gas_price: u64,

    /// Sync status.
    pub sync_status: SyncStatus,

    /// Connected peer count.
    pub peer_count: u64,

    /// Whether node is listening.
    pub listening: bool,

    /// Network version/ID.
    pub network_version: String,

    /// Whether the app should quit.
    pub should_quit: bool,

    /// Last data refresh time.
    pub last_refresh: Instant,

    /// HTTP connection status.
    pub connected: bool,

    /// WebSocket connection status.
    pub ws_connected: bool,

    /// Last error message.
    pub last_error: Option<String>,

    /// Application start time.
    pub start_time: Instant,

    /// Recent blocks from WebSocket.
    pub recent_blocks: VecDeque<BlockHeader>,

    /// Live events log.
    pub live_events: VecDeque<LiveEvent>,

    /// Pending transaction count (from WS).
    pub pending_tx_count: u64,

    // === Mempool View Data ===
    /// Mempool status.
    pub txpool_status: TxPoolStatus,
    /// Mempool content (pending transactions).
    pub txpool_pending: Vec<TxInfo>,
    /// Mempool content (queued transactions).
    pub txpool_queued: Vec<TxInfo>,
    /// Selected transaction index in mempool view.
    pub mempool_selected: usize,

    // === Blocks View Data ===
    /// Blocks list for blocks view.
    pub blocks_list: Vec<BlockInfo>,
    /// Selected block index in blocks view.
    pub blocks_selected: usize,

    // === Peers View Data ===
    /// Connected peers list.
    pub peers_list: Vec<PeerInfo>,
    /// Node info.
    pub node_info: Option<NodeInfo>,
    /// Selected peer index.
    pub peers_selected: usize,
}

impl App {
    /// Create a new application instance.
    pub fn new(rpc_url: String) -> Self {
        Self {
            rpc: RpcClient::new(rpc_url),
            active_tab: Tab::Dashboard,
            block_height: 0,
            chain_id: 0,
            gas_price: 0,
            sync_status: SyncStatus::Synced,
            peer_count: 0,
            listening: false,
            network_version: String::new(),
            should_quit: false,
            last_refresh: Instant::now(),
            connected: false,
            ws_connected: false,
            last_error: None,
            start_time: Instant::now(),
            recent_blocks: VecDeque::with_capacity(MAX_RECENT_BLOCKS),
            live_events: VecDeque::with_capacity(MAX_EVENTS),
            pending_tx_count: 0,
            txpool_status: TxPoolStatus::default(),
            txpool_pending: Vec::new(),
            txpool_queued: Vec::new(),
            mempool_selected: 0,
            blocks_list: Vec::new(),
            blocks_selected: 0,
            peers_list: Vec::new(),
            node_info: None,
            peers_selected: 0,
        }
    }

    /// Handle a WebSocket event.
    pub fn handle_ws_event(&mut self, event: WsEvent) {
        match event {
            WsEvent::Connected => {
                self.ws_connected = true;
                self.add_event("ws", "WebSocket connected");
            }
            WsEvent::Disconnected => {
                self.ws_connected = false;
                self.add_event("ws", "WebSocket disconnected");
            }
            WsEvent::NewHead(header) => {
                // Update block height from WebSocket (more immediate than polling)
                let block_num = header.block_number();
                if block_num > self.block_height {
                    self.block_height = block_num;
                }

                // Add to event log
                self.add_event(
                    "newHeads",
                    &format!(
                        "Block #{} ({} txs)",
                        block_num,
                        header.tx_count()
                    ),
                );

                // Add to recent blocks
                self.recent_blocks.push_front(header);
                if self.recent_blocks.len() > MAX_RECENT_BLOCKS {
                    self.recent_blocks.pop_back();
                }
            }
            WsEvent::PendingTransaction(tx_hash) => {
                self.pending_tx_count += 1;
                self.add_event("pendingTx", &format!("{}...", &tx_hash[..12.min(tx_hash.len())]));
            }
            WsEvent::Error(msg) => {
                self.add_event("error", &msg);
            }
        }
    }

    /// Add an event to the live events log.
    fn add_event(&mut self, event_type: &str, description: &str) {
        self.live_events.push_front(LiveEvent {
            timestamp: Instant::now(),
            event_type: event_type.to_string(),
            description: description.to_string(),
        });

        if self.live_events.len() > MAX_EVENTS {
            self.live_events.pop_back();
        }
    }

    /// Refresh all dashboard data from the node.
    pub async fn refresh(&mut self) -> Result<()> {
        // Clear previous error
        self.last_error = None;

        // Fetch all data concurrently
        let (block_result, chain_result, gas_result, sync_result, peer_result, listen_result, net_result) = tokio::join!(
            self.rpc.get_block_number(),
            self.rpc.get_chain_id(),
            self.rpc.get_gas_price(),
            self.rpc.get_syncing(),
            self.rpc.get_peer_count(),
            self.rpc.get_listening(),
            self.rpc.get_network_version(),
        );

        // Update block height
        match block_result {
            Ok(height) => {
                self.block_height = height;
            }
            Err(_) => {
                // eth_blockNumber may not be implemented yet
            }
        }

        // Update chain ID - use this for connection status
        match chain_result {
            Ok(chain_id) => {
                self.chain_id = chain_id;
                if !self.connected {
                    self.add_event("rpc", "HTTP RPC connected");
                }
                self.connected = true;
            }
            Err(e) => {
                if self.connected {
                    self.add_event("rpc", &format!("RPC error: {}", e));
                }
                self.connected = false;
                self.last_error = Some(format!("Connection: {}", e));
            }
        }

        // Update gas price
        if let Ok(gas_price) = gas_result {
            self.gas_price = gas_price;
        }

        // Update sync status
        if let Ok(sync_status) = sync_result {
            self.sync_status = sync_status;
        }

        // Update peer count
        if let Ok(peer_count) = peer_result {
            self.peer_count = peer_count;
        }

        // Update listening status
        if let Ok(listening) = listen_result {
            self.listening = listening;
        }

        // Update network version
        if let Ok(network_version) = net_result {
            self.network_version = network_version;
        }

        self.last_refresh = Instant::now();
        Ok(())
    }

    /// Refresh mempool data.
    pub async fn refresh_mempool(&mut self) -> Result<()> {
        // Fetch txpool status and content
        let (status_result, content_result) = tokio::join!(
            self.rpc.get_txpool_status(),
            self.rpc.get_txpool_content(),
        );

        if let Ok(status) = status_result {
            self.txpool_status = status;
        }

        if let Ok(content) = content_result {
            self.txpool_pending = content.pending;
            self.txpool_queued = content.queued;
        }

        Ok(())
    }

    /// Refresh blocks list.
    pub async fn refresh_blocks(&mut self) -> Result<()> {
        if self.block_height == 0 {
            return Ok(());
        }

        let mut blocks = Vec::new();
        let start_height = self.block_height;
        let count = MAX_BLOCKS_VIEW.min(start_height as usize);

        // Fetch blocks in parallel (batches of 10)
        for batch_start in (0..count).step_by(10) {
            let batch_end = (batch_start + 10).min(count);
            let mut futures = Vec::new();

            for i in batch_start..batch_end {
                let height = start_height - i as u64;
                futures.push(self.rpc.get_block_by_number(height, false));
            }

            let results = join_all(futures).await;
            for result in results {
                if let Ok(Some(block)) = result {
                    blocks.push(block);
                }
            }
        }

        self.blocks_list = blocks;
        Ok(())
    }

    /// Refresh peers list.
    pub async fn refresh_peers(&mut self) -> Result<()> {
        // Fetch peers and node info concurrently
        let (peers_result, node_result) = tokio::join!(
            self.rpc.get_admin_peers(),
            self.rpc.get_node_info(),
        );

        if let Ok(peers) = peers_result {
            self.peers_list = peers;
        }

        if let Ok(info) = node_result {
            self.node_info = Some(info);
        }

        Ok(())
    }

    /// Format gas price in gwei.
    pub fn gas_price_gwei(&self) -> f64 {
        self.gas_price as f64 / 1_000_000_000.0
    }

    /// Get uptime duration.
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Format uptime as human-readable string.
    pub fn uptime_str(&self) -> String {
        let duration = self.uptime();
        let secs = duration.as_secs();

        let days = secs / 86400;
        let hours = (secs % 86400) / 3600;
        let mins = (secs % 3600) / 60;

        if days > 0 {
            format!("{}d {}h {}m", days, hours, mins)
        } else if hours > 0 {
            format!("{}h {}m", hours, mins)
        } else {
            format!("{}m", mins)
        }
    }

    /// Get node status string.
    pub fn status_str(&self) -> &'static str {
        if !self.connected {
            "DISCONNECTED"
        } else if self.sync_status.is_synced() {
            "RUNNING"
        } else {
            "SYNCING"
        }
    }

    /// Handle key press events.
    pub fn on_key(&mut self, key: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;

        match key {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.should_quit = true;
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                // Force refresh handled in main loop
            }
            // Tab navigation
            KeyCode::Char('d') | KeyCode::Char('D') => {
                self.active_tab = Tab::Dashboard;
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                self.active_tab = Tab::Mempool;
            }
            KeyCode::Char('b') | KeyCode::Char('B') => {
                self.active_tab = Tab::Blocks;
            }
            KeyCode::Char('p') | KeyCode::Char('P') => {
                self.active_tab = Tab::Peers;
            }
            // List navigation
            KeyCode::Up | KeyCode::Char('k') => {
                match self.active_tab {
                    Tab::Mempool => {
                        if self.mempool_selected > 0 {
                            self.mempool_selected -= 1;
                        }
                    }
                    Tab::Blocks => {
                        if self.blocks_selected > 0 {
                            self.blocks_selected -= 1;
                        }
                    }
                    Tab::Peers => {
                        if self.peers_selected > 0 {
                            self.peers_selected -= 1;
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                match self.active_tab {
                    Tab::Mempool => {
                        let max = self.txpool_pending.len().saturating_sub(1);
                        if self.mempool_selected < max {
                            self.mempool_selected += 1;
                        }
                    }
                    Tab::Blocks => {
                        let max = self.blocks_list.len().saturating_sub(1);
                        if self.blocks_selected < max {
                            self.blocks_selected += 1;
                        }
                    }
                    Tab::Peers => {
                        let max = self.peers_list.len().saturating_sub(1);
                        if self.peers_selected < max {
                            self.peers_selected += 1;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// Check if current tab needs data refresh.
    pub fn needs_tab_refresh(&self) -> bool {
        match self.active_tab {
            Tab::Mempool => self.txpool_pending.is_empty() && self.txpool_queued.is_empty(),
            Tab::Blocks => self.blocks_list.is_empty(),
            Tab::Peers => self.peers_list.is_empty(),
            Tab::Dashboard => false,
        }
    }
}
