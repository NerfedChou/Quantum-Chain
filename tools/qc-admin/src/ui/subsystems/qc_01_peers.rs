//! QC-01 Peer Discovery panel renderer.
//!
//! Displays:
//! - Overview: Total peers, buckets used, banned, pending verification
//! - Top peers table (from admin_peers API)
//! - Dependency health

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame,
};

use crate::domain::{App, SubsystemInfo};

/// Render the QC-01 Peer Discovery panel.
pub fn render(frame: &mut Frame, area: Rect, info: &SubsystemInfo, app: &App) {
    // Vertical layout: Overview, Peers Table, Dependencies
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),  // Overview
            Constraint::Min(8),     // Peer table
            Constraint::Length(8),  // Dependencies
        ])
        .split(area);

    render_overview(frame, chunks[0], info);
    render_peer_table(frame, chunks[1], app);
    render_dependencies(frame, chunks[2]);
}

/// Render the overview section.
fn render_overview(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    // Extract metrics from info.metrics JSON if available
    let (total_peers, max_peers, buckets_used, max_buckets, banned, pending, max_pending, oldest_age) =
        extract_metrics(info);

    let text = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Total Peers      "),
            Span::styled(
                format!("{:>5}", total_peers),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" / {:<5}", max_peers),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw("    Buckets Used   "),
            Span::styled(
                format!("{:>3}", buckets_used),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" / {:<3}", max_buckets),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Pending Verify   "),
            Span::styled(
                format!("{:>5}", pending),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" / {:<5}", max_pending),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw("    Banned Peers   "),
            Span::styled(
                format!("{:>3}", banned),
                if banned > 0 {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
        ]),
        Line::from(vec![
            Span::raw("  Oldest Peer Age  "),
            Span::styled(
                oldest_age,
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(text).block(
        Block::default()
            .title(" Overview ")
            .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(paragraph, area);
}

/// Render the peer table.
fn render_peer_table(frame: &mut Frame, area: Rect, app: &App) {
    // Header row
    let header = Row::new(vec![
        "NodeID",
        "IP Address",
        "Port",
        "Rep",
        "Last Seen",
    ])
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
    .bottom_margin(1);

    // Build rows from real peer data
    let rows: Vec<Row> = if app.peers.is_empty() {
        // Show "No peers connected" message
        vec![Row::new(vec![
            "(No peers connected)".to_string(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ])
        .style(Style::default().fg(Color::DarkGray))]
    } else {
        app.peers
            .iter()
            .take(10) // Limit to top 10
            .map(|peer| {
                Row::new(vec![
                    peer.node_id.clone(),
                    peer.ip_address.clone(),
                    peer.port.clone(),
                    peer.reputation.to_string(),
                    peer.last_seen.clone(),
                ])
                .style(Style::default().fg(Color::White))
            })
            .collect()
    };

    let widths = [
        Constraint::Length(14),
        Constraint::Length(15),
        Constraint::Length(6),
        Constraint::Length(4),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .title(format!(" Connected Peers ({}) ", app.peers.len()))
                .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );

    frame.render_widget(table, area);
}

/// Render the dependencies section.
fn render_dependencies(frame: &mut Frame, area: Rect) {
    let text = vec![
        Line::from(vec![
            Span::styled(" OUTBOUND ", Style::default().fg(Color::DarkGray)),
            Span::raw("(I depend on):"),
        ]),
        Line::from(vec![
            Span::raw("   → qc-10 Signature Verification  "),
            Span::styled("●", Style::default().fg(Color::Green)),
            Span::styled(" HEALTHY", Style::default().fg(Color::Green)),
            Span::styled("  (DDoS edge defense)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" INBOUND ", Style::default().fg(Color::DarkGray)),
            Span::raw("(Depends on me):"),
        ]),
        Line::from(vec![
            Span::raw("   ← qc-05 Block Propagation       "),
            Span::styled("●", Style::default().fg(Color::Green)),
            Span::styled(" HEALTHY", Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::raw("   ← qc-07 Bloom Filters           "),
            Span::styled("○", Style::default().fg(Color::DarkGray)),
            Span::styled(" NOT IMPL", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("   ← qc-13 Light Clients           "),
            Span::styled("○", Style::default().fg(Color::DarkGray)),
            Span::styled(" NOT IMPL", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let paragraph = Paragraph::new(text).block(
        Block::default()
            .title(" Dependencies ")
            .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(paragraph, area);
}

/// Extract metrics from subsystem info.
/// Returns (total_peers, max_peers, buckets_used, max_buckets, banned, pending, max_pending, oldest_age)
fn extract_metrics(info: &SubsystemInfo) -> (u32, u32, u32, u32, u32, u32, u32, String) {
    // Try to extract from JSON metrics, fall back to defaults
    if let Some(metrics) = &info.metrics {
        // Field names from qc-16 API response
        let total_peers = metrics.get("peers_connected")
            .or_else(|| metrics.get("total_peers"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let routing_table_size = metrics.get("routing_table_size")
            .and_then(|v| v.as_u64())
            .unwrap_or(total_peers as u64) as u32;
        let buckets_used = metrics.get("buckets_used")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let banned = metrics.get("banned_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let pending = metrics.get("pending_verification")
            .or_else(|| metrics.get("pending_verification_count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let max_pending = metrics.get("max_pending")
            .or_else(|| metrics.get("max_pending_peers"))
            .and_then(|v| v.as_u64())
            .unwrap_or(1024) as u32;
        let oldest_age_secs = metrics.get("oldest_peer_age_secs")
            .or_else(|| metrics.get("oldest_peer_age_seconds"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        
        let oldest_age = format_duration(oldest_age_secs);
        
        // Use routing_table_size if larger than peers_connected
        let display_peers = routing_table_size.max(total_peers);
        
        (display_peers, 5120, buckets_used, 256, banned, pending, max_pending, oldest_age)
    } else {
        // Default/placeholder values
        (0, 5120, 0, 256, 0, 0, 1024, "N/A".to_string())
    }
}

/// Format duration in seconds to human-readable string.
fn format_duration(secs: u64) -> String {
    if secs == 0 {
        return "N/A".to_string();
    }
    
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let secs = secs % 60;
    
    if hours > 0 {
        format!("{}h {}m {}s", hours, mins, secs)
    } else if mins > 0 {
        format!("{}m {}s", mins, secs)
    } else {
        format!("{}s", secs)
    }
}
