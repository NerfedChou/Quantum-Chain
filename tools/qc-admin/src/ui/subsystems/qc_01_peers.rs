//! QC-01 Peer Discovery panel renderer.
//!
//! Displays:
//! - Overview: Total peers, buckets used, banned, pending verification
//! - Top peers table (from admin_peers API)
//! - Dependency health

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::domain::{App, SubsystemInfo};

/// Render the QC-01 Peer Discovery panel.
pub fn render(frame: &mut Frame, area: Rect, info: &SubsystemInfo, app: &App) {
    // Vertical layout: Overview, Peers Table, Dependencies
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),  // Overview
            Constraint::Min(8),     // Peer table
            Constraint::Length(8),  // Dependencies
        ])
        .split(area);

    render_overview(frame, chunks[0], info);
    render_peer_table(frame, chunks[1], app);
    render_dependencies(frame, chunks[2]);
}

/// Render the overview section with collapsed border boxes and bottom separator line.
fn render_overview(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    // Extract metrics from info.metrics JSON if available
    let (total_peers, max_peers, buckets_used, max_buckets, banned, _pending, _max_pending, oldest_age) =
        extract_metrics(info);

    // Single area for the collapsed table
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // Metric boxes (table)
        ])
        .split(area);

    // Collapsed table style: outer border wraps all, inner cells only have RIGHT border
    // Draw outer container first
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    frame.render_widget(outer, sections[0]);

    // Shrink boxes area by 1 on each side to fit inside outer border
    let inner_area = Rect {
        x: sections[0].x + 1,
        y: sections[0].y + 1,
        width: sections[0].width.saturating_sub(2),
        height: sections[0].height.saturating_sub(2),
    };

    let inner_boxes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(inner_area);

    // Box 1: Total Peers (only RIGHT border for cell separator)
    let peers_text = vec![
        Line::from(Span::styled(" Total Peers", Style::default().fg(Color::DarkGray))),
        Line::from(vec![
            Span::styled(
                format!("{}", total_peers),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" / {}", max_peers),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ];
    let peers_box = Paragraph::new(peers_text)
        .alignment(ratatui::layout::Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::RIGHT)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(peers_box, inner_boxes[0]);

    // Box 2: Buckets Used (only RIGHT border)
    let buckets_text = vec![
        Line::from(Span::styled(" Buckets Used", Style::default().fg(Color::DarkGray))),
        Line::from(vec![
            Span::styled(
                format!("{}", buckets_used),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" / {}", max_buckets),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ];
    let buckets_box = Paragraph::new(buckets_text)
        .alignment(ratatui::layout::Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::RIGHT)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(buckets_box, inner_boxes[1]);

    // Box 3: Banned Peers (only RIGHT border)
    let banned_color = if banned > 0 { Color::Red } else { Color::Green };
    let banned_text = vec![
        Line::from(Span::styled(" Banned Peers", Style::default().fg(Color::DarkGray))),
        Line::from(vec![
            Span::styled(
                format!("{}", banned),
                Style::default().fg(banned_color).add_modifier(Modifier::BOLD),
            ),
        ]),
    ];
    let banned_box = Paragraph::new(banned_text)
        .alignment(ratatui::layout::Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::RIGHT)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(banned_box, inner_boxes[2]);

    // Box 4: Oldest Peer Age (NO border - last cell)
    let age_text = vec![
        Line::from(Span::styled(" Oldest Peer", Style::default().fg(Color::DarkGray))),
        Line::from(vec![
            Span::styled(
                oldest_age,
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
        ]),
    ];
    let age_box = Paragraph::new(age_text)
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default());
    frame.render_widget(age_box, inner_boxes[3]);
}

/// Render the peer table with collapsed borders like HTML table.
fn render_peer_table(frame: &mut Frame, area: Rect, app: &App) {
    // Title line above the table
    let title_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };
    let title = Paragraph::new(Line::from(Span::styled(
        format!(" Connected Peers ({}) ", app.peers.len()),
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    )));
    frame.render_widget(title, title_area);

    // Table area below title
    let table_area = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: area.height.saturating_sub(1),
    };

    // Draw outer border for the entire table
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    frame.render_widget(outer, table_area);

    // Inner area for cells
    let inner_area = Rect {
        x: table_area.x + 1,
        y: table_area.y + 1,
        width: table_area.width.saturating_sub(2),
        height: table_area.height.saturating_sub(2),
    };

    // Calculate row heights: 1 for header, rest for data
    let header_height = 1u16;

    // Header row area
    let header_area = Rect {
        x: inner_area.x,
        y: inner_area.y,
        width: inner_area.width,
        height: header_height,
    };

    // Calculate column widths (5 equal columns)
    let col_width = inner_area.width / 5;
    let headers = ["NodeID", "IP Address", "Port", "Rep", "Last Seen"];

    // Render header cells with collapsed borders (only RIGHT border between cells)
    for (i, title) in headers.iter().enumerate() {
        let cell_area = Rect {
            x: header_area.x + (i as u16 * col_width),
            y: header_area.y,
            width: if i == 4 { inner_area.width - (4 * col_width) } else { col_width },
            height: header_height,
        };
        
        // Cell separator: RIGHT border except for last cell
        let borders = if i < 4 { Borders::RIGHT } else { Borders::NONE };
        
        let cell = Paragraph::new(Span::styled(
            *title,
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ))
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().borders(borders).border_style(Style::default().fg(Color::DarkGray)));
        
        frame.render_widget(cell, cell_area);
    }

    // Draw horizontal line between header and data
    let separator_y = header_area.y + header_height;
    if separator_y < table_area.y + table_area.height - 1 {
        let separator = "─".repeat(inner_area.width as usize);
        let sep_line = Paragraph::new(Span::styled(separator, Style::default().fg(Color::DarkGray)));
        frame.render_widget(sep_line, Rect {
            x: inner_area.x,
            y: separator_y,
            width: inner_area.width,
            height: 1,
        });
    }

    // Render data rows
    let data_start_y = separator_y + 1;
    let peers_data: Vec<[String; 5]> = if app.peers.is_empty() {
        vec![["(No peers)".to_string(), String::new(), String::new(), String::new(), String::new()]]
    } else {
        app.peers.iter().take(10).map(|p| {
            [p.node_id.clone(), p.ip_address.clone(), p.port.to_string(), p.reputation.to_string(), p.last_seen.clone()]
        }).collect()
    };

    for (row_idx, row_data) in peers_data.iter().enumerate() {
        let row_y = data_start_y + row_idx as u16;
        if row_y >= table_area.y + table_area.height - 1 {
            break;
        }

        for (col_idx, cell_text) in row_data.iter().enumerate() {
            let cell_area = Rect {
                x: inner_area.x + (col_idx as u16 * col_width),
                y: row_y,
                width: if col_idx == 4 { inner_area.width - (4 * col_width) } else { col_width },
                height: 1,
            };
            
            let borders = if col_idx < 4 { Borders::RIGHT } else { Borders::NONE };
            let style = if app.peers.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            };
            
            let cell = Paragraph::new(Span::styled(cell_text.as_str(), style))
                .alignment(ratatui::layout::Alignment::Center)
                .block(Block::default().borders(borders).border_style(Style::default().fg(Color::DarkGray)));
            
            frame.render_widget(cell, cell_area);
        }
    }
}

