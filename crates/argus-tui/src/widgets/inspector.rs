use argus_core::geo::{
    bearing_sector_tr, haversine_km, haversine_nm, initial_bearing_deg, radar_horizon_km,
};
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
            Constraint::Min(9),    // Secili Hedef / Kayit Teknik Telemetrisi + Foto
            Constraint::Length(4), // 2 Satir Operasyonel Komut + Taktik Cetvel Seridi
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
    } else if state.selected_domain == 52 {
        if let Some(ev) = state.acled_events.get(state.selected_row).or_else(|| state.acled_events.get(0)) {
            let d_km = haversine_km(st_lat, st_lon, ev.lat, ev.lon);
            lines.push(Line::from(Span::styled(
                format!("CATISMA // {} ({})", ev.country, ev.event_type),
                Style::default().fg(TELETEXT_RED).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(vec![
                Span::styled("Konum / Tarih: ", Style::default().fg(FG_SECONDARY)),
                Span::styled(format!("{} | {}", ev.location, ev.event_date), Style::default().fg(FG_PRIMARY)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Aktorler     : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    format!("{} vs {}", ev.actor1, ev.actor2.as_deref().unwrap_or("-")),
                    Style::default().fg(TELETEXT_YELLOW),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Zayiat/Mesafe: ", Style::default().fg(FG_SECONDARY)),
                Span::styled(format!("{} | {:.0} km", ev.fatalities, d_km), Style::default().fg(TELETEXT_CYAN)),
            ]));
            lines.push(Line::from(Span::styled(ev.notes.clone(), Style::default().fg(FG_PRIMARY))));
        }
    } else if state.selected_domain == 54 {
        if let Some(aw) = state.airspace_warnings.get(state.selected_row).or_else(|| state.airspace_warnings.first()) {
            lines.push(Line::from(Span::styled(
                format!("{} // {}", aw.kind, aw.fir_or_zone),
                Style::default().fg(TELETEXT_RED).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(vec![
                Span::styled("Tehlike      : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(aw.hazard.clone(), Style::default().fg(TELETEXT_YELLOW)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Ust Irtifa   : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    aw.top_fl.map(|f| format!("FL{}", f)).unwrap_or_else(|| "SFC-UNL".to_string()),
                    Style::default().fg(TELETEXT_CYAN),
                ),
            ]));
            lines.push(Line::from(Span::styled(aw.summary.clone(), Style::default().fg(FG_PRIMARY))));
        }
    } else if state.selected_domain == 55 {
        if let Some(fc) = state.fusion_clusters.get(state.selected_row).or_else(|| state.fusion_clusters.first()) {
            let d_km = haversine_km(st_lat, st_lon, fc.lat, fc.lon);
            lines.push(Line::from(Span::styled(
                format!("K-4 FUZYON KUMESI // SKOR: {}", fc.score),
                Style::default().fg(TELETEXT_RED).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(vec![
                Span::styled("Alanlar      : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(fc.domains.join(" + "), Style::default().fg(TELETEXT_YELLOW)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Merkez/Mesafe: ", Style::default().fg(FG_SECONDARY)),
                Span::styled(format!("{:.2}N, {:.2}E ({:.0} km)", fc.lat, fc.lon, d_km), Style::default().fg(TELETEXT_CYAN)),
            ]));
            lines.push(Line::from(Span::styled(fc.summary.clone(), Style::default().fg(FG_PRIMARY))));
        }
    } else if state.selected_domain == 1 {
        if let Some(item) = state.news.get(state.selected_row).or_else(|| state.news.get(0)) {
            let sev_col = match item.severity {
                argus_core::types::ThreatSeverity::Emergency
                | argus_core::types::ThreatSeverity::Critical => TELETEXT_RED,
                argus_core::types::ThreatSeverity::Warning => TELETEXT_YELLOW,
                argus_core::types::ThreatSeverity::Advisory => TELETEXT_CYAN,
                argus_core::types::ThreatSeverity::Low => TELETEXT_GREEN,
            };
            lines.push(Line::from(Span::styled(
                format!("OSINT // {}", item.source),
                Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(vec![
                Span::styled("Onem        : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    item.severity.as_str(),
                    Style::default().fg(sev_col).add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Saat (UTC)  : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    item.published.format("%Y-%m-%d %H:%M").to_string(),
                    Style::default().fg(TELETEXT_CYAN),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Baslik      : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(item.title.clone(), Style::default().fg(FG_PRIMARY)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Baglanti    : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(item.link.clone(), Style::default().fg(FG_MUTED)),
            ]));
        }
    } else if state.selected_domain == 4 {
        if let Some(inc) = state.cyber_incidents.get(state.selected_row).or_else(|| state.cyber_incidents.get(0)) {
            let sev_col = match inc.severity {
                argus_core::types::ThreatSeverity::Emergency
                | argus_core::types::ThreatSeverity::Critical => TELETEXT_RED,
                argus_core::types::ThreatSeverity::Warning => TELETEXT_YELLOW,
                _ => TELETEXT_CYAN,
            };
            let ref_id = match (&inc.cve, &inc.asn) {
                (Some(c), Some(a)) => format!("{} | {}", c, a),
                (Some(c), None) => c.clone(),
                (None, Some(a)) => a.clone(),
                (None, None) => "-".to_string(),
            };
            lines.push(Line::from(Span::styled(
                format!("SIBER // {}", inc.event_type),
                Style::default().fg(sev_col).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(vec![
                Span::styled("CVE / ASN   : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(ref_id, Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Baslik      : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(inc.title.clone(), Style::default().fg(FG_PRIMARY)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Hedef       : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(inc.target.clone(), Style::default().fg(TELETEXT_CYAN)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Tespit (UTC): ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    inc.timestamp.format("%Y-%m-%d %H:%M").to_string(),
                    Style::default().fg(FG_MUTED),
                ),
            ]));
        }
    } else if state.selected_domain == 51 {
        if let Some(f) = state.firms_fires.get(state.selected_row).or_else(|| state.firms_fires.get(0)) {
            let d_km = haversine_km(st_lat, st_lon, f.lat, f.lon);
            let brg = initial_bearing_deg(st_lat, st_lon, f.lat, f.lon);
            let temp_c = f.brightness - 273.15;
            lines.push(Line::from(Span::styled(
                format!("TERMAL // {} VIIRS", f.satellite),
                Style::default().fg(TELETEXT_RED).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(vec![
                Span::styled("Parlaklik   : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    format!("{:.1} K ({:.1} C)", f.brightness, temp_c),
                    Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Guvenilirlik: ", Style::default().fg(FG_SECONDARY)),
                Span::styled(format!("%{}", f.confidence), Style::default().fg(TELETEXT_CYAN)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Zaman (UTC) : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    format!("{} {}", f.acq_date, f.acq_time),
                    Style::default().fg(FG_PRIMARY),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Koordinat   : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(format!("{:.4}N, {:.4}E", f.lat, f.lon), Style::default().fg(TELETEXT_CYAN)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Ist. Mesafe : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    format!("{:.1} km ({:.0} deg {})", d_km, brg, bearing_sector_tr(brg)),
                    Style::default().fg(TELETEXT_GREEN),
                ),
            ]));
        }
    } else if state.selected_domain == 53 {
        if let Some(rad) = state.radiation_measurements.get(state.selected_row).or_else(|| state.radiation_measurements.get(0)) {
            let usvh = if rad.unit == "cpm" { rad.value / 334.0 } else { rad.value };
            let (status_txt, status_col) = if usvh >= 1.0 {
                ("KRITIK", TELETEXT_RED)
            } else if usvh >= 0.3 {
                ("YUKSEK", TELETEXT_YELLOW)
            } else {
                ("NORMAL", TELETEXT_GREEN)
            };
            let d_km = haversine_km(st_lat, st_lon, rad.lat, rad.lon);
            let brg = initial_bearing_deg(st_lat, st_lon, rad.lat, rad.lon);
            lines.push(Line::from(Span::styled(
                format!("CBRN // SAFECAST #{}", rad.id),
                Style::default().fg(status_col).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(vec![
                Span::styled("Doz Hizi    : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    format!("{:.4} uSv/h", usvh),
                    Style::default().fg(TELETEXT_CYAN).add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Ham Deger   : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(format!("{:.1} {}", rad.value, rad.unit), Style::default().fg(FG_PRIMARY)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Durum       : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(status_txt, Style::default().fg(status_col).add_modifier(Modifier::BOLD)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Konum       : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    rad.location_name.as_deref().unwrap_or("Geiger Istasyonu").to_string(),
                    Style::default().fg(FG_PRIMARY),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Koord/Mesafe: ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    format!("{:.3}N, {:.3}E ({:.1} km {:.0} deg {})", rad.lat, rad.lon, d_km, brg, bearing_sector_tr(brg)),
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
            let alt_m = t.geo.alt_m.unwrap_or(0.0).max(0.0);
            let los_km = radar_horizon_km(30.0, alt_m);
            let in_los = d_km <= los_km;
            lines.push(Line::from(vec![
                Span::styled("Radar LOS   : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    format!(
                        "{:.0} km ({})",
                        los_km,
                        if in_los { "GORUS ICI" } else { "UFUK ALTI" }
                    ),
                    Style::default().fg(if in_los { TELETEXT_GREEN } else { TELETEXT_RED }),
                ),
            ]));
            let fmt_eta = |speed_kmh: f64| -> String {
                let mins = (d_km / speed_kmh) * 60.0;
                if mins < 1.0 {
                    format!("{:.0}sn", (mins * 60.0).round())
                } else if mins < 60.0 {
                    format!("{:.1}dk", mins)
                } else {
                    let total = mins.round() as u64;
                    format!("{}s{:02}d", total / 60, total % 60)
                }
            };
            lines.push(Line::from(vec![
                Span::styled("Mach ETA    : ", Style::default().fg(FG_SECONDARY)),
                Span::styled(
                    format!("M0.85:{} | M1.6:{}", fmt_eta(903.0), fmt_eta(1700.0)),
                    Style::default().fg(TELETEXT_CYAN),
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
        && !matches!(state.selected_domain, 1 | 3 | 4 | 30 | 50 | 51 | 52 | 53 | 54 | 55)
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

    let detail_border = if state.active_tile == 5 || state.active_view != argus_core::types::ActiveView::Dashboard {
        BORDER_ACTIVE
    } else {
        BORDER_DIM
    };
    let detail_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(detail_border))
        .title(" [ [6] TEKNIK DENETCI (I) | O: Tam Dosya ] ")
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

    // 2. Operasyonel Komut & Taktik Cetvel Butonlari (2 Satir, Tiklanabilir)
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

    let mut cmd_lines = vec![Line::from(pill_spans)];
    if chunks[1].height >= 4 {
        let has_pin_a = state.tactical_ruler.is_some();
        let has_pin_b = state
            .tactical_ruler
            .as_ref()
            .is_some_and(|r| r.pin_b.is_some());
        let ring_km = state
            .tactical_ruler
            .as_ref()
            .and_then(|r| r.threat_ring_km);
        let ruler_pills = [
            (
                "[PinA]",
                HitTarget::RulerSetPinA,
                if has_pin_a { TELETEXT_BLACK } else { TELETEXT_YELLOW },
                if has_pin_a { TELETEXT_YELLOW } else { BG_SURFACE },
            ),
            (
                "[PinB]",
                HitTarget::RulerSetPinB,
                if has_pin_b { TELETEXT_BLACK } else { TELETEXT_CYAN },
                if has_pin_b { TELETEXT_CYAN } else { BG_SURFACE },
            ),
            (
                "[Halka]",
                HitTarget::RulerCycleRing,
                if ring_km.is_some() { TELETEXT_BLACK } else { TELETEXT_RED },
                if ring_km.is_some() { TELETEXT_RED } else { BG_SURFACE },
            ),
            ("[Sifir]", HitTarget::RulerClear, FG_SECONDARY, BG_SURFACE),
        ];
        let mut r_spans = Vec::new();
        let mut rx = chunks[1].x + 1;
        let ry = chunks[1].y + 2;
        for (lbl, target, fg, bg) in ruler_pills {
            let pw = lbl.chars().count() as u16;
            if rx + pw <= max_x {
                r_spans.push(Span::styled(
                    lbl,
                    Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
                ));
                r_spans.push(Span::raw(" "));
                state.hit_tester.register(
                    Rect { x: rx, y: ry, width: pw, height: 1 },
                    target,
                );
                rx += pw + 1;
            }
        }
        if let Some(rk) = ring_km {
            let rtag = format!("{:.0}k", rk);
            if rx + (rtag.chars().count() as u16) <= max_x {
                r_spans.push(Span::styled(rtag, Style::default().fg(TELETEXT_RED)));
            }
        }
        cmd_lines.push(Line::from(r_spans));
    }

    let act_p = Paragraph::new(cmd_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(" [KOMUTLAR (TIKLA)] ")
            .style(Style::default().bg(BG_PANEL)),
    );
    frame.render_widget(act_p, chunks[1]);

    // 3. Canlı Alarm & Watchlist Özeti (Sarilmayan tek-satir satirlar + birebir tiklama kaydi)
    let avail_rows = chunks[2].height.saturating_sub(2) as usize;
    let max_w = chunks[2].width.saturating_sub(3) as usize;
    let mut alert_lines = Vec::new();

    let alerts: Vec<_> = state.alerts.iter().enumerate().collect();
    for (orig_idx, a) in alerts.into_iter().rev() {
        if alert_lines.len() >= avail_rows {
            break;
        }
        let row_offset = alert_lines.len();
        let c = match a.severity {
            argus_core::types::ThreatSeverity::Emergency
            | argus_core::types::ThreatSeverity::Critical => TELETEXT_RED,
            _ => TELETEXT_YELLOW,
        };
        let time_prefix = format!("▸ {} ", a.timestamp.format("%H:%M"));
        let prefix_w = time_prefix.chars().count();
        let msg_clipped: String = a
            .message
            .replace('\n', " ")
            .chars()
            .take(max_w.saturating_sub(prefix_w))
            .collect();
        alert_lines.push(Line::from(vec![
            Span::styled(time_prefix, Style::default().fg(FG_MUTED)),
            Span::styled(msg_clipped, Style::default().fg(c)),
        ]));
        state.hit_tester.register(
            Rect {
                x: chunks[2].x + 1,
                y: chunks[2].y + 1 + row_offset as u16,
                width: chunks[2].width.saturating_sub(2),
                height: 1,
            },
            HitTarget::AlertItem(orig_idx),
        );
    }

    for (fc_idx, fc) in state.fusion_clusters.iter().enumerate() {
        if alert_lines.len() >= avail_rows {
            break;
        }
        let row_offset = alert_lines.len();
        let prefix = format!("▸ [FUZ:{}] ", fc.score);
        let prefix_w = prefix.chars().count();
        let sum_clipped: String = fc
            .summary
            .replace('\n', " ")
            .chars()
            .take(max_w.saturating_sub(prefix_w))
            .collect();
        alert_lines.push(Line::from(vec![
            Span::styled(
                prefix,
                Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD),
            ),
            Span::styled(sum_clipped, Style::default().fg(TELETEXT_CYAN)),
        ]));
        state.hit_tester.register(
            Rect {
                x: chunks[2].x + 1,
                y: chunks[2].y + 1 + row_offset as u16,
                width: chunks[2].width.saturating_sub(2),
                height: 1,
            },
            HitTarget::FusionAlert(fc_idx),
        );
    }

    if alert_lines.is_empty() {
        let empty_msg: String = "Alarm yok (180km askeri, 7x00 & Fuzyon aktif)"
            .chars()
            .take(max_w)
            .collect();
        alert_lines.push(Line::from(Span::styled(
            empty_msg,
            Style::default().fg(FG_MUTED),
        )));
    }

    let alert_p = Paragraph::new(alert_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(format!(
                " [ [7] OTOMATIK ALARMLAR (Tikla: Odakla) | IZL ({}) ] ",
                state.watchlist.len()
            ))
            .style(Style::default().bg(BG_PANEL)),
    );
    frame.render_widget(alert_p, chunks[2]);
}
