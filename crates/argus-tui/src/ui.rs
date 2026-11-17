use crate::photo_ui::PhotoRenderCtx;
use crate::views::{
    air_tracks::render_air_tracks_view, calculator::render_calculator_view,
    cyberint::render_cyberint_view, dashboard::render_dashboard_view,
    geoint::render_geoint_view, masint::render_masint_view, notepad::render_notepad_view,
    osint::render_osint_view, satint::render_satint_view, sea_tracks::render_sea_tracks_view,
    sigint::render_sigint_view,
};
use crate::widgets::dialog::render_dialog;
use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use argus_core::types::ActiveView;
use chrono::Utc;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};

pub fn render_main_ui(frame: &mut Frame, state: &mut AppState, photo_ctx: &mut PhotoRenderCtx) {
    let screen = frame.area();
    state.hit_tester.clear();
    state.pending_photo_slot = None;

    let bg = Block::default().style(Style::default().bg(BG_PRIMARY));
    frame.render_widget(bg, screen);

    if screen.height < 8 || screen.width < 30 {
        return;
    }

    // Modern C4ISR Yerlesimi:
    // - Ust Komuta Güvertesi (3 satir: Sensor Telemetri + Sekme Cubugu + Baglamsal Alt-Arac Cubugu)
    // - Ana Calisma Alani (Tam genislik, her sekmenin kendi ozel cok-sutunlu kokpiti)
    // - Alt Durum ve Kisayol Seridi (1 satir)
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(1),
        ])
        .split(screen);

    let header_area = layout[0];
    let body_area = layout[1];
    let footer_area = layout[2];

    render_header_deck(frame, header_area, state);

    match state.active_view {
        ActiveView::Splash | ActiveView::Dashboard => {
            render_dashboard_view(frame, body_area, state)
        }
        ActiveView::Geoint => render_geoint_view(frame, body_area, state),
        ActiveView::AirTracks => render_air_tracks_view(frame, body_area, state),
        ActiveView::SeaTracks => render_sea_tracks_view(frame, body_area, state),
        ActiveView::Sigint => render_sigint_view(frame, body_area, state),
        ActiveView::Osint => render_osint_view(frame, body_area, state),
        ActiveView::Cyberint => render_cyberint_view(frame, body_area, state),
        ActiveView::Satint => render_satint_view(frame, body_area, state),
        ActiveView::Masint => render_masint_view(frame, body_area, state),
        ActiveView::Notepad => render_notepad_view(frame, body_area, state),
        ActiveView::Calculator => render_calculator_view(frame, body_area, state),
    }

    render_footer_strip(frame, footer_area, state);

    if state.is_dialog_open {
        render_dialog(frame, screen, state);
    }

    photo_ctx.render_active_slot(frame, state);
}

