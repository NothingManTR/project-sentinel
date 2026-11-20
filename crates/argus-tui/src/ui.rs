use crate::photo_ui::PhotoRenderCtx;
use crate::views::{
    air_tracks::render_air_tracks_view, calculator::render_calculator_view,
    cyberint::render_cyberint_view, dashboard::render_dashboard_view,
    geoint::render_geoint_view, masint::render_masint_view, notepad::render_notepad_view,
    osint::render_osint_view, satint::render_satint_view, sea_tracks::render_sea_tracks_view,
    sigint::render_sigint_view,
};
use crate::widgets::{command_palette::render_command_palette, dialog::render_dialog};
use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use argus_core::types::ActiveView;
use chrono::Utc;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
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

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(1),
        ])
        .split(screen);

    render_header_deck(frame, layout[0], state);

    match state.active_view {
        ActiveView::Dashboard => render_dashboard_view(frame, layout[1], state),
        ActiveView::Geoint => render_geoint_view(frame, layout[1], state),
        ActiveView::AirTracks => render_air_tracks_view(frame, layout[1], state),
        ActiveView::SeaTracks => render_sea_tracks_view(frame, layout[1], state),
        ActiveView::Sigint => render_sigint_view(frame, layout[1], state),
        ActiveView::Osint => render_osint_view(frame, layout[1], state),
        ActiveView::Cyberint => render_cyberint_view(frame, layout[1], state),
        ActiveView::Satint => render_satint_view(frame, layout[1], state),
        ActiveView::Masint => render_masint_view(frame, layout[1], state),
        ActiveView::Notepad => render_notepad_view(frame, layout[1], state),
        ActiveView::Calculator => render_calculator_view(frame, layout[1], state),
    }

    render_footer_strip(frame, layout[2], state);

    render_flash_toast(frame, screen, state);

    if state.is_dialog_open {
        render_dialog(frame, screen, state);
    }

    if state.command_palette.is_open {
        render_command_palette(frame, screen, state);
    }

    photo_ctx.render_active_slot(frame, state);
}

fn format_compact_count(n: usize) -> String {
    if n >= 1000 {
        format!("{:.1}k", (n as f64) / 1000.0)
    } else {
        n.to_string()
    }
}

