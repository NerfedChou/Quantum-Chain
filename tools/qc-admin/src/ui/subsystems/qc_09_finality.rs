//! QC-09 Finality panel renderer.
//!
//! Displays:
//! - Overview: Last finalized epoch/block, finality depth, participation
//! - Circuit Breaker status (RUNNING → SYNC → HALTED)
//! - Checkpoint progression (Pending → Justified → Finalized)
//! - Dependency health (V2.3 Choreography pattern)

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::domain::SubsystemInfo;

/// Render the QC-09 Finality panel.
pub fn render(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    // Vertical layout: Overview, Circuit Breaker, Dependencies
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),  // Overview
            Constraint::Length(9),  // Circuit Breaker & Casper FFG
            Constraint::Min(10),    // Dependencies
        ])
        .split(area);

    render_overview(frame, chunks[0], info);
    render_circuit_breaker(frame, chunks[1], info);
    render_dependencies(frame, chunks[2], info);
}

/// Render the overview section.
fn render_overview(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let (last_finalized_epoch, last_finalized_block, finality_depth, 
         last_justified_epoch, participation_percent, _epochs_since_finality) = extract_metrics(info);

    let finality_status = if finality_depth == 0 {
        ("SYNCED", Color::Green)
    } else if finality_depth < 2 {
        ("HEALTHY", Color::Green)
    } else if finality_depth < 4 {
        ("LAGGING", Color::Yellow)
    } else {
        ("STALLED", Color::Red)
    };

    let text = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Last Finalized Epoch  "),
            Span::styled(
                format!("{:<8}", last_finalized_epoch),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Block Height      "),
            Span::styled(
                format!("{}", format_number(last_finalized_block)),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Last Justified Epoch  "),
            Span::styled(
                format!("{:<8}", last_justified_epoch),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw("  Finality Depth    "),
            Span::styled(
                format!("{} epochs", finality_depth),
                Style::default().fg(if finality_depth > 2 { Color::Red } else { Color::Green }),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Participation         "),
            Span::styled(
                format!("{:<8}%", participation_percent),
                if participation_percent >= 67 {
                    Style::default().fg(Color::Green)
                } else if participation_percent >= 50 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Red)
                },
            ),
            Span::raw("  Status            "),
            Span::styled(finality_status.0, Style::default().fg(finality_status.1).add_modifier(Modifier::BOLD)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Casper FFG: ", Style::default().fg(Color::DarkGray)),
            Span::raw("2/3 supermajority required for justification (67%)"),
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

/// Render the circuit breaker section.
fn render_circuit_breaker(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let (circuit_breaker_state, sync_attempts, _consecutive_failures, intervention_count) = 
        extract_circuit_breaker_metrics(info);

    let (state_label, state_color) = match circuit_breaker_state.as_str() {
        "running" | "ok" => ("RUNNING", Color::Green),
        "sync" => ("SYNC", Color::Yellow),
        "halted" | "halted_awaiting_intervention" => ("HALTED", Color::Red),
        _ => ("UNKNOWN", Color::DarkGray),
    };

    // State machine visualization
    let text = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw("  State: "),
            Span::styled(
                format!("{:<10}", state_label),
                Style::default().fg(state_color).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Sync Attempts: "),
            Span::styled(
                format!("{}/3", sync_attempts),
                if sync_attempts > 0 { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::Green) },
            ),
            Span::raw("  Interventions: "),
            Span::styled(
                format!("{}", intervention_count),
                if intervention_count > 0 { Style::default().fg(Color::Red) } else { Style::default().fg(Color::Green) },
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("[RUNNING]", if state_label == "RUNNING" { 
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD) 
            } else { 
                Style::default().fg(Color::DarkGray) 
            }),
            Span::styled(" ──fail──→ ", Style::default().fg(Color::DarkGray)),
            Span::styled("[SYNC {n}]", if state_label == "SYNC" { 
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) 
            } else { 
                Style::default().fg(Color::DarkGray) 
            }),
            Span::styled(" ──3 fails──→ ", Style::default().fg(Color::DarkGray)),
            Span::styled("[HALTED]", if state_label == "HALTED" { 
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD) 
            } else { 
                Style::default().fg(Color::DarkGray) 
            }),
        ]),
        Line::from(vec![
            Span::raw("      "),
            Span::styled("↑                       │", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("      "),
            Span::styled("└── sync success ───────┘", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let paragraph = Paragraph::new(text).block(
        Block::default()
            .title(" Circuit Breaker ")
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
            Span::raw("(AttestationBatch):"),
        ]),
        Line::from(vec![
            Span::raw("   ← qc-08 Consensus               "),
            status_indicator(is_healthy),
            Span::styled("  (validator attestations)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" PUBLISHES ", Style::default().fg(Color::DarkGray)),
            Span::raw("(Choreography):"),
        ]),
        Line::from(vec![
            Span::raw("   → qc-02 Block Storage           "),
            status_indicator(is_healthy),
            Span::styled("  (MarkFinalizedRequest)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("   → Enforcement                   "),
            status_indicator(is_healthy),
            Span::styled("  (SlashableOffenseDetected)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("   → Enforcement                   "),
            status_indicator(is_healthy),
            Span::styled("  (InactivityLeakTriggered)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" SERVES ", Style::default().fg(Color::DarkGray)),
            Span::raw("(FinalityProofRequest):"),
        ]),
        Line::from(vec![
            Span::raw("   → qc-15 Cross-Chain             "),
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
/// Returns (last_finalized_epoch, last_finalized_block, finality_depth, last_justified_epoch, participation_percent, epochs_since_finality)
fn extract_metrics(info: &SubsystemInfo) -> (u64, u64, u64, u64, u64, u64) {
    if let Some(metrics) = &info.metrics {
        let last_finalized_epoch = metrics.get("last_finalized_epoch")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let last_finalized_block = metrics.get("last_finalized_block")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let finality_depth = metrics.get("finality_depth")
            .or_else(|| metrics.get("finality_lag"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let last_justified_epoch = metrics.get("last_justified_epoch")
            .and_then(|v| v.as_u64())
            .unwrap_or(last_finalized_epoch);
        let participation_percent = metrics.get("participation_percent")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let epochs_since_finality = metrics.get("epochs_since_finality")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        (last_finalized_epoch, last_finalized_block, finality_depth, last_justified_epoch, participation_percent, epochs_since_finality)
    } else {
        (0, 0, 0, 0, 0, 0)
    }
}

/// Extract circuit breaker specific metrics.
/// Returns (state, sync_attempts, consecutive_failures, intervention_count)
fn extract_circuit_breaker_metrics(info: &SubsystemInfo) -> (String, u64, u64, u64) {
    if let Some(metrics) = &info.metrics {
        let state = metrics.get("circuit_breaker")
            .or_else(|| metrics.get("circuit_breaker_state"))
            .and_then(|v| v.as_str())
            .unwrap_or("ok")
            .to_string();
        let sync_attempts = metrics.get("sync_attempts")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let consecutive_failures = metrics.get("consecutive_failures")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let intervention_count = metrics.get("intervention_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        (state, sync_attempts, consecutive_failures, intervention_count)
    } else {
        ("ok".to_string(), 0, 0, 0)
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
