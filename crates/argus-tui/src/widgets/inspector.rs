use argus_core::geo::{bearing_sector_tr, haversine_km, haversine_nm, initial_bearing_deg};
use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use argus_core::types::{AircraftMediaStatus, Domain};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// Seçili veya dossier modundaki uçak için canlı fotoğraf kutusunu ve kredi/durum bilgisini çizer.
/// Görselin kendisi frame sonunda `PhotoRenderCtx::render_active_slot` ile piksel olarak basılır.
pub fn render_aircraft_photo_box(frame: &mut Frame, area: Rect, state: &mut AppState, icao: &str) {
    if area.width < 10 || area.height < 4 {
        return;
    }

    let key = icao.to_lowercase();
    let cached = state.aircraft_photo_cache.get(&key).cloned();
    let active_ready = match &state.aircraft_media_status {
        AircraftMediaStatus::Ready(e) if e.icao.eq_ignore_ascii_case(&key) => Some(e.clone()),
        _ => None,
    };
    let entry_opt = cached.or(active_ready);

    let title = if let Some(e) = &entry_opt {
        if let Some(reg) = &e.registration {
            format!(" [ {} | {} ] ", reg, e.icao.to_uppercase())
        } else {
            format!(" [ UCAK FOTOSU: {} ] ", e.icao.to_uppercase())
        }
    } else {
        format!(" [ UCAK FOTOSU: {} ] ", icao.to_uppercase())
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(TELETEXT_CYAN))
        .title(title)
        .style(Style::default().bg(BG_PRIMARY));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 6 || inner.height < 2 {
        return;
    }

    if let Some(entry) = entry_opt {
        if entry.image_bytes.is_some() {
            let has_caption = inner.height >= 4;
            let img_h = if has_caption {
                inner.height.saturating_sub(1)
            } else {
                inner.height
            };
            let img_rect = Rect::new(inner.x, inner.y, inner.width, img_h);
            state.pending_photo_slot = Some((
                key,
                (img_rect.x, img_rect.y, img_rect.width, img_rect.height),
            ));

            if has_caption {
                let credit = entry
                    .photographer
                    .as_deref()
                    .map(|p| format!("Foto: {} ({})", p, entry.source))
                    .unwrap_or_else(|| entry.source.clone());
                let cap_rect = Rect::new(inner.x, inner.y + img_h, inner.width, 1);
                let cap = Paragraph::new(Span::styled(credit, Style::default().fg(FG_MUTED)));
                frame.render_widget(cap, cap_rect);
            }
            return;
        }

        let mut msg_lines = vec![Line::from(Span::styled(
            entry.note.clone(),
            Style::default().fg(TELETEXT_YELLOW),
        ))];
        if let Some(reg) = &entry.registration {
            msg_lines.push(Line::from(Span::styled(
                format!("Tescil: {}", reg),
                Style::default().fg(TELETEXT_CYAN),
            )));
        }
        if let Some(typ) = &entry.aircraft_type {
            msg_lines.push(Line::from(Span::styled(
                format!("Tip   : {}", typ),
                Style::default().fg(FG_PRIMARY),
            )));
        }
        frame.render_widget(Paragraph::new(msg_lines).wrap(Wrap { trim: true }), inner);
        return;
    }

    let status_text = match &state.aircraft_media_status {
        AircraftMediaStatus::Loading { icao: l } if l.eq_ignore_ascii_case(&key) => {
            "Internetten ucak fotografi araniyor..."
        }
        AircraftMediaStatus::NotFound {
            icao: n, reason, ..
        } if n.eq_ignore_ascii_case(&key) => reason.as_str(),
        _ => "Fotograf hazirlaniyor...",
    };

    let info = Paragraph::new(Span::styled(status_text, Style::default().fg(FG_MUTED)))
        .wrap(Wrap { trim: true });
    frame.render_widget(info, inner);
}

