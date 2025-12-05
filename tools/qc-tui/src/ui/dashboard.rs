//! Dashboard UI rendering.

use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

/// Render the main dashboard.
pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Length(9),  // Info panels (Chain + Network + Mempool Summary)
            Constraint::Length(8),  // Recent blocks
            Constraint::Min(6),     // Live events
            Constraint::Length(3),  // Footer
        ])
        .split(frame.area());

    render_header(frame, app, chunks[0]);
    render_info_panels(frame, app, chunks[1]);
    render_recent_blocks(frame, app, chunks[2]);
    render_live_events(frame, app, chunks[3]);
    render_footer(frame, chunks[4]);
}

/// Render the header bar.
fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let status_color = if !app.connected {
        Color::Red
    } else if app.sync_status.is_synced() {
        Color::Green
    } else {
        Color::Yellow
    };

    let status_symbol = if app.connected { "●" } else { "○" };

    let header = Paragraph::new(Line::from(vec![
        Span::raw(" Status: "),
        Span::styled(
            format!("{} {}", status_symbol, app.status_str()),
            Style::default().fg(status_color).add_modifier(Modifier::BOLD),
        ),
        Span::raw("    Uptime: "),
        Span::styled(app.uptime_str(), Style::default().fg(Color::Cyan)),
        Span::raw("    Sync: "),
        Span::styled(
            format!("{}%", app.sync_status.percentage()),
            Style::default().fg(if app.sync_status.is_synced() {
                Color::Green
            } else {
                Color::Yellow
            }),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" QUANTUM-CHAIN NODE ─────────────────────────────────────────── v0.1.0 "),
    );

    frame.render_widget(header, area);
}

/// Render the info panels (Chain + Network + Mempool Summary).
fn render_info_panels(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(area);

    render_chain_info(frame, app, chunks[0]);
    render_network_info(frame, app, chunks[1]);
    render_mempool_summary(frame, app, chunks[2]);
}

/// Render chain information panel.
fn render_chain_info(frame: &mut Frame, app: &App, area: Rect) {
    let finalized = app.block_height.saturating_sub(64);
    
    let text = vec![
        Line::from(vec![
            Span::raw(" Latest:     "),
            Span::styled(
                format!("#{}", format_number(app.block_height)),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw(" Finalized:  "),
            Span::styled(
                format!("#{}", format_number(finalized)),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::raw(" Chain ID:   "),
            Span::styled(
                format!("{} ({})", app.chain_id, chain_name(app.chain_id)),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::raw(" Gas Price:  "),
            Span::styled(
                format!("{:.2} gwei", app.gas_price_gwei()),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::raw(" Block Time: "),
            Span::styled("~12s", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" ⛓ CHAIN ")
        .border_style(Style::default().fg(Color::Blue));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

/// Render network information panel.
fn render_network_info(frame: &mut Frame, app: &App, area: Rect) {
    let listen_symbol = if app.listening { "●" } else { "○" };
    let listen_color = if app.listening { Color::Green } else { Color::Yellow };
    let listen_text = if app.listening { "Accepting" } else { "Local Only" };

    let ws_symbol = if app.ws_connected { "●" } else { "○" };
    let ws_color = if app.ws_connected { Color::Green } else { Color::Red };
    let ws_text = if app.ws_connected { "Streaming" } else { "Offline" };

    let http_symbol = if app.connected { "●" } else { "○" };
    let http_color = if app.connected { Color::Green } else { Color::Red };

    let peer_color = if app.peer_count > 0 { Color::Green } else { Color::Yellow };

    let text = vec![
        Line::from(vec![
            Span::raw(" Peers:      "),
            Span::styled(
                format!("{}", app.peer_count),
                Style::default().fg(peer_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                if app.peer_count == 0 { " (solo)" } else { " connected" },
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::raw(" P2P:        "),
            Span::styled(listen_symbol, Style::default().fg(listen_color)),
            Span::styled(format!(" {}", listen_text), Style::default().fg(listen_color)),
        ]),
        Line::from(vec![
            Span::raw(" HTTP RPC:   "),
            Span::styled(http_symbol, Style::default().fg(http_color)),
            Span::styled(" :8545", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw(" WebSocket:  "),
            Span::styled(ws_symbol, Style::default().fg(ws_color)),
            Span::styled(format!(" {}", ws_text), Style::default().fg(ws_color)),
        ]),
        Line::from(vec![
            Span::raw(" Network ID: "),
            Span::styled(
                app.network_version.clone(),
                Style::default().fg(Color::Cyan),
            ),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" 🌐 NETWORK ")
        .border_style(Style::default().fg(Color::Magenta));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

/// Render mempool summary panel (replaces RPC Health).
fn render_mempool_summary(frame: &mut Frame, app: &App, area: Rect) {
    let pending = app.txpool_status.pending;
    let queued = app.txpool_status.queued;
    let total = pending + queued;
    
    let pending_color = if pending > 0 { Color::Yellow } else { Color::DarkGray };
    let queued_color = if queued > 0 { Color::Cyan } else { Color::DarkGray };

    let text = vec![
        Line::from(vec![
            Span::raw(" Pending:    "),
            Span::styled(
                format!("{}", pending),
                Style::default().fg(pending_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" txs", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw(" Queued:     "),
            Span::styled(
                format!("{}", queued),
                Style::default().fg(queued_color),
            ),
            Span::styled(" txs", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw(" Total:      "),
            Span::styled(
                format!("{}", total),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::raw(" WS Events:  "),
            Span::styled(
                format!("{}", app.pending_tx_count),
                Style::default().fg(Color::Green),
            ),
            Span::styled(" received", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw(" Status:     "),
            Span::styled(
                if pending > 0 { "Processing" } else { "Idle" },
                Style::default().fg(if pending > 0 { Color::Green } else { Color::DarkGray }),
            ),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" 📦 MEMPOOL ")
        .border_style(Style::default().fg(Color::Yellow));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

/// Get chain name from chain ID.
fn chain_name(chain_id: u64) -> &'static str {
    match chain_id {
        1 => "Mainnet",
        5 => "Goerli",
        11155111 => "Sepolia",
        1337 => "DevNet",
        31337 => "Hardhat",
        _ => "Custom",
    }
}



/// Render recent blocks panel.
fn render_recent_blocks(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .recent_blocks
        .iter()
        .enumerate()
        .map(|(i, block)| {
            let age = format_age(i);
            let style = if i == 0 {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!(" #{:<10}", format_number(block.block_number())), style),
                Span::styled(format!(" {:<12}", block.short_hash()), Style::default().fg(Color::Cyan)),
                Span::styled(format!(" {:>3} txs", block.tx_count()), Style::default().fg(Color::Yellow)),
                Span::styled(format!("  {:<10}", age), Style::default().fg(Color::DarkGray)),
                Span::styled(" ✓", Style::default().fg(Color::Green)),
            ]))
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" RECENT BLOCKS (WebSocket) ")
        .border_style(Style::default().fg(Color::Blue));

    let list = if items.is_empty() {
        List::new(vec![ListItem::new(Span::styled(
            " Waiting for blocks...",
            Style::default().fg(Color::DarkGray),
        ))])
    } else {
        List::new(items)
    };

    frame.render_widget(list.block(block), area);
}

/// Render live events panel.
fn render_live_events(frame: &mut Frame, app: &App, area: Rect) {
    let max_events = (area.height as usize).saturating_sub(2);

    let items: Vec<ListItem> = app
        .live_events
        .iter()
        .take(max_events)
        .map(|event| {
            let elapsed = event.timestamp.elapsed();
            let time_str = format_elapsed(elapsed);

            let type_color = match event.event_type.as_str() {
                "newHeads" => Color::Green,
                "pendingTx" => Color::Yellow,
                "ws" => Color::Cyan,
                "error" => Color::Red,
                _ => Color::Gray,
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!(" {:<8}", time_str), Style::default().fg(Color::DarkGray)),
                Span::raw(" │ "),
                Span::styled(format!("{:<12}", event.event_type), Style::default().fg(type_color)),
                Span::raw(" │ "),
                Span::raw(&event.description),
            ]))
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" LIVE EVENTS (WebSocket) ")
        .border_style(Style::default().fg(Color::Green));

    let list = if items.is_empty() {
        List::new(vec![ListItem::new(Span::styled(
            " Waiting for events...",
            Style::default().fg(Color::DarkGray),
        ))])
    } else {
        List::new(items)
    };

    frame.render_widget(list.block(block), area);
}

/// Render the footer bar.
fn render_footer(frame: &mut Frame, area: Rect) {
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" [D] ", Style::default().fg(Color::Yellow)),
        Span::raw("Dashboard  "),
        Span::styled("[M] ", Style::default().fg(Color::Yellow)),
        Span::raw("Mempool  "),
        Span::styled("[B] ", Style::default().fg(Color::Yellow)),
        Span::raw("Blocks  "),
        Span::styled("[P] ", Style::default().fg(Color::Yellow)),
        Span::raw("Peers  "),
        Span::raw("│ "),
        Span::styled("[R] ", Style::default().fg(Color::Yellow)),
        Span::raw("Refresh  "),
        Span::styled("[Q] ", Style::default().fg(Color::Yellow)),
        Span::raw("Quit"),
    ]))
    .block(Block::default().borders(Borders::ALL));

    frame.render_widget(footer, area);
}

/// Format a number with thousand separators.
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().rev().collect();

    for (i, c) in chars.iter().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(*c);
    }

    result.chars().rev().collect()
}

/// Format elapsed time as a human-readable string.
fn format_elapsed(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{}s ago", secs)
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else {
        format!("{}h ago", secs / 3600)
    }
}

/// Format block age based on position in list.
fn format_age(index: usize) -> String {
    match index {
        0 => "just now".to_string(),
        1 => "~12s ago".to_string(),
        2 => "~24s ago".to_string(),
        3 => "~36s ago".to_string(),
        4 => "~48s ago".to_string(),
        _ => format!("~{}s ago", index * 12),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(123), "123");
        assert_eq!(format_number(1234), "1,234");
        assert_eq!(format_number(1234567), "1,234,567");
    }
}
