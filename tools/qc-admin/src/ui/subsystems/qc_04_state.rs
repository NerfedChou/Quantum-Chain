//! QC-04 State Management panel renderer.
//!
//! Displays:
//! - Overview: Accounts, contracts, state root, trie stats
//! - Patricia Merkle Trie health (2x2 grid)
//! - Dependency health (3 horizontal sections)

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
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
            Constraint::Length(5),  // Overview (boxes)
            Constraint::Length(8),  // Trie stats (2x2 grid)
            Constraint::Min(6),     // Dependencies (3 horizontal)
        ])
        .split(area);

    render_overview(frame, chunks[0], info);
    render_trie_stats(frame, chunks[1], info);
    render_dependencies(frame, chunks[2], info);
}

/// Render the overview section with individual metric boxes.
fn render_overview(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let (total_accounts, total_contracts, _current_state_root, cache_size_mb, 
         proofs_generated, snapshots_count, _pruning_depth) = extract_metrics(info);

    // Container block
    let container = Block::default()
        .title(" Overview ")
        .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    
    let inner = container.inner(area);
    frame.render_widget(container, area);

    // Horizontal layout with 5 equal boxes
    let boxes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ])
        .split(inner);

    // Box 1: Accounts
    let accounts_box = Paragraph::new(Line::from(Span::styled(
        format_number(total_accounts),
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .title(" Accounts ")
            .title_style(Style::default().fg(Color::DarkGray))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(accounts_box, boxes[0]);

    // Box 2: Contracts
    let contracts_box = Paragraph::new(Line::from(Span::styled(
        format_number(total_contracts),
        Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .title(" Contracts ")
            .title_style(Style::default().fg(Color::DarkGray))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(contracts_box, boxes[1]);

    // Box 3: Proofs
    let proofs_box = Paragraph::new(Line::from(Span::styled(
        format_number(proofs_generated),
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .title(" Proofs ")
            .title_style(Style::default().fg(Color::DarkGray))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(proofs_box, boxes[2]);

    // Box 4: Snapshots
    let snapshots_box = Paragraph::new(Line::from(Span::styled(
        format!("{}", snapshots_count),
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .title(" Snapshots ")
            .title_style(Style::default().fg(Color::DarkGray))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(snapshots_box, boxes[3]);

    // Box 5: Cache
    let cache_box = Paragraph::new(Line::from(Span::styled(
        format!("{} MB", cache_size_mb),
        Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .title(" Cache ")
            .title_style(Style::default().fg(Color::DarkGray))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(cache_box, boxes[4]);
}

/// Render the Patricia Merkle Trie stats section as 2x2 grid.
fn render_trie_stats(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let (trie_depth, trie_nodes, storage_slots) = extract_trie_metrics(info);

    // Container block
    let container = Block::default()
        .title(" Patricia Merkle Trie ")
        .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    
    let inner = container.inner(area);
    frame.render_widget(container, area);

    // 2x2 grid layout
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(inner);

    let top_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(rows[0]);

    let bottom_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(rows[1]);

    // Top-left: Trie Depth
    let depth_box = Paragraph::new(Line::from(vec![
        Span::styled(
            format!("{}", trie_depth),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" / 64", Style::default().fg(Color::DarkGray)),
    ]))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .title(" Depth ")
            .title_style(Style::default().fg(Color::DarkGray))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(depth_box, top_cols[0]);

    // Top-right: Trie Nodes
    let nodes_box = Paragraph::new(Line::from(Span::styled(
        format_number(trie_nodes),
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .title(" Nodes ")
            .title_style(Style::default().fg(Color::DarkGray))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(nodes_box, top_cols[1]);

    // Bottom-left: Storage Slots
    let slots_box = Paragraph::new(Line::from(Span::styled(
        format_number(storage_slots),
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .title(" Storage Slots ")
            .title_style(Style::default().fg(Color::DarkGray))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(slots_box, bottom_cols[0]);

    // Bottom-right: Hash Algorithm
    let hash_box = Paragraph::new(Line::from(Span::styled(
        "Keccak-256",
        Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .title(" Hash Algo ")
            .title_style(Style::default().fg(Color::DarkGray))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(hash_box, bottom_cols[1]);
}

/// Render the dependencies section with 4 horizontal boxes.
/// Per SPEC-04: subscribes qc-08, publishes qc-02, accepts from qc-06/11/12/14, serves qc-16
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

    // Box 1: SUBSCRIBES TO (BlockValidated from Consensus)
    let subscribes_text = vec![
        Line::from(vec![
            Span::raw("← qc-08 "),
            status_indicator(is_healthy),
        ]),
    ];
    let subscribes_box = Paragraph::new(subscribes_text)
        .block(
            Block::default()
                .title(" SUBSCRIBES ")
                .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(subscribes_box, sections[0]);

    // Box 2: PUBLISHES (StateRootComputed to Block Storage)
    let publishes_text = vec![
        Line::from(vec![
            Span::raw("→ qc-02 "),
            status_indicator(is_healthy),
        ]),
    ];
    let publishes_box = Paragraph::new(publishes_text)
        .block(
            Block::default()
                .title(" PUBLISHES ")
                .title_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(publishes_box, sections[1]);

    // Box 3: ACCEPTS FROM (Mempool, Smart Contracts, Tx Ordering, Sharding)
    let accepts_text = vec![
        Line::from(vec![
            Span::raw("← qc-06 "),
            status_indicator(is_healthy),
        ]),
        Line::from(vec![
            Span::raw("← qc-11 "),
            status_indicator(is_healthy),
        ]),
        Line::from(vec![
            Span::raw("← qc-12 "),
            status_indicator(is_healthy),
        ]),
        Line::from(vec![
            Span::raw("← qc-14 "),
            status_indicator(is_healthy),
        ]),
    ];
    let accepts_box = Paragraph::new(accepts_text)
        .block(
            Block::default()
                .title(" ACCEPTS ")
                .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(accepts_box, sections[2]);

    // Box 4: SERVES (API Gateway for eth_* calls)
    let serves_text = vec![
        Line::from(vec![
            Span::raw("→ qc-16 "),
            status_indicator(is_healthy),
        ]),
    ];
    let serves_box = Paragraph::new(serves_text)
        .block(
            Block::default()
                .title(" SERVES ")
                .title_style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(serves_box, sections[3]);
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