fn render_header_deck(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    // SATIR 0: Kuresel Sensor Durumu, Canli KiwiSDR Mini-Kontrol, Web Harita, Rapor ve Saat
    let mil_air = state.air_targets.iter().filter(|t| t.is_military).count();
    let mil_sea = state.sea_targets.iter().filter(|t| t.is_military).count();
    let threat_badge = if !state.alerts.is_empty() || mil_air > 5 {
        ("TEHDIT:YUKSEK", Color::Rgb(190, 30, 30))
    } else {
        ("TEHDIT:NORMAL", Color::Rgb(20, 110, 60))
    };

    let mut r0 = vec![
        Span::styled(
            " ARGUS // C4ISR ",
            Style::default()
                .fg(TELETEXT_BLACK)
                .bg(TELETEXT_CYAN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {} ", threat_badge.0),
            Style::default()
                .fg(FG_PRIMARY)
                .bg(threat_badge.1)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                " │ ADS-B:{} (Askeri:{}) │ AIS:{} (Harp:{}) │ SGP4:{} │ DEPREM:{} │ EAM:{} ",
                state.air_targets.len(),
                mil_air,
                state.sea_targets.len(),
                mil_sea,
                state.satellites.len(),
                state.earthquakes.len(),
                state.eam_messages.len()
            ),
            Style::default().fg(FG_PRIMARY),
        ),
    ];

    let sdr_lbl = if state.is_radio_playing {
        format!(
            " [R: SDR CANLI {:.0}kHz {:.0}dBm] ",
            state.current_frequency_khz, state.sdr_rssi_dbm
        )
    } else {
        format!(" [R: SDR KAPALI {:.0}kHz] ", state.current_frequency_khz)
    };
    let web_lbl = " [W: WEB HARITA] ";
    let rep_lbl = " [RAPOR] ";
    let snd_lbl = if state.audio_muted {
        " [M: SESSIZ] "
    } else {
        " [M: SES ACIK] "
    };
    let clk_lbl = format!(" {} UTC ", Utc::now().format("%H:%M:%S"));

    let right_len = (sdr_lbl.chars().count()
        + web_lbl.chars().count()
        + rep_lbl.chars().count()
        + snd_lbl.chars().count()
        + clk_lbl.chars().count()) as u16;
    let left_used: u16 = r0.iter().map(|s| s.content.chars().count() as u16).sum();
    let pad = area.width.saturating_sub(left_used + right_len) as usize;
    r0.push(Span::raw(" ".repeat(pad)));

    let mut rx = area.x + area.width.saturating_sub(right_len);

    // SDR Mini Buton
    let sdr_w = sdr_lbl.chars().count() as u16;
    r0.push(Span::styled(
        sdr_lbl,
        Style::default()
            .fg(if state.is_radio_playing {
                TELETEXT_BLACK
            } else {
                TELETEXT_YELLOW
            })
            .bg(if state.is_radio_playing {
                TELETEXT_GREEN
            } else {
                BG_SURFACE
            })
            .add_modifier(Modifier::BOLD),
    ));
    state.hit_tester.register(
        Rect { x: rx, y: rows[0].y, width: sdr_w, height: 1 },
        HitTarget::ToggleRadioAudio,
    );
    rx += sdr_w;

    // Web Harita Butonu
    let web_w = web_lbl.chars().count() as u16;
    r0.push(Span::styled(
        web_lbl,
        Style::default()
            .fg(TELETEXT_CYAN)
            .bg(BG_PRIMARY)
            .add_modifier(Modifier::BOLD),
    ));
    state.hit_tester.register(
        Rect { x: rx, y: rows[0].y, width: web_w, height: 1 },
        HitTarget::OpenWebMap,
    );
    rx += web_w;

    // Rapor Butonu
    let rep_w = rep_lbl.chars().count() as u16;
    r0.push(Span::styled(
        rep_lbl,
        Style::default()
            .fg(TELETEXT_GREEN)
            .bg(BG_SURFACE)
            .add_modifier(Modifier::BOLD),
    ));
    state.hit_tester.register(
        Rect { x: rx, y: rows[0].y, width: rep_w, height: 1 },
        HitTarget::ExportReport,
    );
    rx += rep_w;

    // Ses Butonu
    let snd_w = snd_lbl.chars().count() as u16;
    r0.push(Span::styled(
        snd_lbl,
        Style::default()
            .fg(if state.audio_muted {
                TELETEXT_RED
            } else {
                TELETEXT_GREEN
            })
            .bg(BG_PRIMARY),
    ));
    state.hit_tester.register(
        Rect { x: rx, y: rows[0].y, width: snd_w, height: 1 },
        HitTarget::TrayAudio,
    );
    rx += snd_w;

    // Saat
    let clk_w = clk_lbl.chars().count() as u16;
    r0.push(Span::styled(
        clk_lbl,
        Style::default()
            .fg(TELETEXT_YELLOW)
            .bg(BG_SURFACE)
            .add_modifier(Modifier::BOLD),
    ));
    state.hit_tester.register(
        Rect { x: rx, y: rows[0].y, width: clk_w, height: 1 },
        HitTarget::TrayClock,
    );

    frame.render_widget(
        Paragraph::new(Line::from(r0)).style(Style::default().bg(BG_SURFACE)),
        rows[0],
    );

    // SATIR 1: Sekme Navigasyon Cubugu + Kuresel Arama Kutusu
    let tabs = [
        (ActiveView::Dashboard, "F1:KOMUTA MERKEZI"),
        (ActiveView::Geoint, "F2:HARITA"),
        (ActiveView::AirTracks, "F3:HAVA RADARI"),
        (ActiveView::SeaTracks, "F4:DENIZ FILOSU"),
        (ActiveView::Sigint, "F5:SIGINT/SDR"),
        (ActiveView::Osint, "F6:OSINT"),
        (ActiveView::Cyberint, "F7:SIBER"),
        (ActiveView::Satint, "F8:UYDU"),
        (ActiveView::Masint, "F9:JEOSISMIK"),
        (ActiveView::Notepad, "F10:NOTLAR"),
        (ActiveView::Calculator, "F11:ARACLAR"),
    ];

    let mut r1 = Vec::new();
    let mut tx = rows[1].x;
    for (view, label) in tabs.iter() {
        let is_active = state.active_view == *view;
        let text = if is_active {
            format!(" > {} < ", label)
        } else {
            format!("   {}   ", label)
        };
        let w = text.chars().count() as u16;
        if tx + w < rows[1].x + rows[1].width.saturating_sub(24) {
            let st = if is_active {
                Style::default()
                    .fg(TELETEXT_BLACK)
                    .bg(TELETEXT_CYAN)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG_SECONDARY).bg(BG_PANEL)
            };
            r1.push(Span::styled(text, st));
            r1.push(Span::styled("│", Style::default().fg(BORDER_DIM).bg(BG_PANEL)));
            state.hit_tester.register(
                Rect { x: tx, y: rows[1].y, width: w, height: 1 },
                HitTarget::TaskbarViewTab(*view),
            );
            tx += w + 1;
        }
    }

    // Sagda Kuresel Arama Kutusu
    let search_str = if state.is_search_active {
        format!(" [/ ARA: {}_ ] ", state.search_query)
    } else if !state.search_query.is_empty() {
        format!(" [FILTRE: {}] ", state.search_query)
    } else {
        " [/ KURESEL FILTRE & ARA] ".to_string()
    };
    let sw = search_str.chars().count() as u16;
    if tx + sw < rows[1].x + rows[1].width {
        let fill = (rows[1].x + rows[1].width - tx - sw) as usize;
        r1.push(Span::raw(" ".repeat(fill)));
        let sx = rows[1].x + rows[1].width - sw;
        r1.push(Span::styled(
            search_str,
            Style::default()
                .fg(if state.is_search_active || !state.search_query.is_empty() {
                    TELETEXT_YELLOW
                } else {
                    TELETEXT_CYAN
                })
                .bg(BG_HIGHLIGHT)
                .add_modifier(Modifier::BOLD),
        ));
        state.hit_tester.register(
            Rect { x: tx + fill as u16, y: rows[1].y, width: sw, height: 1 },
            HitTarget::SearchToggle,
        );
        let _ = sx;
    }

    frame.render_widget(
        Paragraph::new(Line::from(r1)).style(Style::default().bg(BG_PANEL)),
        rows[1],
    );

    // SATIR 2: Aktif Sekmeye Gore Baglamsal Hizli Kontrol Seridi (Contextual Sub-Toolbar)
    let mut r2 = Vec::new();
    let mut cx = rows[2].x + 1;
    r2.push(Span::raw(" "));

    match state.active_view {
        ActiveView::Dashboard | ActiveView::Geoint | ActiveView::Splash => {
            let regions = [
                ("[ODAK: Turkiye]", 0),
                ("[Istanbul & Bogazlar]", 1),
                ("[Dogu Akdeniz]", 2),
                ("[Karadeniz]", 3),
                ("[CENTCOM]", 5),
                ("[Kuresel]", 4),
            ];
            for (lbl, idx) in regions {
                let lw = lbl.chars().count() as u16;
                r2.push(Span::styled(
                    lbl,
                    Style::default().fg(TELETEXT_CYAN).bg(BG_SURFACE).add_modifier(Modifier::BOLD),
                ));
                r2.push(Span::raw(" "));
                state.hit_tester.register(
                    Rect { x: cx, y: rows[2].y, width: lw, height: 1 },
                    HitTarget::RegionPreset(idx),
                );
                cx += lw + 1;
            }
            r2.push(Span::styled(" │ KATMANLAR: ", Style::default().fg(FG_MUTED)));
            cx += 14;
            let layers = [
                ("Hava", 0),
                ("Deniz", 1),
                ("Uydu", 2),
                ("Termal", 3),
                ("Deprem", 4),
            ];
            for (lname, lbit) in layers {
                let on = (state.geoint_layers & (1 << lbit)) != 0;
                let tag = if on {
                    format!("[x:{}]", lname)
                } else {
                    format!("[ :{}]", lname)
                };
                let tw = tag.chars().count() as u16;
                r2.push(Span::styled(
                    tag,
                    Style::default().fg(if on { TELETEXT_GREEN } else { FG_MUTED }),
                ));
                r2.push(Span::raw(" "));
                state.hit_tester.register(
                    Rect { x: cx, y: rows[2].y, width: tw, height: 1 },
                    HitTarget::GeointLayer(lbit),
                );
                cx += tw + 1;
            }
        }
        ActiveView::AirTracks => {
            let filters = [
                ("[1: Kuresel Tumu]", 0),
                ("[2: CENTCOM Tumu]", 1),
                ("[3: CENTCOM Askeri]", 2),
                ("[4: Kuresel Askeri]", 3),
                ("[5: TR/EUCOM]", 4),
                ("[6: Acil 7700/7600/7500]", 5),
            ];
            for (lbl, fid) in filters {
                let lw = lbl.chars().count() as u16;
                let active = state.air_filter == fid;
                r2.push(Span::styled(
                    lbl,
                    Style::default()
                        .fg(if active { TELETEXT_BLACK } else { FG_PRIMARY })
                        .bg(if active { TELETEXT_YELLOW } else { BG_SURFACE })
                        .add_modifier(Modifier::BOLD),
                ));
                r2.push(Span::raw(" "));
                state.hit_tester.register(
                    Rect { x: cx, y: rows[2].y, width: lw, height: 1 },
                    HitTarget::AirFilter(fid),
                );
                cx += lw + 1;
            }
        }
        ActiveView::SeaTracks => {
            let filters = [
                ("[1: Tum Donanma & Filo]", 0),
                ("[2: Savas Gemileri]", 1),
                ("[3: Sahil Guvenlik]", 2),
                ("[4: Ticari Gemiler]", 3),
            ];
            for (lbl, fid) in filters {
                let lw = lbl.chars().count() as u16;
                let active = state.sea_filter == fid;
                r2.push(Span::styled(
                    lbl,
                    Style::default()
                        .fg(if active { TELETEXT_BLACK } else { FG_PRIMARY })
                        .bg(if active { TELETEXT_GREEN } else { BG_SURFACE })
                        .add_modifier(Modifier::BOLD),
                ));
                r2.push(Span::raw(" "));
                state.hit_tester.register(
                    Rect { x: cx, y: rows[2].y, width: lw, height: 1 },
                    HitTarget::SeaFilter(fid),
                );
                cx += lw + 1;
            }
        }
        _ => {
            r2.push(Span::styled(
                format!("DURUM: {} │ ISTASYON: {:.4}N, {:.4}E", state.sdr_status, state.station_lat, state.station_lon),
                Style::default().fg(FG_SECONDARY),
            ));
        }
    }

    frame.render_widget(
        Paragraph::new(Line::from(r2)).style(Style::default().bg(BG_PRIMARY)),
        rows[2],
    );
}

