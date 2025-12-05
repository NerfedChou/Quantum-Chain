//! QC-08 Consensus panel renderer.
//!
//! Displays:
//! - Overview: Algorithm mode, validators, current epoch/slot, attestations
//! - Chain head status and validation stats
//! - Dependency health (V2.3 Choreography pattern)

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::domain::SubsystemInfo;

/// Render the QC-08 Consensus panel.
pub fn render(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    // Vertical layout: Overview, Chain Status, Dependencies
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Overview
            Constraint::Length(8),  // Chain/Validation status
            Constraint::Min(10),    // Dependencies
        ])
        .split(area);

    render_overview(frame, chunks[0], info);
    render_chain_status(frame, chunks[1], info);
    render_dependencies(frame, chunks[2], info);
}

/// Render the overview section.
fn render_overview(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let (mode, validators, current_epoch, current_slot, attestations, 
         blocks_validated, validation_failures, min_attestation_percent) = extract_metrics(info);

    let mode_color = match mode.as_str() {
        "PoS" => Color::Green,
        "PBFT" => Color::Cyan,
        _ => Color::Yellow,
    };

    let text = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Algorithm        "),
            Span::styled(
                format!("{:<10}", mode),
                Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Validators        "),
            Span::styled(
                format!("{}", validators),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Current Epoch    "),
            Span::styled(
                format!("{:<10}", current_epoch),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw("  Current Slot      "),
            Span::styled(
                format!("{}", current_slot),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Attestations     "),
            Span::styled(
                format!("{:<10}", attestations),
                Style::default().fg(Color::Green),
            ),
            Span::raw("  Min Threshold     "),
            Span::styled(
                format!("{}%", min_attestation_percent),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Blocks Validated "),
            Span::styled(
                format!("{:<10}", format_number(blocks_validated)),
                Style::default().fg(Color::Green),
            ),
            Span::raw("  Failures          "),
            Span::styled(
                format!("{}", validation_failures),
                if validation_failures > 0 {
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

/// Render the chain status section.
fn render_chain_status(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let (chain_height, head_hash, total_stake, pending_proposals) = extract_chain_metrics(info);

    let text = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Chain Height     "),
            Span::styled(
                format!("{:<12}", format_number(chain_height)),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Pending Proposals "),
            Span::styled(
                format!("{}", pending_proposals),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Total Stake      "),
            Span::styled(
                format_stake(total_stake),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Head Hash        "),
            Span::styled(
                if head_hash.is_empty() {
                    "N/A".to_string()
                } else {
                    format!("0x{}...", &head_hash[..16.min(head_hash.len())])
                },
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(text).block(
        Block::default()
            .title(" Chain Status ")
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
            Span::styled(" PUBLISHES ", Style::default().fg(Color::DarkGray)),
            Span::raw("(Choreography - BlockValidated):"),
        ]),
        Line::from(vec![
            Span::raw("   → qc-02 Block Storage           "),
            status_indicator(is_healthy),
            Span::styled("  (assembler awaits)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("   → qc-03 Transaction Indexing    "),
            status_indicator(is_healthy),
            Span::styled("  (compute merkle root)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("   → qc-04 State Management        "),
            status_indicator(is_healthy),
            Span::styled("  (compute state root)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" QUERIES ", Style::default().fg(Color::DarkGray)),
            Span::raw("(Block Proposal):"),
        ]),
        Line::from(vec![
            Span::raw("   ← qc-06 Mempool                 "),
            status_indicator(is_healthy),
            Span::styled("  (get transactions)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("   ← qc-10 Signature Verification  "),
            status_indicator(is_healthy),
            Span::styled("  (verify attestations)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("   → qc-05 Block Propagation       "),
            status_indicator(is_healthy),
            Span::styled("  (gossip validated block)", Style::default().fg(Color::DarkGray)),
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
/// Returns (mode, validators, current_epoch, current_slot, attestations, blocks_validated, validation_failures, min_attestation_percent)
fn extract_metrics(info: &SubsystemInfo) -> (String, u64, u64, u64, u64, u64, u64, u64) {
    if let Some(metrics) = &info.metrics {
        let mode = metrics.get("mode")
            .or_else(|| metrics.get("algorithm"))
            .and_then(|v| v.as_str())
            .unwrap_or("PoS")
            .to_string();
        let validators = metrics.get("validators")
            .or_else(|| metrics.get("validator_count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let current_epoch = metrics.get("current_epoch")
            .or_else(|| metrics.get("epoch"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let current_slot = metrics.get("current_slot")
            .or_else(|| metrics.get("slot"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let attestations = metrics.get("attestations")
            .or_else(|| metrics.get("attestation_count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let blocks_validated = metrics.get("blocks_validated")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let validation_failures = metrics.get("validation_failures")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let min_attestation_percent = metrics.get("min_attestation_percent")
            .and_then(|v| v.as_u64())
            .unwrap_or(67);

        (mode, validators, current_epoch, current_slot, attestations, blocks_validated, validation_failures, min_attestation_percent)
    } else {
        ("PoS".to_string(), 0, 0, 0, 0, 0, 0, 67)
    }
}

/// Extract chain-specific metrics.
/// Returns (chain_height, head_hash, total_stake, pending_proposals)
fn extract_chain_metrics(info: &SubsystemInfo) -> (u64, String, u128, u64) {
    if let Some(metrics) = &info.metrics {
        let chain_height = metrics.get("chain_height")
            .or_else(|| metrics.get("current_round"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let head_hash = metrics.get("head_hash")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let total_stake = metrics.get("total_stake")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u128;
        let pending_proposals = metrics.get("pending_proposals")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        (chain_height, head_hash, total_stake, pending_proposals)
    } else {
        (0, String::new(), 0, 0)
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

/// Format stake as readable string (e.g., "1.2M ETH").
fn format_stake(stake: u128) -> String {
    // Assume stake is in wei, convert to ETH-like display
    let eth = stake as f64 / 1_000_000_000_000_000_000.0;
    if eth >= 1_000_000.0 {
        format!("{:.1}M", eth / 1_000_000.0)
    } else if eth >= 1_000.0 {
        format!("{:.1}K", eth / 1_000.0)
    } else if eth >= 1.0 {
        format!("{:.1}", eth)
    } else {
        format!("{} wei", stake)
    }
}
