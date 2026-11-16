use argus_core::geo::{bearing_sector_tr, haversine_km, haversine_nm, initial_bearing_deg};
use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub fn render_inspector_panel(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if area.width < 18 || area.height < 8 {
        return;
    }

    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),   // Seçili Hedef / Kayıt Teknik Telemetrisi
            Constraint::Length(6), // Hızlı Operasyonel Aksiyon Butonları
            Constraint::Length(7), // Canlı Alarm & İzleme Listesi (Watchlist) Özeti
        ])
        .split(area);

    // 1. Seçili Hedef veya Kayıt Detayı
    let mut lines = Vec::new();
    let st_lat = state.station_lat;
    let st_lon = state.station_lon;

    let inspected_track = state
        .selected_target_id
        .as_ref()
        .and_then(|id| {
            state
                .air_targets
                .iter()
                .find(|t| &t.id == id)
                .or_else(|| state.sea_targets.iter().find(|t| &t.id == id))
                .or_else(|| state.targets.iter().find(|t| &t.id == id))
        })
        .cloned()
        .or_else(|| match state.selected_domain {
            20 => state.aircraft_at_sorted(state.selected_row).cloned(),
            21 => state.vessel_list().get(state.selected_row).cloned(),
            _ => state.aircraft_at_sorted(0).cloned(),
        });

    if state.selected_domain == 30 {
        let mut eams: Vec<_> = state.eam_messages.iter().collect();
        eams.reverse();
        if let Some(msg) = eams.get(state.selected_row).or_else(|| eams.first()) {
            lines.push(Line::from(Span::styled(
                format!("SIGINT EAM // {}", msg.preamble),
                Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(vec![
                Span::styled("Zaman (UTC) : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(msg.timestamp.format("%Y-%m-%d %H:%M:%S").to_string(), Style::default().fg(FG_PRIMARY)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Frekans     : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(format!("{:.1} kHz {}", msg.frequency_khz, msg.mode), Style::default().fg(TELETEXT_CYAN)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Istasyon    : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(msg.auth_station.clone(), Style::default().fg(TELETEXT_GREEN)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Mesaj Boyutu: ", Style::default().fg(FG_SECONDARY)),
                Span::styled(format!("{} karakter", msg.length_chars), Style::default().fg(FG_PRIMARY)),
            ]));
            lines.push(Line::from(Span::styled(
                "--- KOD BLOKLARI ---",
                Style::default().fg(BORDER_ACTIVE),
            )));
            lines.push(Line::from(Span::styled(
                msg.content.clone(),
                Style::default().fg(TELETEXT_WHITE).add_modifier(Modifier::BOLD),
            )));
        }
    } else if state.selected_domain == 3 {
        if let Some(sat) = state.satellites.get(state.selected_row).or_else(|| state.satellites.first()) {
            let d_km = haversine_km(st_lat, st_lon, sat.lat, sat.lon);
            lines.push(Line::from(Span::styled(
                format!("UYDU // {} (#{})", sat.name, sat.norad_id),
                Style::default().fg(TELETEXT_MAGENTA).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(vec![
                Span::styled("Alt-Nokta   : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(format!("{:.3}N, {:.3}E", sat.lat, sat.lon), Style::default().fg(FG_PRIMARY)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Irtifa / Hiz: ", Style::default().fg(FG_SECONDARY)),
                Span::styled(format!("{:.1} km | {:.2} km/s", sat.alt_km, sat.velocity_kms), Style::default().fg(TELETEXT_CYAN)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Azimut / El : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(format!("{:.1} deg / {:+.1} deg", sat.az_deg, sat.el_deg), Style::default().fg(TELETEXT_GREEN)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Yuzey Mesafe: ", Style::default().fg(FG_SECONDARY)),
                Span::styled(format!("{:.0} km", d_km), Style::default().fg(FG_PRIMARY)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Sonraki AOS : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    if sat.el_deg > 0.0 {
                        "UFUK USTUNDE (AKTIF GECIS)".to_string()
                    } else {
                        format!("T-{} dk {} sn", sat.next_pass_secs / 60, sat.next_pass_secs % 60)
                    },
                    Style::default().fg(TELETEXT_YELLOW),
                ),
            ]));
        }
    } else if state.selected_domain == 50 {
        if let Some(q) = state.earthquakes.get(state.selected_row).or_else(|| state.earthquakes.get(0)) {
            let d_km = haversine_km(st_lat, st_lon, q.lat, q.lon);
            let brg = initial_bearing_deg(st_lat, st_lon, q.lat, q.lon);
            lines.push(Line::from(Span::styled(
                format!("SISMIK // M{:.1} {}", q.mag, q.place),
                Style::default().fg(TELETEXT_RED).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(vec![
                Span::styled("Zaman (UTC) : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(q.time.format("%Y-%m-%d %H:%M:%S").to_string(), Style::default().fg(FG_PRIMARY)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Odak Derinlik: ", Style::default().fg(FG_SECONDARY)),
                Span::styled(format!("{:.1} km", q.depth_km), Style::default().fg(TELETEXT_YELLOW)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Koordinat   : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(format!("{:.4}N, {:.4}E", q.lat, q.lon), Style::default().fg(TELETEXT_CYAN)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Ist. Mesafe : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    format!("{:.1} km ({:.0} deg {})", d_km, brg, bearing_sector_tr(brg)),
                    Style::default().fg(TELETEXT_GREEN),
                ),
            ]));
        }
    }

    if lines.is_empty() {
        if let Some(t) = &inspected_track {
            let d_km = haversine_km(st_lat, st_lon, t.geo.lat, t.geo.lon);
            let d_nm = haversine_nm(st_lat, st_lon, t.geo.lat, t.geo.lon);
            let brg = initial_bearing_deg(st_lat, st_lon, t.geo.lat, t.geo.lon);
            let sec = bearing_sector_tr(brg);
            let is_watched = state.watchlist.contains(&t.id);
            let is_followed = state.follow_target_id.as_deref() == Some(t.id.as_str());

            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} // {} ", t.domain.as_str(), t.callsign),
                    Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("[{}]", t.id),
                    Style::default().fg(TELETEXT_CYAN),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Statu       : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    if t.is_military { "ASKERI / TAKTIK" } else { "SIVIL / TICARI" },
                    Style::default()
                        .fg(if t.is_military { TELETEXT_RED } else { TELETEXT_GREEN })
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    if is_watched { " [IZLEMEDE]" } else { "" },
                    Style::default().fg(TELETEXT_YELLOW),
                ),
                Span::styled(
                    if is_followed { " [KILITLI]" } else { "" },
                    Style::default().fg(TELETEXT_CYAN),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Operator    : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    t.operator_branch
                        .as_deref()
                        .or(t.operator_country.as_deref())
                        .unwrap_or("Belirtilmedi"),
                    Style::default().fg(FG_PRIMARY),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Platform    : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    t.airframe_or_vessel_class
                        .as_deref()
                        .or(t.vessel_type.as_deref())
                        .unwrap_or("Standart"),
                    Style::default().fg(FG_PRIMARY),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Konum WGS84 : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    format!("{:.4}N, {:.4}E", t.geo.lat, t.geo.lon),
                    Style::default().fg(TELETEXT_CYAN),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Mesafe      : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    format!("{:.1} km ({:.1} NM)", d_km, d_nm),
                    Style::default().fg(TELETEXT_GREEN).add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Kerteriz    : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    format!("{:03.0} deg ({})", brg, sec),
                    Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD),
                ),
            ]));
            if let Some(alt) = t.geo.alt_m {
                lines.push(Line::from(vec![
                    Span::styled("Irtifa      : ", Style::default().fg(FG_SECONDARY)),
                    Span::styled(
                        format!("FL{:.0} ({:.0} m / {:.0} ft)", alt * 3.28084 / 100.0, alt, alt * 3.28084),
                        Style::default().fg(FG_PRIMARY),
                    ),
                ]));
            }
            lines.push(Line::from(vec![
                Span::styled("Hiz / Rota  : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    format!(
                        "{:.0} kts ({:.0} km/h) | {:03.0} deg",
                        t.speed_kts.unwrap_or(0.0),
                        t.speed_kts.unwrap_or(0.0) * 1.852,
                        t.heading_deg.unwrap_or(0.0)
                    ),
                    Style::default().fg(FG_PRIMARY),
                ),
            ]));
            if let Some(sq) = &t.squawk {
                lines.push(Line::from(vec![
                    Span::styled("Squawk      : ", Style::default().fg(FG_SECONDARY)),
                    Span::styled(sq, Style::default().fg(TELETEXT_CYAN)),
                ]));
            }
            lines.push(Line::from(vec![
                Span::styled("Son Rapor   : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    t.last_seen.format("%H:%M:%S UTC").to_string(),
                    Style::default().fg(FG_MUTED),
                ),
            ]));
        } else {
            lines.push(Line::from(Span::styled(
                "Hedef secilmedi. Tablodan veya haritadan bir kayda tiklayin.",
                Style::default().fg(FG_MUTED),
            )));
        }
    }

    let detail_p = Paragraph::new(lines)
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_ACTIVE))
                .title(" [TEKNIK DENETCI & TELEMETRI (I)] ")
                .style(Style::default().bg(BG_PANEL).fg(FG_PRIMARY)),
        );
    frame.render_widget(detail_p, chunks[0]);

    // 2. Hızlı Operasyonel Aksiyon Butonları (Tıklanabilir)
    let actions = [
        ("[G] Haritada Hedefe Odakla", 0, TELETEXT_CYAN),
        ("[F] Otomatik Takip Kilidi Ac/Kapat", 1, TELETEXT_GREEN),
        ("[B] Izleme Listesine (Watchlist) Ekle", 2, TELETEXT_YELLOW),
        ("[Ctrl+E] Durum Raporu Disa Aktar (.md)", 3, FG_PRIMARY),
    ];
    let mut act_lines = Vec::new();
    for (i, (label, act_id, col)) in actions.iter().enumerate() {
        act_lines.push(Line::from(Span::styled(
            *label,
            Style::default().fg(*col).add_modifier(Modifier::BOLD),
        )));
        let r_y = chunks[1].y + 1 + i as u16;
        if r_y < chunks[1].y + chunks[1].height.saturating_sub(1) {
            state.hit_tester.register(
                Rect {
                    x: chunks[1].x + 1,
                    y: r_y,
                    width: chunks[1].width.saturating_sub(2),
                    height: 1,
                },
                HitTarget::InspectorAction(*act_id),
            );
        }
    }

    let act_p = Paragraph::new(act_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(" [OPERASYONEL KOMUTLAR (TIKLA)] ")
            .style(Style::default().bg(BG_PANEL)),
    );
    frame.render_widget(act_p, chunks[1]);

    // 3. Canlı Alarm & Watchlist Özeti
    let mut alert_lines = Vec::new();
    let mut alerts_vec: Vec<_> = state.alerts.iter().collect();
    alerts_vec.reverse();
    for a in alerts_vec.into_iter().take(5) {
        let c = match a.severity {
            argus_core::types::ThreatSeverity::Emergency
            | argus_core::types::ThreatSeverity::Critical => TELETEXT_RED,
            _ => TELETEXT_YELLOW,
        };
        alert_lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", a.timestamp.format("%H:%M")),
                Style::default().fg(FG_MUTED),
            ),
            Span::styled(a.message.clone(), Style::default().fg(c)),
        ]));
    }
    if alert_lines.is_empty() {
        alert_lines.push(Line::from(Span::styled(
            "Aktif kritik alarm yok. (180km askeri yaklasma ve Squawk 7700 izleniyor)",
            Style::default().fg(FG_MUTED),
        )));
    }

    let alert_p = Paragraph::new(alert_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(format!(
                " [OTOMATIK ALARMLAR ({}) | IZLEME ({})] ",
                state.alerts.len(),
                state.watchlist.len()
            ))
            .style(Style::default().bg(BG_PANEL)),
    );
    frame.render_widget(alert_p, chunks[2]);
}
