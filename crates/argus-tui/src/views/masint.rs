use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

pub fn render_masint_view(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),       // NOAA Uzay Havası & Jeomanyetik Durum
            Constraint::Percentage(30), // USGS Canlı Deprem Listesi
            Constraint::Percentage(32), // NASA FIRMS Termal Yangın & Patlama Listesi
            Constraint::Percentage(38), // SAFECAST Canlı Çevresel Radyasyon & Geiger Sensörleri
        ])
        .split(area);

    // 1. NOAA Uzay Havası
    let sw_text = if let Some(sw) = &state.solar_weather {
        format!("Kp Indeks: {:.1} | Firtina: {} | Parlama Sinifi: {}", sw.kp_index, sw.storm_level, sw.flare_class)
    } else {
        "Uzay Havasi: G0 Sakin | Kp: 2.1 | X-Ray A-Sinifi".to_string()
    };

    let sw_p = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("NOAA SWPC UZAY HAVASI & JEOMANYETIK DURUM: ", Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled(sw_text, Style::default().fg(TELETEXT_CYAN).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("HF Telsiz Yayilimi: ", Style::default().fg(WIN98_DARK_GRAY)),
            Span::styled("NORMAL (D-Tabakasi Emilimi Dusuk)", Style::default().fg(TELETEXT_GREEN)),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL).title(" [NOAA SWPC SOLAR & JEOMANYETIK IZLEME] ").style(Style::default().bg(WIN98_DARK_GRAY).fg(TELETEXT_WHITE)));
    frame.render_widget(sw_p, main_chunks[0]);

    // 2. USGS Deprem Tablosu
    let q_header_cells = ["KAYNAK", "BUYUKLUK", "DERINLIK", "BOLGE / KONUM", "ZAMAN (UTC)"]
        .into_iter()
        .map(|h| Cell::from(h).style(Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)));
    let q_header = Row::new(q_header_cells).height(1).style(Style::default().bg(WIN98_DARK_GRAY));

    let q_total = state.earthquakes.len();
    let q_avail_height = (main_chunks[1].height.saturating_sub(3) as usize).max(1);
    let q_scroll = state.geo_scroll.min(q_total.saturating_sub(q_avail_height));
    let mut q_rows = Vec::new();
    for (offset, q) in state.earthquakes.iter().skip(q_scroll).take(q_avail_height).enumerate() {
        let actual_idx = q_scroll + offset;
        let mag_col = if q.mag >= 6.0 {
            TELETEXT_RED
        } else if q.mag >= 5.0 {
            TELETEXT_YELLOW
        } else {
            TELETEXT_CYAN
        };

        let cells = vec![
            Cell::from("USGS"),
            Cell::from(format!("M {:.1}", q.mag)),
            Cell::from(format!("{:.1} km", q.depth_km)),
            Cell::from(q.place.clone()),
            Cell::from(q.time.format("%H:%M:%S").to_string()),
        ];
        q_rows.push(Row::new(cells).style(Style::default().fg(mag_col)));

        let row_area = Rect {
            x: main_chunks[1].x + 1,
            y: main_chunks[1].y + 2 + offset as u16,
            width: main_chunks[1].width.saturating_sub(2),
            height: 1,
        };
        state.hit_tester.register(row_area, HitTarget::TableRow(50, actual_idx));
    }

    let q_widths = [
        Constraint::Length(10),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Min(30),
        Constraint::Length(14),
    ];
    let q_table = Table::new(q_rows, q_widths)
        .header(q_header)
        .block(Block::default().borders(Borders::ALL).title(format!(" [USGS & KANDILLI CANLI DEPREM AGISI (M4.5+) (Kaydir: {})] ", q_scroll)).style(Style::default().bg(TELETEXT_BLACK).fg(TELETEXT_WHITE)));
    frame.render_widget(q_table, main_chunks[1]);

    // 3. NASA FIRMS Termal Yangın / Patlama Tablosu
    let f_header_cells = ["UYDU", "KOORDINAT", "PARLAKLIK (KELVIN)", "GUVENILIRLIK", "ZAMAN"]
        .into_iter()
        .map(|h| Cell::from(h).style(Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)));
    let f_header = Row::new(f_header_cells).height(1).style(Style::default().bg(WIN98_DARK_GRAY));

    let f_total = state.firms_fires.len();
    let f_avail_height = (main_chunks[2].height.saturating_sub(3) as usize).max(1);
    let f_scroll = state.geo_scroll.min(f_total.saturating_sub(f_avail_height));
    let mut f_rows = Vec::new();
    for (offset, f) in state.firms_fires.iter().skip(f_scroll).take(f_avail_height).enumerate() {
        let actual_idx = f_scroll + offset;
        let cells = vec![
            Cell::from("VIIRS NRT"),
            Cell::from(format!("{:.3}N, {:.3}E", f.lat, f.lon)),
            Cell::from(format!("{:.1} K", f.brightness)),
            Cell::from(format!("%{}", f.confidence)),
            Cell::from(format!("{} {}", f.acq_date, f.acq_time)),
        ];
        f_rows.push(Row::new(cells).style(Style::default().fg(TELETEXT_RED)));

        let row_area = Rect {
            x: main_chunks[2].x + 1,
            y: main_chunks[2].y + 2 + offset as u16,
            width: main_chunks[2].width.saturating_sub(2),
            height: 1,
        };
        state.hit_tester.register(row_area, HitTarget::TableRow(51, actual_idx));
    }

    let f_widths = [
        Constraint::Length(12),
        Constraint::Length(22),
        Constraint::Length(20),
        Constraint::Length(16),
        Constraint::Min(16),
    ];
    let f_table = Table::new(f_rows, f_widths)
        .header(f_header)
        .block(Block::default().borders(Borders::ALL).title(" [NASA FIRMS TERMAL YANGIN & PATLAMA TESPITI] ").style(Style::default().bg(TELETEXT_BLACK).fg(TELETEXT_WHITE)));
    frame.render_widget(f_table, main_chunks[2]);

    // 4. SAFECAST Çevresel Radyasyon & Geiger Sensörleri Tablosu
    let r_header_cells = ["SENSOR ID", "DOZ HIZI (µSv/h)", "CPM DEGERI", "CBRN DURUMU", "KONUM / ISTASYON", "KOORDINAT", "ZAMAN (UTC)"]
        .into_iter()
        .map(|h| Cell::from(h).style(Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)));
    let r_header = Row::new(r_header_cells).height(1).style(Style::default().bg(WIN98_DARK_GRAY));

    let r_total = state.radiation_measurements.len();
    let r_avail_height = (main_chunks[3].height.saturating_sub(3) as usize).max(1);
    let r_scroll = state.radiation_scroll.min(r_total.saturating_sub(r_avail_height));
    let mut r_rows = Vec::new();
    for (offset, rad) in state.radiation_measurements.iter().skip(r_scroll).take(r_avail_height).enumerate() {
        let actual_idx = r_scroll + offset;
        let usvh = if rad.unit == "cpm" {
            rad.value / 334.0
        } else {
            rad.value
        };
        let (status_text, status_col) = if usvh >= 1.0 {
            ("[!] KRITIK / ALARM", TELETEXT_RED)
        } else if usvh >= 0.3 {
            ("[!] YUKSEK FON", TELETEXT_YELLOW)
        } else {
            ("NORMAL FON", Color::Rgb(50, 255, 120))
        };
        let loc = rad.location_name.as_deref().unwrap_or("Sabit/Mobil Geiger Istasyonu");
        let time_str = rad.captured_at.map(|t| t.format("%H:%M:%S").to_string()).unwrap_or_else(|| "N/A".to_string());

        let cells = vec![
            Cell::from(format!("#{}", rad.id)),
            Cell::from(format!("{:.4} µSv/h", usvh)),
            Cell::from(format!("{:.1} {}", rad.value, rad.unit)),
            Cell::from(status_text),
            Cell::from(loc),
            Cell::from(format!("{:.4}N, {:.4}E", rad.lat, rad.lon)),
            Cell::from(time_str),
        ];
        r_rows.push(Row::new(cells).style(Style::default().fg(status_col)));

        let row_area = Rect {
            x: main_chunks[3].x + 1,
            y: main_chunks[3].y + 2 + offset as u16,
            width: main_chunks[3].width.saturating_sub(2),
            height: 1,
        };
        state.hit_tester.register(row_area, HitTarget::TableRow(53, actual_idx));
    }

    let r_widths = [
        Constraint::Length(12),
        Constraint::Length(18),
        Constraint::Length(14),
        Constraint::Length(22),
        Constraint::Min(24),
        Constraint::Length(22),
        Constraint::Length(14),
    ];
    let r_table = Table::new(r_rows, r_widths)
        .header(r_header)
        .block(Block::default().borders(Borders::ALL).title(format!(" [SAFECAST CANLI CEVRESEL RADYASYON & GEIGER SAYACI (Kaydir: {})] ", state.radiation_scroll)).style(Style::default().bg(TELETEXT_BLACK).fg(TELETEXT_WHITE)));
    frame.render_widget(r_table, main_chunks[3]);
}
