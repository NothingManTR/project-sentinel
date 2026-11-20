use crate::widgets::{inspector::render_inspector_panel, p100_map::render_p100_map};
use argus_core::geo::{haversine_km, initial_bearing_deg, radar_horizon_km};
use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

pub fn render_geoint_view(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if area.width >= 104 {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(27),
                Constraint::Min(40),
                Constraint::Percentage(26),
            ])
            .split(area);

        render_geoint_sidebar(frame, cols[0], state);

        let map_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_ACTIVE))
            .title(" [ GEOINT // TAKTIK OPERASYON HARITASI (V:Stil │ P:Sekil │ G:Koord │ W:Web) ] ")
            .style(Style::default().bg(BG_PRIMARY));
        let map_inner = map_block.inner(cols[1]);
        frame.render_widget(map_block, cols[1]);
        render_p100_map(frame, map_inner, state);

        render_inspector_panel(frame, cols[2], state);
    } else {
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
            .split(area);

        let map_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_ACTIVE))
            .title(" [ GEOINT // TAM EKRAN TAKTIK OPERASYON HARITASI (V:Mod | W:Web | +/-:Zoom) ] ")
            .style(Style::default().bg(BG_PRIMARY));
        let map_inner = map_block.inner(main_chunks[0]);
        frame.render_widget(map_block, main_chunks[0]);
        render_p100_map(frame, map_inner, state);

        render_inspector_panel(frame, main_chunks[1], state);
    }
}

