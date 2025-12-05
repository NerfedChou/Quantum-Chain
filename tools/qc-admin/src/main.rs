//! QC-Admin: Quantum-Chain Admin Control Panel
//!
//! A TUI-based admin panel for monitoring and debugging Quantum-Chain subsystems.

use std::io;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::Mutex;

use qc_admin::api::{AdminApiClient, ApiSubsystemStatus};
use qc_admin::domain::{App, NodeStatus, PeerDisplayInfo, SubsystemId, SubsystemInfo, SubsystemStatus, SystemHealth};
use qc_admin::ui;

/// QC-Admin: Quantum-Chain Admin Control Panel
#[derive(Parser, Debug)]
#[command(name = "qc-admin")]
#[command(about = "TUI admin panel for monitoring Quantum-Chain subsystems")]
struct Args {
    /// JSON-RPC API endpoint URL (where debug_* methods are available)
    #[arg(short, long, default_value = "http://127.0.0.1:8545")]
    endpoint: String,

    /// Refresh interval in seconds
    #[arg(short, long, default_value = "2")]
    refresh: u64,

    /// Run in demo mode with fake data (no API connection required)
    #[arg(long)]
    demo: bool,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let app = Arc::new(Mutex::new(App::new()));

    // Create API client (if not in demo mode)
    let api_client = if args.demo {
        None
    } else {
        match AdminApiClient::new(&args.endpoint) {
            Ok(client) => Some(Arc::new(client)),
            Err(e) => {
                eprintln!("Warning: Failed to create API client: {}", e);
                None
            }
        }
    };

    // Set initial data
    if args.demo {
        set_demo_data(&mut *app.lock().await);
    } else {
        // Fetch initial data
        fetch_and_update(&api_client, &app).await;
    }

    // Spawn background refresh task
    let refresh_app = app.clone();
    let refresh_client = api_client.clone();
    let refresh_interval = Duration::from_secs(args.refresh);
    let demo_mode = args.demo;

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(refresh_interval);
        loop {
            interval.tick().await;
            if demo_mode {
                // In demo mode, just update timestamp
                let mut app = refresh_app.lock().await;
                app.last_refresh = Some(chrono::Utc::now());
            } else {
                fetch_and_update(&refresh_client, &refresh_app).await;
            }
        }
    });

    // Main loop
    let result = run_app(&mut terminal, app.clone()).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = result {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: Arc<Mutex<App>>,
) -> io::Result<()> {
    loop {
        // Draw UI
        {
            let app_guard = app.lock().await;
            terminal.draw(|frame| {
                ui::render(frame, &app_guard);
            })?;
        }

        // Handle input with timeout for potential refresh
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Only handle key press events (not release)
                if key.kind == KeyEventKind::Press {
                    let mut app_guard = app.lock().await;
                    match key.code {
                        KeyCode::Char(c) => app_guard.handle_key(c),
                        KeyCode::Up => app_guard.select_prev(),
                        KeyCode::Down => app_guard.select_next(),
                        KeyCode::Esc => app_guard.handle_key('q'),
                        _ => {}
                    }
                }
            }
        }

        // Check if we should quit
        if app.lock().await.should_quit() {
            return Ok(());
        }
    }
}

