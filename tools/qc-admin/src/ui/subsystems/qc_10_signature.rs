//! QC-10 Signature Verification panel renderer.
//!
//! The most detailed panel - displays:
//! - Overview: Verification counts (ECDSA, BLS), cache stats, malleability rejections
//! - Security Boundaries: Authorized consumers with rate limits (per IPC-MATRIX.md)
//! - Algorithm Details: secp256k1 (ECDSA) and BLS12-381 curves
//! - Zero-Trust reminder for consensus-critical paths
//! - Dependency health (V2.3 Choreography pattern)

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame,
};

use crate::domain::SubsystemInfo;

/// Render the QC-10 Signature Verification panel.
pub fn render(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    // Vertical layout: Overview, Security Boundaries, Algorithm Stats, Dependencies
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),  // Overview
            Constraint::Length(11), // Security Boundaries (IPC-MATRIX)
            Constraint::Length(8),  // Algorithm Stats
            Constraint::Min(8),     // Dependencies
        ])
        .split(area);

    render_overview(frame, chunks[0], info);
    render_security_boundaries(frame, chunks[1], info);
    render_algorithm_stats(frame, chunks[2], info);
    render_dependencies(frame, chunks[3], info);
}

/// Render the overview section.
fn render_overview(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let metrics = extract_metrics(info);

    let text = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw("  ECDSA Verifications  "),
            Span::styled(
                format!("{:<12}", format_number(metrics.ecdsa_verifications)),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  BLS Verifications   "),
            Span::styled(
                format!("{}", format_number(metrics.bls_verifications)),
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Batch Verifications  "),
            Span::styled(
                format!("{:<12}", format_number(metrics.batch_verifications)),
                Style::default().fg(Color::Green),
            ),
            Span::raw("  Cache Hit Rate      "),
            Span::styled(
                format!("{:.1}%", metrics.cache_hit_rate),
                if metrics.cache_hit_rate > 80.0 {
                    Style::default().fg(Color::Green)
                } else if metrics.cache_hit_rate > 50.0 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Red)
                },
            ),
        ]),
        Line::from(vec![
            Span::raw("  Malleability Rejects "),
            Span::styled(
                format!("{:<12}", metrics.malleability_rejections),
                if metrics.malleability_rejections > 0 {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
            Span::raw("  Rate Limit Hits     "),
            Span::styled(
                format!("{}", metrics.rate_limit_hits),
                if metrics.rate_limit_hits > 0 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Cache Size: "),
            Span::styled(format!("{}", format_number(metrics.cache_size)), Style::default().fg(Color::DarkGray)),
            Span::raw("  |  Avg Latency: "),
            Span::styled(format!("{:.2}μs", metrics.avg_latency_us), Style::default().fg(Color::DarkGray)),
            Span::raw("  |  EIP-2: "),
            Span::styled("ENFORCED", Style::default().fg(Color::Green)),
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

/// Render the security boundaries section (per IPC-MATRIX.md).
fn render_security_boundaries(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let metrics = extract_metrics(info);

    let text = vec![
        Line::from(vec![
            Span::styled(" AUTHORIZED CONSUMERS ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled("(IPC-MATRIX.md Subsystem 10)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("   qc-01 Peer Discovery       "),
            Span::styled("100/sec ", Style::default().fg(Color::Yellow)),
            rate_bar(metrics.rate_qc01, 100),
            Span::styled("  VerifyNodeIdentity ONLY", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("   qc-05 Block Propagation    "),
            Span::styled("1K/sec  ", Style::default().fg(Color::Cyan)),
            rate_bar(metrics.rate_qc05, 1000),
            Span::styled("  VerifySignatureRequest", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("   qc-06 Mempool              "),
            Span::styled("1K/sec  ", Style::default().fg(Color::Cyan)),
            rate_bar(metrics.rate_qc06, 1000),
            Span::styled("  VerifyTransactionRequest", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("   qc-08 Consensus            "),
            Span::styled("∞       ", Style::default().fg(Color::Green)),
            Span::styled("████████", Style::default().fg(Color::Green)),
            Span::styled("  ALL + BatchVerify", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("   qc-09 Finality             "),
            Span::styled("∞       ", Style::default().fg(Color::Green)),
            Span::styled("████████", Style::default().fg(Color::Green)),
            Span::styled("  VerifySignatureRequest", Style::default().fg(Color::DarkGray)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" FORBIDDEN: ", Style::default().fg(Color::Red)),
            Span::styled("qc-02, qc-03, qc-04, qc-07, qc-11..qc-15", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let paragraph = Paragraph::new(text).block(
        Block::default()
            .title(" Security Boundaries ")
            .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(paragraph, area);
}

/// Render the algorithm statistics section.
fn render_algorithm_stats(frame: &mut Frame, area: Rect, _info: &SubsystemInfo) {
    let text = vec![
        Line::raw(""),
        Line::from(vec![
            Span::styled(" ECDSA ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("(secp256k1)", Style::default().fg(Color::DarkGray)),
            Span::raw("                    "),
            Span::styled(" BLS ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::styled("(BLS12-381 G1/G2)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("   Sig Size: 65 bytes (r,s,v)      "),
            Span::raw("   Sig Size: 48 bytes (G1 compressed)"),
        ]),
        Line::from(vec![
            Span::raw("   PubKey: 65 bytes (uncompressed) "),
            Span::raw("   PubKey: 96 bytes (G2 compressed)"),
        ]),
        Line::from(vec![
            Span::raw("   Recovery: "),
            Span::styled("YES", Style::default().fg(Color::Green)),
            Span::raw(" (v=27/28)          "),
            Span::raw("   Aggregation: "),
            Span::styled("YES", Style::default().fg(Color::Green)),
            Span::raw(" (multi-sig)"),
        ]),
        Line::raw(""),
    ];

    let paragraph = Paragraph::new(text).block(
        Block::default()
            .title(" Algorithm Details ")
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
            Span::styled(" ⚠️  ZERO-TRUST WARNING ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled("(IPC-MATRIX.md)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("   qc-08 and qc-09 MUST re-verify signatures before consensus/finality decisions"),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" FORWARDS VERIFIED TXS ", Style::default().fg(Color::DarkGray)),
            Span::raw("(AddTransactionRequest):"),
        ]),
        Line::from(vec![
            Span::raw("   → qc-06 Mempool                 "),
            status_indicator(is_healthy),
            Span::styled("  (sender address + valid flag)", Style::default().fg(Color::DarkGray)),
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

/// Create a rate usage bar.
fn rate_bar(current: u64, limit: u64) -> Span<'static> {
    let ratio = if limit > 0 { (current as f64 / limit as f64).min(1.0) } else { 0.0 };
    let filled = (ratio * 8.0) as usize;
    let empty = 8 - filled;
    
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));
    let color = if ratio > 0.9 {
        Color::Red
    } else if ratio > 0.7 {
        Color::Yellow
    } else {
        Color::Green
    };
    
    Span::styled(bar, Style::default().fg(color))
}

/// Metrics extracted from subsystem info.
struct SignatureMetrics {
    ecdsa_verifications: u64,
    bls_verifications: u64,
    batch_verifications: u64,
    cache_hit_rate: f64,
    cache_size: u64,
    malleability_rejections: u64,
    rate_limit_hits: u64,
    avg_latency_us: f64,
    rate_qc01: u64,
    rate_qc05: u64,
    rate_qc06: u64,
}

/// Extract metrics from subsystem info.
fn extract_metrics(info: &SubsystemInfo) -> SignatureMetrics {
    if let Some(metrics) = &info.metrics {
        SignatureMetrics {
            ecdsa_verifications: metrics.get("ecdsa_verifications")
                .or_else(|| metrics.get("verifications_total"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            bls_verifications: metrics.get("bls_verifications")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            batch_verifications: metrics.get("batch_verifications")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            cache_hit_rate: metrics.get("cache_hit_rate")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            cache_size: metrics.get("cache_size")
                .and_then(|v| v.as_u64())
                .unwrap_or(10000),
            malleability_rejections: metrics.get("malleability_rejections")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            rate_limit_hits: metrics.get("rate_limit_hits")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            avg_latency_us: metrics.get("avg_latency_us")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            rate_qc01: metrics.get("rate_qc01")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            rate_qc05: metrics.get("rate_qc05")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            rate_qc06: metrics.get("rate_qc06")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
        }
    } else {
        SignatureMetrics {
            ecdsa_verifications: 0,
            bls_verifications: 0,
            batch_verifications: 0,
            cache_hit_rate: 0.0,
            cache_size: 10000,
            malleability_rejections: 0,
            rate_limit_hits: 0,
            avg_latency_us: 0.0,
            rate_qc01: 0,
            rate_qc05: 0,
            rate_qc06: 0,
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
