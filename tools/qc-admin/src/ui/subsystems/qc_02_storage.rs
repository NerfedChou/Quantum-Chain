//! QC-02 Block Storage panel renderer.
//!
//! Displays:
//! - Overview: Latest block, finalized block, disk usage, pending assemblies
//! - Assembly Status (Stateful Assembler pattern)
//! - Dependency health (Choreography pattern)

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::domain::{App, SubsystemInfo};

/// Render the QC-02 Block Storage panel.
pub fn render(frame: &mut Frame, area: Rect, info: &SubsystemInfo, app: &App) {
    // Vertical layout: Overview, Assembly Status, Dependencies
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // Overview (reduced height with boxes)
            Constraint::Min(8),     // Assembly status table
            Constraint::Length(8),  // Dependencies (side-by-side)
        ])
        .split(area);

    render_overview(frame, chunks[0], info);
    render_assembly_status(frame, chunks[1], app);
    render_dependencies(frame, chunks[2], info);
}

/// Render the overview section with collapsed border boxes (like qc-01).
fn render_overview(frame: &mut Frame, area: Rect, info: &SubsystemInfo) {
    let (latest_block, finalized_block, _total_blocks, disk_used, disk_total, pending_assemblies, _assembly_timeout) =
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

    let pending_color = if pending_assemblies > 0 { Color::Yellow } else { Color::Green };

    // Outer border wraps all cells (collapsed table style)
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    frame.render_widget(outer, area);

    // Inner area for cells
    let inner_area = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
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

    // Box 1: Latest Block (RIGHT border for cell separator)
    let latest_text = vec![
        Line::from(Span::styled(" Latest Block", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled(
            format!("#{}", format_number(latest_block)),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
    ];
    let latest_box = Paragraph::new(latest_text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::RIGHT).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(latest_box, inner_boxes[0]);

    // Box 2: Finalized Block (RIGHT border)
    let finalized_text = vec![
        Line::from(Span::styled(" Finalized", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled(
            format!("#{}", format_number(finalized_block)),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )),
    ];
    let finalized_box = Paragraph::new(finalized_text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::RIGHT).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(finalized_box, inner_boxes[1]);

    // Box 3: Disk Usage (RIGHT border)
    let disk_text = vec![
        Line::from(Span::styled(format!(" Disk {}%", disk_percent), Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled(
            format!("{}/{} GB", format_size_gb(disk_used), format_size_gb(disk_total)),
            Style::default().fg(disk_color).add_modifier(Modifier::BOLD),
        )),
    ];
    let disk_box = Paragraph::new(disk_text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::RIGHT).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(disk_box, inner_boxes[2]);

    // Box 4: Pending Assemblies (NO border - last cell)
    let pending_text = vec![
        Line::from(Span::styled(" Pending", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled(
            format!("{}", pending_assemblies),
            Style::default().fg(pending_color).add_modifier(Modifier::BOLD),
        )),
    ];
    let pending_box = Paragraph::new(pending_text)
        .alignment(Alignment::Center)
        .block(Block::default());
    frame.render_widget(pending_box, inner_boxes[3]);
}

/// Render the assembly status table with collapsed borders like HTML table.
fn render_assembly_status(frame: &mut Frame, area: Rect, app: &App) {
    // Title line above the table
    let title_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };
    let title = Paragraph::new(Line::from(Span::styled(
        format!(" Assembly Status ({}) ", app.pending_assemblies.len()),
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
    let headers = ["Block Hash", "BlockValidated", "MerkleRoot", "StateRoot", "Status"];

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
        .alignment(Alignment::Center)
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
    let assemblies_data: Vec<[String; 5]> = if app.pending_assemblies.is_empty() {
        vec![["(No pending assemblies)".to_string(), String::new(), String::new(), String::new(), String::new()]]
    } else {
        app.pending_assemblies.iter().take(5).map(|assembly| {
            let block_hash = format!("0x{}...", &assembly.block_hash[..8.min(assembly.block_hash.len())]);
            let has_block = if assembly.has_block { "✓" } else { "○" };
            let has_merkle = if assembly.has_merkle { "✓" } else { "○" };
            let has_state = if assembly.has_state { "✓" } else { "○" };
            let status = if assembly.has_block && assembly.has_merkle && assembly.has_state {
                "READY"
            } else {
                "WAITING"
            };
            [block_hash, has_block.to_string(), has_merkle.to_string(), has_state.to_string(), status.to_string()]
        }).collect()
    };

    for (row_idx, row_data) in assemblies_data.iter().enumerate() {
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
            let style = if app.pending_assemblies.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            };
            
            let cell = Paragraph::new(Span::styled(cell_text.as_str(), style))
                .alignment(Alignment::Center)
                .block(Block::default().borders(borders).border_style(Style::default().fg(Color::DarkGray)));
            
            frame.render_widget(cell, cell_area);
        }
    }
}

/// Render the dependencies section with side-by-side layout (no borders on inner sections).
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

    // Split into two side-by-side sections (50/50)
    let sides = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(inner);

    // Left side: SUBSCRIBES TO - only RIGHT border for separator
    let subscribes_text = vec![
        Line::from(Span::styled("SUBSCRIBES TO", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  ← qc-08 BlockValidated "),
            status_indicator(is_healthy),
        ]),
        Line::from(vec![
            Span::raw("  ← qc-03 MerkleRoot     "),
            status_indicator(is_healthy),
        ]),
        Line::from(vec![
            Span::raw("  ← qc-04 StateRoot      "),
            status_indicator(is_healthy),
        ]),
    ];
    let subscribes_box = Paragraph::new(subscribes_text)
        .block(
            Block::default()
                .borders(Borders::RIGHT)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(subscribes_box, sides[0]);

    // Right side: PROVIDES TO - no borders
    let provides_text = vec![
        Line::from(Span::styled("PROVIDES TO", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  → qc-06 StorageConfirm "),
            status_indicator(is_healthy),
        ]),
        Line::from(vec![
            Span::raw("  → qc-03 TxHashResponse "),
            status_indicator(is_healthy),
        ]),
        Line::from(vec![
            Span::raw("  → qc-09 MarkFinalized  "),
            status_indicator(is_healthy),
        ]),
    ];
    let provides_box = Paragraph::new(provides_text)
        .block(Block::default());
    frame.render_widget(provides_box, sides[1]);
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
            .unwrap_or(500 * 1024 * 1024 * 1024);
        let pending_assemblies = metrics.get("pending_assemblies")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let assembly_timeout = metrics.get("assembly_timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(30) as u32;

        (latest_block, finalized_block, total_blocks, disk_used, disk_total, pending_assemblies, assembly_timeout)
    } else {
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
