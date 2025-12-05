//! QC-04 State Management panel renderer.
//!
//! Displays:
//! - Overview: Accounts, contracts, state root, trie stats
//! - Patricia Merkle Trie health
//! - Dependency health (V2.3 Choreography pattern)

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::domain::SubsystemInfo;

/// Render the QC-04 State Management panel.
pub fn render(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    // Vertical layout: Overview, Trie Stats, Dependencies
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Overview
            Constraint::Length(8),  // Trie stats
            Constraint::Min(10),    // Dependencies
        ])
        .split(area);

    render_overview(frame, chunks[0], info);
    render_trie_stats(frame, chunks[1], info);
    render_dependencies(frame, chunks[2], info);
}

/// Render the overview section.
fn render_overview(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let (total_accounts, total_contracts, current_state_root, cache_size_mb, 
         proofs_generated, snapshots_count, pruning_depth) = extract_metrics(info);

    let text = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Total Accounts   "),
            Span::styled(
                format!("{:<12}", format_number(total_accounts)),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Contracts        "),
            Span::styled(
                format!("{}", format_number(total_contracts)),
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Proofs Generated "),
            Span::styled(
                format!("{:<12}", format_number(proofs_generated)),
                Style::default().fg(Color::Green),
            ),
            Span::raw("  Snapshots        "),
            Span::styled(
                format!("{}", snapshots_count),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Pruning Depth    "),
            Span::styled(
                format!("{:<12} blocks", pruning_depth),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw("  Cache Size       "),
            Span::styled(
                format!("{} MB", cache_size_mb),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Current State Root  "),
            Span::styled(
                if current_state_root.is_empty() {
                    "EMPTY_TRIE_ROOT".to_string()
                } else {
                    format!("0x{}...", &current_state_root[..16.min(current_state_root.len())])
                },
                Style::default().fg(Color::Cyan),
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

/// Render the Patricia Merkle Trie stats section.
fn render_trie_stats(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let (_, _, _, _, _, _, _) = extract_metrics(info);
    
    // Extract trie-specific metrics
    let (trie_depth, trie_nodes, storage_slots) = extract_trie_metrics(info);

    let text = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Trie Depth       "),
            Span::styled(
                format!("{:<4}", trie_depth),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " / 64 max",
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw("       Trie Nodes     "),
            Span::styled(
                format_number(trie_nodes),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Storage Slots    "),
            Span::styled(
                format!("{:<12}", format_number(storage_slots)),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw("  Hash Algorithm   "),
            Span::styled(
                "Keccak-256",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                "  INVARIANT-3: Deterministic Root | INVARIANT-4: Proof Validity",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(text).block(
        Block::default()
            .title(" Patricia Merkle Trie ")
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
            Span::styled(" SUBSCRIBES TO ", Style::default().fg(Color::DarkGray)),
            Span::raw("(Choreography - BlockValidated):"),
        ]),
        Line::from(vec![
            Span::raw("   ← qc-08 Consensus               "),
            status_indicator(is_healthy),
            Span::styled("  (triggers state transition)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" PUBLISHES ", Style::default().fg(Color::DarkGray)),
            Span::raw("(Choreography - StateRootComputed):"),
        ]),
        Line::from(vec![
            Span::raw("   → qc-02 Block Storage           "),
            status_indicator(is_healthy),
            Span::styled("  (completes block assembly)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" SERVES ", Style::default().fg(Color::DarkGray)),
            Span::raw("(API Queries via qc-16):"),
        ]),
        Line::from(vec![
            Span::raw("   → eth_getBalance                "),
            status_indicator(is_healthy),
        ]),
        Line::from(vec![
            Span::raw("   → eth_getCode                   "),
            status_indicator(is_healthy),
        ]),
        Line::from(vec![
            Span::raw("   → eth_getStorageAt              "),
            status_indicator(is_healthy),
        ]),
        Line::from(vec![
            Span::raw("   → eth_getProof                  "),
            status_indicator(is_healthy),
            Span::styled("  (light client proofs)", Style::default().fg(Color::DarkGray)),
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
/// Returns (total_accounts, total_contracts, current_state_root, cache_size_mb, proofs_generated, snapshots_count, pruning_depth)
fn extract_metrics(info: &SubsystemInfo) -> (u64, u64, String, u64, u64, u64, u64) {
    if let Some(metrics) = &info.metrics {
        let total_accounts = metrics.get("total_accounts")
            .or_else(|| metrics.get("accounts"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let total_contracts = metrics.get("total_contracts")
            .or_else(|| metrics.get("contracts"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let current_state_root = metrics.get("current_state_root")
            .or_else(|| metrics.get("state_root"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let cache_size_mb = metrics.get("cache_size_mb")
            .and_then(|v| v.as_u64())
            .unwrap_or(512);
        let proofs_generated = metrics.get("proofs_generated")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let snapshots_count = metrics.get("snapshots_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let pruning_depth = metrics.get("pruning_depth")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);

        (total_accounts, total_contracts, current_state_root, cache_size_mb, proofs_generated, snapshots_count, pruning_depth)
    } else {
        (0, 0, String::new(), 512, 0, 0, 1000)
    }
}

/// Extract trie-specific metrics.
/// Returns (trie_depth, trie_nodes, storage_slots)
fn extract_trie_metrics(info: &SubsystemInfo) -> (u64, u64, u64) {
    if let Some(metrics) = &info.metrics {
        let trie_depth = metrics.get("trie_depth")
            .or_else(|| metrics.get("max_depth"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let trie_nodes = metrics.get("trie_nodes")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let storage_slots = metrics.get("storage_slots")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        (trie_depth, trie_nodes, storage_slots)
    } else {
        (0, 0, 0)
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
