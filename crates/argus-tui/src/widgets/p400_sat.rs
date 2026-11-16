use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    widgets::{Cell, Row, Table},
    Frame,
};

pub fn render_p400_sat(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if area.height < 2 || area.width < 10 {
        return;
    }

    let header_cells = ["UYDU ADI", "NORAD", "İRTİFA", "HIZ", "AZ / EL", "GEÇİŞ GERİ SAYIMI"]
        .into_iter()
        .map(|h| Cell::from(h).style(Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).style(Style::default().bg(WIN98_DARK_GRAY));

    let visible_rows = area.height.saturating_sub(1) as usize;
    let start_idx = state.sat_scroll.min(state.satellites.len().saturating_sub(1));

    let mut rows = Vec::new();

    for (row_offset, sat) in state.satellites.iter().skip(start_idx).take(visible_rows).enumerate() {
        let actual_idx = start_idx + row_offset;

        let countdown_str = format!("{:02}:{:02} dk", sat.next_pass_secs / 60, sat.next_pass_secs % 60);

        let row_style = if sat.next_pass_secs < 300 {
            Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(TELETEXT_MAGENTA)
        };

        let cells = vec![
            Cell::from(sat.name.clone()),
            Cell::from(sat.norad_id.to_string()),
            Cell::from(format!("{:.1} km", sat.alt_km)),
            Cell::from(format!("{:.2} km/s", sat.velocity_kms)),
            Cell::from(format!("{:.0}°/{:.0}°", sat.az_deg, sat.el_deg)),
            Cell::from(countdown_str),
        ];

        rows.push(Row::new(cells).style(row_style));

        let row_area = Rect {
            x: area.x,
            y: area.y + 1 + row_offset as u16,
            width: area.width,
            height: 1,
        };
        state.hit_tester.register(row_area, HitTarget::TableRow(3, actual_idx));
    }

    let widths = [
        Constraint::Length(16),
        Constraint::Length(8),
        Constraint::Length(11),
        Constraint::Length(11),
        Constraint::Length(12),
        Constraint::Min(16),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .style(Style::default().bg(TELETEXT_BLACK));

    frame.render_widget(table, area);
}
