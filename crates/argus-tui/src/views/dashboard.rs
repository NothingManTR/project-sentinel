use crate::widgets::{inspector::render_inspector_panel, p100_map::render_p100_map};
use argus_core::geo::{bearing_sector_tr, haversine_km, initial_bearing_deg};
use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use argus_core::types::Domain;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

pub fn render_dashboard_view(frame: &mut Frame, area: Rect, state: &mut AppState) {
    // [H-2] Panel Buyutme (Z / Bosluk / 1-6) Destegi
    if let Some(zoomed) = state.zoomed_tile {
        match zoomed {
            0 => {
                let map_block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(BORDER_ACTIVE))
                    .title(" [ [0] COGRAFI OPERASYON SAHASI - TAM EKRAN (Z/Esc: Kucult) ] ")
                    .style(Style::default().bg(BG_PRIMARY));
                let map_inner = map_block.inner(area);
                frame.render_widget(map_block, area);
                render_p100_map(frame, map_inner, state);
                return;
            }
            1 => {
                render_osint_and_cyber_feed(frame, area, state);
                return;
            }
            2 => {
                render_proximity_radar_table(frame, area, state);
                return;
            }
            3 => {
                render_sat_and_seismic_feed(frame, area, state);
                return;
            }
            4 => {
                render_sigint_eam_feed(frame, area, state);
                return;
            }
            5 => {
                render_inspector_panel(frame, area, state);
                return;
            }
            _ => {}
        }
    }

    // 3-Sutunlu C4ISR Komuta Merkezi Mimarisi:
    // Sol (%48): Taktik Harita [0] (Ust) + Yakinlik Radari Tablosu [2] (Alt)
    // Orta (%27): Canli Coklu-Istihbarat Akis Sutunu ([4] SIGINT EAM + [3] Uydu/Sismik/NOTAM + [1] OSINT/Catisma/Siber)
    // Sag (%25): [5] Kalici Hedef Denetcisi, Telemetri ve Operasyonel Komut Paneli
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(48),
            Constraint::Percentage(27),
            Constraint::Percentage(25),
        ])
        .split(area);

    // =========================================================================
    // SUTUN 1: TAKTIK HARITA (UST %56) + YAKINLIK RADARI TABLOSU (ALT %44)
    // =========================================================================
    let left_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(56), Constraint::Percentage(44)])
        .split(cols[0]);

    let map_border = if state.active_tile == 0 {
        BORDER_ACTIVE
    } else {
        BORDER_DIM
    };
    let map_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(map_border))
        .title(" [ [1] COGRAFI SAHASI (Z: Buyut | Tikla: Sec | Tekerlek: Zoom) ] ")
        .style(Style::default().bg(BG_PRIMARY));
    let map_inner = map_block.inner(left_rows[0]);
    frame.render_widget(map_block, left_rows[0]);
    render_p100_map(frame, map_inner, state);

    render_proximity_radar_table(frame, left_rows[1], state);

    // =========================================================================
    // SUTUN 2: CANLI COKLU-ISTIHBARAT AKIS SUTUNU (3 DIKEY PANEL)
    // =========================================================================
    let mid_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(cols[1]);

    render_sigint_eam_feed(frame, mid_rows[0], state);
    render_sat_and_seismic_feed(frame, mid_rows[1], state);
    render_osint_and_cyber_feed(frame, mid_rows[2], state);

    // =========================================================================
    // SUTUN 3: KALICI HEDEF DENETCISI VE KOMUT PANELI
    // =========================================================================
    render_inspector_panel(frame, cols[2], state);
}

