use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use argus_core::types::ActiveView;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

#[derive(Debug, Clone)]
pub enum PaletteCommand {
    SwitchView(ActiveView),
    RegionPreset(usize),
    ToggleLayer(usize),
    TriggerAi(usize),
    OpenFeedHealth,
    OpenWebMap,
    ExportMarkdown,
    ExportHtml,
    ToggleShapes,
    CycleMapStyle,
    ToggleDvrPlay,
    CycleDvrArchive,
    ToggleTrackHistory,
    OpenGeoGoTo,
    ToggleAudio,
    ToggleSdr,
    FocusTarget(String),
}

#[derive(Debug, Clone)]
pub struct PaletteEntry {
    pub category: &'static str,
    pub label: String,
    pub hint: String,
    pub command: PaletteCommand,
}

pub fn build_palette_entries(state: &AppState) -> Vec<PaletteEntry> {
    let q = state.command_palette.query.trim().to_lowercase();

    let mut all = vec![
        PaletteEntry { category: "SEKME", label: "F1: OZET (Komuta Konsolu)".into(), hint: "F1".into(), command: PaletteCommand::SwitchView(ActiveView::Dashboard) },
        PaletteEntry { category: "SEKME", label: "F2: HARITA (Tam Ekran GEOINT & Go-To)".into(), hint: "F2".into(), command: PaletteCommand::SwitchView(ActiveView::Geoint) },
        PaletteEntry { category: "SEKME", label: "F3: HAVA (ADS-B Hava Sahasi)".into(), hint: "F3".into(), command: PaletteCommand::SwitchView(ActiveView::AirTracks) },
        PaletteEntry { category: "SEKME", label: "F4: DENIZ (AIS Filo & 6-Bogaz)".into(), hint: "F4".into(), command: PaletteCommand::SwitchView(ActiveView::SeaTracks) },
        PaletteEntry { category: "SEKME", label: "F5: SIGINT (KiwiSDR HF & EAM)".into(), hint: "F5".into(), command: PaletteCommand::SwitchView(ActiveView::Sigint) },
        PaletteEntry { category: "SEKME", label: "F6: OSINT (Haberler, Catisma & EVI)".into(), hint: "F6".into(), command: PaletteCommand::SwitchView(ActiveView::Osint) },
        PaletteEntry { category: "SEKME", label: "F7: SIBER (Shodan, BGP, CISA, IODA)".into(), hint: "F7".into(), command: PaletteCommand::SwitchView(ActiveView::Cyberint) },
        PaletteEntry { category: "SEKME", label: "F8: UYDU (SGP4 Polar Radar)".into(), hint: "F8".into(), command: PaletteCommand::SwitchView(ActiveView::Satint) },
        PaletteEntry { category: "SEKME", label: "F9: MASINT (Deprem, Termal, CBRN, NOTAM, Alarmlar)".into(), hint: "F9".into(), command: PaletteCommand::SwitchView(ActiveView::Masint) },
        PaletteEntry { category: "SEKME", label: "F10: NOTLAR & AI Rapor Arsivi".into(), hint: "F10".into(), command: PaletteCommand::SwitchView(ActiveView::Notepad) },
        PaletteEntry { category: "SEKME", label: "F11: ARAC (Hesap Makinesi & Cevirici)".into(), hint: "F11".into(), command: PaletteCommand::SwitchView(ActiveView::Calculator) },
        PaletteEntry { category: "AI", label: "AI SITREP — Mod 1: Kuresel Flashpoint Analizi".into(), hint: "Shift+A".into(), command: PaletteCommand::TriggerAi(0) },
        PaletteEntry { category: "AI", label: "AI SITREP — Mod 5: Kirmizi Hucre / OPFOR".into(), hint: "Mod 5".into(), command: PaletteCommand::TriggerAi(4) },
        PaletteEntry { category: "AI", label: "AI SITREP — Mod 6: Delta / Degisim Analizi".into(), hint: "Mod 6".into(), command: PaletteCommand::TriggerAi(5) },
        PaletteEntry { category: "SISTEM", label: "Sensor & API Saglik Matrisi (16 Kaynak)".into(), hint: "API".into(), command: PaletteCommand::OpenFeedHealth },
        PaletteEntry { category: "HARITA", label: "Koordinata Git (Lat, Lon Girisi)".into(), hint: "F2".into(), command: PaletteCommand::OpenGeoGoTo },
        PaletteEntry { category: "HARITA", label: "Canli Web Harita Ac (127.0.0.1:8090)".into(), hint: "Shift+W".into(), command: PaletteCommand::OpenWebMap },
        PaletteEntry { category: "HARITA", label: "Platform Sekillerini Ac/Kapat".into(), hint: "P".into(), command: PaletteCommand::ToggleShapes },
        PaletteEntry { category: "HARITA", label: "Harita Render Stilini Degistir".into(), hint: "V".into(), command: PaletteCommand::CycleMapStyle },
        PaletteEntry { category: "DVR", label: "DVR Replay Oynat / Duraklat".into(), hint: "DVR".into(), command: PaletteCommand::ToggleDvrPlay },
        PaletteEntry { category: "DVR", label: "DVR JSONL Arsiv Dosyasi Degistir".into(), hint: "Arsiv".into(), command: PaletteCommand::CycleDvrArchive },
        PaletteEntry { category: "DVR", label: "Secili Hedef Gunluk DVR Rota Izini Ac/Kapat".into(), hint: "Iz".into(), command: PaletteCommand::ToggleTrackHistory },
        PaletteEntry { category: "RAPOR", label: "Markdown Istihbarat Raporu Disa Aktar".into(), hint: "Ctrl+E".into(), command: PaletteCommand::ExportMarkdown },
        PaletteEntry { category: "RAPOR", label: "HTML Komuta Brifing Paketi Olustur".into(), hint: "HTML".into(), command: PaletteCommand::ExportHtml },
        PaletteEntry { category: "BOLGE", label: "Odak: Turkiye (TR)".into(), hint: "39N,35E".into(), command: PaletteCommand::RegionPreset(0) },
        PaletteEntry { category: "BOLGE", label: "Odak: Istanbul & Bogazlar (IST)".into(), hint: "41N,29E".into(), command: PaletteCommand::RegionPreset(1) },
        PaletteEntry { category: "BOLGE", label: "Odak: Dogu Akdeniz (D.Akd)".into(), hint: "35N,33E".into(), command: PaletteCommand::RegionPreset(2) },
        PaletteEntry { category: "BOLGE", label: "Odak: Karadeniz (Karad)".into(), hint: "43N,34E".into(), command: PaletteCommand::RegionPreset(3) },
        PaletteEntry { category: "BOLGE", label: "Odak: CENTCOM / Orta Dogu".into(), hint: "26N,46E".into(), command: PaletteCommand::RegionPreset(5) },
        PaletteEntry { category: "BOLGE", label: "Odak: Kuresel Gorunum (Tum Dunya)".into(), hint: "Global".into(), command: PaletteCommand::RegionPreset(4) },
        PaletteEntry { category: "KATMAN", label: "Katman Ac/Kapat: Hava (ADS-B)".into(), hint: "Air".into(), command: PaletteCommand::ToggleLayer(0) },
        PaletteEntry { category: "KATMAN", label: "Katman Ac/Kapat: Deniz (AIS)".into(), hint: "Sea".into(), command: PaletteCommand::ToggleLayer(1) },
        PaletteEntry { category: "KATMAN", label: "Katman Ac/Kapat: Uydu (SGP4)".into(), hint: "Sat".into(), command: PaletteCommand::ToggleLayer(2) },
        PaletteEntry { category: "KATMAN", label: "Katman Ac/Kapat: NOTAM / SIGMET".into(), hint: "Ntm".into(), command: PaletteCommand::ToggleLayer(11) },
        PaletteEntry { category: "KATMAN", label: "Katman Ac/Kapat: K-4 Fuzyon".into(), hint: "Fuz".into(), command: PaletteCommand::ToggleLayer(10) },
        PaletteEntry { category: "KATMAN", label: "Katman Ac/Kapat: Denizalti Kablolar".into(), hint: "Kbl".into(), command: PaletteCommand::ToggleLayer(8) },
        PaletteEntry { category: "SES", label: "Uygulama Seslerini Ac/Kapat".into(), hint: "M".into(), command: PaletteCommand::ToggleAudio },
        PaletteEntry { category: "SIGINT", label: "Canli KiwiSDR Radyo Akisini Ac/Kapat".into(), hint: "R".into(), command: PaletteCommand::ToggleSdr },
    ];

    let mut filtered: Vec<PaletteEntry> = if q.is_empty() {
        all
    } else {
        all.retain(|e| {
            e.label.to_lowercase().contains(&q)
                || e.category.to_lowercase().contains(&q)
                || e.hint.to_lowercase().contains(&q)
        });
        // Kullanici hedef cagri kodu / ICAO / MMSI yazdiysa canli hedeflerden eslesenleri de ekle
        if q.len() >= 2 {
            let mut matched_targets = 0usize;
            for t in state.air_targets.iter().chain(state.sea_targets.iter()) {
                if matched_targets >= 8 {
                    break;
                }
                if t.callsign.to_lowercase().contains(&q) || t.id.to_lowercase().contains(&q) {
                    let dom = if t.domain == argus_core::types::Domain::Air { "HAVA" } else { "DENIZ" };
                    all.push(PaletteEntry {
                        category: "HEDEF",
                        label: format!("{} [{}] ({})", t.callsign, t.id, dom),
                        hint: format!("{:.2}N,{:.2}E", t.geo.lat, t.geo.lon),
                        command: PaletteCommand::FocusTarget(t.id.clone()),
                    });
                    matched_targets += 1;
                }
            }
        }
        all
    };

    filtered.truncate(40);
    filtered
}

