use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Sparkline, Table},
    Frame,
};

pub fn render_p600_geo(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if area.height < 4 || area.width < 10 {
        return;
    }

    let chunks = Layout::default()
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(area);

    let header_cells = ["KAYNAK / TÜR", "KONUM / BÖLGE", "BÜYÜKLÜK / ETKİ", "ZAMAN / DETAY"]
        .into_iter()
        .map(|h| Cell::from(h).style(Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).style(Style::default().bg(WIN98_DARK_GRAY));

    let mut all_items: Vec<(Row, HitTarget)> = Vec::new();

    // 1. Canlı Depremler
    for (q_idx, quake) in state.earthquakes.iter().enumerate() {
        let mag_color = if quake.mag >= 6.0 {
            TELETEXT_RED
        } else if quake.mag >= 5.0 {
            TELETEXT_YELLOW
        } else {
            TELETEXT_CYAN
        };

        let cells = vec![
            Cell::from("USGS DEPREM"),
            Cell::from(quake.place.clone()),
            Cell::from(format!("M {:.1} ({:.0}km)", quake.mag, quake.depth_km)),
            Cell::from(quake.time.format("%H:%M:%S UTC").to_string()),
        ];

        all_items.push((
            Row::new(cells).style(Style::default().fg(mag_color)),
            HitTarget::TableRow(50, q_idx),
        ));
    }

    // 2. NASA FIRMS Termal Yangın ve Patlamalar
    for (f_idx, fire) in state.firms_fires.iter().enumerate() {
        let cells = vec![
            Cell::from("NASA FIRMS"),
            Cell::from(format!("{:.2}N, {:.2}E", fire.lat, fire.lon)),
            Cell::from(format!("{:.0} K (%{})", fire.brightness, fire.confidence)),
            Cell::from(format!("{} {}", fire.acq_date, fire.acq_time)),
        ];

        all_items.push((
            Row::new(cells).style(Style::default().fg(TELETEXT_RED)),
            HitTarget::TableRow(51, f_idx),
        ));
    }

    // 3. ACLED Canlı Çatışmalar
    for (a_idx, acled) in state.acled_events.iter().enumerate() {
        let cells = vec![
            Cell::from(format!("ACLED {}", acled.event_type)),
            Cell::from(format!("{}, {}", acled.location, acled.country)),
            Cell::from(format!("Zayiat: {}", acled.fatalities)),
            Cell::from(acled.actor1.clone()),
        ];

        all_items.push((
            Row::new(cells).style(Style::default().fg(TELETEXT_YELLOW)),
            HitTarget::TableRow(52, a_idx),
        ));
    }

    let visible_rows = chunks[0].height.saturating_sub(1) as usize;
    let max_scroll = all_items.len().saturating_sub(visible_rows);
    state.geo_scroll = state.geo_scroll.min(max_scroll);
    let start_idx = state.geo_scroll;

    let mut rows = Vec::new();
    for (row_offset, (row, target)) in all_items
        .into_iter()
        .skip(start_idx)
        .take(visible_rows)
        .enumerate()
    {
        rows.push(row);
        let row_area = Rect {
            x: chunks[0].x,
            y: chunks[0].y + 1 + row_offset as u16,
            width: chunks[0].width,
            height: 1,
        };
        state.hit_tester.register(row_area, target);
    }

    let widths = [
        Constraint::Length(15),
        Constraint::Length(22),
        Constraint::Length(16),
        Constraint::Min(16),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .style(Style::default().bg(TELETEXT_BLACK));

    frame.render_widget(table, chunks[0]);

    // Alt Panel: NOAA Solar Weather + Deprem Sparkline
    let sub_chunks = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(chunks[1]);

    let sw_text = if let Some(sw) = &state.solar_weather {
        format!("Kp Indeks: {:.1} | Durum: {} | Parlama: {}", sw.kp_index, sw.storm_level, sw.flare_class)
    } else {
        "Uzay Havası: G0 (Sakin) | Kp: 2.1".to_string()
    };

    let p = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("NOAA UZAY HAVASI: ", Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled(sw_text, Style::default().fg(TELETEXT_CYAN)),
        ]),
        Line::from(vec![
            Span::styled("Sismik Ağ: ", Style::default().fg(TELETEXT_GREEN)),
            Span::styled(format!("{} Deprem Kaydedildi | FIRMS: {} Yangın", state.earthquakes.len(), state.firms_fires.len()), Style::default().fg(TELETEXT_WHITE)),
        ]),
    ]).style(Style::default().bg(WIN98_DARK_GRAY));
    frame.render_widget(p, sub_chunks[0]);

    // Deprem Büyüklük Sparkline
    let mag_data: Vec<u64> = state.earthquakes.iter().map(|q| (q.mag * 10.0) as u64).collect();
    let sparkline_data = if mag_data.is_empty() { vec![40, 45, 52, 48, 55, 43, 61, 49] } else { mag_data };
    let sparkline = Sparkline::default()
        .data(&sparkline_data)
        .style(Style::default().fg(TELETEXT_YELLOW).bg(WIN98_DARK_GRAY));
    frame.render_widget(sparkline, sub_chunks[1]);
}
