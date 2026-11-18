use crate::widgets::{inspector::render_inspector_panel, p100_map::render_p100_map};
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
            Constraint::Min(8),    // Secili Afet / Termal / CBRN / NOTAM Tablosu
        ])
        .split(split[0]);

    // Sag Sutun: Ust %42 Mini-Harita + Alt %58 Canli Denetci Paneli
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(split[1]);

    let map_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER_ACTIVE))
        .title(" [ JEOFIZIK, CBRN & HAVA SAHASI MINI-HARITASI ] ")
        .style(Style::default().bg(BG_PRIMARY));
    let map_inner = map_block.inner(right_chunks[0]);
    frame.render_widget(map_block, right_chunks[0]);
    render_p100_map(frame, map_inner, state);

    render_inspector_panel(frame, right_chunks[1], state);

    render_masint_header_bar(frame, left_chunks[0], state);

    let table_area = left_chunks[1];
    match state.masint_tab {
        1 => render_firms_table(frame, table_area, state),
        2 => render_radiation_table(frame, table_area, state),
        3 => render_airspace_table(frame, table_area, state),
        _ => render_earthquakes_table(frame, table_area, state),
    }
}

fn render_masint_header_bar(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let (sw_text, hf_impact, sw_col) = if let Some(sw) = &state.solar_weather {
        let impact = if sw.kp_index >= 5.0 {
            "JEOMANYETIK FIRTINA: HF Telsiz Kesintisi / Kutup Emilimi"
        } else {
            "NORMAL (D-Tabakasi HF Yayilim Emilimi Dusuk)"
        };
        let col = if sw.kp_index >= 5.0 {
            TELETEXT_RED
        } else {
            TELETEXT_GREEN
        };
        (
            format!(
                "Kp Indeks: {:.1} │ Firtina: {} │ X-Ray Parlama: {}",
                sw.kp_index, sw.storm_level, sw.flare_class
            ),
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
        (0usize, format!("[1:DEPREM({})]", state.earthquakes.len())),
        (1usize, format!("[2:TERMAL({})]", state.firms_fires.len())),
        (
            2usize,
            format!("[3:CBRN({})]", state.radiation_measurements.len()),
        ),
        (
            3usize,
            format!("[4:NOTAM({})]", state.airspace_warnings.len()),
        ),
    ];

    let active_tab = if state.masint_tab <= 3 {
        state.masint_tab
    } else {
        0
    };

    let mut tab_spans = vec![Span::styled(
        "KATMAN: ",
        Style::default().fg(FG_MUTED).add_modifier(Modifier::BOLD),
    )];
    let mut tx = area.x + 1 + 8;
    let max_x = area.x + area.width.saturating_sub(1);

    for (tid, lbl) in &tabs {
        let active = active_tab == *tid;
        let w = lbl.chars().count() as u16;
        if tx + w <= max_x {
            state.hit_tester.register(
                Rect {
                    x: tx,
                    y: area.y + 2,
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
    tab_spans.push(Span::styled(
        format!("│ HF: {}", hf_impact),
        Style::default().fg(sw_col),
    ));

    let sw_p = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "NOAA SWPC: ",
                Style::default()
                    .fg(TELETEXT_YELLOW)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                sw_text,
                Style::default()
                    .fg(TELETEXT_CYAN)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(tab_spans),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(" [ MASINT // NOAA SWPC + SISMIK + TERMAL + CBRN + NOTAM ] ")
            .style(Style::default().bg(BG_PANEL).fg(FG_PRIMARY)),
    );
    frame.render_widget(sw_p, area);
}

fn render_firms_table(frame: &mut Frame, table_area: Rect, state: &mut AppState) {
    let avail_h = table_area.height.saturating_sub(3) as usize;
    let f_total = state.firms_fires.len();
    let scroll = state.geo_scroll.min(f_total.saturating_sub(1));
    let header = masint_table_header(&[
        "UYDU",
        "KOORDINAT",
        "PARLAKLIK (K / C)",
        "GUVENILIRLIK",
        "ZAMAN",
    ]);

    let mut rows = Vec::new();
    for (offset, f) in state
        .firms_fires
        .iter()
        .skip(scroll)
        .take(avail_h)
        .enumerate()
    {
        let actual_idx = scroll + offset;
        let is_sel = (state.selected_domain == 51 && state.selected_row == actual_idx)
            || (state.selected_domain != 51 && offset == 0);
        let row_st = masint_row_style(is_sel, offset, TELETEXT_RED, false);
        let sat_cell = if is_sel {
            format!("> {} VIIRS", f.satellite)
        } else {
            format!("  {} VIIRS", f.satellite)
        };

        rows.push(
            Row::new(vec![
                Cell::from(sat_cell),
                Cell::from(format!("{:.3}N, {:.3}E", f.lat, f.lon)),
                Cell::from(format!(
                    "{:.1} K ({:.0} C)",
                    f.brightness,
                    f.brightness - 273.15
                )),
                Cell::from(format!("%{}", f.confidence)),
                Cell::from(format!("{} {}", f.acq_date, f.acq_time)),
            ])
            .style(row_st),
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
            Constraint::Length(16),
            Constraint::Length(22),
            Constraint::Length(22),
            Constraint::Length(14),
            Constraint::Min(16),
        ],
    )
    .header(header)
    .block(masint_table_block(format!(
        " [ NASA FIRMS TERMAL ANOMALI & YANGIN ({}/{}) ] ",
        (scroll + 1).min(f_total),
        f_total
    )));
    frame.render_widget(tbl, table_area);
    render_masint_scrollbar(frame, table_area, f_total, avail_h, scroll);
}

fn render_radiation_table(frame: &mut Frame, table_area: Rect, state: &mut AppState) {
    let avail_h = table_area.height.saturating_sub(3) as usize;
    let r_total = state.radiation_measurements.len();
    let scroll = state.radiation_scroll.min(r_total.saturating_sub(1));
    let header = masint_table_header(&[
        "SENSOR ID",
        "DOZ HIZI",
        "HAM DEGER",
        "CBRN DURUM",
        "KONUM",
        "KOORDINAT",
    ]);

    let mut rows = Vec::new();
    for (offset, rad) in state
        .radiation_measurements
        .iter()
        .skip(scroll)
        .take(avail_h)
        .enumerate()
    {
        let actual_idx = scroll + offset;
        let is_sel = (state.selected_domain == 53 && state.selected_row == actual_idx)
            || (state.selected_domain != 53 && offset == 0);
        let usvh = if rad.unit == "cpm" {
            rad.value / 334.0
        } else {
            rad.value
        };
        let (status_text, status_col) = if usvh >= 1.0 {
            ("[!] KRITIK / CBRN", TELETEXT_RED)
        } else if usvh >= 0.3 {
            ("[!] YUKSEK FON", TELETEXT_YELLOW)
        } else {
            ("NORMAL FON", TELETEXT_GREEN)
        };
        let loc = rad.location_name.as_deref().unwrap_or("Geiger Istasyonu");
        let row_st = masint_row_style(is_sel, offset, status_col, usvh >= 1.0);
        let id_cell = if is_sel {
            format!("> #{}", rad.id)
        } else {
            format!("  #{}", rad.id)
        };

        rows.push(
            Row::new(vec![
                Cell::from(id_cell),
                Cell::from(format!("{:.4} uSv/h", usvh)),
                Cell::from(format!("{:.1} {}", rad.value, rad.unit)),
                Cell::from(status_text),
                Cell::from(loc),
                Cell::from(format!("{:.3}N, {:.3}E", rad.lat, rad.lon)),
            ])
            .style(row_st),
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
            Constraint::Length(14),
            Constraint::Length(15),
            Constraint::Length(13),
            Constraint::Length(18),
            Constraint::Min(20),
            Constraint::Length(18),
        ],
    )
    .header(header)
    .block(masint_table_block(format!(
        " [ SAFECAST CANLI RADYASYON & GEIGER AGI ({}/{}) ] ",
        (scroll + 1).min(r_total),
        r_total
    )));
    frame.render_widget(tbl, table_area);
    render_masint_scrollbar(frame, table_area, r_total, avail_h, scroll);
}

fn render_airspace_table(frame: &mut Frame, table_area: Rect, state: &mut AppState) {
    let avail_h = table_area.height.saturating_sub(3) as usize;
    let n_total = state.airspace_warnings.len();
    let scroll = state.geo_scroll.min(n_total.saturating_sub(1));
    let header = masint_table_header(&["TUR", "FIR / BOLGE", "TEHLIKE", "UST IRTIFA", "OZET"]);

    let mut rows = Vec::new();
    for (offset, aw) in state
        .airspace_warnings
        .iter()
        .skip(scroll)
        .take(avail_h)
        .enumerate()
    {
        let actual_idx = scroll + offset;
        let is_sel = (state.selected_domain == 54 && state.selected_row == actual_idx)
            || (state.selected_domain != 54 && offset == 0);
        let (fg_col, is_bold) = match aw.kind.as_str() {
            "NOTAM-CONFLICT" => (TELETEXT_RED, true),
            "GPS-JAM" => (TELETEXT_MAGENTA, false),
            _ => (TELETEXT_YELLOW, false),
        };
        let row_st = masint_row_style(is_sel, offset, fg_col, is_bold);
        let kind_cell = if is_sel {
            format!("> {}", aw.kind)
        } else {
            format!("  {}", aw.kind)
        };
        let fl = aw
            .top_fl
            .map(|f| format!("FL{}", f))
            .unwrap_or_else(|| "SFC-UNL".to_string());

        rows.push(
            Row::new(vec![
                Cell::from(kind_cell),
                Cell::from(aw.fir_or_zone.as_str()),
                Cell::from(aw.hazard.as_str()),
                Cell::from(fl),
                Cell::from(aw.summary.as_str()),
            ])
            .style(row_st),
        );
        state.hit_tester.register(
            Rect {
                x: table_area.x + 1,
                y: table_area.y + 2 + offset as u16,
                width: table_area.width.saturating_sub(2),
                height: 1,
            },
            HitTarget::TableRow(54, actual_idx),
        );
    }

    let tbl = Table::new(
        rows,
        [
            Constraint::Length(18),
            Constraint::Length(22),
            Constraint::Length(18),
            Constraint::Length(11),
            Constraint::Min(24),
        ],
    )
    .header(header)
    .block(masint_table_block(format!(
        " [ CANLI NOTAM / SIGMET / GPS-JAM HAVA SAHASI UYARILARI ({}/{}) ] ",
        (scroll + 1).min(n_total),
        n_total
    )));
    frame.render_widget(tbl, table_area);
    render_masint_scrollbar(frame, table_area, n_total, avail_h, scroll);
}

fn render_earthquakes_table(frame: &mut Frame, table_area: Rect, state: &mut AppState) {
    let avail_h = table_area.height.saturating_sub(3) as usize;
    let q_total = state.earthquakes.len();
    let scroll = state.geo_scroll.min(q_total.saturating_sub(1));
    let header = masint_table_header(&[
        "KAYNAK",
        "BUYUKLUK",
        "DERINLIK",
        "BOLGE / KONUM",
        "ZAMAN (UTC)",
    ]);

    let mut rows = Vec::new();
    for (offset, q) in state
        .earthquakes
        .iter()
        .skip(scroll)
        .take(avail_h)
        .enumerate()
    {
        let actual_idx = scroll + offset;
        let is_sel = (state.selected_domain == 50 && state.selected_row == actual_idx)
            || (state.selected_domain != 50 && offset == 0);
        let mag_col = if q.mag >= 6.0 {
            TELETEXT_RED
        } else if q.mag >= 4.5 {
            TELETEXT_YELLOW
        } else {
            TELETEXT_CYAN
        };
        let row_st = masint_row_style(is_sel, offset, mag_col, q.mag >= 6.0);
        let src = if q.id.starts_with("KANDILLI") {
            "KANDILLI"
        } else if q.id.starts_with("EMSC") {
            "EMSC"
        } else {
            "USGS"
        };
        let src_cell = if is_sel {
            format!("> {}", src)
        } else {
            format!("  {}", src)
        };

        rows.push(
            Row::new(vec![
                Cell::from(src_cell),
                Cell::from(format!("M {:.1}", q.mag)),
                Cell::from(format!("{:.1} km", q.depth_km)),
                Cell::from(q.place.as_str()),
                Cell::from(q.time.format("%H:%M:%S").to_string()),
            ])
            .style(row_st),
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
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(11),
            Constraint::Min(28),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .block(masint_table_block(format!(
        " [ USGS, KANDILLI & EMSC CANLI DEPREM AGI ({}/{}) ] ",
        (scroll + 1).min(q_total),
        q_total
    )));
    frame.render_widget(tbl, table_area);
    render_masint_scrollbar(frame, table_area, q_total, avail_h, scroll);
}

fn masint_table_header<'a>(cols: &[&'a str]) -> Row<'a> {
    Row::new(cols.iter().map(|&h| {
        Cell::from(h).style(
            Style::default()
                .fg(TELETEXT_YELLOW)
                .add_modifier(Modifier::BOLD),
        )
    }))
    .height(1)
    .style(Style::default().bg(BG_SURFACE))
}

fn masint_table_block(title: String) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER_DIM))
        .title(title)
        .style(Style::default().bg(BG_PRIMARY).fg(FG_PRIMARY))
}

fn masint_row_style(is_sel: bool, offset: usize, base_fg: Color, bold: bool) -> Style {
    if is_sel {
        Style::default()
            .fg(TELETEXT_YELLOW)
            .bg(BG_HIGHLIGHT)
            .add_modifier(Modifier::BOLD)
    } else {
        let zebra_bg = if offset.is_multiple_of(2) {
            BG_PANEL
        } else {
            BG_PRIMARY
        };
        let st = Style::default().fg(base_fg).bg(zebra_bg);
        if bold {
            st.add_modifier(Modifier::BOLD)
        } else {
            st
        }
    }
}

fn render_masint_scrollbar(
    frame: &mut Frame,
    table_area: Rect,
    total: usize,
    avail_h: usize,
    scroll: usize,
) {
    if total > avail_h {
        let mut sb = ScrollbarState::new(total).position(scroll);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(BORDER_ACTIVE).bg(BG_PANEL));
        frame.render_stateful_widget(scrollbar, table_area, &mut sb);
    }
}