pub fn render_command_palette(frame: &mut Frame, screen: Rect, state: &mut AppState) {
    if !state.command_palette.is_open || screen.width < 44 || screen.height < 12 {
        return;
    }

    let entries = build_palette_entries(state);
    let total = entries.len();
    if total > 0 && state.command_palette.selected_idx >= total {
        state.command_palette.selected_idx = total - 1;
    } else if total == 0 {
        state.command_palette.selected_idx = 0;
    }

    let w = screen.width.saturating_sub(8).clamp(44, 78);
    let h = screen.height.saturating_sub(6).clamp(10, 18);
    let area = Rect {
        x: screen.x + (screen.width.saturating_sub(w)) / 2,
        y: screen.y + (screen.height.saturating_sub(h)) / 3,
        width: w,
        height: h,
    };

    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(TELETEXT_CYAN).add_modifier(Modifier::BOLD))
        .title(" [ KOMUT PALETI (Ctrl+K) │ Sekme, Aksiyon veya Hedef Ara │ Enter: Calistir │ Esc: Kapat ] ")
        .style(Style::default().bg(BG_PANEL));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(3)])
        .split(inner);

    let input_line = Line::from(vec![
        Span::styled(
            " > ",
            Style::default()
                .fg(TELETEXT_BLACK)
                .bg(TELETEXT_CYAN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {}_", state.command_palette.query),
            Style::default()
                .fg(TELETEXT_YELLOW)
                .bg(BG_SURFACE)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(input_line).style(Style::default().bg(BG_SURFACE)),
        chunks[0],
    );

    let list_h = chunks[1].height as usize;
    let sel = state.command_palette.selected_idx;
    let start = if sel >= list_h { sel + 1 - list_h } else { 0 };

    let mut lines = Vec::new();
    for (vis_i, (idx, entry)) in entries.iter().enumerate().skip(start).take(list_h).enumerate() {
        let is_sel = idx == sel;
        let bg = if is_sel { BG_HIGHLIGHT } else { BG_PANEL };
        let cat_col = match entry.category {
            "AI" => TELETEXT_YELLOW,
            "HEDEF" => TELETEXT_GREEN,
            "DVR" => TELETEXT_MAGENTA,
            "SISTEM" => TELETEXT_RED,
            _ => TELETEXT_CYAN,
        };
        let prefix = if is_sel { "▸ " } else { "  " };
        let cat_str = format!("[{:^6}] ", entry.category);
        let hint_str = format!(" ({})", entry.hint);
        let avail_lbl = (chunks[1].width as usize)
            .saturating_sub(prefix.chars().count() + cat_str.chars().count() + hint_str.chars().count());
        let clipped_lbl: String = entry.label.chars().take(avail_lbl).collect();
        let pad = avail_lbl.saturating_sub(clipped_lbl.chars().count());

        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(TELETEXT_YELLOW).bg(bg).add_modifier(Modifier::BOLD)),
            Span::styled(cat_str, Style::default().fg(cat_col).bg(bg).add_modifier(Modifier::BOLD)),
            Span::styled(
                clipped_lbl,
                Style::default()
                    .fg(if is_sel { TELETEXT_YELLOW } else { FG_PRIMARY })
                    .bg(bg)
                    .add_modifier(if is_sel { Modifier::BOLD } else { Modifier::empty() }),
            ),
            Span::styled(" ".repeat(pad), Style::default().bg(bg)),
            Span::styled(hint_str, Style::default().fg(FG_MUTED).bg(bg)),
        ]));
        state.hit_tester.register(
            Rect {
                x: chunks[1].x,
                y: chunks[1].y + vis_i as u16,
                width: chunks[1].width,
                height: 1,
            },
            HitTarget::CommandPaletteItem(idx),
        );
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  Eslesen komut veya canli hedef bulunamadi.",
            Style::default().fg(FG_MUTED),
        )));
    }
    frame.render_widget(Paragraph::new(lines), chunks[1]);
}