fn render_geoint_sidebar(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // Koordinata Git (Go-To Lat,Lon)
            Constraint::Min(10),    // Katman Istatistikleri (Tiklanabilir)
            Constraint::Length(10), // Taktik Cetvel & DVR Ozeti
        ])
        .split(area);

    // 1. Koordinata Git (Go-To Lat,Lon)
    let goto_border = if state.geoint_goto_active {
        TELETEXT_YELLOW
    } else {
        BORDER_DIM
    };
    let goto_box = if state.geoint_goto_active {
        format!(" [ {}_ ]", state.geoint_goto_input)
    } else if state.geoint_goto_input.is_empty() {
        format!(" {:.3}, {:.3}", state.map_center_lat, state.map_center_lon)
    } else {
        format!(" {}", state.geoint_goto_input)
    };
    let goto_p = Paragraph::new(vec![
        Line::from(Span::styled(
            "Ornek: 39.92, 32.85",
            Style::default().fg(FG_MUTED),
        )),
        Line::from(Span::styled(
            goto_box,
            Style::default()
                .fg(if state.geoint_goto_active { TELETEXT_BLACK } else { TELETEXT_CYAN })
                .bg(if state.geoint_goto_active { TELETEXT_YELLOW } else { BG_SURFACE })
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            if state.geoint_goto_active {
                "Enter: Git │ Esc: Iptal"
            } else {
                "Tikla / [G] Koord Gir"
            },
            Style::default().fg(FG_SECONDARY),
        )),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(goto_border))
            .title(" [ KOORDINATA GIT ] ")
            .style(Style::default().bg(BG_PANEL)),
    );
    frame.render_widget(goto_p, chunks[0]);
    state.hit_tester.register(chunks[0], HitTarget::GeoGoToInput);

    // 2. Canli Katman Istatistikleri & Hizli Anahtarlar
    let mil_air = state.air_targets.iter().filter(|t| t.is_military).count();
    let mil_sea = state.sea_targets.iter().filter(|t| t.is_military).count();
    let layer_items: [(&str, usize, String, ratatui::style::Color); 11] = [
        ("Hava (ADS-B)", 0, format!("{} ({}M)", state.air_targets.len(), mil_air), TELETEXT_CYAN),
        ("Deniz (AIS)", 1, format!("{} ({}W)", state.sea_targets.len(), mil_sea), TELETEXT_GREEN),
        ("Uydu (SGP4)", 2, format!("{}", state.satellites.len()), TELETEXT_MAGENTA),
        ("Deprem/Termal", 3, format!("{}/{}", state.earthquakes.len(), state.firms_fires.len()), TELETEXT_YELLOW),
        ("Catisma (GDELT)", 4, format!("{}", state.acled_events.len()), TELETEXT_RED),
        ("EW GPS-JAM", 5, format!("{}", state.gps_jamming_zones.len()), TELETEXT_MAGENTA),
        ("KiwiSDR HF", 7, format!("{}", state.sdr_nodes.len()), TELETEXT_GREEN),
        ("Denizalti Kbl", 8, format!("{}", state.subsea_cables.len()), TELETEXT_CYAN),
        ("Hava Durumu", 9, format!("{}", state.weather_points.len()), TELETEXT_CYAN),
        ("K-4 Fuzyon", 10, format!("{}", state.fusion_clusters.len()), TELETEXT_RED),
        ("NOTAM/SIGMET", 11, format!("{}", state.airspace_warnings.len()), TELETEXT_YELLOW),
    ];

    let max_l = chunks[1].height.saturating_sub(2) as usize;
    let mut layer_lines = Vec::new();
    for (row_i, (name, bit, cnt, col)) in layer_items.iter().take(max_l).enumerate() {
        let on = (state.geoint_layers & (1u16 << *bit)) != 0;
        let mark = if on { "[x]" } else { "[ ]" };
        layer_lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", mark),
                Style::default()
                    .fg(if on { TELETEXT_GREEN } else { FG_MUTED })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:<13}", name),
                Style::default().fg(if on { FG_PRIMARY } else { FG_MUTED }),
            ),
            Span::styled(
                cnt.clone(),
                Style::default()
                    .fg(if on { *col } else { FG_MUTED })
                    .add_modifier(if on { Modifier::BOLD } else { Modifier::empty() }),
            ),
        ]));
        state.hit_tester.register(
            Rect {
                x: chunks[1].x + 1,
                y: chunks[1].y + 1 + row_i as u16,
                width: chunks[1].width.saturating_sub(2),
                height: 1,
            },
            HitTarget::GeointLayer(*bit),
        );
    }

    let layers_p = Paragraph::new(layer_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(" [ KATMANLAR (TIKLA) ] ")
            .style(Style::default().bg(BG_PANEL)),
    );
    frame.render_widget(layers_p, chunks[1]);

    // 3. Taktik Cetvel, Radar Ufku & DVR Ozeti
    let mut sum_lines = Vec::new();
    if let Some(r) = &state.tactical_ruler {
        let (la, loa, lbl_a) = &r.pin_a;
        sum_lines.push(Line::from(vec![
            Span::styled("PinA: ", Style::default().fg(TELETEXT_YELLOW)),
            Span::styled(format!("{} ({:.1},{:.1})", lbl_a, la, loa), Style::default().fg(FG_PRIMARY)),
        ]));
        if let Some((lb, lob, lbl_b)) = &r.pin_b {
            let d_km = haversine_km(*la, *loa, *lb, *lob);
            let brg = initial_bearing_deg(*la, *loa, *lb, *lob);
            sum_lines.push(Line::from(vec![
                Span::styled("PinB: ", Style::default().fg(TELETEXT_CYAN)),
                Span::styled(lbl_b.clone(), Style::default().fg(FG_PRIMARY)),
            ]));
            sum_lines.push(Line::from(Span::styled(
                format!("Mes: {:.0}km ({:.0}NM) {:03.0}°", d_km, d_km / 1.852, brg),
                Style::default().fg(TELETEXT_GREEN).add_modifier(Modifier::BOLD),
            )));
        }
        if let Some(rk) = r.threat_ring_km {
            sum_lines.push(Line::from(Span::styled(
                format!("Halka: {:.0} km │ LOS(10km): {:.0}km", rk, radar_horizon_km(50.0, 10000.0)),
                Style::default().fg(TELETEXT_RED),
            )));
        }
    } else {
        sum_lines.push(Line::from(Span::styled(
            "Cetvel: Kapali (PinA/PinB)",
            Style::default().fg(FG_MUTED),
        )));
    }

    sum_lines.push(Line::from(Span::styled(
        format!(
            "DVR: {} │ {} Kare",
            if state.dvr.replay_offset_mins == 0 && state.dvr.active_archive.is_none() {
                "CANLI".to_string()
            } else {
                format!("-{}dk ({}x)", state.dvr.replay_offset_mins, state.dvr.play_speed)
            },
            state.dvr.snapshots.len()
        ),
        Style::default().fg(if state.dvr.is_playing { TELETEXT_YELLOW } else { TELETEXT_CYAN }),
    )));
    sum_lines.push(Line::from(Span::styled(
        format!("Dark-Ship: {} │ AAR: {}", state.dark_ships.len(), state.aar_pairs.len()),
        Style::default().fg(TELETEXT_MAGENTA),
    )));
    sum_lines.push(Line::from(Span::styled(
        format!("Stil: {}", state.map_style.label()),
        Style::default().fg(FG_SECONDARY),
    )));

    let sum_p = Paragraph::new(sum_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(" [ CETVEL & DVR ] ")
            .style(Style::default().bg(BG_PANEL)),
    );
    frame.render_widget(sum_p, chunks[2]);
}