fn render_proximity_radar_table(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let header_cells = [
        "HEDEF / CAGRI",
        "ALAN",
        "STATU",
        "MESAFE",
        "KERTERIZ",
        "IRTIFA/ROTA",
        "HIZ",
    ]
    .into_iter()
    .map(|h| Cell::from(h).style(Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells)
        .height(1)
        .style(Style::default().bg(BG_SURFACE));

    let st_lat = state.station_lat;
    let st_lon = state.station_lon;
    let q_lower = state.search_query.to_lowercase();

    state.ensure_proximity_indices();
    let visible_rows = area.height.saturating_sub(3) as usize;
    let (total_targets, page_indices): (usize, Vec<(usize, usize)>) = if q_lower.is_empty() {
        let total = state.cached_prox_indices.len();
        let max_scroll = total.saturating_sub(1);
        if state.target_scroll > max_scroll {
            state.target_scroll = max_scroll;
        }
        let items = state
            .cached_prox_indices
            .iter()
            .skip(state.target_scroll)
            .take(visible_rows)
            .map(|&buf_idx| (buf_idx, buf_idx))
            .collect();
        (total, items)
    } else {
        let matched: Vec<(usize, usize)> = state
            .cached_prox_indices
            .iter()
            .filter_map(|&buf_idx| {
                let t = state.targets.get(buf_idx)?;
                if t.callsign.to_lowercase().contains(&q_lower)
                    || t.id.to_lowercase().contains(&q_lower)
                    || t.info.to_lowercase().contains(&q_lower)
                {
                    Some((buf_idx, buf_idx))
                } else {
                    None
                }
            })
            .collect();
        let total = matched.len();
        let max_scroll = total.saturating_sub(1);
        if state.target_scroll > max_scroll {
            state.target_scroll = max_scroll;
        }
        let items = matched
            .into_iter()
            .skip(state.target_scroll)
            .take(visible_rows)
            .collect();
        (total, items)
    };

    let mut rows = Vec::new();
    for (offset, (orig_idx, buf_idx)) in page_indices.into_iter().enumerate() {
        let Some(track) = state.targets.get(buf_idx) else {
            continue;
        };
        let is_selected = state.selected_target_id.as_deref() == Some(track.id.as_str());
        let d_km = haversine_km(st_lat, st_lon, track.geo.lat, track.geo.lon);
        let brg = initial_bearing_deg(st_lat, st_lon, track.geo.lat, track.geo.lon);

        let domain_tag = match track.domain {
            Domain::Air => Span::styled("HAVA ", Style::default().fg(TELETEXT_CYAN)),
            Domain::Sea => Span::styled("DENIZ", Style::default().fg(TELETEXT_GREEN)),
            _ => Span::styled("DIGER", Style::default().fg(FG_SECONDARY)),
        };

        let stat_tag = if track.is_military {
            Span::styled(
                "ASKERI",
                Style::default().fg(TELETEXT_RED).add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled("SIVIL ", Style::default().fg(FG_SECONDARY))
        };

        let alt_or_cog = if track.domain == Domain::Air {
            track
                .geo
                .alt_m
                .map(|a| format!("FL{:.0}", a * 3.28084 / 100.0))
                .unwrap_or_else(|| "N/A".to_string())
        } else {
            track
                .heading_deg
                .map(|h| format!("{:03.0} deg", h))
                .unwrap_or_else(|| "-".to_string())
        };

        let spd_str = track
            .speed_kts
            .map(|s| format!("{:.0}kt", s))
            .unwrap_or_else(|| "-".to_string());

        let row_style = if is_selected {
            Style::default()
                .fg(TELETEXT_YELLOW)
                .bg(BG_HIGHLIGHT)
                .add_modifier(Modifier::BOLD)
        } else if offset % 2 == 0 {
            Style::default().fg(FG_PRIMARY).bg(BG_PANEL)
        } else {
            Style::default().fg(FG_PRIMARY).bg(BG_PRIMARY)
        };

        let cs_label = if is_selected {
            format!("> {}", track.callsign)
        } else {
            format!("  {}", track.callsign)
        };

        rows.push(
            Row::new(vec![
                Cell::from(cs_label),
                Cell::from(Line::from(domain_tag)),
                Cell::from(Line::from(stat_tag)),
                Cell::from(format!("{:.0} km", d_km)).style(Style::default().fg(TELETEXT_CYAN)),
                Cell::from(format!("{:03.0} {}", brg, bearing_sector_tr(brg))),
                Cell::from(alt_or_cog),
                Cell::from(spd_str),
            ])
            .style(row_style),
        );

        let row_area = Rect {
            x: area.x + 1,
            y: area.y + 2 + offset as u16,
            width: area.width.saturating_sub(2),
            height: 1,
        };
        state
            .hit_tester
            .register(row_area, HitTarget::TableRow(2, orig_idx));
    }

    let widths = [
        Constraint::Length(14),
        Constraint::Length(6),
        Constraint::Length(7),
        Constraint::Length(9),
        Constraint::Length(8),
        Constraint::Length(9),
        Constraint::Min(6),
    ];

    let radar_border = if state.active_tile == 2 { BORDER_ACTIVE } else { BORDER_DIM };
    let table = Table::new(rows, widths).header(header).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(radar_border))
            .title(format!(
                " [ [3] BIRLESIK HAVA & DENIZ RADAR AKISI ({} Hedef) ] ",
                total_targets
            ))
            .style(Style::default().bg(BG_PANEL)),
    );
    frame.render_widget(table, area);
}

