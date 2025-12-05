//! QC-02 Block Storage panel renderer.
//!
//! Displays:
//! - Overview: Latest block, finalized block, disk usage, pending assemblies
//! - Assembly Status (Stateful Assembler pattern)
//! - Dependency health (Choreography pattern)

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame,
};

use crate::domain::{App, SubsystemInfo};

/// Render the QC-02 Block Storage panel.
pub fn render(frame: &mut Frame, area: Rect, info: &SubsystemInfo, app: &App) {
    // Vertical layout: Overview, Assembly Status, Dependencies
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),  // Overview
            Constraint::Min(8),     // Assembly status table
            Constraint::Length(12), // Dependencies (choreography)
        ])
        .split(area);

    render_overview(frame, chunks[0], info);
    render_assembly_status(frame, chunks[1], app);
    render_dependencies(frame, chunks[2], info);
}

/// Render the overview section.
fn render_overview(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let (latest_block, finalized_block, _total_blocks, disk_used, disk_total, pending_assemblies, assembly_timeout) =
        extract_metrics(info);

    let disk_percent = if disk_total > 0 {
        (disk_used as f64 / disk_total as f64 * 100.0) as u32
    } else {
        0
    };

    let disk_color = if disk_percent > 90 {
        Color::Red
    } else if disk_percent > 75 {
        Color::Yellow
    } else {
        Color::Green
    };

    let text = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Latest Block     "),
            Span::styled(
                format!("#{:<12}", format_number(latest_block)),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Chain Height   "),
            Span::styled(
                format!("{}", format_number(latest_block)),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Finalized Block  "),
            Span::styled(
                format!("#{:<12}", format_number(finalized_block)),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Disk Usage     "),
            Span::styled(
                format!("{} / {} GB", format_size_gb(disk_used), format_size_gb(disk_total)),
                Style::default().fg(disk_color),
            ),
            Span::styled(
                format!(" ({}%)", disk_percent),
                Style::default().fg(disk_color),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Pending Assembly "),
            Span::styled(
                format!("{:<2}", pending_assemblies),
                if pending_assemblies > 0 {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
            Span::raw("             Assembly Timeout  "),
            Span::styled(
                format!("{}s", assembly_timeout),
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

/// Render the assembly status table (Stateful Assembler pattern).
fn render_assembly_status(frame: &mut Frame, area: Rect, app: &App) {
    // Header row
    let header = Row::new(vec![
        "Block Hash",
        "BlockValidated",
        "MerkleRoot",
        "StateRoot",
        "Status",
    ])
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
    .bottom_margin(1);

    // Build rows from pending assemblies
    let rows: Vec<Row> = if app.pending_assemblies.is_empty() {
        vec![Row::new(vec![
            "(No pending assemblies)".to_string(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ])
        .style(Style::default().fg(Color::DarkGray))]
    } else {
        app.pending_assemblies
            .iter()
            .take(5) // Limit to 5
            .map(|assembly| {
                let block_hash = format!("0x{}...", &assembly.block_hash[..8]);
                let has_block = if assembly.has_block { "✓" } else { "○" };
                let has_merkle = if assembly.has_merkle { "✓" } else { "○" };
                let has_state = if assembly.has_state { "✓" } else { "○" };
                let status = if assembly.has_block && assembly.has_merkle && assembly.has_state {
                    "READY"
                } else {
                    "WAITING"
                };

                Row::new(vec![
                    block_hash,
                    has_block.to_string(),
                    has_merkle.to_string(),
                    has_state.to_string(),
                    status.to_string(),
                ])
                .style(Style::default().fg(Color::White))
            })
            .collect()
    };

    let widths = [
        Constraint::Length(14),
        Constraint::Length(14),
        Constraint::Length(11),
        Constraint::Length(10),
        Constraint::Length(8),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .title(format!(" Assembly Status (Stateful Assembler) ({}) ", app.pending_assemblies.len()))
                .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );

    frame.render_widget(table, area);
}

/// Render the dependencies section (V2.3 Choreography pattern).
fn render_dependencies(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    // Check if subsystems are healthy based on info
    let is_healthy = matches!(info.status, crate::domain::SubsystemStatus::Running);
    let consensus_healthy = is_healthy;
    let tx_indexing_healthy = is_healthy; // Placeholder - would check actual status
    let state_mgmt_healthy = is_healthy;
    let mempool_healthy = is_healthy;
    let finality_healthy = is_healthy;

    let text = vec![
        Line::from(vec![
            Span::styled(" SUBSCRIBES TO ", Style::default().fg(Color::DarkGray)),
            Span::raw("(Choreography - Write Path):"),
        ]),
        Line::from(vec![
            Span::raw("   ← qc-08 BlockValidated          "),
            status_indicator(consensus_healthy),
        ]),
        Line::from(vec![
            Span::raw("   ← qc-03 MerkleRootComputed      "),
            status_indicator(tx_indexing_healthy),
        ]),
        Line::from(vec![
            Span::raw("   ← qc-04 StateRootComputed       "),
            status_indicator(state_mgmt_healthy),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" PROVIDES TO ", Style::default().fg(Color::DarkGray)),
            Span::raw("(V2.3 Data Retrieval):"),
        ]),
        Line::from(vec![
            Span::raw("   → qc-06 BlockStorageConfirmation "),
            status_indicator(mempool_healthy),
        ]),
        Line::from(vec![
            Span::raw("   → qc-03 TransactionHashesResponse"),
            status_indicator(tx_indexing_healthy),
        ]),
        Line::from(vec![
            Span::raw("   → qc-09 MarkFinalized           "),
            status_indicator(finality_healthy),
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
/// Returns (latest_block, finalized_block, total_blocks, disk_used_bytes, disk_total_bytes, pending_assemblies, assembly_timeout)
fn extract_metrics(info: &SubsystemInfo) -> (u64, u64, u64, u64, u64, u32, u32) {
    if let Some(metrics) = &info.metrics {
        let latest_block = metrics.get("latest_height")
            .or_else(|| metrics.get("latest_block"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let finalized_block = metrics.get("finalized_height")
            .or_else(|| metrics.get("finalized_block"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let total_blocks = metrics.get("total_blocks")
            .and_then(|v| v.as_u64())
            .unwrap_or(latest_block.saturating_add(1));
        let disk_used = metrics.get("disk_used_bytes")
            .or_else(|| metrics.get("disk_usage"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let disk_total = metrics.get("disk_total_bytes")
            .or_else(|| metrics.get("disk_capacity_bytes"))
            .or_else(|| metrics.get("disk_capacity"))
            .and_then(|v| v.as_u64())
            .unwrap_or(500 * 1024 * 1024 * 1024); // Default 500GB
        let pending_assemblies = metrics.get("pending_assemblies")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let assembly_timeout = metrics.get("assembly_timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(30) as u32;

        (latest_block, finalized_block, total_blocks, disk_used, disk_total, pending_assemblies, assembly_timeout)
    } else {
        // Default/placeholder values
        (0, 0, 0, 0, 500 * 1024 * 1024 * 1024, 0, 30)
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

/// Format bytes as GB.
fn format_size_gb(bytes: u64) -> String {
    let gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    format!("{:.1}", gb)
}
