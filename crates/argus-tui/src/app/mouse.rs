use super::App;
use crate::audio::SoundEffect;
use crate::widgets::command_palette::build_palette_entries;
use argus_core::mouse::HitTarget;
use argus_core::types::{ActiveView, TacticalRuler};
use chrono::Utc;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use std::time::{Duration, Instant};

impl App {
    pub(super) async fn handle_mouse(&mut self, mouse: MouseEvent) {
        let col = mouse.column;
        let row = mouse.row;

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let mut st = self.state.write();
                let target = st.hit_tester.hit_test(col, row);

                if let Some(t) = target {
                    let now = Instant::now();
                    let is_double_click = if let Some(last_t) = &self.last_click_target {
                        *last_t == t && now.duration_since(self.last_click_time) < Duration::from_millis(500)
                    } else {
                        false
                    };

                    self.last_click_time = now;
                    self.last_click_target = Some(t);

                    match t {
                        HitTarget::TaskbarViewTab(view) => {
                            st.set_view(view);
                            if view == ActiveView::AirTracks {
                                st.sync_air_selection();
                            } else if view == ActiveView::Notepad {
                                st.refresh_notepad_files();
                            }
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::TrayAudio => {
                            st.audio_muted = !st.audio_muted;
                            self.audio.set_muted(st.audio_muted);
                        }
                        HitTarget::TrayClock => {
                            let now_str = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
                            let lat = st.station_lat;
                            let lon = st.station_lon;
                            let active_title = st.active_view.title();
                            st.open_dialog("Zaman ve Konum Bilgisi", vec![
                                format!("ZULU (UTC)       : {}", now_str),
                                "Yerel İstasyon   : Sentinel Istanbul Node (TR)".to_string(),
                                format!("Koordinatlar     : {:.4}° N, {:.4}° E", lat, lon),
                                format!("Aktif Çalışma    : {}", active_title),
                            ]);
                            self.audio.play(SoundEffect::Blip);
                        }
                        HitTarget::DialogClose => {
                            st.ai_prompt_input_active = false;
                            st.ai_followup_input_active = false;
                            st.close_dialog();
                        }
                        HitTarget::DialogSaveFile => {
                            if let Some(path) = st.save_dialog_to_markdown() {
                                st.dialog_content.push(format!("[*] DOSYAYA KAYDEDILDI: {}", path));
                            }
                            self.audio.play(SoundEffect::Blip);
                        }
                        HitTarget::DialogExportHtml => {
                            if let Some(path) = st.export_html_briefing_deck() {
                                st.dialog_content.push(format!("[*] HTML BRIFING KAYDEDILDI: {}", path));
                            }
                            self.audio.play(SoundEffect::Blip);
                        }
                        HitTarget::DialogExportNote => {
                            Self::append_dialog_to_notepad(&mut st);
                            self.audio.play(SoundEffect::Blip);
                        }
                        HitTarget::AiMode(mid) => {
                            if mid == 3 {
                                st.ai_mode = 3;
                                st.ai_followup_input_active = false;
                                st.ai_prompt_input_active = true;
                                self.audio.play(SoundEffect::Click);
                            } else {
                                st.ai_mode = mid;
                                st.ai_prompt_input_active = false;
                                st.ai_followup_input_active = false;
                                Self::spawn_ai_sitrep(&self.state, &mut st, true);
                                self.audio.play(SoundEffect::Blip);
                            }
                        }
                        HitTarget::AiRefresh => {
                            Self::spawn_ai_sitrep(&self.state, &mut st, true);
                            self.audio.play(SoundEffect::Blip);
                        }
                        HitTarget::AiFollowUpToggle => {
                            st.ai_prompt_input_active = false;
                            st.ai_followup_input_active = !st.ai_followup_input_active;
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::NotepadFile(idx) => {
                            st.select_notepad_file(idx);
                            st.notepad.focus_sidebar = true;
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::NotepadAction(act) => {
                            Self::run_notepad_action(&mut st, act);
                            self.audio.play(SoundEffect::Blip);
                        }
                        HitTarget::AirFilter(idx) => {
                            Self::apply_air_filter(&mut st, idx);
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::SeaFilter(idx) => {
                            st.sea_filter = idx;
                            st.sea_scroll = 0;
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::OsintCategory(cat) => {
                            Self::apply_osint_category(&mut st, cat);
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::OsintEviCountry(evi_idx) => {
                            if let Some(m) = st.escalation_metrics.get(evi_idx).cloned() {
                                st.search_query = m.country_name.clone();
                                Self::apply_osint_category(&mut st, 5);
                                st.live_status = format!(
                                    "EVI FILTRE: {} (Skor:{} │ {:+}% │ {} olay)",
                                    m.country_name, m.evi_score, m.delta_pct, m.event_count
                                );
                                self.audio.play(SoundEffect::Blip);
                            }
                        }
                        HitTarget::CalcKey(ch) => {
                            if ch == '\t' {
                                st.calculator.conv_mode = (st.calculator.conv_mode + 1) % 4;
                                st.calc_convert();
                            } else if ch == '!' {
                                st.calculator.conv_mode = 0;
                                st.calc_convert();
                            } else if ch == '@' {
                                st.calculator.conv_mode = 1;
                                st.calc_convert();
                            } else if ch == '#' {
                                st.calculator.conv_mode = 2;
                                st.calc_convert();
                            } else if ch == '$' {
                                st.calculator.conv_mode = 3;
                                st.calc_convert();
                            } else {
                                st.calc_input_char(ch);
                            }
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::GeointLayer(idx) => {
                            st.geoint_layers ^= 1 << idx;
                            let _ = st.save_session();
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::SigintPreset(idx) => {
                            if let Some((freq, mode)) = st.radio_presets.get(idx).map(|p| (p.frequency_khz, p.mode.clone())) {
                                st.current_frequency_khz = freq;
                                st.sdr_mode = mode;
                                Self::sync_radio_state(&self.audio, &st);
                                self.audio.play(SoundEffect::Click);
                            }
                        }
                        HitTarget::ToggleSdrFreqInput => {
                            st.sdr_freq_input_active = !st.sdr_freq_input_active;
                            st.sdr_freq_input.clear();
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::CycleSdrMode => {
                            st.sdr_mode = match st.sdr_mode.as_str() {
                                "USB" => "LSB",
                                "LSB" => "AM",
                                "AM" => "CW",
                                _ => "USB",
                            }
                            .to_string();
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::ToggleSdrRecord => {
                            st.sdr_recording = !st.sdr_recording;
                            self.audio.play(SoundEffect::Blip);
                        }
                        HitTarget::ToggleSdrScanner => {
                            Self::toggle_sdr_scanner(&mut st);
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::ToggleSdrVox => {
                            Self::toggle_sdr_vox(&mut st);
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::ToggleSdrCw => {
                            Self::toggle_sdr_cw(&mut st);
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::MapSdrNode(idx) => {
                            if let Some(node) = st.sdr_nodes.get(idx).cloned() {
                                st.active_sdr_idx = idx;
                                st.sdr_scroll = idx;
                                st.is_radio_playing = true;
                                Self::sync_radio_state(&self.audio, &st);
                                st.live_status = format!(
                                    "HARITA SDR SECILDI: {} ({} | SNR {}dB)",
                                    node.host_port, node.location, node.snr
                                );
                                self.audio.play(SoundEffect::Blip);
                            }
                        }
                        HitTarget::MasintTab(tid) => {
                            Self::apply_masint_tab(&mut st, tid);
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::ShodanPreset(idx) => {
                            st.shodan_input_ip = match idx {
                                0 => "1.1.1.1",
                                1 => "8.8.8.8",
                                _ => "9.9.9.9",
                            }
                            .to_string();
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::ToggleRadioAudio => {
                            st.is_radio_playing = !st.is_radio_playing;
                            Self::sync_radio_state(&self.audio, &st);
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::MapStyleToggle => {
                            st.map_style = st.map_style.next();
                            st.live_status = format!("HARITA MODU: {}", st.map_style.label());
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::ToggleMapAircraftShapes => {
                            Self::toggle_map_aircraft_shapes(&mut st);
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::DefconBadge => {
                            Self::open_defcon_dialog(&mut st);
                            self.audio.play(SoundEffect::Blip);
                        }
                        HitTarget::TriggerAiAnalysis => {
                            Self::spawn_ai_sitrep(&self.state, &mut st, false);
                            self.audio.play(SoundEffect::Blip);
                        }
                        HitTarget::MapTarget(tile_idx, row_idx) => {
                            Self::select_item_for_inspector(&mut st, tile_idx, row_idx);
                            if is_double_click && Self::open_item_dossier(&mut st, tile_idx, row_idx) {
                                self.audio.play(SoundEffect::Blip);
                            } else {
                                self.audio.play(SoundEffect::Click);
                            }
                        }
                        HitTarget::SelectSdrNode(idx) => {
                            if idx < st.sdr_nodes.len() {
                                st.sigint_focus = 0;
                                st.sdr_scroll = idx;
                                st.active_sdr_idx = idx;
                                if is_double_click && !st.is_radio_playing {
                                    st.is_radio_playing = true;
                                    Self::sync_radio_state(&self.audio, &st);
                                }
                                self.audio.play(SoundEffect::Click);
                            }
                        }
                        HitTarget::SdrNodeList => {
                            st.sigint_focus = 0;
                        }
                        HitTarget::ToggleSdrAutoFailover => {
                            st.sdr_auto_failover = !st.sdr_auto_failover;
                            st.sdr_status = if st.sdr_auto_failover {
                                "OTO-GECIS ACIK: Yanit vermeyen alicidan siradakine gecilir".to_string()
                            } else {
                                "ALICI KILITLI: Secili alicida uzun sureli baglanti denenecek".to_string()
                            };
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::InspectorAction(act_id) => {
                            match act_id {
                                0 => Self::focus_selected_on_map(&mut st),
                                1 => Self::toggle_selected_follow(&mut st),
                                2 => Self::toggle_selected_watchlist(&mut st),
                                3 => {
                                    let _ = st.export_intelligence_report();
                                }
                                _ => {}
                            }
                            self.audio.play(SoundEffect::Blip);
                        }
                        HitTarget::RegionPreset(p_idx) => {
                            Self::apply_region_preset(&mut st, p_idx);
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::OpenWebMap => {
                            Self::launch_web_map(&mut st);
                            self.audio.play(SoundEffect::Blip);
                        }
                        HitTarget::ExportReport => {
                            let _ = st.export_intelligence_report();
                            self.audio.play(SoundEffect::Blip);
                        }
                        HitTarget::SortColumn(tile_idx, col) => {
                            if tile_idx == 20 {
                                if st.air_sort_col == col {
                                    st.air_sort_asc = !st.air_sort_asc;
                                } else {
                                    st.air_sort_col = col;
                                    st.air_sort_asc = false;
                                }
                                st.air_scroll = 0;
                            } else if tile_idx == 21 {
                                if st.sea_sort_col == col {
                                    st.sea_sort_asc = !st.sea_sort_asc;
                                } else {
                                    st.sea_sort_col = col;
                                    st.sea_sort_asc = false;
                                }
                                st.sea_scroll = 0;
                            }
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::SearchToggle => {
                            st.is_search_active = !st.is_search_active;
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::SearchClear => {
                            st.search_query.clear();
                            st.is_search_active = false;
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::HelpButton => {
                            Self::open_help_dialog(&mut st);
                            self.audio.play(SoundEffect::Blip);
                        }
                        HitTarget::FeedHealthModal => {
                            Self::open_feed_health_dialog(&mut st);
                            self.audio.play(SoundEffect::Blip);
                        }
                        HitTarget::CommandPaletteOpen => {
                            st.command_palette.is_open = !st.command_palette.is_open;
                            st.command_palette.query.clear();
                            st.command_palette.selected_idx = 0;
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::CommandPaletteItem(idx) => {
                            let entries = build_palette_entries(&st);
                            if let Some(entry) = entries.into_iter().nth(idx) {
                                self.execute_palette_command(&mut st, entry.command);
                                self.audio.play(SoundEffect::Blip);
                            }
                        }
                        HitTarget::GeoGoToInput => {
                            st.geoint_goto_active = true;
                            st.geoint_goto_input.clear();
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::TableRow(tile_idx, row_idx) => {
                            if tile_idx < 6 {
                                st.active_tile = tile_idx;
                            }
                            Self::select_item_for_inspector(&mut st, tile_idx, row_idx);
                            if is_double_click && Self::open_item_dossier(&mut st, tile_idx, row_idx) {
                                self.audio.play(SoundEffect::Blip);
                            } else {
                                self.audio.play(SoundEffect::Click);
                            }
                        }
                        HitTarget::AlertItem(alert_idx) => {
                            if st.active_view == ActiveView::Dashboard {
                                st.active_tile = 6;
                            }
                            let a_len = st.alerts.len();
                            if alert_idx < a_len {
                                st.alert_scroll = a_len.saturating_sub(1 + alert_idx);
                            }
                            if let Some(sfx) = Self::handle_alert_click(&mut st, alert_idx, is_double_click) {
                                self.audio.play(sfx);
                            }
                        }
                        HitTarget::FusionAlert(fc_idx) => {
                            if st.active_view == ActiveView::Dashboard {
                                st.active_tile = 6;
                            }
                            st.alert_scroll = st.alerts.len() + fc_idx;
                            if Self::focus_fusion_cluster(&mut st, fc_idx, is_double_click) {
                                self.audio.play(SoundEffect::Blip);
                            } else {
                                self.audio.play(SoundEffect::Click);
                            }
                        }
                        HitTarget::StraitGate(idx) => {
                            if let Some(g) = st.strait_gates.get(idx).cloned() {
                                st.follow_target_id = None;
                                st.map_center_lat = g.center_lat;
                                st.map_center_lon = g.center_lon;
                                st.map_zoom = 5.5;
                                st.live_status = format!(
                                    "BOGAZ ODAK: {} (K->G:{} G->K:{})",
                                    g.name, g.fwd_count, g.rev_count
                                );
                                self.audio.play(SoundEffect::Click);
                            }
                        }
                        HitTarget::FlashAlertFocus => {
                            if let Some(f) = st.active_flash_alert.clone() {
                                if let Some(cur) = &mut st.active_flash_alert {
                                    cur.acked = true;
                                }
                                st.follow_target_id = None;
                                st.map_center_lat = f.lat;
                                st.map_center_lon = f.lon;
                                st.map_zoom = st.map_zoom.max(4.5);
                                if let Some(tid) = f.target_id {
                                    st.selected_target_id = Some(tid);
                                }
                                st.set_view(ActiveView::Dashboard);
                                self.audio.play(SoundEffect::Blip);
                            }
                        }
                        HitTarget::FlashAlertAck => {
                            if let Some(f) = &mut st.active_flash_alert {
                                f.acked = true;
                            }
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::DvrStep(step) => {
                            st.dvr.step_offset(step);
                            st.live_status = if st.dvr.replay_offset_mins == 0 && st.dvr.active_archive.is_none() {
                                "DVR: CANLI AKIS".to_string()
                            } else {
                                format!("DVR REPLAY: -{} dakika", st.dvr.replay_offset_mins)
                            };
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::DvrPlayToggle => {
                            st.dvr.toggle_play();
                            st.live_status = if st.dvr.is_playing {
                                format!("DVR OYNATILIYOR ({}x │ -{}dk)", st.dvr.play_speed, st.dvr.replay_offset_mins)
                            } else {
                                "DVR DURAKLATILDI".to_string()
                            };
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::DvrSpeedCycle => {
                            st.dvr.cycle_speed();
                            st.live_status = format!("DVR OYNATMA HIZI: {}x", st.dvr.play_speed);
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::DvrCycleArchive => {
                            if let Some(arch) = st.dvr.cycle_archive_in(std::path::Path::new("recordings")) {
                                st.live_status = format!("DVR ARSIV: {} ({} kare)", arch, st.dvr.snapshots.len());
                            } else {
                                st.live_status = "DVR: CANLI AKIS MODUNA DONULDU".to_string();
                            }
                            self.audio.play(SoundEffect::Blip);
                        }
                        HitTarget::DvrTrackHistoryToggle => {
                            st.dvr.show_track_history = !st.dvr.show_track_history;
                            st.live_status = if st.dvr.show_track_history {
                                "DVR GUNLUK HEDEF ROTA IZI ACIK".to_string()
                            } else {
                                "DVR GUNLUK HEDEF ROTA IZI KAPALI".to_string()
                            };
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::RulerSetPinA => {
                            let pin_a = Self::resolve_ruler_pin(&mut st);
                            let lbl_a = pin_a.2.clone();
                            if let Some(ruler) = &mut st.tactical_ruler {
                                ruler.pin_a = pin_a;
                            } else {
                                st.tactical_ruler = Some(TacticalRuler {
                                    pin_a,
                                    pin_b: None,
                                    threat_ring_km: None,
                                });
                            }
                            st.live_status = format!("CETVEL PIN-A: {}", lbl_a);
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::RulerSetPinB => {
                            let pin_b = Self::resolve_ruler_pin(&mut st);
                            let lbl_b = pin_b.2.clone();
                            let st_lat = st.station_lat;
                            let st_lon = st.station_lon;
                            if let Some(ruler) = &mut st.tactical_ruler {
                                ruler.pin_b = Some(pin_b);
                            } else {
                                st.tactical_ruler = Some(TacticalRuler {
                                    pin_a: (st_lat, st_lon, "ISTASYON".to_string()),
                                    pin_b: Some(pin_b),
                                    threat_ring_km: None,
                                });
                            }
                            st.live_status = format!("CETVEL PIN-B: {}", lbl_b);
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::RulerCycleRing => {
                            let st_lat = st.station_lat;
                            let st_lon = st.station_lon;
                            let (next_ring, pin_lbl) = {
                                let ruler = st.tactical_ruler.get_or_insert_with(|| TacticalRuler {
                                    pin_a: (st_lat, st_lon, "ISTASYON".to_string()),
                                    pin_b: None,
                                    threat_ring_km: None,
                                });
                                ruler.threat_ring_km = match ruler.threat_ring_km {
                                    None => Some(45.0),
                                    Some(r) if r < 100.0 => Some(160.0),
                                    Some(r) if r < 300.0 => Some(380.0),
                                    _ => None,
                                };
                                (ruler.threat_ring_km, ruler.pin_a.2.clone())
                            };
                            st.live_status = match next_ring {
                                Some(r) => format!("TEHDIT HALKASI: {:.0} km ({})", r, pin_lbl),
                                None => "TEHDIT HALKASI KAPALI".to_string(),
                            };
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::RulerClear => {
                            st.tactical_ruler = None;
                            st.live_status = "TAKTIK CETVEL TEMIZLENDI".to_string();
                            self.audio.play(SoundEffect::Click);
                        }
                        _ => {}
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                let mut st = self.state.write();
                let (dx, dy) = st.hit_tester.on_drag_update(col, row);
                // Haritada fareyle sürükleyince (Pan)
                if st.active_view == ActiveView::Geoint || st.active_tile == 0 || st.zoomed_tile == Some(0) {
                    st.map_center_lon -= (dx as f64) * 0.4 / st.map_zoom;
                    st.map_center_lat = (st.map_center_lat + (dy as f64) * 0.2 / st.map_zoom).clamp(-85.0, 85.0);
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                let mut st = self.state.write();
                st.hit_tester.on_drag_end();
            }
            MouseEventKind::ScrollUp => {
                let mut st = self.state.write();
                if st.is_dialog_open {
                    st.dialog_scroll = st.dialog_scroll.saturating_sub(2);
                } else if st.active_view == ActiveView::AirTracks {
                    st.air_scroll = st.air_scroll.saturating_sub(2);
                } else if st.active_view == ActiveView::SeaTracks {
                    st.sea_scroll = st.sea_scroll.saturating_sub(2);
                } else if st.active_view == ActiveView::Masint {
                    if st.masint_tab == 4 {
                        st.alert_scroll = st.alert_scroll.saturating_sub(2);
                    } else {
                        st.radiation_scroll = st.radiation_scroll.saturating_sub(2);
                        st.geo_scroll = st.geo_scroll.saturating_sub(2);
                    }
                } else if st.active_view == ActiveView::Sigint {
                    if matches!(st.hit_tester.hit_test(col, row), Some(HitTarget::SdrNodeList) | Some(HitTarget::SelectSdrNode(_))) {
                        st.sdr_scroll = st.sdr_scroll.saturating_sub(2);
                    } else {
                        st.eam_scroll = st.eam_scroll.saturating_sub(2);
                    }
                } else if st.active_view == ActiveView::Osint {
                    st.news_scroll = st.news_scroll.saturating_sub(2);
                } else if st.active_view == ActiveView::Satint {
                    st.sat_scroll = st.sat_scroll.saturating_sub(2);
                } else if st.active_view == ActiveView::Cyberint {
                    st.cyber_scroll = st.cyber_scroll.saturating_sub(2);
                } else if st.active_view == ActiveView::Notepad {
                    st.notepad.scroll = st.notepad.scroll.saturating_sub(2);
                } else if st.active_view == ActiveView::Geoint || st.active_tile == 0 || st.zoomed_tile == Some(0) {
                    st.map_zoom = (st.map_zoom * 1.25).min(50.0);
                } else {
                    match st.active_tile {
                        1 => st.news_scroll = st.news_scroll.saturating_sub(2),
                        2 => st.target_scroll = st.target_scroll.saturating_sub(2),
                        3 => st.sat_scroll = st.sat_scroll.saturating_sub(2),
                        4 => st.cyber_scroll = st.cyber_scroll.saturating_sub(2),
                        5 => st.geo_scroll = st.geo_scroll.saturating_sub(2),
                        6 => st.alert_scroll = st.alert_scroll.saturating_sub(2),
                        _ => {}
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                let mut st = self.state.write();
                if st.is_dialog_open {
                    st.dialog_scroll = st.dialog_scroll.saturating_add(2);
                } else if st.active_view == ActiveView::AirTracks {
                    st.air_scroll = st.air_scroll.saturating_add(2);
                } else if st.active_view == ActiveView::SeaTracks {
                    st.sea_scroll = st.sea_scroll.saturating_add(2);
                } else if st.active_view == ActiveView::Masint {
                    if st.masint_tab == 4 {
                        st.alert_scroll = st.alert_scroll.saturating_add(2);
                    } else {
                        st.radiation_scroll = st.radiation_scroll.saturating_add(2);
                        st.geo_scroll = st.geo_scroll.saturating_add(2);
                    }
                } else if st.active_view == ActiveView::Sigint {
                    if matches!(st.hit_tester.hit_test(col, row), Some(HitTarget::SdrNodeList) | Some(HitTarget::SelectSdrNode(_))) {
                        let max = st.sdr_nodes.len().saturating_sub(1);
                        st.sdr_scroll = (st.sdr_scroll + 2).min(max);
                    } else {
                        st.eam_scroll = st.eam_scroll.saturating_add(2);
                    }
                } else if st.active_view == ActiveView::Osint {
                    st.news_scroll = st.news_scroll.saturating_add(2);
                } else if st.active_view == ActiveView::Satint {
                    st.sat_scroll = st.sat_scroll.saturating_add(2);
                } else if st.active_view == ActiveView::Cyberint {
                    st.cyber_scroll = st.cyber_scroll.saturating_add(2);
                } else if st.active_view == ActiveView::Notepad {
                    st.notepad.scroll = st.notepad.scroll.saturating_add(2);
                } else if st.active_view == ActiveView::Geoint || st.active_tile == 0 || st.zoomed_tile == Some(0) {
                    st.map_zoom = (st.map_zoom / 1.25).max(0.25);
                } else {
                    match st.active_tile {
                        1 => st.news_scroll = st.news_scroll.saturating_add(2),
                        2 => st.target_scroll = st.target_scroll.saturating_add(2),
                        3 => st.sat_scroll = st.sat_scroll.saturating_add(2),
                        4 => st.cyber_scroll = st.cyber_scroll.saturating_add(2),
                        5 => st.geo_scroll = st.geo_scroll.saturating_add(2),
                        6 => st.alert_scroll = st.alert_scroll.saturating_add(2),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

}
