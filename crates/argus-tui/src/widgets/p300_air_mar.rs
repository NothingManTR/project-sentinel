use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use argus_core::types::Domain;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    widgets::{Cell, Row, Table},
    Frame,
};

pub fn render_p300_air_mar(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if area.height < 2 || area.width < 10 {
        return;
    }

    let header_cells = ["ID / ÇAĞRI", "ALAN", "İRTİFA", "HIZ", "SQUAWK", "BİLGİ"]
        .into_iter()
        .map(|h| Cell::from(h).style(Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).style(Style::default().bg(WIN98_DARK_GRAY));

    let targets: Vec<_> = state.targets.iter().collect();
    let total_targets = targets.len();
    let visible_rows = area.height.saturating_sub(1) as usize;
    let start_idx = state.target_scroll.min(total_targets.saturating_sub(1));

    let mut rows = Vec::new();

    for (row_offset, track) in targets.iter().skip(start_idx).take(visible_rows).enumerate() {
        let actual_idx = start_idx + row_offset;
        let is_selected = state.selected_target_id.as_deref() == Some(&track.id);

        let is_emergency = track.squawk.as_deref() == Some("7700") || track.squawk.as_deref() == Some("7600");

        let row_style = if is_emergency {
            Style::default().fg(TELETEXT_RED).add_modifier(Modifier::BOLD)
        } else if is_selected {
            Style::default().fg(TELETEXT_BLACK).bg(TELETEXT_CYAN).add_modifier(Modifier::BOLD)
        } else {
            match track.domain {
                Domain::Air => Style::default().fg(TELETEXT_CYAN),
                Domain::Sea => Style::default().fg(TELETEXT_GREEN),
                _ => Style::default().fg(TELETEXT_WHITE),
            }
        };

        let alt_str = if let Some(alt) = track.geo.alt_m {
            format!("FL{:.0}", (alt * 3.28084 / 100.0).round())
        } else {
            "0".to_string()
        };

        let spd_str = track.speed_kts.map(|s| format!("{:.0}kt", s)).unwrap_or_else(|| "-".to_string());
        let sqwk_str = track.squawk.clone().unwrap_or_else(|| "-".to_string());

        let cells = vec![
            Cell::from(track.callsign.clone()),
            Cell::from(track.domain.as_str()),
            Cell::from(alt_str),
            Cell::from(spd_str),
            Cell::from(sqwk_str),
            Cell::from(track.info.clone()),
        ];

        rows.push(Row::new(cells).style(row_style));

        // Satır tıklama alanı
        let row_area = Rect {
            x: area.x,
            y: area.y + 1 + row_offset as u16,
            width: area.width,
            height: 1,
        };
        state.hit_tester.register(row_area, HitTarget::TableRow(2, actual_idx));
    }

    let widths = [
        Constraint::Length(12),
        Constraint::Length(7),
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Min(14),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .style(Style::default().bg(TELETEXT_BLACK));

    frame.render_widget(table, area);
}
