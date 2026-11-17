use crate::widgets::p100_map::render_p100_map;
use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::Marker,
    text::{Line, Span},
    widgets::{
        canvas::{Canvas, Circle, Line as CanvasLine},
        Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table,
    },
    Frame,
};

pub fn render_satint_view(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if area.width < 24 || area.height < 10 {
        return;
    }
    frame.render_widget(Clear, area);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(46), Constraint::Percentage(54)])
        .split(area);

    // Sol: Ust Polar Radar Skobu (%55) + Alt Kuresel Yorunge Izi Haritasi (%45)
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(main_chunks[0]);

    let radar_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER_ACTIVE))
        .title(" [ POLAR RADAR SKOBU (AZ/EL - SGP4) ] ")
        .style(Style::default().bg(BG_PRIMARY).fg(TELETEXT_CYAN));

    let radar_canvas = Canvas::default()
        .marker(Marker::Braille)
        .x_bounds([-100.0, 100.0])
        .y_bounds([-100.0, 100.0])
        .block(radar_block)
        .paint(|ctx| {
            ctx.draw(&Circle { x: 0.0, y: 0.0, radius: 90.0, color: Color::Rgb(44, 56, 72) });
            ctx.draw(&Circle { x: 0.0, y: 0.0, radius: 60.0, color: Color::Rgb(36, 46, 60) });
            ctx.draw(&Circle { x: 0.0, y: 0.0, radius: 30.0, color: Color::Rgb(28, 36, 48) });
            ctx.draw(&CanvasLine { x1: 0.0, y1: -95.0, x2: 0.0, y2: 95.0, color: Color::Rgb(44, 56, 72) });
            ctx.draw(&CanvasLine { x1: -95.0, y1: 0.0, x2: 95.0, y2: 0.0, color: Color::Rgb(44, 56, 72) });

            for (idx, sat) in state.satellites.iter().enumerate() {
                let az_rad = (sat.az_deg - 90.0).to_radians();
                let r = 90.0 - sat.el_deg.max(0.0);
                let x = r * az_rad.cos();
                let y = -r * az_rad.sin();

                let is_sel = state.selected_domain == 3 && state.selected_row == idx;
                let col = if is_sel {
                    TELETEXT_YELLOW
                } else if sat.el_deg > 10.0 {
                    TELETEXT_GREEN
                } else if sat.next_pass_secs < 600 {
                    TELETEXT_CYAN
                } else {
                    TELETEXT_MAGENTA
                };

                ctx.draw(&Circle {
                    x,
                    y,
                    radius: if is_sel { 4.0 } else { 2.4 },
                    color: col,
                });
            }
        });
    frame.render_widget(radar_canvas, left_chunks[0]);

    let sat_map_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER_DIM))
        .title(" [ UYDU YORUNGE IZI (GROUND TRACK - 90 DK) ] ")
        .style(Style::default().bg(BG_PRIMARY));
    let sat_map_inner = sat_map_block.inner(left_chunks[1]);
    frame.render_widget(sat_map_block, left_chunks[1]);
    render_p100_map(frame, sat_map_inner, state);

    // Sag: Uydu Tablosu + Secili Uydu Telemetri Ozeti
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(6)])
        .split(main_chunks[1]);

    let header_cells = ["UYDU", "NORAD", "IRTIFA", "HIZ", "AZ / EL", "GECIS"]
        .into_iter()
        .map(|h| Cell::from(h).style(Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).style(Style::default().bg(BG_SURFACE));

    let mut rows = Vec::new();
    let total_sats = state.satellites.len();
    let visible_rows = right_chunks[0].height.saturating_sub(3) as usize;
    let max_scroll = total_sats.saturating_sub(1);
    if state.sat_scroll > max_scroll {
        state.sat_scroll = max_scroll;
    }
    let start_idx = state.sat_scroll;

    for (row_offset, (idx, sat)) in state
        .satellites
        .iter()
        .enumerate()
        .skip(start_idx)
        .take(visible_rows)
        .enumerate()
    {
        let is_sel = row_offset == 0 || (state.selected_domain == 3 && state.selected_row == idx);
        let is_visible = sat.el_deg > 0.0;
        let row_style = if is_sel {
            Style::default().fg(TELETEXT_YELLOW).bg(BG_HIGHLIGHT).add_modifier(Modifier::BOLD)
        } else if is_visible {
            Style::default().fg(TELETEXT_GREEN).add_modifier(Modifier::BOLD)
        } else if sat.next_pass_secs < 300 {
            Style::default().fg(TELETEXT_CYAN)
        } else {
            Style::default().fg(TELETEXT_MAGENTA)
        };

        let countdown_str = if sat.el_deg > 0.0 {
            "UFUKTA (AKTIF)".to_string()
        } else {
            format!("T-{:02}:{:02} dk", sat.next_pass_secs / 60, sat.next_pass_secs % 60)
        };
        let prefix_name = if is_sel {
            format!("> {}", sat.name)
        } else {
            format!("  {}", sat.name)
        };

        rows.push(
            Row::new(vec![
                Cell::from(prefix_name),
                Cell::from(sat.norad_id.to_string()),
                Cell::from(format!("{:.0}km", sat.alt_km)),
                Cell::from(format!("{:.1}k/s", sat.velocity_kms)),
                Cell::from(format!("{:.0}°/{:+.0}°", sat.az_deg, sat.el_deg)),
                Cell::from(countdown_str),
            ])
            .style(row_style),
        );

        let row_area = Rect {
            x: right_chunks[0].x + 1,
            y: right_chunks[0].y + 2 + row_offset as u16,
            width: right_chunks[0].width.saturating_sub(2),
            height: 1,
        };
        state.hit_tester.register(row_area, HitTarget::TableRow(3, idx));
    }

    let widths = [
        Constraint::Length(18),
        Constraint::Length(8),
        Constraint::Length(9),
        Constraint::Length(9),
        Constraint::Length(13),
        Constraint::Min(14),
    ];

    let table = Table::new(rows, widths).header(header).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(format!(
                " [ CELESTRAK & SGP4 YORUNGE TAKIBI ({}/{}) ] ",
                (start_idx + 1).min(total_sats),
                total_sats
            ))
            .style(Style::default().bg(BG_PRIMARY).fg(FG_PRIMARY)),
    );
    frame.render_widget(table, right_chunks[0]);

    if total_sats > visible_rows {
        let mut sb_state = ScrollbarState::new(total_sats).position(start_idx);
        let sb = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(TELETEXT_CYAN).bg(BG_SURFACE));
        frame.render_stateful_widget(sb, right_chunks[0], &mut sb_state);
    }

    let sel_sat = state
        .satellites
        .get(state.selected_row)
        .or_else(|| state.satellites.first());
    let detail_lines = if let Some(s) = sel_sat {
        vec![
            Line::from(vec![
                Span::styled("SECILI UYDU : ", Style::default().fg(FG_MUTED)),
                Span::styled(
                    format!("{} (NORAD #{})", s.name, s.norad_id),
                    Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" │ Konum: ", Style::default().fg(FG_MUTED)),
                Span::styled(
                    format!("{:.2}°N, {:.2}°E", s.lat, s.lon),
                    Style::default().fg(TELETEXT_CYAN),
                ),
            ]),
            Line::from(vec![
                Span::styled("Yorunge     : ", Style::default().fg(FG_MUTED)),
                Span::styled(
                    format!("{:.1} km | {:.2} km/s | Ileri Iz: {} nokta (90 dk)", s.alt_km, s.velocity_kms, s.ground_track.len()),
                    Style::default().fg(TELETEXT_GREEN),
                ),
            ]),
            Line::from(vec![
                Span::styled("Model       : ", Style::default().fg(FG_MUTED)),
                Span::styled("CelesTrak GP + SGP4 WGS-84 + wheretheiss.at", Style::default().fg(FG_SECONDARY)),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled("CelesTrak uydu verisi yukleniyor...", Style::default().fg(FG_MUTED)))]
    };

    let aos_p = Paragraph::new(detail_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(" [ YER ISTASYONU & YORUNGE TELEMETRISI ] ")
            .style(Style::default().bg(BG_PANEL).fg(FG_PRIMARY)),
    );
    frame.render_widget(aos_p, right_chunks[1]);
}
