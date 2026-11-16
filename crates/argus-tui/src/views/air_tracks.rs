use crate::widgets::{inspector::render_inspector_panel, p100_map::render_p100_map};
use argus_core::geo::{bearing_sector_tr, haversine_km, initial_bearing_deg};
use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table,
    },
    Frame,
};

pub fn render_air_tracks_view(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if area.width < 20 || area.height < 6 {
        return;
    }

    frame.render_widget(Clear, area);

    // Sol (%68): Hava Radari Tablosu | Sag (%32): Canli Hedef Mini-Haritasi (Ust) + Teknik Denetci (Alt)
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
        .split(area);

    let left_area = split[0];
    let right_area = split[1];

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(right_area);

    let mini_map_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER_ACTIVE))
        .title(" [ HAVA SEKTOR MINI-HARITASI ] ")
        .style(Style::default().bg(BG_PRIMARY));
    let mini_map_inner = mini_map_block.inner(right_chunks[0]);
    frame.render_widget(mini_map_block, right_chunks[0]);
    render_p100_map(frame, mini_map_inner, state);

    render_inspector_panel(frame, right_chunks[1], state);

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // HUD Taktik Ozet Kartlari
            Constraint::Min(6),    // Ana Radar Tablosu
        ])
        .split(left_area);

    let hud_bar = main_layout[0];
    let table_area = main_layout[1];

    let all_count = state.air_targets.len();
    let mil_count = state.air_targets.iter().filter(|t| t.is_military).count();
    let cent_count = state
        .air_targets
        .iter()
        .filter(|t| AppState::is_centcom_track(t))
        .count();
    let cent_mil = state
        .air_targets
        .iter()
        .filter(|t| t.is_military && AppState::is_centcom_track(t))
        .count();
    let emg_count = state
        .air_targets
        .iter()
        .filter(|t| matches!(t.squawk.as_deref(), Some("7700") | Some("7600") | Some("7500")))
        .count();

    // 2. HUD Özet Kartları
    let hud_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(hud_bar);

    let hud_items = [
        (
            "KURESEL ADS-B",
            format!("{} Iz / {} Askeri", all_count, mil_count),
            TELETEXT_CYAN,
        ),
        (
            "CENTCOM AOR",
            format!("{} Iz / {} Askeri", cent_count, cent_mil),
            if cent_mil > 0 { TELETEXT_RED } else { FG_SECONDARY },
        ),
        (
            "ACIL DURUM",
            format!("{} Squawk 7x00", emg_count),
            if emg_count > 0 { TELETEXT_RED } else { TELETEXT_GREEN },
        ),
        (
            "AKTIF FILTRE",
            match state.air_filter {
                1 => "CENTCOM Tumu",
                2 => "CENTCOM Askeri",
                3 => "Kuresel Askeri",
                4 => "TR / EUCOM",
                5 => "Acil Squawk",
                _ => "Kuresel Tumu",
            }
            .to_string(),
            TELETEXT_YELLOW,
        ),
    ];

    for (i, (label, val, col)) in hud_items.iter().enumerate() {
        let card_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_DIM))
            .style(Style::default().bg(BG_PANEL));
        let card_inner = card_block.inner(hud_layout[i]);
        frame.render_widget(card_block, hud_layout[i]);

        let p = Paragraph::new(vec![
            Line::from(Span::styled(
                *label,
                Style::default().fg(FG_SECONDARY).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                val.as_str(),
                Style::default().fg(*col).add_modifier(Modifier::BOLD),
            )),
        ]);
        frame.render_widget(p, card_inner);
    }

    // 3. Hava Trafik Tablosu (Kopyalamadan indeks tabanli filtreleme ve sayfalama)
    let q_lower = state.search_query.to_lowercase();
    let sorted_indices = state.aircraft_sorted_indices();
    let air_list: Vec<(usize, usize)> = sorted_indices
        .into_iter()
        .enumerate()
        .filter(|(_, buf_idx)| {
            if q_lower.is_empty() {
                true
            } else if let Some(t) = state.air_targets.get(*buf_idx) {
                t.callsign.to_lowercase().contains(&q_lower)
                    || t.id.to_lowercase().contains(&q_lower)
                    || t.operator_country.as_deref().unwrap_or("").to_lowercase().contains(&q_lower)
                    || t.operator_branch.as_deref().unwrap_or("").to_lowercase().contains(&q_lower)
                    || t.info.to_lowercase().contains(&q_lower)
            } else {
                false
            }
        })
        .collect();
    let total_air = air_list.len();

    let sort_ind = |col: usize| -> &'static str {
        if state.air_sort_col == col {
            if state.air_sort_asc { " (+)" } else { " (-)" }
        } else {
            ""
        }
    };

    let h_call = format!("CAGRI ADI{}", sort_ind(1));
    let h_stat = format!("SINIF{}", sort_ind(2));
    let h_dist = format!("MESAFE{}", sort_ind(5));
    let h_alt = format!("IRTIFA{}", sort_ind(3));
    let h_spd = format!("HIZ{}", sort_ind(4));

    let header_titles = [
        h_call.as_str(),
        "ICAO",
        h_stat.as_str(),
        h_dist.as_str(),
        "KERTERIZ",
        "KUVVET / OPERATOR",
        "PLATFORM TIPI",
        h_alt.as_str(),
        h_spd.as_str(),
        "SQUAWK",
        "BOLGE",
    ];
    let header_cells = header_titles
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)));

    let header = Row::new(header_cells).style(Style::default().bg(BG_SURFACE)).height(1);

    if table_area.height >= 3 {
        let header_y = table_area.y + 1;
        state.hit_tester.register(
            Rect { x: table_area.x + 1, y: header_y, width: 13, height: 1 },
            HitTarget::SortColumn(20, 1),
        );
        state.hit_tester.register(
            Rect { x: table_area.x + 22, y: header_y, width: 10, height: 1 },
            HitTarget::SortColumn(20, 2),
        );
        state.hit_tester.register(
            Rect { x: table_area.x + 33, y: header_y, width: 11, height: 1 },
            HitTarget::SortColumn(20, 5),
        );
        state.hit_tester.register(
            Rect { x: table_area.x + 92, y: header_y, width: 12, height: 1 },
            HitTarget::SortColumn(20, 3),
        );
        state.hit_tester.register(
            Rect { x: table_area.x + 105, y: header_y, width: 9, height: 1 },
            HitTarget::SortColumn(20, 4),
        );
    }

    let rows_height = table_area.height.saturating_sub(3) as usize;
    let max_scroll = total_air.saturating_sub(1);
    if state.air_scroll > max_scroll {
        state.air_scroll = max_scroll;
    }
    let mut rows = Vec::new();
    let st_lat = state.station_lat;
    let st_lon = state.station_lon;

    for (offset, &(orig_idx, buf_idx)) in air_list
        .iter()
        .skip(state.air_scroll)
        .take(rows_height)
        .enumerate()
    {
        let Some(track) = state.air_targets.get(buf_idx) else {
            continue;
        };
        let is_selected =
            offset == 0 || state.selected_target_id.as_deref() == Some(track.id.as_str());
        let is_followed = state.follow_target_id.as_deref() == Some(track.id.as_str());

        let status_span = if track.is_military {
            Span::styled("[ASKERI]", Style::default().fg(TELETEXT_RED).add_modifier(Modifier::BOLD))
        } else if track.squawk.as_deref() == Some("7700") {
            Span::styled("[7700]", Style::default().fg(TELETEXT_RED).add_modifier(Modifier::BOLD))
        } else {
            Span::styled("[SIVIL]", Style::default().fg(TELETEXT_GREEN))
        };

        let d_km = haversine_km(st_lat, st_lon, track.geo.lat, track.geo.lon);
        let brg = initial_bearing_deg(st_lat, st_lon, track.geo.lat, track.geo.lon);
        let dist_str = format!("{:.0} km", d_km);
        let brg_str = format!("{:03.0} {}", brg, bearing_sector_tr(brg));

        let alt_str = if let Some(alt) = track.geo.alt_m {
            format!("FL{:.0}", alt * 3.28084 / 100.0)
        } else {
            "N/A".to_string()
        };

        let spd_str = track
            .speed_kts
            .map(|s| format!("{:.0}kt", s))
            .unwrap_or_else(|| "-".to_string());
        let sqwk_str = track.squawk.as_deref().unwrap_or("----");
        let branch_str = track
            .operator_branch
            .as_deref()
            .or(track.operator_country.as_deref())
            .unwrap_or("Sivil");
        let class_str = track
            .airframe_or_vessel_class
            .as_deref()
            .unwrap_or("Standart");
        let theater_str = if AppState::is_centcom_track(track) {
            "CENTCOM"
        } else {
            track.origin.as_deref().unwrap_or("KURESEL")
        };
        let theater_col = match theater_str {
            "CENTCOM" => TELETEXT_RED,
            "TR/EUCOM" => TELETEXT_CYAN,
            "NORTHCOM" | "INDOPACOM" => TELETEXT_YELLOW,
            "AFRICOM" | "SOUTHCOM" => TELETEXT_GREEN,
            _ => FG_SECONDARY,
        };

        let call_display = if is_followed {
            format!("* {}", track.callsign)
        } else if offset == 0 {
            format!("> {}", track.callsign)
        } else {
            format!("  {}", track.callsign)
        };

        let row_style = if is_selected {
            Style::default()
                .fg(TELETEXT_YELLOW)
                .bg(BG_HIGHLIGHT)
                .add_modifier(Modifier::BOLD)
        } else if track.is_military {
            Style::default()
                .fg(Color::Rgb(255, 210, 170))
                .bg(if offset % 2 == 0 {
                    Color::Rgb(24, 14, 16)
                } else {
                    Color::Rgb(16, 10, 12)
                })
        } else {
            Style::default()
                .fg(FG_PRIMARY)
                .bg(if offset % 2 == 0 { BG_PANEL } else { BG_PRIMARY })
        };

        let cells = vec![
            Cell::from(call_display).style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from(track.id.as_str()),
            Cell::from(Line::from(status_span)),
            Cell::from(dist_str).style(Style::default().fg(TELETEXT_CYAN)),
            Cell::from(brg_str),
            Cell::from(branch_str),
            Cell::from(class_str),
            Cell::from(alt_str),
            Cell::from(spd_str),
            Cell::from(sqwk_str),
            Cell::from(theater_str).style(Style::default().fg(theater_col)),
        ];

        rows.push(Row::new(cells).style(row_style));

        let row_area = Rect {
            x: table_area.x + 1,
            y: table_area.y + 2 + offset as u16,
            width: table_area.width.saturating_sub(2),
            height: 1,
        };
        state.hit_tester.register(row_area, HitTarget::TableRow(20, orig_idx));
    }

    let widths = [
        Constraint::Length(13), // Callsign
        Constraint::Length(8),  // ICAO
        Constraint::Length(10), // Sınıf
        Constraint::Length(10), // Mesafe
        Constraint::Length(9),  // Kerteriz
        Constraint::Length(19), // Operatör
        Constraint::Length(22), // Gövde
        Constraint::Length(9),  // İrtifa
        Constraint::Length(8),  // Hız
        Constraint::Length(7),  // Squawk
        Constraint::Min(9),     // Bölge / AOR
    ];

    let table = Table::new(rows, widths).header(header).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(format!(
                " HAVA SAHASI RADARI (SECILI: #{} / TOPLAM: {}) -- [Sutun Basligina Tikla: Sirala | Satira Tikla: Denetci] ",
                (state.air_scroll + 1).min(total_air),
                total_air
            )),
    );

    frame.render_widget(table, table_area);

    if total_air > rows_height {
        let mut sb_state = ScrollbarState::new(total_air).position(state.air_scroll);
        let sb = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(TELETEXT_CYAN).bg(BG_SURFACE));
        frame.render_stateful_widget(sb, table_area, &mut sb_state);
    }
}
