use crate::widgets::p100_map::render_p100_map;
use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table},
    Frame,
};

pub fn render_masint_view(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if area.width < 24 || area.height < 10 {
        return;
    }
    frame.render_widget(Clear, area);

    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
        .split(area);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // NOAA Uzay Havasi & Sekme Cubugu
            Constraint::Min(8),    // Secili Afet / Termal / CBRN Tablosu
        ])
        .split(split[0]);

    // Sagda Canli Mini-Harita + Istasyon Olay Ozeti
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(split[1]);

    let map_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER_ACTIVE))
        .title(" [ JEOFIZIK & CBRN MINI-HARITASI ] ")
        .style(Style::default().bg(BG_PRIMARY));
    let map_inner = map_block.inner(right_chunks[0]);
    frame.render_widget(map_block, right_chunks[0]);
    render_p100_map(frame, map_inner, state);

    // 1. NOAA SWPC Banner (Sifir sahte veri fallback!)
    let (sw_text, hf_impact, sw_col) = if let Some(sw) = &state.solar_weather {
        let impact = if sw.kp_index >= 5.0 {
            "JEOMANYETIK FIRTINA: HF Telsiz Kesintisi / Kutup Emilimi"
        } else {
            "NORMAL (D-Tabakasi HF Yayilim Emilimi Dusuk)"
        };
        let col = if sw.kp_index >= 5.0 { TELETEXT_RED } else { TELETEXT_GREEN };
        (
            format!("Kp Indeks: {:.1} │ Firtina: {} │ X-Ray Parlama: {}", sw.kp_index, sw.storm_level, sw.flare_class),
            impact,
            col,
        )
    } else {
        (
            "NOAA SWPC canli uzay havasi verisi bekleniyor...".to_string(),
            "Baglanti bekleniyor...",
            FG_MUTED,
        )
    };

    let tabs = [
        (0usize, format!("[1: DEPREM ({})]", state.earthquakes.len())),
        (1usize, format!("[2: FIRMS TERMAL ({})]", state.firms_fires.len())),
        (2usize, format!("[3: SAFECAST CBRN ({})]", state.radiation_measurements.len())),
    ];
    let mut tab_spans = vec![
        Span::styled("VERI KATMANI: ", Style::default().fg(FG_MUTED).add_modifier(Modifier::BOLD)),
    ];
    let mut tx = left_chunks[0].x + 1 + 14;
    for (tid, lbl) in &tabs {
        let active = state.masint_tab == *tid;
        let w = lbl.chars().count() as u16;
        if tx + w < left_chunks[0].x + left_chunks[0].width {
            state.hit_tester.register(
                Rect {
                    x: tx,
                    y: left_chunks[0].y + 2,
                    width: w,
                    height: 1,
                },
                HitTarget::MasintTab(*tid),
            );
            tab_spans.push(Span::styled(
                lbl.clone(),
                Style::default()
                    .fg(if active { TELETEXT_BLACK } else { FG_PRIMARY })
                    .bg(if active { TELETEXT_YELLOW } else { BG_SURFACE })
                    .add_modifier(Modifier::BOLD),
            ));
            tab_spans.push(Span::raw(" "));
            tx += w + 1;
        }
    }
    tab_spans.push(Span::styled(format!(" │ HF: {}", hf_impact), Style::default().fg(sw_col)));

    let sw_p = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("NOAA SWPC: ", Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled(sw_text, Style::default().fg(TELETEXT_CYAN).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(tab_spans),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(" [ MASINT // NOAA SWPC + SISMIK + TERMAL + CBRN ] ")
            .style(Style::default().bg(BG_PANEL).fg(FG_PRIMARY)),
    );
    frame.render_widget(sw_p, left_chunks[0]);

    // 2. Aktif Sekme Tablosu
    let table_area = left_chunks[1];
    let avail_h = table_area.height.saturating_sub(3) as usize;

    match state.masint_tab {
        1 => {
            let f_total = state.firms_fires.len();
            let scroll = state.geo_scroll.min(f_total.saturating_sub(1));
            let header = Row::new(
                ["UYDU", "KOORDINAT", "PARLAKLIK (K / C)", "GUVENILIRLIK", "ZAMAN"]
                    .into_iter()
                    .map(|h| Cell::from(h).style(Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD))),
            )
            .style(Style::default().bg(BG_SURFACE));

            let mut rows = Vec::new();
            for (offset, f) in state.firms_fires.iter().skip(scroll).take(avail_h).enumerate() {
                let actual_idx = scroll + offset;
                rows.push(
                    Row::new(vec![
                        Cell::from(format!("{} VIIRS", f.satellite)),
                        Cell::from(format!("{:.3}N, {:.3}E", f.lat, f.lon)),
                        Cell::from(format!("{:.1} K ({:.0} C)", f.brightness, f.brightness - 273.15)),
                        Cell::from(format!("%{}", f.confidence)),
                        Cell::from(format!("{} {}", f.acq_date, f.acq_time)),
                    ])
                    .style(Style::default().fg(TELETEXT_RED)),
                );
                state.hit_tester.register(
                    Rect {
                        x: table_area.x + 1,
                        y: table_area.y + 2 + offset as u16,
                        width: table_area.width.saturating_sub(2),
                        height: 1,
                    },
                    HitTarget::TableRow(51, actual_idx),
                );
            }
            let tbl = Table::new(
                rows,
                [
                    Constraint::Length(14),
                    Constraint::Length(22),
                    Constraint::Length(22),
                    Constraint::Length(14),
                    Constraint::Min(16),
                ],
            )
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(BORDER_DIM))
                    .title(format!(" [ NASA FIRMS TERMAL ANOMALI & YANGIN ({}/{}) ] ", (scroll + 1).min(f_total), f_total))
                    .style(Style::default().bg(BG_PRIMARY).fg(FG_PRIMARY)),
            );
            frame.render_widget(tbl, table_area);
            if f_total > avail_h {
                let mut sb = ScrollbarState::new(f_total).position(scroll);
                frame.render_stateful_widget(Scrollbar::new(ScrollbarOrientation::VerticalRight), table_area, &mut sb);
            }
        }
        2 => {
            let r_total = state.radiation_measurements.len();
            let scroll = state.radiation_scroll.min(r_total.saturating_sub(1));
            let header = Row::new(
                ["SENSOR ID", "DOZ HIZI", "HAM DEGER", "CBRN DURUM", "KONUM", "KOORDINAT"]
                    .into_iter()
                    .map(|h| Cell::from(h).style(Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD))),
            )
            .style(Style::default().bg(BG_SURFACE));

            let mut rows = Vec::new();
            for (offset, rad) in state.radiation_measurements.iter().skip(scroll).take(avail_h).enumerate() {
                let actual_idx = scroll + offset;
                let usvh = if rad.unit == "cpm" { rad.value / 334.0 } else { rad.value };
                let (status_text, status_col) = if usvh >= 1.0 {
                    ("[!] KRITIK / CBRN", TELETEXT_RED)
                } else if usvh >= 0.3 {
                    ("[!] YUKSEK FON", TELETEXT_YELLOW)
                } else {
                    ("NORMAL FON", TELETEXT_GREEN)
                };
                let loc = rad.location_name.as_deref().unwrap_or("Geiger Istasyonu");
                rows.push(
                    Row::new(vec![
                        Cell::from(format!("#{}", rad.id)),
                        Cell::from(format!("{:.4} uSv/h", usvh)),
                        Cell::from(format!("{:.1} {}", rad.value, rad.unit)),
                        Cell::from(status_text),
                        Cell::from(loc),
                        Cell::from(format!("{:.3}N, {:.3}E", rad.lat, rad.lon)),
                    ])
                    .style(Style::default().fg(status_col)),
                );
                state.hit_tester.register(
                    Rect {
                        x: table_area.x + 1,
                        y: table_area.y + 2 + offset as u16,
                        width: table_area.width.saturating_sub(2),
                        height: 1,
                    },
                    HitTarget::TableRow(53, actual_idx),
                );
            }
            let tbl = Table::new(
                rows,
                [
                    Constraint::Length(12),
                    Constraint::Length(15),
                    Constraint::Length(13),
                    Constraint::Length(18),
                    Constraint::Min(20),
                    Constraint::Length(18),
                ],
            )
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(BORDER_DIM))
                    .title(format!(" [ SAFECAST CANLI RADYASYON & GEIGER AGI ({}/{}) ] ", (scroll + 1).min(r_total), r_total))
                    .style(Style::default().bg(BG_PRIMARY).fg(FG_PRIMARY)),
            );
            frame.render_widget(tbl, table_area);
            if r_total > avail_h {
                let mut sb = ScrollbarState::new(r_total).position(scroll);
                frame.render_stateful_widget(Scrollbar::new(ScrollbarOrientation::VerticalRight), table_area, &mut sb);
            }
        }
        _ => {
            let q_total = state.earthquakes.len();
            let scroll = state.geo_scroll.min(q_total.saturating_sub(1));
            let header = Row::new(
                ["KAYNAK", "BUYUKLUK", "DERINLIK", "BOLGE / KONUM", "ZAMAN (UTC)"]
                    .into_iter()
                    .map(|h| Cell::from(h).style(Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD))),
            )
            .style(Style::default().bg(BG_SURFACE));

            let mut rows = Vec::new();
            for (offset, q) in state.earthquakes.iter().skip(scroll).take(avail_h).enumerate() {
                let actual_idx = scroll + offset;
                let mag_col = if q.mag >= 6.0 {
                    TELETEXT_RED
                } else if q.mag >= 4.5 {
                    TELETEXT_YELLOW
                } else {
                    TELETEXT_CYAN
                };
                let src = if q.id.starts_with("KANDILLI") {
                    "KANDILLI"
                } else if q.id.starts_with("EMSC") {
                    "EMSC"
                } else {
                    "USGS"
                };
                rows.push(
                    Row::new(vec![
                        Cell::from(src),
                        Cell::from(format!("M {:.1}", q.mag)),
                        Cell::from(format!("{:.1} km", q.depth_km)),
                        Cell::from(q.place.as_str()),
                        Cell::from(q.time.format("%H:%M:%S").to_string()),
                    ])
                    .style(Style::default().fg(mag_col)),
                );
                state.hit_tester.register(
                    Rect {
                        x: table_area.x + 1,
                        y: table_area.y + 2 + offset as u16,
                        width: table_area.width.saturating_sub(2),
                        height: 1,
                    },
                    HitTarget::TableRow(50, actual_idx),
                );
            }
            let tbl = Table::new(
                rows,
                [
                    Constraint::Length(10),
                    Constraint::Length(10),
                    Constraint::Length(11),
                    Constraint::Min(28),
                    Constraint::Length(12),
                ],
            )
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(BORDER_DIM))
                    .title(format!(" [ USGS, KANDILLI & EMSC CANLI DEPREM AGI ({}/{}) ] ", (scroll + 1).min(q_total), q_total))
                    .style(Style::default().bg(BG_PRIMARY).fg(FG_PRIMARY)),
            );
            frame.render_widget(tbl, table_area);
            if q_total > avail_h {
                let mut sb = ScrollbarState::new(q_total).position(scroll);
                frame.render_stateful_widget(Scrollbar::new(ScrollbarOrientation::VerticalRight), table_area, &mut sb);
            }
        }
    }

    // Sag alt: Olay Ozeti ve Jeofizik Kaynak Durumu
    let summary_lines = vec![
        Line::from(vec![
            Span::styled("Aktif Deprem   : ", Style::default().fg(FG_MUTED)),
            Span::styled(format!("{} Kayit (USGS / Kandilli / EMSC)", state.earthquakes.len()), Style::default().fg(TELETEXT_YELLOW)),
        ]),
        Line::from(vec![
            Span::styled("Termal Anomali : ", Style::default().fg(FG_MUTED)),
            Span::styled(format!("{} VIIRS Nokta", state.firms_fires.len()), Style::default().fg(TELETEXT_RED)),
        ]),
        Line::from(vec![
            Span::styled("Radyasyon CBRN : ", Style::default().fg(FG_MUTED)),
            Span::styled(format!("{} Safecast Sensoru", state.radiation_measurements.len()), Style::default().fg(TELETEXT_GREEN)),
        ]),
        Line::from(""),
        Line::from(Span::styled("[1/2/3] Sekme Sec | [Satira Tikla] Tam Analiz Dosyasi", Style::default().fg(FG_SECONDARY))),
    ];
    let info_p = Paragraph::new(summary_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(" [ SENSOR OZETI ] ")
            .style(Style::default().bg(BG_PANEL).fg(FG_PRIMARY)),
    );
    frame.render_widget(info_p, right_chunks[1]);
}