/// Render the dependencies section with side-by-side Outbound/Inbound boxes.
fn render_dependencies(frame: &mut Frame, area: Rect) {
    // Container block
    let container = Block::default()
        .title(" Dependencies ")
        .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    
    let inner = container.inner(area);
    frame.render_widget(container, area);

    // Split into two side-by-side sections (50/50)
    let sides = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(inner);

    // Left side: OUTBOUND (I depend on) - only RIGHT border for separator
    let outbound_text = vec![
        Line::from(Span::styled("OUTBOUND", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  → qc-10 Signature  "),
            Span::styled("● HEALTHY", Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("    (DDoS edge defense)", Style::default().fg(Color::DarkGray)),
        ]),
    ];
    let outbound_box = Paragraph::new(outbound_text)
        .block(
            Block::default()
                .borders(Borders::RIGHT)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(outbound_box, sides[0]);

    // Right side: INBOUND (Depends on me) - no borders
    let inbound_text = vec![
        Line::from(Span::styled("INBOUND", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  ← qc-05 Block Prop   "),
            Span::styled("● HEALTHY", Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::raw("  ← qc-07 Bloom        "),
            Span::styled("○ NOT IMPL", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("  ← qc-13 Light Client "),
            Span::styled("○ NOT IMPL", Style::default().fg(Color::DarkGray)),
        ]),
    ];
    let inbound_box = Paragraph::new(inbound_text)
        .block(Block::default());
    frame.render_widget(inbound_box, sides[1]);
}

/// Extract metrics from subsystem info.
/// Returns (total_peers, max_peers, buckets_used, max_buckets, banned, pending, max_pending, oldest_age)
fn extract_metrics(info: &SubsystemInfo) -> (u32, u32, u32, u32, u32, u32, u32, String) {
    // Try to extract from JSON metrics, fall back to defaults
    if let Some(metrics) = &info.metrics {
        // Field names from qc-16 API response
        let total_peers = metrics.get("peers_connected")
            .or_else(|| metrics.get("total_peers"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let routing_table_size = metrics.get("routing_table_size")
            .and_then(|v| v.as_u64())
            .unwrap_or(total_peers as u64) as u32;
        let buckets_used = metrics.get("buckets_used")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let banned = metrics.get("banned_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let pending = metrics.get("pending_verification")
            .or_else(|| metrics.get("pending_verification_count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let max_pending = metrics.get("max_pending")
            .or_else(|| metrics.get("max_pending_peers"))
            .and_then(|v| v.as_u64())
            .unwrap_or(1024) as u32;
        let oldest_age_secs = metrics.get("oldest_peer_age_secs")
            .or_else(|| metrics.get("oldest_peer_age_seconds"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        
        let oldest_age = format_duration(oldest_age_secs);
        
        // Use routing_table_size if larger than peers_connected
        let display_peers = routing_table_size.max(total_peers);
        
        (display_peers, 5120, buckets_used, 256, banned, pending, max_pending, oldest_age)
    } else {
        // Default/placeholder values
        (0, 5120, 0, 256, 0, 0, 1024, "N/A".to_string())
    }
}

/// Format duration in seconds to human-readable string.
fn format_duration(secs: u64) -> String {
    if secs == 0 {
        return "N/A".to_string();
    }
    
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let secs = secs % 60;
    
    if hours > 0 {
        format!("{}h {}m {}s", hours, mins, secs)
    } else if mins > 0 {
        format!("{}m {}s", mins, secs)
    } else {
        format!("{}s", secs)
    }
}