pub fn render_inspector_panel(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if area.width < 18 || area.height < 8 {
        return;
    }

    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(11),   // Secili Hedef / Kayit Teknik Telemetrisi + Foto
            Constraint::Length(3), // Kompakt Tek Satir Operasyonel Komut Seridi
            Constraint::Length(7), // Canli Alarm & Izleme Listesi (Watchlist) Ozeti
        ])
        .split(area);

    // 1. Seçili Hedef veya Kayıt Detayı
    let mut lines = Vec::new();
    let st_lat = state.station_lat;
    let st_lon = state.station_lon;

    let inspected_track = state.inspected_target();

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
            if let Some(anom) = state.track_anomalies.get(&t.id) {
                lines.push(Line::from(vec![
                    Span::styled("Anomali     : ", Style::default().fg(FG_SECONDARY)),
                    Span::styled(
                        format!("[!] {}", anom),
                        Style::default().fg(TELETEXT_RED).add_modifier(Modifier::BOLD),
                    ),
                ]));
            }
            if t.domain == Domain::Air {
                let cs_key = t.callsign.trim().to_uppercase();
                if let Some(sched) = state.flight_schedule_cache.get(&cs_key) {
                    let route_str = format!(
                        "{} -> {}",
                        sched.origin_code.as_deref().unwrap_or("?"),
                        sched.dest_code.as_deref().unwrap_or("?")
                    );
                    lines.push(Line::from(vec![
                        Span::styled("Ucus Rotasi : ", Style::default().fg(FG_SECONDARY)),
                        Span::styled(route_str, Style::default().fg(TELETEXT_YELLOW)),
                    ]));
                } else if let Some(rt) = &t.destination {
                    if !rt.trim().is_empty() {
                        lines.push(Line::from(vec![
                            Span::styled("Ucus Rotasi : ", Style::default().fg(FG_SECONDARY)),
                            Span::styled(rt.clone(), Style::default().fg(TELETEXT_YELLOW)),
                        ]));
                    }
                }
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

    let show_air_photo = !state.is_dialog_open
        && !matches!(state.selected_domain, 3 | 30 | 50)
        && inspected_track
            .as_ref()
            .is_some_and(|t| t.domain == Domain::Air);

    if show_air_photo {
        if let Some(t) = &inspected_track {
            let key = t.id.to_lowercase();
            if let Some(entry) = state.aircraft_photo_cache.get(&key) {
                if let Some(reg) = &entry.registration {
                    lines.push(Line::from(vec![
                        Span::styled("Tescil      : ", Style::default().fg(FG_SECONDARY)),
                        Span::styled(reg.clone(), Style::default().fg(TELETEXT_CYAN)),
                    ]));
                }
            }
        }
    }

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_ACTIVE))
        .title(" [TEKNIK DENETCI (I) | O: Tam Dosya] ")
        .style(Style::default().bg(BG_PANEL).fg(FG_PRIMARY));
    let detail_inner = detail_block.inner(chunks[0]);
    frame.render_widget(detail_block, chunks[0]);

    let air_icao_opt = if show_air_photo {
        inspected_track.as_ref().map(|t| t.id.clone())
    } else {
        None
    };

    if let Some(air_icao) = air_icao_opt {
        if detail_inner.width >= 44 && detail_inner.height >= 8 {
            let split = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(54), Constraint::Percentage(46)])
                .split(detail_inner);
            frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), split[0]);
            render_aircraft_photo_box(frame, split[1], state, &air_icao);
        } else if detail_inner.height >= 17 && detail_inner.width >= 20 {
            let split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(9), Constraint::Length(9)])
                .split(detail_inner);
            frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), split[0]);
            render_aircraft_photo_box(frame, split[1], state, &air_icao);
        } else {
            frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), detail_inner);
        }
    } else {
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), detail_inner);
    }

    // 2. Kompakt Tek Satir Operasyonel Komut Butonlari (Tiklanabilir)
    let act_pills = [
        ("[G:Odak]", HitTarget::InspectorAction(0), TELETEXT_CYAN),
        ("[F:Takip]", HitTarget::InspectorAction(1), TELETEXT_GREEN),
        ("[B:Izle]", HitTarget::InspectorAction(2), TELETEXT_YELLOW),
        ("[Rapor]", HitTarget::InspectorAction(3), FG_PRIMARY),
        ("[A:AI]", HitTarget::TriggerAiAnalysis, TELETEXT_YELLOW),
    ];
    let mut pill_spans = Vec::new();
    let mut px = chunks[1].x + 1;
    let py = chunks[1].y + 1;
    let max_x = chunks[1].x + chunks[1].width.saturating_sub(1);
    for (lbl, target, col) in act_pills {
        let pw = lbl.chars().count() as u16;
        if px + pw <= max_x {
            pill_spans.push(Span::styled(
                lbl,
                Style::default().fg(col).bg(BG_SURFACE).add_modifier(Modifier::BOLD),
            ));
            pill_spans.push(Span::raw(" "));
            state.hit_tester.register(
                Rect { x: px, y: py, width: pw, height: 1 },
                target,
            );
            px += pw + 1;
        }
    }

    let act_p = Paragraph::new(Line::from(pill_spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(" [KOMUTLAR (TIKLA)] ")
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
    if !state.fusion_clusters.is_empty() && alert_lines.len() < 4 {
        for fc in state.fusion_clusters.iter().take(2) {
            alert_lines.push(Line::from(vec![
                Span::styled(
                    format!("[FUZ:{}] ", fc.score),
                    Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD),
                ),
                Span::styled(fc.summary.clone(), Style::default().fg(TELETEXT_CYAN)),
            ]));
        }
    }
    if alert_lines.is_empty() {
        alert_lines.push(Line::from(Span::styled(
            "Alarm yok (180km askeri, 7x00 & Fuzyon aktif)",
            Style::default().fg(FG_MUTED),
        )));
    }

    let alert_p = Paragraph::new(alert_lines).wrap(Wrap { trim: true }).block(
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
