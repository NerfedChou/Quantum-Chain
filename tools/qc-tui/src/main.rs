//! QC-TUI: Terminal User Interface for Quantum-Chain node monitoring.
//!
//! This is an external client tool that communicates with the node
//! via qc-16 API Gateway (JSON-RPC). It has no special access to
//! node internals - same access level as any external wallet or dApp.
//!
//! ## Usage
//!
//! ```bash
//! # Connect to localhost (default)
//! qc-tui
//!
//! # Connect to remote node
//! qc-tui --rpc-url http://node.example.com:8545
//! ```

mod app;
mod rpc;
mod ui;
mod ws;

use std::io;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use tokio::sync::mpsc;

use app::App;
use ws::WsClient;

/// Quantum-Chain Terminal User Interface
#[derive(Parser, Debug)]
#[command(name = "qc-tui")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// JSON-RPC endpoint URL
    #[arg(long, default_value = "http://localhost:8545")]
    rpc_url: String,

    /// WebSocket endpoint URL
    #[arg(long, default_value = "ws://localhost:8546")]
    ws_url: String,

    /// Data refresh interval in milliseconds
    #[arg(long, default_value = "2000")]
    refresh_ms: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Setup terminal with panic hook for cleanup
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Attempt terminal cleanup on panic
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(args.rpc_url);
    let refresh_interval = Duration::from_millis(args.refresh_ms);

    // Create WebSocket event channel
    let (ws_tx, ws_rx) = mpsc::channel(100);

    // Start WebSocket client
    let mut ws_client = WsClient::new(args.ws_url, ws_tx);
    ws_client.start().await?;

    // Run the app
    let result = run_app(&mut terminal, &mut app, refresh_interval, ws_rx).await;

    // Cleanup WebSocket
    ws_client.stop().await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
    }

    Ok(())
}

/// Main application loop.
async fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    refresh_interval: Duration,
    mut ws_rx: mpsc::Receiver<ws::WsEvent>,
) -> Result<()> {
    // Initial data fetch
    let _ = app.refresh().await;

    loop {
        // Draw UI
        terminal.draw(|frame| ui::render(frame, app))?;

        // Use a short poll timeout to handle both events and WS messages
        let poll_timeout = Duration::from_millis(100);

        // Check for WebSocket events (non-blocking)
        while let Ok(event) = ws_rx.try_recv() {
            app.handle_ws_event(event);
        }

        // Track previous tab for tab-switch detection
        let prev_tab = app.active_tab;

        // Handle terminal events with timeout
        handle_terminal_events(app, poll_timeout).await?;

        // Refresh data when switching tabs (if needed)
        if app.active_tab != prev_tab && app.needs_tab_refresh() {
            refresh_tab_data(app).await;
        }

        // Periodic HTTP refresh (less frequent since WS updates block height)
        if app.last_refresh.elapsed() >= refresh_interval {
            let _ = app.refresh().await;
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

/// Handle terminal key events.
async fn handle_terminal_events(app: &mut App, poll_timeout: Duration) -> Result<()> {
    if !event::poll(poll_timeout)? {
        return Ok(());
    }
    
    let Event::Key(key) = event::read()? else {
        return Ok(());
    };
    
    if key.kind != KeyEventKind::Press {
        return Ok(());
    }
    
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
            app.should_quit = true;
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            refresh_tab_data(app).await;
        }
        _ => {
            app.on_key(key.code);
        }
    }
    
    Ok(())
}

/// Refresh data for the current active tab.
async fn refresh_tab_data(app: &mut App) {
    use crate::app::Tab;
    
    match app.active_tab {
        Tab::Dashboard => { let _ = app.refresh().await; }
        Tab::Mempool => { let _ = app.refresh_mempool().await; }
        Tab::Blocks => { let _ = app.refresh_blocks().await; }
        Tab::Peers => { let _ = app.refresh_peers().await; }
    }
}