fn defcon_style(level_opt: Option<u8>) -> (String, Color) {
    match level_opt {
        Some(1) => (" DEFCON 1 ".to_string(), Color::Rgb(175, 42, 42)),
        Some(2) => (" DEFCON 2 ".to_string(), Color::Rgb(165, 72, 42)),
        Some(3) => (" DEFCON 3 ".to_string(), Color::Rgb(158, 122, 44)),
        Some(4) => (" DEFCON 4 ".to_string(), Color::Rgb(48, 86, 128)),
        Some(5) => (" DEFCON 5 ".to_string(), Color::Rgb(40, 92, 64)),
        _ => (" DEFCON:-- ".to_string(), BG_HIGHLIGHT),
    }
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

    // SATIR 0: ARGUS + OSINT DEFCON Status + Kompakt Telemetri + Sag Kontroller & Saat
    let mil_air = state.air_targets.iter().filter(|t| t.is_military).count();
    let mil_sea = state.sea_targets.iter().filter(|t| t.is_military).count();
    let (defcon_lbl, defcon_bg) = defcon_style(state.defcon_status.as_ref().map(|d| d.overall_level));
    let brand_lbl = " ARGUS ";
    let defcon_w = defcon_lbl.chars().count() as u16;
    let brand_w = brand_lbl.chars().count() as u16;

    let wx_tag = state
        .station_weather
        .as_ref()
        .map(|w| format!(" │ {:.0}C {:.0}kt", w.temp_c, w.wind_kts))
        .unwrap_or_default();

    let metrics_str = format!(
        " AIR:{}({}M) │ SEA:{}({}W) │ SAT:{} │ GEO:{} │ EAM:{} │ CAT:{} │ ALR:{}{} ",
        format_compact_count(state.air_targets.len()),
        mil_air,
        state.sea_targets.len(),
        mil_sea,
        state.satellites.len(),
        state.earthquakes.len(),
        state.eam_messages.len(),
        state.acled_events.len(),
        state.alerts.len(),
        wx_tag,
    );

    let mut r0 = vec![
        Span::styled(
            brand_lbl,
            Style::default()
                .fg(TELETEXT_BLACK)
                .bg(TELETEXT_CYAN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            defcon_lbl,
            Style::default()
                .fg(FG_PRIMARY)
                .bg(defcon_bg)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    state.hit_tester.register(
        Rect {
            x: rows[0].x + brand_w,
            y: rows[0].y,
            width: defcon_w,
            height: 1,
        },
        HitTarget::DefconBadge,
    );

    let (ok_feeds, tot_feeds) = state.feed_health_counts();
    let api_lbl = format!(" [API:{}/{}] ", ok_feeds, tot_feeds);
    let api_col = if tot_feeds == 0 {
        FG_MUTED
    } else if ok_feeds == tot_feeds {
        TELETEXT_GREEN
    } else if ok_feeds * 2 >= tot_feeds {
        TELETEXT_YELLOW
    } else {
        TELETEXT_RED
    };

    let sdr_lbl = if state.is_radio_playing {
        format!(" [SDR:{:.0}k {:.0}dB] ", state.current_frequency_khz, state.sdr_rssi_dbm)
    } else {
        format!(" [SDR:{:.0}k] ", state.current_frequency_khz)
    };
    let ai_lbl = if state.ai_analysis_running {
        " [AI:..] "
    } else {
        " [AI:SITREP] "
    };
    let web_lbl = " [Web] ";
    let rep_lbl = " [Rapor] ";
    let snd_lbl = if state.audio_muted { " [Sus] " } else { " [Ses] " };
    let clk_lbl = format!(" {}Z ", Utc::now().format("%H:%M:%S"));

    let right_len = (api_lbl.chars().count()
        + sdr_lbl.chars().count()
        + ai_lbl.chars().count()
        + web_lbl.chars().count()
        + rep_lbl.chars().count()
        + snd_lbl.chars().count()
        + clk_lbl.chars().count()) as u16;

    let max_metrics = area.width.saturating_sub(brand_w + defcon_w + right_len + 1) as usize;
    let clipped_metrics: String = metrics_str.chars().take(max_metrics).collect();
    r0.push(Span::styled(clipped_metrics.clone(), Style::default().fg(FG_PRIMARY)));

    let left_used = brand_w + defcon_w + clipped_metrics.chars().count() as u16;
    let pad = area.width.saturating_sub(left_used + right_len) as usize;
    r0.push(Span::raw(" ".repeat(pad)));

    let mut rx = area.x + area.width.saturating_sub(right_len);

    let api_w = api_lbl.chars().count() as u16;
    r0.push(Span::styled(
        api_lbl,
        Style::default()
            .fg(api_col)
            .bg(BG_PRIMARY)
            .add_modifier(Modifier::BOLD),
    ));
    state.hit_tester.register(
        Rect { x: rx, y: rows[0].y, width: api_w, height: 1 },
        HitTarget::FeedHealthModal,
    );
    rx += api_w;

    let sdr_w = sdr_lbl.chars().count() as u16;
    r0.push(Span::styled(
        sdr_lbl,
        Style::default()
            .fg(if state.is_radio_playing { TELETEXT_BLACK } else { TELETEXT_YELLOW })
            .bg(if state.is_radio_playing { TELETEXT_GREEN } else { BG_SURFACE })
            .add_modifier(Modifier::BOLD),
    ));
    state.hit_tester.register(
        Rect { x: rx, y: rows[0].y, width: sdr_w, height: 1 },
        HitTarget::ToggleRadioAudio,
    );
    rx += sdr_w;

    let ai_w = ai_lbl.chars().count() as u16;
    r0.push(Span::styled(
        ai_lbl,
        Style::default()
            .fg(TELETEXT_BLACK)
            .bg(if state.ai_analysis_running { TELETEXT_RED } else { TELETEXT_YELLOW })
            .add_modifier(Modifier::BOLD),
    ));
    state.hit_tester.register(
        Rect { x: rx, y: rows[0].y, width: ai_w, height: 1 },
        HitTarget::TriggerAiAnalysis,
    );
    rx += ai_w;

    let web_w = web_lbl.chars().count() as u16;
    r0.push(Span::styled(
        web_lbl,
        Style::default().fg(TELETEXT_CYAN).bg(BG_PRIMARY).add_modifier(Modifier::BOLD),
    ));
    state.hit_tester.register(
        Rect { x: rx, y: rows[0].y, width: web_w, height: 1 },
        HitTarget::OpenWebMap,
    );
    rx += web_w;

    let rep_w = rep_lbl.chars().count() as u16;
    r0.push(Span::styled(
        rep_lbl,
        Style::default().fg(TELETEXT_GREEN).bg(BG_SURFACE).add_modifier(Modifier::BOLD),
    ));
    state.hit_tester.register(
        Rect { x: rx, y: rows[0].y, width: rep_w, height: 1 },
        HitTarget::ExportReport,
    );
    rx += rep_w;

    let snd_w = snd_lbl.chars().count() as u16;
    r0.push(Span::styled(
        snd_lbl,
        Style::default()
            .fg(if state.audio_muted { TELETEXT_RED } else { TELETEXT_GREEN })
            .bg(BG_PRIMARY),
    ));
    state.hit_tester.register(
        Rect { x: rx, y: rows[0].y, width: snd_w, height: 1 },
        HitTarget::TrayAudio,
    );
    rx += snd_w;

    let clk_w = clk_lbl.chars().count() as u16;
    r0.push(Span::styled(
        clk_lbl,
        Style::default().fg(TELETEXT_YELLOW).bg(BG_SURFACE).add_modifier(Modifier::BOLD),
    ));
    state.hit_tester.register(
        Rect { x: rx, y: rows[0].y, width: clk_w, height: 1 },
        HitTarget::TrayClock,
    );

    frame.render_widget(
        Paragraph::new(Line::from(r0)).style(Style::default().bg(BG_SURFACE)),
        rows[0],
    );

    // SATIR 1: 11 Sekmenin Tamami Tek Satirda Eksiksiz + Sagda Arama Kutusu
    let tabs = [
        (ActiveView::Dashboard, "F1:OZET"),
        (ActiveView::Geoint, "F2:HARITA"),
        (ActiveView::AirTracks, "F3:HAVA"),
        (ActiveView::SeaTracks, "F4:DENIZ"),
        (ActiveView::Sigint, "F5:SIGINT"),
        (ActiveView::Osint, "F6:OSINT"),
        (ActiveView::Cyberint, "F7:SIBER"),
        (ActiveView::Satint, "F8:UYDU"),
        (ActiveView::Masint, "F9:MASINT"),
        (ActiveView::Notepad, "F10:NOT"),
        (ActiveView::Calculator, "F11:ARAC"),
    ];

    let search_str = if state.is_search_active {
        format!(" [/ ARA: {}_ ] ", state.search_query)
    } else if !state.search_query.is_empty() {
        format!(" [FILTRE: {}] ", state.search_query)
    } else {
        " [/ Filtre] ".to_string()
    };
    let sw = search_str.chars().count() as u16;
    let tab_budget = rows[1].x + rows[1].width.saturating_sub(sw + 1);

    let mut r1 = Vec::new();
    let mut tx = rows[1].x;
    for (idx, (view, label)) in tabs.iter().enumerate() {
        let is_active = state.active_view == *view;
        let text = if is_active {
            format!(" ▸{} ", label)
        } else {
            format!(" {} ", label)
        };
        let w = text.chars().count() as u16;
        let sep_w = if idx > 0 { 1 } else { 0 };
        if tx + sep_w + w <= tab_budget {
            if idx > 0 {
                r1.push(Span::styled("│", Style::default().fg(FG_MUTED).bg(BG_PANEL)));
                tx += 1;
            }
            let st = if is_active {
                Style::default()
                    .fg(TELETEXT_BLACK)
                    .bg(TELETEXT_CYAN)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG_SECONDARY).bg(BG_PANEL)
            };
            r1.push(Span::styled(text, st));
            state.hit_tester.register(
                Rect { x: tx, y: rows[1].y, width: w, height: 1 },
                HitTarget::TaskbarViewTab(*view),
            );
            tx += w;
        }
    }

    if rows[1].x + rows[1].width >= tx + sw {
        let fill = (rows[1].x + rows[1].width - tx - sw) as usize;
        if fill > 0 {
            r1.push(Span::raw(" ".repeat(fill)));
        }
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
    }

    frame.render_widget(
        Paragraph::new(Line::from(r1)).style(Style::default().bg(BG_PANEL)),
        rows[1],
    );

    // SATIR 2: Kompakt Baglamsal Kontrol Seridi
    let mut r2 = Vec::new();
    let mut cx = rows[2].x + 1;
    r2.push(Span::raw(" "));

    match state.active_view {
        ActiveView::Dashboard | ActiveView::Geoint => {
            let regions = [
                ("[TR]", 0),
                ("[IST]", 1),
                ("[D.Akd]", 2),
                ("[Krdnz]", 3),
                ("[CENT]", 5),
                ("[Dunya]", 4),
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
            r2.push(Span::styled("│ ", Style::default().fg(BORDER_DIM)));
            cx += 2;

            let layers = [
                ("Air", 0),
                ("Sea", 1),
                ("Sat", 2),
                ("Fire", 3),
                ("Geo", 4),
                ("SDR", 7),
                ("Kbl", 8),
                ("Met", 9),
                ("Fuz", 10),
                ("Ntm", 11),
            ];
            for (lname, lbit) in layers {
                let on = (state.geoint_layers & (1 << lbit)) != 0;
                let tag = if on {
                    format!("[x:{}]", lname)
                } else {
                    format!("[ :{}]", lname)
                };
                let tw = tag.chars().count() as u16;
                if cx + tw + 9 <= rows[2].x + rows[2].width {
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
            let sh_on = state.map_aircraft_shapes;
            let sh_tag = if sh_on { "[x:Sekil]" } else { "[ :Sekil]" };
            let sh_w = sh_tag.chars().count() as u16;
            if cx + sh_w <= rows[2].x + rows[2].width {
                r2.push(Span::styled(
                    sh_tag,
                    Style::default().fg(if sh_on { TELETEXT_YELLOW } else { FG_MUTED }),
                ));
                state.hit_tester.register(
                    Rect { x: cx, y: rows[2].y, width: sh_w, height: 1 },
                    HitTarget::ToggleMapAircraftShapes,
                );
                cx += sh_w;
            }
            let dvr_mins = state.dvr.replay_offset_mins;
            let dvr_lbl = if dvr_mins > 0 {
                format!("[DVR:-{}d]", dvr_mins)
            } else {
                "[DVR:CANLI]".to_string()
            };
            let dvr_w = dvr_lbl.chars().count() as u16;
            if cx + 3 + dvr_w <= rows[2].x + rows[2].width {
                r2.push(Span::styled(" │ ", Style::default().fg(BORDER_DIM)));
                cx += 3;
                r2.push(Span::styled(
                    dvr_lbl,
                    Style::default()
                        .fg(if dvr_mins > 0 { TELETEXT_BLACK } else { TELETEXT_CYAN })
                        .bg(if dvr_mins > 0 { TELETEXT_YELLOW } else { BG_PRIMARY })
                        .add_modifier(Modifier::BOLD),
                ));
                state.hit_tester.register(
                    Rect { x: cx, y: rows[2].y, width: dvr_w, height: 1 },
                    if dvr_mins > 0 {
                        HitTarget::DvrStep(0)
                    } else {
                        HitTarget::DvrStep(-5)
                    },
                );
                cx += dvr_w;
                let step_lbl = "[-15d]";
                let step_w = step_lbl.chars().count() as u16;
                if cx + 1 + step_w <= rows[2].x + rows[2].width {
                    r2.push(Span::raw(" "));
                    cx += 1;
                    r2.push(Span::styled(
                        step_lbl,
                        Style::default().fg(FG_SECONDARY).add_modifier(Modifier::BOLD),
                    ));
                    state.hit_tester.register(
                        Rect { x: cx, y: rows[2].y, width: step_w, height: 1 },
                        HitTarget::DvrStep(-15),
                    );
                }
            }
        }
        ActiveView::AirTracks => {
            let filters = [
                ("[1:Kuresel]", 0),
                ("[2:CENTCOM]", 1),
                ("[3:CENT-Mil]", 2),
                ("[4:Krs-Mil]", 3),
                ("[5:TR/EU]", 4),
                ("[6:Acil 7x00]", 5),
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
                ("[1:Tum Filo]", 0),
                ("[2:Harp]", 1),
                ("[3:SahilGuv]", 2),
                ("[4:Ticari]", 3),
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
            let alarm_cnt = state.alerts.len();
            let anomaly_cnt = state.track_anomalies.len();
            let fusion_cnt = state.fusion_clusters.len();
            let conflict_cnt = state.acled_events.len();
            let notam_cnt = state.airspace_warnings.len();

            r2.push(Span::styled(
                format!("ISTASYON: {:.2}N,{:.2}E", state.station_lat, state.station_lon),
                Style::default().fg(FG_SECONDARY),
            ));
            r2.push(Span::styled(" │ ", Style::default().fg(BORDER_DIM)));
            r2.push(Span::styled(
                format!("ALARM:{}", alarm_cnt),
                Style::default()
                    .fg(if alarm_cnt > 0 { TELETEXT_RED } else { FG_MUTED })
                    .add_modifier(if alarm_cnt > 0 { Modifier::BOLD } else { Modifier::empty() }),
            ));
            r2.push(Span::styled(" │ ", Style::default().fg(BORDER_DIM)));
            r2.push(Span::styled(
                format!("ANOMALI:{}", anomaly_cnt),
                Style::default().fg(TELETEXT_YELLOW),
            ));
            r2.push(Span::styled(" │ ", Style::default().fg(BORDER_DIM)));
            r2.push(Span::styled(
                format!("FUZYON:{}", fusion_cnt),
                Style::default().fg(TELETEXT_CYAN),
            ));
            r2.push(Span::styled(" │ ", Style::default().fg(BORDER_DIM)));
            r2.push(Span::styled(
                format!("CATISMA:{}", conflict_cnt),
                Style::default().fg(TELETEXT_RED),
            ));
            r2.push(Span::styled(" │ ", Style::default().fg(BORDER_DIM)));
            r2.push(Span::styled(
                format!("NOTAM:{}", notam_cnt),
                Style::default().fg(TELETEXT_MAGENTA),
            ));
            r2.push(Span::styled(" │ ", Style::default().fg(BORDER_DIM)));
            r2.push(Span::styled(
                state.live_status.clone(),
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
    let ctx_hints = match state.active_view {
        ActiveView::Dashboard => {
            " [1-7/Tab] Panel │ [Z] Buyut │ [P] Sekiller │ [Enter/O] Dosya │ [Ctrl+K] Komut │ [Shift+A] AI "
        }
        ActiveView::Geoint => {
            " [WASD/Ok] Gez │ [+/-] Zoom │ [G] Koord Git │ [V] Stil │ [P] Sekil │ [Shift+W] Web Harita "
        }
        ActiveView::AirTracks => {
            " [1-6] AOR Filtre │ [Ok/PgUpDn] Sec │ [Enter/O] Foto & Rota Dosya │ [F] Takip │ [B] Izle │ [/] Ara "
        }
        ActiveView::SeaTracks => {
            " [1-4] Gemi Filtre │ [Ok/PgUpDn] Sec │ [Enter] Gemi Dosyasi │ [F] Takip │ [B] Izle │ [/] Ara "
        }
        ActiveView::Sigint => {
            " [S] Ses │ [F] Frekans │ [U] Mod │ [L] Kilit │ [V] WAV │ [T] Tarama │ [X] VOX │ [C] CW │ [N] Sonraki "
        }
        ActiveView::Osint => {
            " [0-4] Haber Filtre │ [5] GDELT Catisma │ [EVI Bar Tikla] Ulke Filtre │ [/] Ara │ [Enter] Dosya "
        }
        ActiveView::Satint => " [Ok/PgUpDn] Uydu Sec │ [Enter] SGP4 Dosyasi │ [/] Uydu Ara │ [F2] Yer Izi Haritada ",
        ActiveView::Masint => {
            " [1] Deprem [2] Termal [3] CBRN [4] NOTAM [5] Alarmlar │ [/] Ara │ [Ok/Enter/Tikla] Odak & Dosya "
        }
        ActiveView::Cyberint => " [IP Yaz+Enter] Hibrit BGP/Port Tara │ [/] CVE/ASN Ara │ [Ok/Ctrl+O] CISA/IODA Detay ",
        ActiveView::Notepad => " [Tab] Arsiv/Editor │ [Ctrl+S] Kaydet │ [Ctrl+N] Yeni │ [Ctrl+H] HTML │ [/] Ara │ [F5] Zaman ",
        _ => " [F1-F11] Sekme │ [Ctrl+K] Komut Paleti │ [Shift+A] AI SITREP │ [Ctrl+E] Rapor │ [Q] Cikis ",
    };

    let cmd_lbl = " [Ctrl+K:Komut] ";
    let cw = cmd_lbl.chars().count() as u16;
    let help_lbl = " [?:Yardim] ";
    let hw = help_lbl.chars().count() as u16;
    let badge_lbl = " KISAYOL ";
    let bw = badge_lbl.chars().count() as u16;
    let right_w = cw + hw;
    let max_hint = area.width.saturating_sub(bw + right_w) as usize;
    let clipped_hint: String = ctx_hints.chars().take(max_hint).collect();
    let used = bw + clipped_hint.chars().count() as u16;

    let mut spans = vec![
        Span::styled(
            badge_lbl,
            Style::default()
                .fg(TELETEXT_BLACK)
                .bg(TELETEXT_YELLOW)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(clipped_hint, Style::default().fg(FG_PRIMARY).bg(BG_SURFACE)),
    ];

    if area.width >= bw + right_w {
        let pad = area.width.saturating_sub(used + right_w) as usize;
        if pad > 0 {
            spans.push(Span::styled(" ".repeat(pad), Style::default().bg(BG_SURFACE)));
        }
        let cx = area.x + area.width - right_w;
        spans.push(Span::styled(
            cmd_lbl,
            Style::default()
                .fg(TELETEXT_YELLOW)
                .bg(BG_PANEL)
                .add_modifier(Modifier::BOLD),
        ));
        state.hit_tester.register(
            Rect { x: cx, y: area.y, width: cw, height: 1 },
            HitTarget::CommandPaletteOpen,
        );

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

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_flash_toast(frame: &mut Frame, screen: Rect, state: &mut AppState) {
    let Some((flash, rem_secs)) = state.active_flash_banner() else {
        return;
    };
    if screen.width < 36 || screen.height < 8 {
        return;
    }

    let toast_w = screen.width.saturating_sub(4).clamp(34, 66);
    let toast_h = 3u16;
    let tx = screen.x + screen.width.saturating_sub(toast_w + 2);
    let ty = screen.y + screen.height.saturating_sub(toast_h + 2);
    let toast_rect = Rect {
        x: tx,
        y: ty,
        width: toast_w,
        height: toast_h,
    };

    let focus_lbl = "[ODAKLA]";
    let ack_lbl = format!("[{}s│X]", rem_secs);
    let focus_w = focus_lbl.chars().count() as u16;
    let ack_w = ack_lbl.chars().count() as u16;
    let inner_w = toast_w.saturating_sub(2);
    let msg_budget = inner_w.saturating_sub(focus_w + ack_w + 3) as usize;
    let msg_str: String = if flash.message.chars().count() > msg_budget && msg_budget >= 3 {
        let mut s: String = flash.message.chars().take(msg_budget - 2).collect();
        s.push_str("..");
        s
    } else {
        flash.message.chars().take(msg_budget).collect()
    };
    let msg_w = msg_str.chars().count() as u16;
    let pad_w = inner_w.saturating_sub(1 + msg_w + 1 + focus_w + 1 + ack_w) as usize;

    let toast_bg = Color::Rgb(42, 12, 16);
    let block = Block::default()
        .title(Span::styled(
            format!(" [! ACIL DURUM IKAZI - {}sn] ", rem_secs),
            Style::default()
                .fg(TELETEXT_WHITE)
                .bg(TELETEXT_RED)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(
            Style::default()
                .fg(TELETEXT_RED)
                .bg(toast_bg)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().bg(toast_bg));

    let inner = block.inner(toast_rect);
    frame.render_widget(Clear, toast_rect);
    frame.render_widget(block, toast_rect);

    let line = Line::from(vec![
        Span::styled(" ", Style::default().bg(toast_bg)),
        Span::styled(
            msg_str,
            Style::default()
                .fg(TELETEXT_WHITE)
                .bg(toast_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ".repeat(pad_w + 1), Style::default().bg(toast_bg)),
        Span::styled(
            focus_lbl,
            Style::default()
                .fg(TELETEXT_BLACK)
                .bg(TELETEXT_YELLOW)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ", Style::default().bg(toast_bg)),
        Span::styled(
            ack_lbl,
            Style::default()
                .fg(TELETEXT_WHITE)
                .bg(TELETEXT_RED)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(Paragraph::new(line), inner);

    let ack_x = inner.x + inner.width.saturating_sub(ack_w);
    state.hit_tester.register(
        Rect {
            x: toast_rect.x,
            y: toast_rect.y,
            width: toast_rect.width.saturating_sub(ack_w + 1),
            height: toast_rect.height,
        },
        HitTarget::FlashAlertFocus,
    );
    state.hit_tester.register(
        Rect {
            x: ack_x,
            y: toast_rect.y,
            width: ack_w + 1,
            height: toast_rect.height,
        },
        HitTarget::FlashAlertAck,
    );
}
