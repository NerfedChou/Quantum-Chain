//! QC-03 Transaction Indexing panel renderer.
//!
//! Displays:
//! - Overview: Total indexed, cached trees, proofs generated/verified
//! - Merkle Tree Cache status (INVARIANT-5)
//! - Dependency health (V2.2 Choreography pattern)

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};

use crate::domain::SubsystemInfo;

/// Render the QC-03 Transaction Indexing panel.
pub fn render(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    // Vertical layout: Overview, Cache Status, Dependencies
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),  // Overview
            Constraint::Length(6),  // Cache gauge
            Constraint::Min(10),    // Dependencies
        ])
        .split(area);

    render_overview(frame, chunks[0], info);
    render_cache_status(frame, chunks[1], info);
    render_dependencies(frame, chunks[2], info);
}

/// Render the overview section.
fn render_overview(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let (total_indexed, cached_trees, max_cached, proofs_generated, proofs_verified, last_merkle_root) =
        extract_metrics(info);

    let cache_percent = if max_cached > 0 {
        (cached_trees as f64 / max_cached as f64 * 100.0) as u32
    } else {
        0
    };

    let text = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Total Indexed    "),
            Span::styled(
                format!("{:<12}", format_number(total_indexed)),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Proofs Generated  "),
            Span::styled(
                format!("{}", format_number(proofs_generated)),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Cached Trees     "),
            Span::styled(
                format!("{:<4}", cached_trees),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" / {:<5}", max_cached),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw("    Proofs Verified   "),
            Span::styled(
                format!("{}", format_number(proofs_verified)),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Cache Usage      "),
            Span::styled(
                format!("{}%", cache_percent),
                if cache_percent > 90 {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                } else if cache_percent > 75 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Last Merkle Root "),
            Span::styled(
                if last_merkle_root.is_empty() {
                    "N/A".to_string()
                } else {
                    format!("0x{}...", &last_merkle_root[..16.min(last_merkle_root.len())])
                },
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

/// Render the cache status gauge (INVARIANT-5 visualization).
fn render_cache_status(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let (_, cached_trees, max_cached, _, _, _) = extract_metrics(info);

    let cache_ratio = if max_cached > 0 {
        cached_trees as f64 / max_cached as f64
    } else {
        0.0
    };

    let gauge_color = if cache_ratio > 0.9 {
        Color::Red
    } else if cache_ratio > 0.75 {
        Color::Yellow
    } else {
        Color::Green
    };

    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(" Merkle Tree Cache (INVARIANT-5: Bounded) ")
                .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .gauge_style(Style::default().fg(gauge_color))
        .ratio(cache_ratio.min(1.0))
        .label(format!(
            "{} / {} trees ({:.1}%)",
            cached_trees,
            max_cached,
            cache_ratio * 100.0
        ));

    frame.render_widget(gauge, area);
}

/// Render the dependencies section (V2.2 Choreography pattern).
fn render_dependencies(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let is_healthy = matches!(info.status, crate::domain::SubsystemStatus::Running);

    let text = vec![
        Line::from(vec![
            Span::styled(" SUBSCRIBES TO ", Style::default().fg(Color::DarkGray)),
            Span::raw("(Choreography - BlockValidated):"),
        ]),
        Line::from(vec![
            Span::raw("   ← qc-08 Consensus               "),
            status_indicator(is_healthy),
            Span::styled("  (BlockValidated event)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" PUBLISHES ", Style::default().fg(Color::DarkGray)),
            Span::raw("(Choreography - MerkleRootComputed):"),
        ]),
        Line::from(vec![
            Span::raw("   → qc-02 Block Storage           "),
            status_indicator(is_healthy),
            Span::styled("  (triggers block write)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" QUERIES ", Style::default().fg(Color::DarkGray)),
            Span::raw("(V2.3 Data Retrieval):"),
        ]),
        Line::from(vec![
            Span::raw("   ← qc-02 GetTransactionHashes    "),
            status_indicator(is_healthy),
        ]),
        Line::from(vec![
            Span::raw("   → qc-16 MerkleProofResponse     "),
            status_indicator(is_healthy),
            Span::styled("  (API proof requests)", Style::default().fg(Color::DarkGray)),
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

/// Create a status indicator span.
fn status_indicator(healthy: bool) -> Span<'static> {
    if healthy {
        Span::styled("● HEALTHY", Style::default().fg(Color::Green))
    } else {
        Span::styled("● DOWN", Style::default().fg(Color::Red))
    }
}

/// Extract metrics from subsystem info.
/// Returns (total_indexed, cached_trees, max_cached, proofs_generated, proofs_verified, last_merkle_root)
fn extract_metrics(info: &SubsystemInfo) -> (u64, usize, usize, u64, u64, String) {
    if let Some(metrics) = &info.metrics {
        let total_indexed = metrics.get("total_indexed")
            .or_else(|| metrics.get("merkle_trees"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cached_trees = metrics.get("cached_trees")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let max_cached = metrics.get("max_cached_trees")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000) as usize;
        let proofs_generated = metrics.get("proofs_generated")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let proofs_verified = metrics.get("proofs_verified")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let last_merkle_root = metrics.get("last_merkle_root")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        (total_indexed, cached_trees, max_cached, proofs_generated, proofs_verified, last_merkle_root)
    } else {
        (0, 0, 1000, 0, 0, String::new())
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
