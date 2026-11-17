use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Table,
    },
    Frame,
};

pub fn render_sigint_view(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // Frekans Tuneri, Presetler & Ayri Durum Seridi
            Constraint::Min(13),    // Gercek KiwiSDR DFT Selalesi & Canli SDR Alici Secici
            Constraint::Length(11), // Canli EAM Akisi Tablosu
        ])
        .split(area);

    // 1. Frekans Tuneri & Preset Butonları (Saf ASCII)
    let mut preset_spans = Vec::new();
    let prefix_str = "FREKANS SECIMI: ";
    preset_spans.push(Span::styled(
        prefix_str,
        Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD),
    ));

    let mut preset_x = chunks[0].x + 1 + prefix_str.chars().count() as u16;
    for (idx, preset) in state.radio_presets.iter().enumerate() {
        let is_current = (state.current_frequency_khz - preset.frequency_khz).abs() < 0.1;
        let style = if is_current {
            Style::default()
                .fg(TELETEXT_BLACK)
                .bg(TELETEXT_GREEN)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(FG_PRIMARY)
                .bg(BG_SURFACE)
                .add_modifier(Modifier::BOLD)
        };
        let label = format!(" [{}:{:.0}k {}] ", idx + 1, preset.frequency_khz, preset.mode);
        let btn_len = label.chars().count() as u16;

        if preset_x + btn_len < chunks[0].x + chunks[0].width {
            let btn_rect = Rect {
                x: preset_x,
                y: chunks[0].y + 2,
                width: btn_len,
                height: 1,
            };
            state.hit_tester.register(btn_rect, HitTarget::SigintPreset(idx));
            preset_x += btn_len + 1;

            preset_spans.push(Span::styled(label, style));
            preset_spans.push(Span::raw(" "));
        }
    }

    let active_mode = state.sdr_mode.clone();

    let hdr_prefix = "TUNER: ";
    let freq_str = if state.sdr_freq_input_active {
        format!(" [F: {}_ kHz] ", state.sdr_freq_input)
    } else {
        format!(" [F:{:.1}kHz] ", state.current_frequency_khz)
    };
    let mode_btn_label = format!(" [U:{}] ", active_mode);
    let audio_btn_label = if state.is_radio_playing {
        " [S:ACIK] "
    } else {
        " [S:KAPALI] "
    };
    let failover_btn_label = if state.sdr_auto_failover {
        " [L:OTO-GECIS] "
    } else {
        " [L:KILITLI] "
    };
    let rec_btn_label = if state.sdr_recording {
        " [V:KAYITTA] "
    } else {
        " [V:WAV] "
    };
    let scan_btn_label = if state.sdr_scanner_enabled && state.sdr_scan_locked {
        " [T:KILITLI] "
    } else if state.sdr_scanner_enabled {
        " [T:TARIYOR] "
    } else {
        " [T:TARAMA] "
    };
    let vox_btn_label = if state.sdr_vox_enabled {
        " [X:VOX-ACIK] "
    } else {
        " [X:VOX] "
    };
    let cw_btn_label = if state.sdr_cw_enabled {
        " [C:CW-ACIK] "
    } else {
        " [C:CW] "
    };

    let mut cur_x = chunks[0].x + 1 + hdr_prefix.chars().count() as u16;
    let register_tuner_btn = |state: &mut AppState, x: &mut u16, label: &str, target: HitTarget| {
        let w = label.chars().count() as u16;
        if *x < chunks[0].x + chunks[0].width {
            let aw = (chunks[0].x + chunks[0].width - *x).min(w);
            state.hit_tester.register(
                Rect { x: *x, y: chunks[0].y + 1, width: aw, height: 1 },
                target,
            );
        }
        *x += w + 1;
    };

    register_tuner_btn(state, &mut cur_x, &freq_str, HitTarget::ToggleSdrFreqInput);
    register_tuner_btn(state, &mut cur_x, &mode_btn_label, HitTarget::CycleSdrMode);
    register_tuner_btn(state, &mut cur_x, audio_btn_label, HitTarget::ToggleRadioAudio);
    register_tuner_btn(state, &mut cur_x, failover_btn_label, HitTarget::ToggleSdrAutoFailover);
    register_tuner_btn(state, &mut cur_x, rec_btn_label, HitTarget::ToggleSdrRecord);
    cur_x += 4; // "  │  " ayirici genisligi (5 - 1)
    register_tuner_btn(state, &mut cur_x, scan_btn_label, HitTarget::ToggleSdrScanner);
    register_tuner_btn(state, &mut cur_x, vox_btn_label, HitTarget::ToggleSdrVox);
    register_tuner_btn(state, &mut cur_x, cw_btn_label, HitTarget::ToggleSdrCw);

    let audio_status = if state.is_radio_playing {
        Span::styled(
            audio_btn_label,
            Style::default()
                .fg(TELETEXT_BLACK)
                .bg(TELETEXT_GREEN)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            audio_btn_label,
            Style::default()
                .fg(TELETEXT_YELLOW)
                .bg(BG_HIGHLIGHT)
                .add_modifier(Modifier::BOLD),
        )
    };

    let failover_status = if state.sdr_auto_failover {
        Span::styled(
            failover_btn_label,
            Style::default()
                .fg(TELETEXT_CYAN)
                .bg(BG_SURFACE)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            failover_btn_label,
            Style::default()
                .fg(TELETEXT_BLACK)
                .bg(TELETEXT_YELLOW)
                .add_modifier(Modifier::BOLD),
        )
    };

    let rec_status = if state.sdr_recording {
        Span::styled(
            rec_btn_label,
            Style::default()
                .fg(TELETEXT_BLACK)
                .bg(TELETEXT_RED)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            rec_btn_label,
            Style::default()
                .fg(FG_PRIMARY)
                .bg(BG_SURFACE)
                .add_modifier(Modifier::BOLD),
        )
    };

    let toggle_span = |lbl: &'static str, active: bool, active_bg: Color| {
        if active {
            Span::styled(
                lbl,
                Style::default()
                    .fg(TELETEXT_BLACK)
                    .bg(active_bg)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                lbl,
                Style::default()
                    .fg(FG_SECONDARY)
                    .bg(BG_SURFACE)
                    .add_modifier(Modifier::BOLD),
            )
        }
    };

    let scan_status = toggle_span(
        scan_btn_label,
        state.sdr_scanner_enabled,
        if state.sdr_scan_locked { TELETEXT_RED } else { TELETEXT_GREEN },
    );
    let vox_status = toggle_span(vox_btn_label, state.sdr_vox_enabled, TELETEXT_GREEN);
    let cw_status = toggle_span(cw_btn_label, state.sdr_cw_enabled, TELETEXT_CYAN);

    let extra_status = if !state.sdr_rec_path.is_empty() {
        format!("DURUM: {} [{}]", state.sdr_status, state.sdr_rec_path)
    } else {
        format!("DURUM: {}", state.sdr_status)
    };

    let tuner_p = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                hdr_prefix,
                Style::default().fg(FG_PRIMARY).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                freq_str,
                Style::default()
                    .fg(if state.sdr_freq_input_active { TELETEXT_BLACK } else { TELETEXT_CYAN })
                    .bg(if state.sdr_freq_input_active { TELETEXT_YELLOW } else { BG_SURFACE })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                mode_btn_label,
                Style::default().fg(TELETEXT_CYAN).bg(BG_SURFACE).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            audio_status,
            Span::raw(" "),
            failover_status,
            Span::raw(" "),
            rec_status,
            Span::raw("  │  "),
            scan_status,
            Span::raw(" "),
            vox_status,
            Span::raw(" "),
            cw_status,
        ]),
        Line::from(preset_spans),
        Line::from(Span::styled(extra_status, Style::default().fg(FG_SECONDARY))),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(" [GERCEK ZAMANLI KIWISDR WEBSOCKET (/SND) KISA DALGA ALICISI] ")
            .style(Style::default().bg(BG_PANEL).fg(FG_PRIMARY)),
    );
    frame.render_widget(tuner_p, chunks[0]);

    // 2. Orta Panel: Sol = Gercek DFT Spektrum Selalesi, Sag = Canli KiwiSDR Sunucu Listesi & S-Meter
    let mid_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(chunks[1]);

    let mut waterfall_lines = Vec::new();
    waterfall_lines.push(Line::from(vec![Span::styled(
        format!(
            " 300Hz        1000Hz      {:.1} kHz ({})      2200Hz       3300Hz",
            state.current_frequency_khz, active_mode
        ),
        Style::default().fg(TELETEXT_CYAN),
    )]));

    if state.waterfall_data.is_empty() {
        waterfall_lines.push(Line::from(Span::styled(
            " [!] Canli DFT spektrumu icin 'S' tusuna basarak KiwiSDR baglantisini acin.",
            Style::default().fg(FG_MUTED),
        )));
    } else {
        for row in state.waterfall_data.iter().rev().take(9) {
            let mut row_spans = Vec::new();
            for val in row.iter() {
                let (ch, col) = if *val > 180 {
                    ("#", TELETEXT_RED)
                } else if *val > 130 {
                    ("=", TELETEXT_YELLOW)
                } else if *val > 80 {
                    ("+", TELETEXT_GREEN)
                } else if *val > 40 {
                    (":", TELETEXT_CYAN)
                } else {
                    (".", FG_MUTED)
                };
                row_spans.push(Span::styled(ch, Style::default().fg(col)));
            }
            waterfall_lines.push(Line::from(row_spans));
        }
    }

    if state.sdr_cw_enabled {
        let cw_text = if state.sdr_cw_text.is_empty() {
            "(600Hz ton bekleniyor...)".to_string()
        } else {
            state.sdr_cw_text.clone()
        };
        waterfall_lines.push(Line::from(vec![
            Span::styled(
                format!(" CW MORZE ({} WPM): ", state.sdr_cw_wpm),
                Style::default().fg(TELETEXT_BLACK).bg(TELETEXT_CYAN).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(cw_text, Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)),
        ]));
    }

    if state.sdr_scanner_enabled || !state.sdr_scan_hits.is_empty() {
        waterfall_lines.push(Line::from(Span::styled(
            format!(
                " --- TARAMA YAKALAMALARI ({}) | Esik: >=11.0dB SNR ---",
                state.sdr_scan_hits.len()
            ),
            Style::default().fg(TELETEXT_GREEN).add_modifier(Modifier::BOLD),
        )));
        for hit in state.sdr_scan_hits.iter().rev().take(4) {
            let rec_tag = if hit.recorded_file.is_some() { " [WAV]" } else { "" };
            waterfall_lines.push(Line::from(vec![
                Span::styled(
                    format!(" {} ", hit.timestamp.format("%H:%M:%S")),
                    Style::default().fg(FG_MUTED),
                ),
                Span::styled(
                    format!("{:.1} kHz {} ", hit.frequency_khz, hit.mode),
                    Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("+{:.1}dB ({:.0}dBm) @ {}{}", hit.snr_db, hit.rssi_dbm, hit.host_port, rec_tag),
                    Style::default().fg(TELETEXT_CYAN),
                ),
            ]));
        }
    }

    let waterfall_p = Paragraph::new(waterfall_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(format!(
                " [CANLI PCM DFT SPEKTRUMU | S-METER: {:.1} dBm (+{:.1}dB SNR) | {} Hz] ",
                state.sdr_rssi_dbm, state.sdr_snr_db, state.sdr_sample_rate
            ))
            .style(Style::default().bg(BG_PRIMARY).fg(FG_PRIMARY)),
    );
    frame.render_widget(waterfall_p, mid_chunks[0]);

    // Sag: Canli KiwiSDR Sunucu Secici (rx.linkfanel.net uzerinden cekilen gercek alicilar)
    // Kaydirilabilir tablo: Yukari/Asagi/PgUp/PgDn/Home/End ile gezin, Enter/Tik ile baglan, Tab ile odak degistir
    let list_area = mid_chunks[1];
    let q_lower = state.search_query.to_lowercase();
    let visible_nodes: Vec<(usize, _)> = state
        .sdr_nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| {
            q_lower.is_empty()
                || n.host_port.to_lowercase().contains(&q_lower)
                || n.location.to_lowercase().contains(&q_lower)
                || n.name.to_lowercase().contains(&q_lower)
        })
        .collect();
    let total_nodes = visible_nodes.len();
    if total_nodes > 0 && state.sdr_scroll >= total_nodes {
        state.sdr_scroll = total_nodes - 1;
    }
    let focused = state.sigint_focus == 0;

    // Ic alan: kenarliklar (2) + baslik satiri (1)
    let visible = (list_area.height.saturating_sub(3) as usize).max(1);
    let cursor = state.sdr_scroll;
    let offset = if cursor >= visible { cursor + 1 - visible } else { 0 };

    state.hit_tester.register(list_area, HitTarget::SdrNodeList);

    let node_header = Row::new(
        ["#", "ALICI (HOST:PORT)", "KONUM", "SNR", "KANAL"]
            .into_iter()
            .map(|h| Cell::from(h).style(Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD))),
    )
    .style(Style::default().bg(BG_SURFACE));

    let mut node_rows = Vec::new();
    if total_nodes == 0 {
        node_rows.push(Row::new(vec![
            Cell::from(""),
            Cell::from("rx.linkfanel.net alici dizini sorgulaniyor veya filtre bos..."),
        ]).style(Style::default().fg(FG_MUTED)));
    } else {
        for (row_off, (idx, node)) in visible_nodes
            .iter()
            .copied()
            .skip(offset)
            .take(visible)
            .enumerate()
        {
            let is_active = state.active_sdr_idx == idx;
            let is_cursor = cursor == offset + row_off;
            let marker = match (is_active, is_cursor) {
                (true, true) => ">*",
                (true, false) => " *",
                (false, true) => "> ",
                _ => "  ",
            };
            let free = node.users_max.saturating_sub(node.users);
            let snr_col = if node.snr >= 25 {
                TELETEXT_GREEN
            } else if node.snr >= 15 {
                TELETEXT_YELLOW
            } else {
                TELETEXT_RED
            };
            let style = if is_cursor && focused {
                Style::default().fg(TELETEXT_BLACK).bg(TELETEXT_CYAN).add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default().fg(TELETEXT_GREEN).bg(BG_HIGHLIGHT).add_modifier(Modifier::BOLD)
            } else if is_cursor {
                Style::default().fg(TELETEXT_YELLOW).bg(BG_HIGHLIGHT)
            } else {
                Style::default().fg(FG_PRIMARY)
            };
            node_rows.push(
                Row::new(vec![
                    Cell::from(format!("{}{:>3}", marker, idx + 1)),
                    Cell::from(node.host_port.clone()),
                    Cell::from(node.location.clone()),
                    Cell::from(format!("{}dB", node.snr)).style(Style::default().fg(snr_col)),
                    Cell::from(format!("{}/{} ({} bos)", node.users, node.users_max, free)),
                ])
                .style(style),
            );

            let ry = list_area.y + 2 + row_off as u16;
            if ry < list_area.y + list_area.height.saturating_sub(1) {
                state.hit_tester.register(
                    Rect {
                        x: list_area.x + 1,
                        y: ry,
                        width: list_area.width.saturating_sub(2),
                        height: 1,
                    },
                    HitTarget::SelectSdrNode(idx),
                );
            }
        }
    }

    let node_widths = [
        Constraint::Length(6),
        Constraint::Percentage(36),
        Constraint::Percentage(34),
        Constraint::Length(5),
        Constraint::Min(10),
    ];
    let focus_tag = if focused { "ODAK" } else { "Tab: Odakla" };
    let nodes_table = Table::new(node_rows, node_widths).header(node_header).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if focused { BORDER_ACTIVE } else { BORDER_DIM }))
            .title(format!(
                " [KIWISDR ALICILARI {}/{} | {} | Ok/PgUp/PgDn: Gez  Enter/Tik: Baglan  N: Sonraki] ",
                if total_nodes == 0 { 0 } else { cursor + 1 },
                total_nodes,
                focus_tag
            ))
            .style(Style::default().bg(BG_PANEL).fg(FG_PRIMARY)),
    );
    frame.render_widget(nodes_table, list_area);

    if total_nodes > visible {
        let mut sb_state = ScrollbarState::new(total_nodes).position(cursor);
        let sb = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(TELETEXT_CYAN).bg(BG_SURFACE));
        frame.render_stateful_widget(sb, list_area, &mut sb_state);
    }

    // 3. Canlı EAM Akışı Tablosu
    let header_cells = [
        "ZAMAN (UTC)",
        "FREKANS",
        "ISTASYON",
        "PREAMBLE",
        "BOYUT",
        "MESAJ KOD BLOKLARI",
        "DURUM",
    ]
    .into_iter()
    .map(|h| Cell::from(h).style(Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).style(Style::default().bg(BG_SURFACE));

    let mut rows = Vec::new();
    let mut eams: Vec<_> = state.eam_messages.iter().collect();
    eams.reverse();
    let visible_rows = chunks[2].height.saturating_sub(3) as usize;
    let max_scroll = eams.len().saturating_sub(1);
    if state.eam_scroll > max_scroll {
        state.eam_scroll = max_scroll;
    }
    let start_idx = state.eam_scroll;

    for (row_offset, msg) in eams.iter().skip(start_idx).take(visible_rows).enumerate() {
        let actual_idx = start_idx + row_offset;
        let is_flash = msg.is_flash_override;
        let row_style = if row_offset == 0 {
            Style::default()
                .fg(TELETEXT_YELLOW)
                .bg(BG_HIGHLIGHT)
                .add_modifier(Modifier::BOLD)
        } else if is_flash {
            Style::default().fg(TELETEXT_RED).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(TELETEXT_CYAN)
        };

        let status_tag = if is_flash { "FLASH / SKYKING" } else { "STANDART EAM" };
        let time_prefix = if row_offset == 0 {
            format!("> {}", msg.timestamp.format("%H:%M:%S"))
        } else {
            format!("  {}", msg.timestamp.format("%H:%M:%S"))
        };

        let cells = vec![
            Cell::from(time_prefix),
            Cell::from(format!("{:.1} kHz", msg.frequency_khz)),
            Cell::from(msg.auth_station.clone()),
            Cell::from(msg.preamble.clone()),
            Cell::from(format!("{}B", msg.length_chars)),
            Cell::from(msg.content.clone()),
            Cell::from(status_tag),
        ];

        rows.push(Row::new(cells).style(row_style));

        let row_area = Rect {
            x: chunks[2].x + 1,
            y: chunks[2].y + 2 + row_offset as u16,
            width: chunks[2].width.saturating_sub(2),
            height: 1,
        };
        state.hit_tester.register(row_area, HitTarget::TableRow(30, actual_idx));
    }

    let widths = [
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(16),
        Constraint::Length(10),
        Constraint::Length(7),
        Constraint::Min(30),
        Constraint::Length(16),
    ];

    let table = Table::new(rows, widths).header(header).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(" [EAM.WATCH DOGRULANMIS HFGCS MESAJ AKISI] ")
            .style(Style::default().bg(BG_PRIMARY).fg(FG_PRIMARY)),
    );
    frame.render_widget(table, chunks[2]);
}