fn render_sigint_eam_feed(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(
            format!("SDR: {:.1}kHz │ ", state.current_frequency_khz),
            Style::default().fg(TELETEXT_CYAN).add_modifier(Modifier::BOLD),
        ),
        Span::styled(&state.sdr_status, Style::default().fg(TELETEXT_GREEN)),
    ]));

    let mut eams: Vec<_> = state.eam_messages.iter().collect();
    eams.reverse();
    let avail = area.height.saturating_sub(3) as usize;

    for (idx, msg) in eams.iter().take(avail).enumerate() {
        let col = if msg.is_flash_override {
            TELETEXT_RED
        } else {
            TELETEXT_YELLOW
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", msg.timestamp.format("%H:%M")),
                Style::default().fg(FG_MUTED),
            ),
            Span::styled(
                format!("[{}] ", msg.preamble),
                Style::default().fg(col).add_modifier(Modifier::BOLD),
            ),
            Span::styled(msg.content.clone(), Style::default().fg(FG_PRIMARY)),
        ]));
        let ry = area.y + 2 + idx as u16;
        if ry < area.y + area.height.saturating_sub(1) {
            state.hit_tester.register(
                Rect {
                    x: area.x + 1,
                    y: ry,
                    width: area.width.saturating_sub(2),
                    height: 1,
                },
                HitTarget::TableRow(30, idx),
            );
        }
    }

    let sig_border = if state.active_tile == 4 { BORDER_ACTIVE } else { BORDER_DIM };
    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(sig_border))
            .title(" [ [5] SIGINT // HFGCS EAM & KIWISDR (F5) ] ")
            .style(Style::default().bg(BG_PANEL)),
    );
    frame.render_widget(p, area);
}

fn render_sat_and_seismic_feed(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let mut lines = Vec::new();
    let avail = area.height.saturating_sub(2) as usize;
    let part = (avail / 3).max(2);

    let mut row_y = area.y + 1;
    for (idx, sat) in state.satellites.iter().take(part.min(avail)).enumerate() {
        let status = if sat.el_deg > 0.0 {
            format!("EL:{:+.0} deg (UFUKTA)", sat.el_deg)
        } else {
            format!("AOS: {}dk", sat.next_pass_secs / 60)
        };
        lines.push(Line::from(vec![
            Span::styled("[UYDU] ", Style::default().fg(TELETEXT_MAGENTA).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:<14} ", sat.name), Style::default().fg(FG_PRIMARY)),
            Span::styled(format!("{:.0}km │ {}", sat.alt_km, status), Style::default().fg(TELETEXT_CYAN)),
        ]));
        if row_y < area.y + area.height.saturating_sub(1) {
            state.hit_tester.register(
                Rect {
                    x: area.x + 1,
                    y: row_y,
                    width: area.width.saturating_sub(2),
                    height: 1,
                },
                HitTarget::TableRow(3, idx),
            );
            row_y += 1;
        }
    }

    for (q_idx, q) in state
        .earthquakes
        .iter()
        .enumerate()
        .take(part.min(avail.saturating_sub(lines.len())))
    {
        let col = if q.mag >= 5.0 {
            TELETEXT_RED
        } else {
            TELETEXT_YELLOW
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("[M{:.1}] ", q.mag),
                Style::default().fg(col).add_modifier(Modifier::BOLD),
            ),
            Span::styled(q.place.clone(), Style::default().fg(FG_PRIMARY)),
        ]));
        if row_y < area.y + area.height.saturating_sub(1) {
            state.hit_tester.register(
                Rect {
                    x: area.x + 1,
                    y: row_y,
                    width: area.width.saturating_sub(2),
                    height: 1,
                },
                HitTarget::TableRow(50, q_idx),
            );
            row_y += 1;
        }
    }

    for (n_idx, aw) in state
        .airspace_warnings
        .iter()
        .enumerate()
        .take(avail.saturating_sub(lines.len()))
    {
        let col = match aw.kind.as_str() {
            "NOTAM-CONFLICT" => TELETEXT_RED,
            "GPS-JAM" => TELETEXT_MAGENTA,
            _ => TELETEXT_YELLOW,
        };
        lines.push(Line::from(vec![
            Span::styled(
                "[NOTAM] ",
                Style::default().fg(col).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}: {}", aw.fir_or_zone, aw.hazard),
                Style::default().fg(FG_PRIMARY),
            ),
        ]));
        if row_y < area.y + area.height.saturating_sub(1) {
            state.hit_tester.register(
                Rect {
                    x: area.x + 1,
                    y: row_y,
                    width: area.width.saturating_sub(2),
                    height: 1,
                },
                HitTarget::TableRow(54, n_idx),
            );
            row_y += 1;
        }
    }

    let sat_border = if state.active_tile == 3 { BORDER_ACTIVE } else { BORDER_DIM };
    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(sat_border))
            .title(" [ [4] UYDU (F8), SISMIK & NOTAM (F9) ] ")
            .style(Style::default().bg(BG_PANEL)),
    );
    frame.render_widget(p, area);
}

