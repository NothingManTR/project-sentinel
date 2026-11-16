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
        Block, Borders, Cell, Paragraph, Row, Table,
    },
    Frame,
};

pub fn render_satint_view(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    // 1. Sol: Polar Radar Skobu (Azimuth / Elevation Polar Plot)
    let radar_block = Block::default()
        .borders(Borders::ALL)
        .title(" [POLAR RADAR SKOBU (AZ/EL)] ")
        .style(Style::default().bg(Color::Rgb(6, 12, 24)).fg(TELETEXT_CYAN));

    let radar_canvas = Canvas::default()
        .marker(Marker::Braille)
        .x_bounds([-100.0, 100.0])
        .y_bounds([-100.0, 100.0])
        .block(radar_block)
        .paint(|ctx| {
            // Radar çemberleri: Horizon (90), 60°, 30°
            ctx.draw(&Circle { x: 0.0, y: 0.0, radius: 90.0, color: Color::Rgb(30, 60, 90) });
            ctx.draw(&Circle { x: 0.0, y: 0.0, radius: 60.0, color: Color::Rgb(25, 50, 75) });
            ctx.draw(&Circle { x: 0.0, y: 0.0, radius: 30.0, color: Color::Rgb(20, 40, 60) });

            // Çapraz eksen çizgileri (Kuzey-Güney, Doğu-Batı)
            ctx.draw(&CanvasLine { x1: 0.0, y1: -95.0, x2: 0.0, y2: 95.0, color: Color::Rgb(30, 60, 90) });
            ctx.draw(&CanvasLine { x1: -95.0, y1: 0.0, x2: 95.0, y2: 0.0, color: Color::Rgb(30, 60, 90) });

            // Uyduların Azimuth/Elevation polar projeksiyonu
            for sat in state.satellites.iter() {
                let az_rad = (sat.az_deg - 90.0).to_radians();
                let r = 90.0 - sat.el_deg.max(0.0);
                let x = r * az_rad.cos();
                let y = -r * az_rad.sin();

                let col = if sat.el_deg > 10.0 {
                    TELETEXT_GREEN
                } else if sat.next_pass_secs < 600 {
                    TELETEXT_YELLOW
                } else {
                    TELETEXT_MAGENTA
                };

                ctx.draw(&Circle { x, y, radius: 2.2, color: col });
            }
        });
    frame.render_widget(radar_canvas, main_chunks[0]);

    // 2. Sağ: Uydu Tablosu ve TLE Verisi
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(6)])
        .split(main_chunks[1]);

    let header_cells = ["UYDU", "NORAD", "IRTITA", "HIZ", "AZ / EL", "GECIS"]
        .into_iter()
        .map(|h| Cell::from(h).style(Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).style(Style::default().bg(WIN98_DARK_GRAY));

    let mut rows = Vec::new();
    let total_sats = state.satellites.len();
    let visible_rows = right_chunks[0].height.saturating_sub(3) as usize;
    let max_scroll = total_sats.saturating_sub(visible_rows.max(1));
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
        let is_visible = sat.el_deg > 0.0;
        let row_style = if is_visible {
            Style::default().fg(TELETEXT_GREEN).add_modifier(Modifier::BOLD)
        } else if sat.next_pass_secs < 300 {
            Style::default().fg(TELETEXT_YELLOW)
        } else {
            Style::default().fg(TELETEXT_MAGENTA)
        };

        let countdown_str = format!("{:02}:{:02} dk", sat.next_pass_secs / 60, sat.next_pass_secs % 60);

        let cells = vec![
            Cell::from(sat.name.clone()),
            Cell::from(sat.norad_id.to_string()),
            Cell::from(format!("{:.0}km", sat.alt_km)),
            Cell::from(format!("{:.1}k/s", sat.velocity_kms)),
            Cell::from(format!("{:.0}°/{:.0}°", sat.az_deg, sat.el_deg)),
            Cell::from(countdown_str),
        ];

        rows.push(Row::new(cells).style(row_style));

        let row_area = Rect {
            x: right_chunks[0].x,
            y: right_chunks[0].y + 2 + row_offset as u16,
            width: right_chunks[0].width,
            height: 1,
        };
        state.hit_tester.register(row_area, HitTarget::TableRow(3, idx));
    }

    let widths = [
        Constraint::Length(16),
        Constraint::Length(8),
        Constraint::Length(9),
        Constraint::Length(9),
        Constraint::Length(12),
        Constraint::Min(10),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(" [CELESTRAK & SGP4 YORUNGE TAKIBI] ").style(Style::default().bg(TELETEXT_BLACK).fg(TELETEXT_WHITE)));
    frame.render_widget(table, right_chunks[0]);

    // Alt: İstasyon AOS/LOS Durumu
    let aos_p = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Yorunge Modeli: ", Style::default().fg(WIN98_DARK_GRAY)),
            Span::styled("SGP4 WGS-84 Balistik Yayilim", Style::default().fg(WIN98_BLACK).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Takip Edilen: ", Style::default().fg(WIN98_DARK_GRAY)),
            Span::styled(format!("{} Aktif Uydu | ISS, Tiangong, Starlink, KeyHole", state.satellites.len()), Style::default().fg(TELETEXT_CYAN)),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL).title(" [YER ISTASYONU SKOBU] ").style(Style::default().bg(WIN98_SILVER).fg(WIN98_BLACK)));
    frame.render_widget(aos_p, right_chunks[1]);
}
