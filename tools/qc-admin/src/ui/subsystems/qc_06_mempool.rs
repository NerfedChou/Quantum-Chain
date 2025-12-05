//! QC-06 Mempool panel renderer.
//!
//! Displays:
//! - Overview: Pending txs, gas usage, memory, RBF stats
//! - Two-Phase Commit status (PENDING → PENDING_INCLUSION → CONFIRMED)
//! - Dependency health (V2.3 Choreography pattern)

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::domain::SubsystemInfo;

/// Render the QC-06 Mempool panel.
pub fn render(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    // Vertical layout: Overview, Two-Phase Commit, Dependencies
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Overview
            Constraint::Length(7),  // Two-Phase Commit gauge
            Constraint::Min(10),    // Dependencies
        ])
        .split(area);

    render_overview(frame, chunks[0], info);
    render_two_phase_commit(frame, chunks[1], info);
    render_dependencies(frame, chunks[2], info);
}

/// Render the overview section.
fn render_overview(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let (pending_count, pending_inclusion, total_gas, memory_bytes, oldest_tx_age_ms,
         max_transactions, max_per_account, min_gas_price_gwei) = extract_metrics(info);

    let pool_usage = if max_transactions > 0 {
        ((pending_count + pending_inclusion) as f64 / max_transactions as f64 * 100.0) as u32
    } else {
        0
    };

    let text = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Pending Txs      "),
            Span::styled(
                format!("{:<10}", format_number(pending_count)),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Pending Inclusion "),
            Span::styled(
                format!("{}", format_number(pending_inclusion)),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Total Gas        "),
            Span::styled(
                format!("{:<10}", format_gas(total_gas)),
                Style::default().fg(Color::Green),
            ),
            Span::raw("  Memory Usage      "),
            Span::styled(
                format_bytes(memory_bytes),
                Style::default().fg(Color::Magenta),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Oldest Tx Age    "),
            Span::styled(
                format!("{:<10}", format_duration(oldest_tx_age_ms)),
                if oldest_tx_age_ms > 60000 {
                    Style::default().fg(Color::Red)
                } else if oldest_tx_age_ms > 30000 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
            Span::raw("  Pool Usage        "),
            Span::styled(
                format!("{}%", pool_usage),
                if pool_usage > 90 {
                    Style::default().fg(Color::Red)
                } else if pool_usage > 75 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Max Txs: "),
            Span::styled(format!("{}", max_transactions), Style::default().fg(Color::DarkGray)),
            Span::raw("  |  Max/Account: "),
            Span::styled(format!("{}", max_per_account), Style::default().fg(Color::DarkGray)),
            Span::raw("  |  Min Gas: "),
            Span::styled(format!("{} gwei", min_gas_price_gwei), Style::default().fg(Color::DarkGray)),
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

/// Render the Two-Phase Commit status section.
fn render_two_phase_commit(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let (pending_count, pending_inclusion, _, _, _, _max_transactions, _, _) = extract_metrics(info);

    // State machine visualization
    let text = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("[PENDING]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(" ──propose──→ ", Style::default().fg(Color::DarkGray)),
            Span::styled("[PENDING_INCLUSION]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(" ──confirm──→ ", Style::default().fg(Color::DarkGray)),
            Span::styled("[DELETED]", Style::default().fg(Color::Red)),
        ]),
        Line::from(vec![
            Span::raw("                              "),
            Span::styled("│", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("                              "),
            Span::styled("└── timeout/reject ──→ [PENDING]", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let paragraph = Paragraph::new(text).block(
        Block::default()
            .title(format!(
                " Two-Phase Commit ({} pending, {} in-flight) ",
                pending_count, pending_inclusion
            ))
            .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(paragraph, area);
}

/// Render the dependencies section (V2.3 Choreography pattern).
fn render_dependencies(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let is_healthy = matches!(info.status, crate::domain::SubsystemStatus::Running);

    let text = vec![
        Line::from(vec![
            Span::styled(" RECEIVES FROM ", Style::default().fg(Color::DarkGray)),
            Span::raw("(AddTransactionRequest):"),
        ]),
        Line::from(vec![
            Span::raw("   ← qc-10 Signature Verification  "),
            status_indicator(is_healthy),
            Span::styled("  (verified transactions)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" SERVES ", Style::default().fg(Color::DarkGray)),
            Span::raw("(GetTransactionsRequest):"),
        ]),
        Line::from(vec![
            Span::raw("   → qc-08 Consensus               "),
            status_indicator(is_healthy),
            Span::styled("  (txs for block proposal)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("   → qc-05 Block Propagation       "),
            status_indicator(is_healthy),
            Span::styled("  (compact block lookup)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" CONFIRMS FROM ", Style::default().fg(Color::DarkGray)),
            Span::raw("(BlockStorageConfirmation):"),
        ]),
        Line::from(vec![
            Span::raw("   ← qc-02 Block Storage           "),
            status_indicator(is_healthy),
            Span::styled("  (delete confirmed txs)", Style::default().fg(Color::DarkGray)),
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
/// Returns (pending_count, pending_inclusion, total_gas, memory_bytes, oldest_tx_age_ms, max_transactions, max_per_account, min_gas_price_gwei)
fn extract_metrics(info: &SubsystemInfo) -> (u64, u64, u64, u64, u64, u64, u64, u64) {
    if let Some(metrics) = &info.metrics {
        let pending_count = metrics.get("pending_count")
            .or_else(|| metrics.get("pending_txs"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let pending_inclusion = metrics.get("pending_inclusion_count")
            .or_else(|| metrics.get("pending_inclusion"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let total_gas = metrics.get("total_gas")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let memory_bytes = metrics.get("memory_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let oldest_tx_age_ms = metrics.get("oldest_tx_age_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let max_transactions = metrics.get("max_transactions")
            .and_then(|v| v.as_u64())
            .unwrap_or(5000);
        let max_per_account = metrics.get("max_per_account")
            .and_then(|v| v.as_u64())
            .unwrap_or(16);
        let min_gas_price_gwei = metrics.get("min_gas_price_gwei")
            .and_then(|v| v.as_u64())
            .unwrap_or(1);

        (pending_count, pending_inclusion, total_gas, memory_bytes, oldest_tx_age_ms, max_transactions, max_per_account, min_gas_price_gwei)
    } else {
        (0, 0, 0, 0, 0, 5000, 16, 1)
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

/// Format gas as readable string (e.g., "1.2M", "500K").
fn format_gas(gas: u64) -> String {
    if gas >= 1_000_000_000 {
        format!("{:.1}B", gas as f64 / 1_000_000_000.0)
    } else if gas >= 1_000_000 {
        format!("{:.1}M", gas as f64 / 1_000_000.0)
    } else if gas >= 1_000 {
        format!("{:.1}K", gas as f64 / 1_000.0)
    } else {
        gas.to_string()
    }
}

/// Format bytes as readable string (e.g., "1.2 MB", "500 KB").
fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1_024 {
        format!("{:.1} KB", bytes as f64 / 1_024.0)
    } else {
        format!("{} B", bytes)
    }
}

/// Format duration in milliseconds as readable string.
fn format_duration(ms: u64) -> String {
    if ms >= 60_000 {
        format!("{:.1} min", ms as f64 / 60_000.0)
    } else if ms >= 1_000 {
        format!("{:.1} sec", ms as f64 / 1_000.0)
    } else {
        format!("{} ms", ms)
    }
}
