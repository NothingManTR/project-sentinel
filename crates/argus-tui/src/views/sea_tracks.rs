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

pub fn render_sea_tracks_view(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if area.width < 20 || area.height < 6 {
        return;
    }

    frame.render_widget(Clear, area);

    // Sol (%68): Deniz Filosu Tablosu | Sag (%32): Canli Deniz Mini-Haritasi (Ust) + Teknik Denetci (Alt)
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
        .title(" [ DENIZ & BOGAZLAR MINI-HARITASI ] ")
        .style(Style::default().bg(BG_PRIMARY));
    let mini_map_inner = mini_map_block.inner(right_chunks[0]);
    frame.render_widget(mini_map_block, right_chunks[0]);
    render_p100_map(frame, mini_map_inner, state);

    render_inspector_panel(frame, right_chunks[1], state);

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Taktik HUD Ozet Kartlari (Baslik + Deger)
            Constraint::Length(1), // 6-Bogaz Gatekeeper + Dark-Ship Seridi
            Constraint::Min(6),    // Ana Donanma Tablosu
        ])
        .split(left_area);

    let hud_bar = main_layout[0];
    let gate_bar = main_layout[1];
    let table_area = main_layout[2];

    let all_vessels = state.sea_targets.iter().count();
    let warship_count = state
        .sea_targets
        .iter()
        .filter(|t| {
            t.is_military
                || t.vessel_type.as_deref() == Some("Warship")
                || t.vessel_type.as_deref() == Some("Savaş Gemisi")
                || t.callsign.starts_with("TCG")
                || t.callsign.starts_with("USS")
                || t.callsign.starts_with("HMS")
        })
        .count();
    let cg_count = state
        .sea_targets
        .iter()
        .filter(|t| {
            t.vessel_type.as_deref() == Some("Coast Guard")
                || t.vessel_type.as_deref() == Some("Sahil Güvenlik")
                || t.callsign.starts_with("TCSG")
        })
        .count();
    let com_count = all_vessels.saturating_sub(warship_count + cg_count);
    let _ = com_count;

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
        ("IZLENEN GEMI", format!("{} Aktif Gemi Izi", all_vessels), TELETEXT_GREEN),
        (
            "HARP GEMILERI",
            format!("{} Muharip Platform", warship_count),
            if warship_count > 0 { TELETEXT_RED } else { FG_SECONDARY },
        ),
        (
            "SAHIL GUVENLIK",
            format!("{} Karakol/SAR Botu", cg_count),
            if cg_count > 0 { TELETEXT_CYAN } else { FG_SECONDARY },
        ),
        ("VERI KAYNAGI", "AISStream VHF Canli".to_string(), TELETEXT_YELLOW),
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

    // 1.5 6-Bogaz Gatekeeper & Dark-Ship Seridi
    {
        let dark_cnt = state.dark_ships.len();
        let dark_lbl = format!("[DARK-SHIP:{}]", dark_cnt);
        let dark_w = dark_lbl.chars().count() as u16;
        let prefix = " BOGAZ:";
        let prefix_w = prefix.chars().count() as u16;

        let mut gate_spans = vec![Span::styled(
            prefix,
            Style::default().fg(TELETEXT_CYAN).bg(BG_SURFACE).add_modifier(Modifier::BOLD),
        )];
        let mut gx = gate_bar.x + prefix_w;
        let max_gate_x = gate_bar.x + gate_bar.width.saturating_sub(dark_w + 1);

        if state.strait_gates.is_empty() {
            let wait_lbl = " Kapilar hazirlaniyor...";
            let ww = wait_lbl.chars().count() as u16;
            if gx + ww <= max_gate_x {
                gate_spans.push(Span::styled(wait_lbl, Style::default().fg(FG_MUTED).bg(BG_SURFACE)));
                gx += ww;
            }
        } else {
            let gates: Vec<_> = state
                .strait_gates
                .iter()
                .enumerate()
                .map(|(idx, g)| {
                    let code: String = g.id.chars().take(3).collect::<String>().to_uppercase();
                    let badge = format!("[{}:{}/{}]", code, g.fwd_count, g.rev_count);
                    let fg = if g.mil_count > 0 {
                        TELETEXT_RED
                    } else if g.fwd_count + g.rev_count > 0 {
                        TELETEXT_GREEN
                    } else {
                        FG_SECONDARY
                    };
                    (idx, badge, fg)
                })
                .collect();

            for (idx, badge, fg) in gates {
                let bw = badge.chars().count() as u16;
                if gx + 1 + bw <= max_gate_x {
                    gate_spans.push(Span::styled(" ", Style::default().bg(BG_SURFACE)));
                    gx += 1;
                    gate_spans.push(Span::styled(
                        badge,
                        Style::default().fg(fg).bg(BG_PANEL).add_modifier(Modifier::BOLD),
                    ));
                    state.hit_tester.register(
                        Rect { x: gx, y: gate_bar.y, width: bw, height: 1 },
                        HitTarget::StraitGate(idx),
                    );
                    gx += bw;
                }
            }
        }

        let end_x = gate_bar.x + gate_bar.width;
        if end_x >= gx + dark_w {
            let pad = (end_x - gx - dark_w) as usize;
            if pad > 0 {
                gate_spans.push(Span::styled(" ".repeat(pad), Style::default().bg(BG_SURFACE)));
            }
            gate_spans.push(Span::styled(
                dark_lbl,
                if dark_cnt > 0 {
                    Style::default().fg(TELETEXT_WHITE).bg(TELETEXT_RED).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(FG_SECONDARY).bg(BG_SURFACE).add_modifier(Modifier::BOLD)
                },
            ));
        }

        frame.render_widget(
            Paragraph::new(Line::from(gate_spans)).style(Style::default().bg(BG_SURFACE)),
            gate_bar,
        );
    }

    state.ensure_sea_indices();
    let q_lower = state.search_query.to_lowercase();
    let filtered_indices: Option<Vec<(usize, usize)>> = if q_lower.is_empty() {
        None
    } else {
        Some(
            state
                .cached_sea_indices
                .iter()
                .enumerate()
                .filter_map(|(orig_idx, &buf_idx)| {
                    let t = state.sea_targets.get(buf_idx)?;
                    if t.callsign.to_lowercase().contains(&q_lower)
                        || t.id.to_lowercase().contains(&q_lower)
                        || t.operator_country.as_deref().unwrap_or("").to_lowercase().contains(&q_lower)
                        || t.operator_branch.as_deref().unwrap_or("").to_lowercase().contains(&q_lower)
                        || t.info.to_lowercase().contains(&q_lower)
                    {
                        Some((orig_idx, buf_idx))
                    } else {
                        None
                    }
                })
                .collect(),
        )
    };
    let total_sea = match &filtered_indices {
        Some(v) => v.len(),
        None => state.cached_sea_indices.len(),
    };

    let sort_ind = |col: usize| -> &'static str {
        if state.sea_sort_col == col {
            if state.sea_sort_asc { " (+)" } else { " (-)" }
        } else {
            ""
        }
    };

    let h_name = format!("GEMI ADI{}", sort_ind(1));
    let h_stat = format!("SINIF{}", sort_ind(2));
    let h_dist = format!("MESAFE{}", sort_ind(4));
    let h_spd = format!("HIZ{}", sort_ind(3));

    let header_titles = [
        h_name.as_str(),
        "MMSI",
        h_stat.as_str(),
        h_dist.as_str(),
        "KERTERIZ",
        "BAYRAK DEVLETI",
        "GEMI TIPI VE KUNYESI",
        h_spd.as_str(),
        "ROTA",
    ];
    let header_cells = header_titles
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)));

    let header = Row::new(header_cells).style(Style::default().bg(BG_SURFACE)).height(1);

    if table_area.height >= 3 {
        let header_y = table_area.y + 1;
        state.hit_tester.register(
            Rect { x: table_area.x + 1, y: header_y, width: 18, height: 1 },
            HitTarget::SortColumn(21, 1),
        );
        state.hit_tester.register(
            Rect { x: table_area.x + 31, y: header_y, width: 15, height: 1 },
            HitTarget::SortColumn(21, 2),
        );
        state.hit_tester.register(
            Rect { x: table_area.x + 47, y: header_y, width: 11, height: 1 },
            HitTarget::SortColumn(21, 4),
        );
        state.hit_tester.register(
            Rect { x: table_area.x + 112, y: header_y, width: 9, height: 1 },
            HitTarget::SortColumn(21, 3),
        );
    }

    let rows_height = table_area.height.saturating_sub(3) as usize;
    let max_scroll = total_sea.saturating_sub(1);
    if state.sea_scroll > max_scroll {
        state.sea_scroll = max_scroll;
    }
    let mut rows = Vec::new();
    let st_lat = state.station_lat;
    let st_lon = state.station_lon;

    let page_items: Vec<(usize, usize)> = match &filtered_indices {
        Some(v) => v
            .iter()
            .skip(state.sea_scroll)
            .take(rows_height)
            .copied()
            .collect(),
        None => state
            .cached_sea_indices
            .iter()
            .enumerate()
            .skip(state.sea_scroll)
            .take(rows_height)
            .map(|(orig_idx, &buf_idx)| (orig_idx, buf_idx))
            .collect(),
    };

    for (offset, (orig_idx, buf_idx)) in page_items.into_iter().enumerate() {
        let Some(track) = state.sea_targets.get(buf_idx) else {
            continue;
        };
        let is_selected =
            offset == 0 || state.selected_target_id.as_deref() == Some(track.id.as_str());
        let is_followed = state.follow_target_id.as_deref() == Some(track.id.as_str());

        let is_warship = track.is_military
            || track.vessel_type.as_deref() == Some("Warship")
            || track.vessel_type.as_deref() == Some("Savaş Gemisi")
            || track.callsign.starts_with("TCG")
            || track.callsign.starts_with("USS")
            || track.callsign.starts_with("HMS");
        let is_cg = track.vessel_type.as_deref() == Some("Coast Guard")
            || track.vessel_type.as_deref() == Some("Sahil Güvenlik")
            || track.callsign.starts_with("TCSG");

        let status_span = if is_warship {
            Span::styled("[SAVAS GEMISI]", Style::default().fg(TELETEXT_RED).add_modifier(Modifier::BOLD))
        } else if is_cg {
            Span::styled("[SAHIL GUV.]", Style::default().fg(TELETEXT_CYAN).add_modifier(Modifier::BOLD))
        } else {
            Span::styled("[TICARI]", Style::default().fg(TELETEXT_GREEN))
        };

        let d_km = haversine_km(st_lat, st_lon, track.geo.lat, track.geo.lon);
        let brg = initial_bearing_deg(st_lat, st_lon, track.geo.lat, track.geo.lon);
        let dist_str = format!("{:.0} km", d_km);
        let brg_str = format!("{:03.0} {}", brg, bearing_sector_tr(brg));

        let spd_str = track
            .speed_kts
            .map(|s| format!("{:.1}kt", s))
            .unwrap_or_else(|| "0.0kt".to_string());
        let hdg_str = track
            .heading_deg
            .map(|h| format!("{:.0}", h))
            .unwrap_or_else(|| "-".to_string());
        let country_str = track.operator_country.as_deref().unwrap_or("Bilinmeyen");
        let class_str = track
            .airframe_or_vessel_class
            .as_deref()
            .or(track.vessel_type.as_deref())
            .unwrap_or("Kargo / Tanker");

        let name_display = if is_followed {
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
        } else if is_warship {
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
            Cell::from(name_display).style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from(track.id.as_str()),
            Cell::from(Line::from(status_span)),
            Cell::from(dist_str).style(Style::default().fg(TELETEXT_CYAN)),
            Cell::from(brg_str),
            Cell::from(country_str),
            Cell::from(class_str),
            Cell::from(spd_str),
            Cell::from(hdg_str),
        ];

        rows.push(Row::new(cells).style(row_style));

        let row_area = Rect {
            x: table_area.x + 1,
            y: table_area.y + 2 + offset as u16,
            width: table_area.width.saturating_sub(2),
            height: 1,
        };
        state.hit_tester.register(row_area, HitTarget::TableRow(21, orig_idx));
    }

    let widths = [
        Constraint::Length(18), // Gemi Adı
        Constraint::Length(11), // MMSI
        Constraint::Length(15), // Sınıf
        Constraint::Length(10), // Mesafe
        Constraint::Length(9),  // Kerteriz
        Constraint::Length(16), // Bayrak
        Constraint::Min(20),    // Gemi Tipi
        Constraint::Length(9),  // Hız
        Constraint::Length(7),  // Rota
    ];

    let table = Table::new(rows, widths).header(header).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(format!(
                " DENIZ FILOSU & BOGAZLAR (SECILI: #{} / TOPLAM: {}) -- [Sutun Basligina Tikla: Sirala | Satira Tikla: Denetci] ",
                (state.sea_scroll + 1).min(total_sea),
                total_sea
            )),
    );

    frame.render_widget(table, table_area);

    if total_sea > rows_height {
        let mut sb_state = ScrollbarState::new(total_sea).position(state.sea_scroll);
        let sb = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(TELETEXT_GREEN).bg(BG_SURFACE));
        frame.render_stateful_widget(sb, table_area, &mut sb_state);
    }
}