fn render_footer_strip(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let mut spans = vec![
        Span::styled(
            " KISAYOLLAR: ",
            Style::default()
                .fg(TELETEXT_BLACK)
                .bg(TELETEXT_YELLOW)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " [F1-F11] Sekme Degistir │ [Tikla/Oklar] Hedef Sec │ [Enter/CiftTik] Tam Dosya │ [G] Haritada Odakla │ [F] Takip │ [B] Watchlist │ [R/S] Canli SDR Radyo │ [W] Web Harita ",
            Style::default().fg(FG_PRIMARY).bg(BG_SURFACE),
        ),
    ];

    let help_lbl = " [?: KILAVUZ] ";
    let hw = help_lbl.chars().count() as u16;
    let used: u16 = spans.iter().map(|s| s.content.chars().count() as u16).sum();
    if area.width > used + hw {
        spans.push(Span::styled(
            " ".repeat((area.width - used - hw) as usize),
            Style::default().bg(BG_SURFACE),
        ));
        let hx = area.x + area.width - hw;
        spans.push(Span::styled(
            help_lbl,
            Style::default()
                .fg(TELETEXT_BLACK)
                .bg(TELETEXT_CYAN)
                .add_modifier(Modifier::BOLD),
        ));
        state.hit_tester.register(
            Rect { x: hx, y: area.y, width: hw, height: 1 },
            HitTarget::HelpButton,
        );
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(BG_SURFACE)),
        area,
    );
}
