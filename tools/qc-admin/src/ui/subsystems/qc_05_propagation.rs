//! QC-05 Block Propagation panel renderer.
//!
//! Displays:
//! - Overview: Blocks propagated, peers reached, compact block stats
//! - Gossip metrics (BIP152 compact block relay)
//! - Dependency health (V2.3 Choreography pattern)

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};

use crate::domain::SubsystemInfo;

/// Render the QC-05 Block Propagation panel.
pub fn render(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    // Vertical layout: Overview, Gossip Metrics, Dependencies
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),  // Overview
            Constraint::Length(7),  // Gossip/compact block gauge
            Constraint::Min(10),    // Dependencies
        ])
        .split(area);

    render_overview(frame, chunks[0], info);
    render_gossip_metrics(frame, chunks[1], info);
    render_dependencies(frame, chunks[2], info);
}

/// Render the overview section with metrics in individual boxes (flex, space-around).
fn render_overview(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let (blocks_propagated, peers_reached, avg_propagation_ms, compact_success_rate,
         fanout, seen_cache_size, announcements_received) = extract_metrics(info);

    // Container block
    let container = Block::default()
        .title(" Overview ")
        .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    
    let inner = container.inner(area);
    frame.render_widget(container, area);

    // Split into 6 boxes horizontally (space-around effect)
    let boxes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(16),
            Constraint::Percentage(17),
            Constraint::Percentage(17),
            Constraint::Percentage(17),
            Constraint::Percentage(16),
            Constraint::Percentage(17),
        ])
        .split(inner);

    // Box 1: Blocks Propagated
    let box1 = Paragraph::new(vec![
        Line::raw(""),
        Line::from(Span::styled(
            format_number(blocks_propagated),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
    ])
    .alignment(ratatui::layout::Alignment::Center)
    .block(Block::default().title(" Blocks ").borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(box1, boxes[0]);

    // Box 2: Peers Reached
    let box2 = Paragraph::new(vec![
        Line::raw(""),
        Line::from(Span::styled(
            format!("{}", peers_reached),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )),
    ])
    .alignment(ratatui::layout::Alignment::Center)
    .block(Block::default().title(" Peers ").borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(box2, boxes[1]);

    // Box 3: Avg Propagation
    let prop_color = if avg_propagation_ms > 1000 {
        Color::Red
    } else if avg_propagation_ms > 500 {
        Color::Yellow
    } else {
        Color::Green
    };
    let box3 = Paragraph::new(vec![
        Line::raw(""),
        Line::from(Span::styled(
            format!("{}ms", avg_propagation_ms),
            Style::default().fg(prop_color).add_modifier(Modifier::BOLD),
        )),
    ])
    .alignment(ratatui::layout::Alignment::Center)
    .block(Block::default().title(" Avg Prop ").borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(box3, boxes[2]);

    // Box 4: Announcements
    let box4 = Paragraph::new(vec![
        Line::raw(""),
        Line::from(Span::styled(
            format_number(announcements_received),
            Style::default().fg(Color::Cyan),
        )),
    ])
    .alignment(ratatui::layout::Alignment::Center)
    .block(Block::default().title(" Announces ").borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(box4, boxes[3]);

    // Box 5: Gossip Fanout
    let box5 = Paragraph::new(vec![
        Line::raw(""),
        Line::from(Span::styled(
            format!("{}", fanout),
            Style::default().fg(Color::White),
        )),
    ])
    .alignment(ratatui::layout::Alignment::Center)
    .block(Block::default().title(" Fanout ").borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(box5, boxes[4]);

    // Box 6: Seen Cache
    let box6 = Paragraph::new(vec![
        Line::raw(""),
        Line::from(Span::styled(
            format_number(seen_cache_size),
            Style::default().fg(Color::White),
        )),
    ])
    .alignment(ratatui::layout::Alignment::Center)
    .block(Block::default().title(" Cache ").borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(box6, boxes[5]);
}

/// Render the gossip metrics section (BIP152 compact block stats).
fn render_gossip_metrics(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let (_, _, _, compact_success_rate, _, _, _) = extract_metrics(info);
    let (avg_missing_txs, blocks_last_hour) = extract_compact_metrics(info);

    let success_ratio = (compact_success_rate / 100.0).min(1.0).max(0.0);

    let gauge_color = if compact_success_rate > 90.0 {
        Color::Green
    } else if compact_success_rate > 70.0 {
        Color::Yellow
    } else {
        Color::Red
    };

    // Split area into gauge and stats
    let inner_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .margin(1)
        .split(area);

    // Render the gauge
    let gauge = Gauge::default()
        .block(Block::default())
        .gauge_style(Style::default().fg(gauge_color))
        .ratio(success_ratio)
        .label(format!("Compact Block Reconstruction: {:.1}%", compact_success_rate));

    // Render stats text
    let stats_text = vec![
        Line::from(vec![
            Span::raw("Avg Missing Txs: "),
            Span::styled(
                format!("{:.1}", avg_missing_txs),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::raw("Blocks/Hour:     "),
            Span::styled(
                format!("{}", blocks_last_hour),
                Style::default().fg(Color::Cyan),
            ),
        ]),
    ];

    let stats_paragraph = Paragraph::new(stats_text);

    // Render both in their areas
    let block = Block::default()
        .title(" BIP152 Compact Block Relay ")
        .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    frame.render_widget(block, area);
    frame.render_widget(gauge, inner_chunks[0]);
    frame.render_widget(stats_paragraph, inner_chunks[1]);
}

/// Render the dependencies section with 4 horizontal boxes.
/// Per SPEC-05: receives qc-08, gossips via qc-01, queries qc-06/qc-10
fn render_dependencies(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let is_healthy = matches!(info.status, crate::domain::SubsystemStatus::Running);

    // Container block
    let container = Block::default()
        .title(" Dependencies ")
        .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    
    let inner = container.inner(area);
    frame.render_widget(container, area);

    // Split into 4 horizontal sections
    let sections = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(inner);

    // Box 1: RECEIVES FROM (PropagateBlockRequest from Consensus)
    let receives_text = vec![
        Line::from(vec![
            Span::raw("← qc-08 "),
            status_indicator(is_healthy),
        ]),
    ];
    let receives_box = Paragraph::new(receives_text)
        .block(
            Block::default()
                .title(" RECEIVES ")
                .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(receives_box, sections[0]);

    // Box 2: GOSSIPS TO (Peer Discovery for peer list, Network Peers)
    let gossips_text = vec![
        Line::from(vec![
            Span::raw("→ qc-01 "),
            status_indicator(is_healthy),
        ]),
        Line::from(vec![
            Span::raw("→ Peers "),
            status_indicator(is_healthy),
        ]),
    ];
    let gossips_box = Paragraph::new(gossips_text)
        .block(
            Block::default()
                .title(" GOSSIPS ")
                .title_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(gossips_box, sections[1]);

    // Box 3: QUERIES (Mempool for tx lookup, Sig Verify for blocks)
    let queries_text = vec![
        Line::from(vec![
            Span::raw("↔ qc-06 "),
            status_indicator(is_healthy),
        ]),
        Line::from(vec![
            Span::raw("→ qc-10 "),
            status_indicator(is_healthy),
        ]),
    ];
    let queries_box = Paragraph::new(queries_text)
        .block(
            Block::default()
                .title(" QUERIES ")
                .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(queries_box, sections[2]);

    // Box 4: SENDS TO (Consensus for received block validation)
    let sends_text = vec![
        Line::from(vec![
            Span::raw("→ qc-08 "),
            status_indicator(is_healthy),
        ]),
    ];
    let sends_box = Paragraph::new(sends_text)
        .block(
            Block::default()
                .title(" SENDS TO ")
                .title_style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(sends_box, sections[3]);
}

/// Create a status indicator span.
fn status_indicator(healthy: bool) -> Span<'static> {
    if healthy {
        Span::styled("● HEALTHY", Style::default().fg(Color::Green))
    } else {
        Span::styled("● DOWN", Style::default().fg(Color::Red))
    }
}

/// Extract metrics from subsystem info.
/// Returns (blocks_propagated, peers_reached, avg_propagation_ms, compact_success_rate, fanout, seen_cache_size, announcements_received)
fn extract_metrics(info: &SubsystemInfo) -> (u64, u64, u64, f64, u64, u64, u64) {
    if let Some(metrics) = &info.metrics {
        let blocks_propagated = metrics.get("blocks_propagated")
            .or_else(|| metrics.get("blocks_relayed"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let peers_reached = metrics.get("peers_reached")
            .or_else(|| metrics.get("peers_sent"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let avg_propagation_ms = metrics.get("avg_propagation_time_ms")
            .or_else(|| metrics.get("average_propagation_time_ms"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let compact_success_rate = metrics.get("compact_block_success_rate")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let fanout = metrics.get("fanout")
            .and_then(|v| v.as_u64())
            .unwrap_or(8);
        let seen_cache_size = metrics.get("seen_cache_size")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let announcements_received = metrics.get("announcements_received")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        (blocks_propagated, peers_reached, avg_propagation_ms, compact_success_rate, fanout, seen_cache_size, announcements_received)
    } else {
        (0, 0, 0, 0.0, 8, 0, 0)
    }
}

/// Extract compact block specific metrics.
/// Returns (avg_missing_txs, blocks_last_hour)
fn extract_compact_metrics(info: &SubsystemInfo) -> (f64, u64) {
    if let Some(metrics) = &info.metrics {
        let avg_missing_txs = metrics.get("average_missing_txs")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let blocks_last_hour = metrics.get("blocks_propagated_last_hour")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        (avg_missing_txs, blocks_last_hour)
    } else {
        (0.0, 0)
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
