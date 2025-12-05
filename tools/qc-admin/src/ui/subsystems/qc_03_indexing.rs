//! QC-03 Transaction Indexing panel renderer.
//!
//! Displays:
//! - Overview: Total indexed, cached trees, proofs generated/verified (collapsed table style)
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
            Constraint::Length(4),  // Overview (collapsed table)
            Constraint::Length(5),  // Cache gauge
            Constraint::Min(6),     // Dependencies
        ])
        .split(area);

    render_overview(frame, chunks[0], info);
    render_cache_status(frame, chunks[1], info);
    render_dependencies(frame, chunks[2], info);
}

/// Render the overview section as collapsed table cells.
fn render_overview(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let (total_indexed, cached_trees, max_cached, proofs_generated, proofs_verified, last_block_height, avg_tree_depth) =
        extract_metrics(info);

    // 6 metric boxes in collapsed table style
    let boxes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 6),
            Constraint::Ratio(1, 6),
            Constraint::Ratio(1, 6),
            Constraint::Ratio(1, 6),
            Constraint::Ratio(1, 6),
            Constraint::Ratio(1, 6),
        ])
        .split(area);

    // Box 1: Total Indexed
    let indexed_text = vec![
        Line::from(Span::styled(" Indexed", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled(
            format!(" {}", format_number(total_indexed)),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
    ];
    let indexed_box = Paragraph::new(indexed_text)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(indexed_box, boxes[0]);

    // Box 2: Cached Trees
    let cached_text = vec![
        Line::from(Span::styled(" Cached", Style::default().fg(Color::DarkGray))),
        Line::from(vec![
            Span::styled(format!(" {}", cached_trees), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(format!("/{}", max_cached), Style::default().fg(Color::DarkGray)),
        ]),
    ];
    let cached_box = Paragraph::new(cached_text)
        .block(Block::default().borders(Borders::TOP | Borders::BOTTOM | Borders::RIGHT).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(cached_box, boxes[1]);

    // Box 3: Proofs Generated
    let gen_text = vec![
        Line::from(Span::styled(" Generated", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled(
            format!(" {}", format_number(proofs_generated)),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )),
    ];
    let gen_box = Paragraph::new(gen_text)
        .block(Block::default().borders(Borders::TOP | Borders::BOTTOM | Borders::RIGHT).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(gen_box, boxes[2]);

    // Box 4: Proofs Verified
    let ver_text = vec![
        Line::from(Span::styled(" Verified", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled(
            format!(" {}", format_number(proofs_verified)),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )),
    ];
    let ver_box = Paragraph::new(ver_text)
        .block(Block::default().borders(Borders::TOP | Borders::BOTTOM | Borders::RIGHT).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(ver_box, boxes[3]);

    // Box 5: Last Block Height
    let height_text = vec![
        Line::from(Span::styled(" Last Block", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled(
            format!(" #{}", last_block_height.map(|h| format_number(h)).unwrap_or_else(|| "-".to_string())),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
    ];
    let height_box = Paragraph::new(height_text)
        .block(Block::default().borders(Borders::TOP | Borders::BOTTOM | Borders::RIGHT).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(height_box, boxes[4]);

    // Box 6: Avg Tree Depth
    let depth_text = vec![
        Line::from(Span::styled(" Tree Depth", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled(
            format!(" {}", avg_tree_depth.map(|d| d.to_string()).unwrap_or_else(|| "-".to_string())),
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        )),
    ];
    let depth_box = Paragraph::new(depth_text)
        .block(Block::default().borders(Borders::TOP | Borders::BOTTOM | Borders::RIGHT).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(depth_box, boxes[5]);
}

/// Render the cache status gauge (INVARIANT-5 visualization).
fn render_cache_status(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let (_, cached_trees, max_cached, _, _, _, _) = extract_metrics(info);

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

/// Render the dependencies section without borders (clean style).
/// Per SPEC-03: subscribes qc-08, publishes qc-02, queries qc-02, serves qc-13/qc-16
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

    // Split into 2 horizontal sections: Inbound | Outbound
    let sections = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(inner);

    // Left: INBOUND (subscribes from qc-08)
    let inbound_text = vec![
        Line::from(Span::styled("INBOUND", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::raw("  ← qc-08 BlockValidated "),
            status_indicator(is_healthy),
        ]),
        Line::from(vec![
            Span::raw("  ↔ qc-02 GetTxHashes "),
            status_indicator(is_healthy),
        ]),
    ];
    let inbound_para = Paragraph::new(inbound_text);
    frame.render_widget(inbound_para, sections[0]);

    // Right: OUTBOUND (publishes to qc-02, serves qc-13/qc-16)
    let outbound_text = vec![
        Line::from(Span::styled("OUTBOUND", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::raw("  → qc-02 MerkleRootComputed "),
            status_indicator(is_healthy),
        ]),
        Line::from(vec![
            Span::raw("  → qc-13 MerkleProofs "),
            status_indicator(is_healthy),
        ]),
        Line::from(vec![
            Span::raw("  → qc-16 Admin/API "),
            status_indicator(is_healthy),
        ]),
    ];
    let outbound_para = Paragraph::new(outbound_text);
    frame.render_widget(outbound_para, sections[1]);
}

/// Create a status indicator span.
fn status_indicator(healthy: bool) -> Span<'static> {
    if healthy {
        Span::styled("● OK", Style::default().fg(Color::Green))
    } else {
        Span::styled("● DOWN", Style::default().fg(Color::Red))
    }
}

/// Extract metrics from subsystem info.
fn extract_metrics(info: &SubsystemInfo) -> (u64, usize, usize, u64, u64, Option<u64>, Option<u8>) {
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
        let last_block_height = metrics.get("last_block_height")
            .and_then(|v| v.as_u64());
        let avg_tree_depth = metrics.get("avg_tree_depth")
            .and_then(|v| v.as_u64())
            .map(|d| d as u8);

        (total_indexed, cached_trees, max_cached, proofs_generated, proofs_verified, last_block_height, avg_tree_depth)
    } else {
        (0, 0, 1000, 0, 0, None, None)
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
