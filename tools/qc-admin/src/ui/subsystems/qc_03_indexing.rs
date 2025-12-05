//! QC-03 Transaction Indexing panel renderer.
//!
//! Displays:
//! - Overview: Total indexed, cached trees, proofs generated/verified (collapsed table style)
//! - Merkle Tree Cache status (INVARIANT-5)
//! - Sync Metrics: Head Lag, Sync Speed, E2E Latency
//! - Traffic Pattern: Tx count per block (bar chart)
//! - Dependency health (V2.2 Choreography pattern)

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Bar, BarChart, BarGroup, Block, Borders, Gauge, Paragraph},
    Frame,
};

use crate::domain::SubsystemInfo;

/// Render the QC-03 Transaction Indexing panel.
pub fn render(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    // Vertical layout: Overview, Cache Status, Sync Metrics, Traffic Pattern, Dependencies
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),   // Overview (collapsed table)
            Constraint::Length(5),   // Cache gauge
            Constraint::Length(4),   // Sync metrics (Head Lag, Sync Speed, Latency)
            Constraint::Min(10),     // Traffic Pattern bar chart (fills remaining space)
            Constraint::Length(6),   // Dependencies (compact)
        ])
        .split(area);

    render_overview(frame, chunks[0], info);
    render_cache_status(frame, chunks[1], info);
    render_sync_metrics(frame, chunks[2], info);
    render_traffic_pattern(frame, chunks[3], info);
    render_dependencies(frame, chunks[4], info);
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

/// Render sync metrics: Head Lag, Sync Speed, End-to-End Latency (horizontal with vertical separators).
fn render_sync_metrics(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let (head_lag, sync_speed, e2e_latency) = extract_sync_metrics(info);

    // 3 columns with vertical line separators
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(area);

    // Head Lag (blocks behind)
    let lag_color = if head_lag == 0 {
        Color::Green
    } else if head_lag <= 2 {
        Color::Yellow
    } else {
        Color::Red
    };
    let lag_text = vec![
        Line::from(Span::styled(" Head Lag", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled(
            format!(" {} blocks", head_lag),
            Style::default().fg(lag_color).add_modifier(Modifier::BOLD),
        )),
    ];
    let lag_para = Paragraph::new(lag_text)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(lag_para, cols[0]);

    // Sync Speed (blocks/sec)
    let speed_color = if sync_speed >= 100.0 {
        Color::Green
    } else if sync_speed >= 10.0 {
        Color::Yellow
    } else {
        Color::Red
    };
    let speed_text = vec![
        Line::from(Span::styled(" Sync Speed", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled(
            format!(" {:.1} blk/s", sync_speed),
            Style::default().fg(speed_color).add_modifier(Modifier::BOLD),
        )),
    ];
    let speed_para = Paragraph::new(speed_text)
        .block(Block::default().borders(Borders::TOP | Borders::BOTTOM | Borders::RIGHT).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(speed_para, cols[1]);

    // End-to-End Latency (ms)
    let latency_color = if e2e_latency <= 1000 {
        Color::Green
    } else if e2e_latency <= 5000 {
        Color::Yellow
    } else {
        Color::Red
    };
    let latency_text = vec![
        Line::from(Span::styled(" E2E Latency", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled(
            format!(" {}ms", e2e_latency),
            Style::default().fg(latency_color).add_modifier(Modifier::BOLD),
        )),
    ];
    let latency_para = Paragraph::new(latency_text)
        .block(Block::default().borders(Borders::TOP | Borders::BOTTOM | Borders::RIGHT).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(latency_para, cols[2]);
}

/// Render the Traffic Pattern bar chart showing tx count per block.
fn render_traffic_pattern(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let block_tx_counts = extract_traffic_data(info);

    // Create bars for each block
    let bars: Vec<Bar> = block_tx_counts
        .iter()
        .map(|(block_num, tx_count)| {
            let color = if *tx_count > 200 {
                Color::Red
            } else if *tx_count > 100 {
                Color::Yellow
            } else {
                Color::Green
            };
            Bar::default()
                .value(*tx_count)
                .label(Line::from(format!("#{}", block_num % 1000)))
                .style(Style::default().fg(color))
        })
        .collect();

    let bar_chart = BarChart::default()
        .block(
            Block::default()
                .title(" Traffic Pattern (Tx/Block) ")
                .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .data(BarGroup::default().bars(&bars))
        .bar_width(5)
        .bar_gap(1)
        .bar_style(Style::default().fg(Color::Cyan))
        .value_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    frame.render_widget(bar_chart, area);
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

/// Extract sync metrics: Head Lag, Sync Speed, End-to-End Latency.
fn extract_sync_metrics(info: &SubsystemInfo) -> (u64, f64, u64) {
    if let Some(metrics) = &info.metrics {
        let head_lag = metrics.get("head_lag")
            .or_else(|| metrics.get("block_lag"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let sync_speed = metrics.get("sync_speed")
            .or_else(|| metrics.get("blocks_per_second"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let e2e_latency = metrics.get("e2e_latency_ms")
            .or_else(|| metrics.get("end_to_end_latency"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        (head_lag, sync_speed, e2e_latency)
    } else {
        (0, 0.0, 0)
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

/// Extract traffic pattern data: (block_number, tx_count) for last N blocks.
fn extract_traffic_data(info: &SubsystemInfo) -> Vec<(u64, u64)> {
    if let Some(metrics) = &info.metrics {
        // Try to get block_tx_counts array from metrics
        if let Some(traffic) = metrics.get("block_tx_counts") {
            if let Some(arr) = traffic.as_array() {
                return arr
                    .iter()
                    .filter_map(|v| {
                        let block_num = v.get("block")?.as_u64()?;
                        let tx_count = v.get("tx_count")?.as_u64()?;
                        Some((block_num, tx_count))
                    })
                    .collect();
            }
        }

        // Fallback: generate from last_block_height with simulated data
        if let Some(last_height) = metrics.get("last_block_height").and_then(|v| v.as_u64()) {
            let mut data = Vec::new();
            for i in 0..15 {
                let block_num = last_height.saturating_sub(14 - i);
                // Simulated tx count based on block number (replace with real data when available)
                let tx_count = ((block_num * 7 + 13) % 150) + 20;
                data.push((block_num, tx_count));
            }
            return data;
        }
    }

    // Default empty data - shows placeholder blocks
    (0..15).map(|i| (1000 + i, 0)).collect()
}