fn render_osint_and_cyber_feed(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let mut lines = Vec::new();
    let avail = area.height.saturating_sub(2) as usize;
    let part = (avail / 3).max(2);
    let mut row_y = area.y + 1;

    for (idx, item) in state.news.iter().enumerate().take(part.min(avail)) {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", item.published.format("%H:%M")),
                Style::default().fg(FG_MUTED),
            ),
            Span::styled(
                format!("[{}] ", item.source),
                Style::default().fg(TELETEXT_YELLOW),
            ),
            Span::styled(item.title.clone(), Style::default().fg(FG_PRIMARY)),
        ]));
        if row_y < area.y + area.height.saturating_sub(1) {
            state.hit_tester.register(
                Rect {
                    x: area.x + 1,
                    y: row_y,
                    width: area.width.saturating_sub(2),
                    height: 1,
                },
                HitTarget::TableRow(1, idx),
            );
            row_y += 1;
        }
    }

    for (a_idx, ev) in state
        .acled_events
        .iter()
        .enumerate()
        .take(part.min(avail.saturating_sub(lines.len())))
    {
        let col = if ev.fatalities > 0 { TELETEXT_RED } else { TELETEXT_YELLOW };
        lines.push(Line::from(vec![
            Span::styled(
                "[CATISMA] ",
                Style::default().fg(col).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}: {} ({})", ev.country, ev.location, ev.event_type),
                Style::default().fg(FG_PRIMARY),
            ),
        ]));
        if row_y < area.y + area.height.saturating_sub(1) {
            state.hit_tester.register(
                Rect {
                    x: area.x + 1,
                    y: row_y,
                    width: area.width.saturating_sub(2),
                    height: 1,
                },
                HitTarget::TableRow(52, a_idx),
            );
            row_y += 1;
        }
    }

    for (c_idx, inc) in state
        .cyber_incidents
        .iter()
        .enumerate()
        .take(avail.saturating_sub(lines.len()))
    {
        let tag = inc.cve.as_deref().or(inc.asn.as_deref()).unwrap_or("SIBER");
        lines.push(Line::from(vec![
            Span::styled(
                format!("[{}] ", tag),
                Style::default().fg(TELETEXT_RED).add_modifier(Modifier::BOLD),
            ),
            Span::styled(inc.title.clone(), Style::default().fg(FG_PRIMARY)),
        ]));
        if row_y < area.y + area.height.saturating_sub(1) {
            state.hit_tester.register(
                Rect {
                    x: area.x + 1,
                    y: row_y,
                    width: area.width.saturating_sub(2),
                    height: 1,
                },
                HitTarget::TableRow(4, c_idx),
            );
            row_y += 1;
        }
    }

    let os_border = if state.active_tile == 1 { BORDER_ACTIVE } else { BORDER_DIM };
    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(os_border))
            .title(" [ [2] OSINT HABER, CATISMA (F6) & SIBER (F7) ] ")
            .style(Style::default().bg(BG_PANEL)),
    );
    frame.render_widget(p, area);
}
