//! QC-16 API Gateway panel renderer.
//!
//! Displays:
//! - Overview: Request counts, latency, success rate
//! - Ports: HTTP/RPC (8545), WebSocket (8546), Admin (8080)
//! - Method Tiers: Public, Protected, Admin access controls
//! - Rate Limiting: Per-IP limits, rejection stats
//! - WebSocket: Active connections, subscriptions
//! - Dependency health (routes to all subsystems)

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::domain::SubsystemInfo;

/// Render the QC-16 API Gateway panel.
pub fn render(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    // Vertical layout: Overview, Ports & Tiers, Rate Limiting & WebSocket, Routes
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),  // Overview
            Constraint::Length(10), // Ports & Method Tiers
            Constraint::Length(9),  // Rate Limiting & WebSocket
            Constraint::Min(8),     // Routes to subsystems
        ])
        .split(area);

    render_overview(frame, chunks[0], info);
    render_ports_and_tiers(frame, chunks[1], info);
    render_rate_limit_and_websocket(frame, chunks[2], info);
    render_routes(frame, chunks[3], info);
}

/// Render the overview section.
fn render_overview(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let metrics = extract_metrics(info);

    let success_rate = if metrics.requests_total > 0 {
        (metrics.requests_success as f64 / metrics.requests_total as f64) * 100.0
    } else {
        100.0
    };

    let text = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Total Requests     "),
            Span::styled(
                format!("{:<12}", format_number(metrics.requests_total)),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Success Rate       "),
            Span::styled(
                format!("{:.1}%", success_rate),
                if success_rate > 99.0 {
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                } else if success_rate > 95.0 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Red)
                },
            ),
        ]),
        Line::from(vec![
            Span::raw("  Write Requests     "),
            Span::styled(
                format!("{:<12}", format_number(metrics.write_requests)),
                Style::default().fg(Color::Magenta),
            ),
            Span::raw("  Avg Latency        "),
            Span::styled(
                format!("{:.2}ms", metrics.avg_latency_ms),
                if metrics.avg_latency_ms < 50.0 {
                    Style::default().fg(Color::Green)
                } else if metrics.avg_latency_ms < 200.0 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Red)
                },
            ),
        ]),
        Line::from(vec![
            Span::raw("  Pending Requests   "),
            Span::styled(
                format!("{:<12}", metrics.pending_requests),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw("  Timeouts           "),
            Span::styled(
                format!("{}", metrics.pending_timeouts),
                if metrics.pending_timeouts > 0 {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Green)
                },
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

/// Render the ports and method tiers section.
fn render_ports_and_tiers(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let is_healthy = matches!(info.status, crate::domain::SubsystemStatus::Running);

    let text = vec![
        Line::from(vec![
            Span::styled(" ENDPOINTS ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::raw("   HTTP/JSON-RPC  :8545  "),
            status_indicator(is_healthy),
            Span::styled("   eth_*, web3_*, net_*", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("   WebSocket      :8546  "),
            status_indicator(is_healthy),
            Span::styled("   eth_subscribe, real-time events", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("   Admin API      :8080  "),
            status_indicator(is_healthy),
            Span::styled("   admin_*, debug_* (localhost only)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" METHOD TIERS ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled("(SPEC-16 Section 3)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("   Tier 1 ", Style::default().fg(Color::Green)),
            Span::raw("Public     "),
            Span::styled("No auth required     ", Style::default().fg(Color::DarkGray)),
            Span::raw("eth_getBalance, eth_sendRawTransaction"),
        ]),
        Line::from(vec![
            Span::styled("   Tier 2 ", Style::default().fg(Color::Yellow)),
            Span::raw("Protected  "),
            Span::styled("API key OR localhost ", Style::default().fg(Color::DarkGray)),
            Span::raw("txpool_*, admin_peers"),
        ]),
        Line::from(vec![
            Span::styled("   Tier 3 ", Style::default().fg(Color::Red)),
            Span::raw("Admin      "),
            Span::styled("Localhost + API key  ", Style::default().fg(Color::DarkGray)),
            Span::raw("admin_addPeer, debug_*"),
        ]),
    ];

    let paragraph = Paragraph::new(text).block(
        Block::default()
            .title(" Endpoints & Access Control ")
            .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(paragraph, area);
}

/// Render the rate limiting and WebSocket section.
fn render_rate_limit_and_websocket(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let metrics = extract_metrics(info);

    // Split horizontally for rate limit and websocket
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Rate Limiting
    let rate_limit_text = vec![
        Line::from(vec![
            Span::styled(" Per-IP Token Bucket ", Style::default().fg(Color::Yellow)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Reads:  100/sec (burst: 200)"),
        ]),
        Line::from(vec![
            Span::raw("  Writes: 10/sec  (burst: 20)"),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Rejected: "),
            Span::styled(
                format!("{}", metrics.rate_limit_rejected),
                if metrics.rate_limit_rejected > 0 {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
        ]),
    ];

    let rate_limit_paragraph = Paragraph::new(rate_limit_text).block(
        Block::default()
            .title(" Rate Limiting ")
            .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(rate_limit_paragraph, chunks[0]);

    // WebSocket
    let ws_text = vec![
        Line::from(vec![
            Span::styled(" Real-time Subscriptions ", Style::default().fg(Color::Magenta)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Connections:   "),
            Span::styled(
                format!("{}", metrics.websocket_connections),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Subscriptions: "),
            Span::styled(
                format!("{}", metrics.websocket_subscriptions),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Messages Sent: "),
            Span::styled(
                format!("{}", format_number(metrics.websocket_messages)),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ];

    let ws_paragraph = Paragraph::new(ws_text).block(
        Block::default()
            .title(" WebSocket ")
            .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(ws_paragraph, chunks[1]);
}

/// Render the routes to subsystems section.
fn render_routes(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let is_healthy = matches!(info.status, crate::domain::SubsystemStatus::Running);

    let text = vec![
        Line::from(vec![
            Span::styled(" ROUTES TO SUBSYSTEMS ", Style::default().fg(Color::DarkGray)),
            Span::raw("(IPC Handler dispatches to):"),
        ]),
        Line::from(vec![
            Span::raw("   → qc-02 Block Storage        "),
            status_indicator(is_healthy),
            Span::styled("  eth_getBlock*, eth_getTransaction*", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("   → qc-04 State Management     "),
            status_indicator(is_healthy),
            Span::styled("  eth_getBalance, eth_getCode, eth_call", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("   → qc-06 Mempool              "),
            status_indicator(is_healthy),
            Span::styled("  eth_sendRawTransaction, txpool_*", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("   → qc-01 Peer Discovery       "),
            status_indicator(is_healthy),
            Span::styled("  admin_peers, admin_addPeer", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("   → node-runtime               "),
            status_indicator(is_healthy),
            Span::styled("  debug_subsystemHealth, eth_syncing", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let paragraph = Paragraph::new(text).block(
        Block::default()
            .title(" Request Routing ")
            .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(paragraph, area);
}

/// Create a status indicator span.
fn status_indicator(healthy: bool) -> Span<'static> {
    if healthy {
        Span::styled("● UP", Style::default().fg(Color::Green))
    } else {
        Span::styled("● DOWN", Style::default().fg(Color::Red))
    }
}

/// Metrics extracted from subsystem info.
struct GatewayMetrics {
    requests_total: u64,
    requests_success: u64,
    requests_error: u64,
    write_requests: u64,
    avg_latency_ms: f64,
    pending_requests: u64,
    pending_timeouts: u64,
    rate_limit_rejected: u64,
    websocket_connections: u64,
    websocket_subscriptions: u64,
    websocket_messages: u64,
}

/// Extract metrics from subsystem info.
fn extract_metrics(info: &SubsystemInfo) -> GatewayMetrics {
    if let Some(metrics) = &info.metrics {
        GatewayMetrics {
            requests_total: metrics.get("requests_total")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            requests_success: metrics.get("requests_success")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            requests_error: metrics.get("requests_error")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            write_requests: metrics.get("write_requests")
                .or_else(|| metrics.get("writes"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            avg_latency_ms: metrics.get("avg_latency_ms")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            pending_requests: metrics.get("pending_requests")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            pending_timeouts: metrics.get("pending_timeouts")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            rate_limit_rejected: metrics.get("rate_limit_rejected")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            websocket_connections: metrics.get("websocket_connections")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            websocket_subscriptions: metrics.get("websocket_subscriptions")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            websocket_messages: metrics.get("websocket_messages")
                .or_else(|| metrics.get("websocket_messages_sent"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
        }
    } else {
        GatewayMetrics {
            requests_total: 0,
            requests_success: 0,
            requests_error: 0,
            write_requests: 0,
            avg_latency_ms: 0.0,
            pending_requests: 0,
            pending_timeouts: 0,
            rate_limit_rejected: 0,
            websocket_connections: 0,
            websocket_subscriptions: 0,
            websocket_messages: 0,
        }
    }
}

/// Format a number with thousand separators.
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.insert(0, ',');
        }
        result.insert(0, c);
    }
    result
}