/// Fetch data from API and update app state.
async fn fetch_and_update(
    api_client: &Option<Arc<AdminApiClient>>,
    app: &Arc<Mutex<App>>,
) {
    let Some(client) = api_client else {
        return;
    };

    let mut app_guard = app.lock().await;

    // Fetch system metrics
    match client.get_system_metrics().await {
        Ok(metrics) => {
            app_guard.system_health = SystemHealth {
                cpu_percent: metrics.cpu_percent,
                memory_percent: metrics.memory_percent,
                node_status: NodeStatus::Running,
            };
            app_guard.error_message = None;
        }
        Err(e) => {
            app_guard.error_message = Some(format!("System metrics: {}", e));
        }
    }

    // Fetch subsystem health
    match client.get_subsystem_health().await {
        Ok(response) => {
            for health in response.subsystems {
                // Parse subsystem ID from string like "qc-01"
                let id_num: u8 = health
                    .id
                    .trim_start_matches("qc-")
                    .parse()
                    .unwrap_or(0);

                let subsystem_id = match id_num {
                    1 => Some(SubsystemId::PeerDiscovery),
                    2 => Some(SubsystemId::BlockStorage),
                    3 => Some(SubsystemId::TransactionIndexing),
                    4 => Some(SubsystemId::StateManagement),
                    5 => Some(SubsystemId::BlockPropagation),
                    6 => Some(SubsystemId::Mempool),
                    7 => Some(SubsystemId::BloomFilters),
                    8 => Some(SubsystemId::Consensus),
                    9 => Some(SubsystemId::Finality),
                    10 => Some(SubsystemId::SignatureVerification),
                    11 => Some(SubsystemId::SmartContracts),
                    12 => Some(SubsystemId::TransactionOrdering),
                    13 => Some(SubsystemId::LightClientSync),
                    14 => Some(SubsystemId::Sharding),
                    15 => Some(SubsystemId::CrossChain),
                    16 => Some(SubsystemId::ApiGateway),
                    _ => None,
                };

                if let Some(id) = subsystem_id {
                    let status = match health.status {
                        ApiSubsystemStatus::Running => SubsystemStatus::Running,
                        ApiSubsystemStatus::Stopped => SubsystemStatus::Stopped,
                        ApiSubsystemStatus::Degraded => SubsystemStatus::Warning,
                        ApiSubsystemStatus::Error => SubsystemStatus::Stopped,
                        ApiSubsystemStatus::Unknown => SubsystemStatus::Stopped,
                        ApiSubsystemStatus::NotImplemented => SubsystemStatus::NotImplemented,
                    };

                    app_guard.update_subsystem(SubsystemInfo {
                        id,
                        status,
                        warning_message: None,
                        metrics: health.specific_metrics,
                    });
                }
            }
            app_guard.last_refresh = Some(chrono::Utc::now());
            app_guard.error_message = None;
            app_guard.system_health.node_status = NodeStatus::Running;
        }
        Err(e) => {
            // If we can't reach the API, mark as stopped
            app_guard.system_health.node_status = NodeStatus::Stopped;
            app_guard.error_message = Some(format!("API: {}", e));
        }
    }

    // Fetch peer list for qc-01 panel
    match client.get_peers().await {
        Ok(peers) => {
            app_guard.peers = peers
                .into_iter()
                .map(|p| {
                    // Parse IP:port from remote_address or enode
                    let (ip, port) = parse_peer_address(&p.remote_address, &p.enode);
                    // Truncate node ID for display
                    let short_id = if p.id.len() > 12 {
                        format!("{}...", &p.id[..12])
                    } else if !p.id.is_empty() {
                        p.id.clone()
                    } else {
                        "unknown".to_string()
                    };
                    
                    PeerDisplayInfo {
                        node_id: short_id,
                        ip_address: ip,
                        port,
                        reputation: 50, // Default reputation
                        last_seen: "now".to_string(),
                    }
                })
                .collect();
        }
        Err(_) => {
            // Keep existing peers on error
        }
    }
}

/// Parse IP and port from peer address or enode URL.
fn parse_peer_address(remote_addr: &str, enode: &str) -> (String, String) {
    // Try remote_address first (format: "ip:port")
    if !remote_addr.is_empty() {
        if let Some((ip, port)) = remote_addr.rsplit_once(':') {
            return (ip.to_string(), port.to_string());
        }
    }
    
    // Try enode URL (format: "enode://nodeid@ip:port")
    if let Some(at_pos) = enode.find('@') {
        let addr_part = &enode[at_pos + 1..];
        if let Some((ip, port)) = addr_part.rsplit_once(':') {
            return (ip.to_string(), port.to_string());
        }
    }
    
    ("unknown".to_string(), "0".to_string())
}

/// Set demo data for development/testing.
fn set_demo_data(app: &mut App) {
    // Set system health
    app.system_health.cpu_percent = 45.0;
    app.system_health.memory_percent = 62.0;
    app.system_health.node_status = NodeStatus::Running;

    // Set last refresh
    app.last_refresh = Some(chrono::Utc::now());

    // Update implemented subsystems to Running
    for id in SubsystemId::ALL {
        if id.is_implemented() {
            let metrics = if id == SubsystemId::PeerDiscovery {
                Some(serde_json::json!({
                    "total_peers": 47,
                    "buckets_used": 12,
                    "banned_count": 3,
                    "pending_verification_count": 5,
                    "max_pending_peers": 1024,
                    "oldest_peer_age_seconds": 9252
                }))
            } else {
                None
            };

            app.update_subsystem(SubsystemInfo {
                id,
                status: SubsystemStatus::Running,
                warning_message: None,
                metrics,
            });
        }
    }
}
