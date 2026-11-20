use super::App;
use crate::audio::{AudioManager, SoundEffect};
use crate::widgets::command_palette::PaletteCommand;
use argus_core::state::AppState;
use argus_core::types::{
    ActiveView, AircraftMediaStatus, Domain, PendingNotification, TargetTrack, ThreatSeverity,
};
use chrono::Utc;
use parking_lot::RwLock;
use std::sync::Arc;

impl App {
    pub(super) fn spawn_flight_schedule(
        state_arc: &Arc<RwLock<AppState>>,
        st: &mut AppState,
        track: &TargetTrack,
    ) {
        let cs = track.callsign.trim().to_uppercase();
        if cs.len() < 3 || cs == track.id.to_uppercase() {
            return;
        }
        if st.flight_schedule_cache.contains_key(&cs) || !st.inflight_schedule_keys.insert(cs.clone())
        {
            return;
        }
        let known_route = track.destination.clone();
        let fa_key = st.config.api_keys.flightaware_key.clone();
        let state_arc = Arc::clone(state_arc);
        tokio::spawn(async move {
            let res = argus_ingest::flight_schedule::fetch_flight_schedule(
                &cs,
                known_route.as_deref(),
                fa_key.as_deref(),
            )
            .await;
            let mut lock = state_arc.write();
            lock.inflight_schedule_keys.remove(&cs);
            if let Some(info) = res {
                if lock.flight_schedule_cache.len() >= 128 {
                    lock.flight_schedule_cache.clear();
                }
                lock.flight_schedule_cache.insert(cs, info);
                lock.total_events_received = lock.total_events_received.wrapping_add(1);
            }
        });
    }


    pub(super) fn append_dialog_to_notepad(st: &mut AppState) {
        let stamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
        let exported = format!(
            "\n--- [{}] {} ---\n{}\n",
            stamp,
            st.dialog_title,
            st.dialog_content.join("\n")
        );
        let prev_active = st.notepad.active_file.clone();
        st.notepad.active_file = "notes.txt".to_string();
        let mut base = std::fs::read_to_string("notes.txt").unwrap_or_else(|_| st.notepad.content.clone());
        base.push_str(&exported);
        st.notepad.content = base;
        st.save_notepad();
        if prev_active != "notes.txt" {
            if let Ok(orig) = std::fs::read_to_string(&prev_active) {
                st.notepad.active_file = prev_active;
                st.notepad.content = orig;
            }
        }
        st.dialog_content
            .push("[*] RAPOR notes.txt DOSYASINA AKTARILDI".to_string());
    }

    pub(super) fn run_notepad_action(st: &mut AppState, act: usize) {
        match act {
            0 => st.save_notepad(),
            1 => {
                let stamp = format!("\n[{}] ", Utc::now().format("%H:%M %d/%m/%Y UTC"));
                st.notepad.content.push_str(&stamp);
            }
            2 => {
                if let Some(t) = st.inspected_target() {
                    let line = format!(
                        "\n- [{}] {} ({}) @ {:.3}N, {:.3}E | Hiz:{:.0}kt | {}\n",
                        Utc::now().format("%H:%M UTC"),
                        t.callsign,
                        t.id,
                        t.geo.lat,
                        t.geo.lon,
                        t.speed_kts.unwrap_or(0.0),
                        t.info
                    );
                    st.notepad.content.push_str(&line);
                    st.notepad.status_message = format!("Hedef eklendi: {}", t.callsign);
                } else {
                    st.notepad.status_message = "Eklenecek secili hedef bulunmuyor".to_string();
                }
            }
            3 => {
                let _ = st.create_new_notepad_file();
            }
            4 => {
                let _ = st.export_html_briefing_deck();
            }
            5 => {
                let _ = st.delete_selected_notepad_file();
            }
            _ => {}
        }
    }

    pub(super) fn spawn_ai_sitrep(state_arc: &Arc<RwLock<AppState>>, st: &mut AppState, force_refresh: bool) {
        if st.ai_analysis_running {
            return;
        }
        let key_opt = st
            .config
            .api_keys
            .gemini_key
            .clone()
            .filter(|k| !k.trim().is_empty());
        let Some(api_key) = key_opt else {
            st.open_dialog(
                "GEMINI AI ANALIZ ARACI (KURULUM)",
                vec![
                    "=== GEMINI API ANAHTARI GEREKLI ===".to_string(),
                    "Canli AI istihbarat raporu olusturmak icin config.toml veya ortam degiskeni tanimlayin:".to_string(),
                    "1) config.toml [api_keys] altina ekleyin:".to_string(),
                    "   gemini_key = \"AIza...\"".to_string(),
                    "   gemini_model = \"gemini-3.8-flash\"  # (opsiyonel)".to_string(),
                    "2) Veya baslatmadan once ortam degiskeni verin:".to_string(),
                    "   export ARGUS_GEMINI_KEY=\"AIza...\"".to_string(),
                ],
            );
            return;
        };

        if !force_refresh {
            if let Some((ts, rep)) = &st.ai_last_report {
                let mut lines: Vec<String> = rep.lines().map(|l| l.to_string()).collect();
                lines.push(format!(
                    "[*] Onbellek raporu ({} UTC) — Yenile:[R] │ Mod:[1-6] │ Takip Sorusu:[F]",
                    ts.format("%H:%M")
                ));
                st.open_dialog(
                    format!("GEMINI AI ANALIZ RAPORU ({} UTC)", ts.format("%H:%M")),
                    lines,
                );
                return;
            }
        }

        let model_opt = st.config.api_keys.gemini_model.clone();
        let prompt = argus_ingest::gemini_intel::build_mode_prompt(st, st.ai_mode, &st.ai_custom_prompt);
        let mode_label = match st.ai_mode {
            1 => "MOD 2: SECILI HEDEF",
            2 => "MOD 3: BOLGESEL KRIZ",
            3 => "MOD 4: OZEL SORU",
            4 => "MOD 5: KIRMIZI HUCRE",
            5 => "MOD 6: DELTA DEGISIM",
            _ => "MOD 1: FLASHPOINT",
        };
        st.ai_analysis_running = true;
        st.ai_followup_input_active = false;
        st.live_status = format!("GEMINI AI ({}) OLUSTURULUYOR...", mode_label);

        let state_arc = Arc::clone(state_arc);
        tokio::spawn(async move {
            let res = argus_ingest::gemini_intel::generate_ai_sitrep(
                &api_key,
                model_opt.as_deref(),
                &prompt,
            )
            .await;
            let mut lock = state_arc.write();
            lock.ai_analysis_running = false;
            match res {
                Ok(report) => {
                    let now = Utc::now();
                    let title = format!("GEMINI AI ANALIZ RAPORU ({} UTC)", now.format("%H:%M"));
                    let saved_path = lock.auto_save_ai_report_markdown(&title, &report);
                    lock.ai_last_report = Some((now, report.clone()));
                    lock.ai_conversation.clear();
                    lock.ai_conversation.push(("GEMINI".to_string(), report.clone()));
                    lock.live_status = "GEMINI AI SITREP HAZIR ([F] ile takip sorusu sorabilirsiniz)".to_string();
                    let mut lines: Vec<String> = report.lines().map(|l| l.to_string()).collect();
                    if let Some(p) = saved_path {
                        lines.push(format!("[*] Otomatik arsivlendi: {}", p));
                    }
                    lock.open_dialog(title, lines);
                }
                Err(e) => {
                    lock.live_status = "GEMINI AI HATA".to_string();
                    lock.open_dialog(
                        "GEMINI AI ANALIZ HATASI",
                        vec![
                            "=== API ISTEGI BASARISIZ ===".to_string(),
                            format!("Detay: {:#}", e),
                        ],
                    );
                }
            }
            lock.total_events_received = lock.total_events_received.wrapping_add(1);
        });
    }

    pub(super) fn spawn_ai_followup(
        state_arc: &Arc<RwLock<AppState>>,
        st: &mut AppState,
        question: String,
    ) {
        let q = question.trim().to_string();
        if q.is_empty() || st.ai_analysis_running {
            return;
        }
        let Some(api_key) = st
            .config
            .api_keys
            .gemini_key
            .clone()
            .filter(|k| !k.trim().is_empty())
        else {
            st.live_status = "GEMINI API ANAHTARI TANIMLI DEGIL".to_string();
            return;
        };

        let model_opt = st.config.api_keys.gemini_model.clone();
        let prompt = argus_ingest::gemini_intel::build_followup_prompt(st, &st.ai_conversation, &q);
        st.ai_conversation.push(("ANALIST".to_string(), q.clone()));
        st.dialog_content.push(String::new());
        st.dialog_content.push(format!("=== ANALIST TAKIP SORUSU ({} UTC) ===", Utc::now().format("%H:%M")));
        st.dialog_content.push(format!("> {}", q));
        st.dialog_content.push("[*] Gemini yaniti hazirlaniyor...".to_string());
        st.dialog_scroll = st.dialog_content.len().saturating_sub(12) as u16;
        st.ai_analysis_running = true;
        st.live_status = "GEMINI AI TAKIP SORUSU YANITLANIYOR...".to_string();

        let state_arc = Arc::clone(state_arc);
        tokio::spawn(async move {
            let res = argus_ingest::gemini_intel::generate_ai_sitrep(
                &api_key,
                model_opt.as_deref(),
                &prompt,
            )
            .await;
            let mut lock = state_arc.write();
            lock.ai_analysis_running = false;
            if lock.dialog_content.last().is_some_and(|l| l.contains("hazirlaniyor")) {
                lock.dialog_content.pop();
            }
            match res {
                Ok(ans) => {
                    lock.ai_conversation.push(("GEMINI".to_string(), ans.clone()));
                    lock.dialog_content.push("--- GEMINI ANALIST YANITI ---".to_string());
                    for line in ans.lines() {
                        lock.dialog_content.push(line.to_string());
                    }
                    let full_text = lock.dialog_content.join("\n");
                    lock.ai_last_report = Some((Utc::now(), full_text));
                    lock.live_status = "GEMINI TAKIP YANITI EKLENDI".to_string();
                }
                Err(e) => {
                    lock.dialog_content.push(format!("[!] TAKIP SORUSU HATASI: {:#}", e));
                    lock.live_status = "GEMINI TAKIP HATA".to_string();
                }
            }
            lock.total_events_received = lock.total_events_received.wrapping_add(1);
        });
    }

    pub(super) fn spawn_aircraft_photo(state_arc: &Arc<RwLock<AppState>>, st: &mut AppState, icao: &str) {
        let key = icao.trim().to_lowercase();
        if key.len() < 4 {
            return;
        }
        if let Some(cached) = st.aircraft_photo_cache.get(&key).cloned() {
            if cached.image_bytes.is_some() {
                st.aircraft_media_status = AircraftMediaStatus::Ready(cached);
            } else {
                st.aircraft_media_status = AircraftMediaStatus::NotFound {
                    icao: key,
                    registration: cached.registration,
                    reason: cached.note,
                };
            }
            return;
        }
        if !st.inflight_photo_icaos.insert(key.clone()) {
            return;
        }
        st.aircraft_media_status = AircraftMediaStatus::Loading { icao: key.clone() };
        let state_arc = Arc::clone(state_arc);
        tokio::spawn(async move {
            let entry = argus_ingest::aircraft_photo::fetch_aircraft_photo(&key).await;
            let mut lock = state_arc.write();
            lock.inflight_photo_icaos.remove(&key);
            let has_img = entry.image_bytes.is_some();
            lock.store_aircraft_photo(entry.clone());
            if has_img {
                lock.aircraft_media_status = AircraftMediaStatus::Ready(entry);
            } else {
                lock.aircraft_media_status = AircraftMediaStatus::NotFound {
                    icao: key,
                    registration: entry.registration,
                    reason: entry.note,
                };
            }
            lock.total_events_received = lock.total_events_received.wrapping_add(1);
        });
    }


    pub(super) fn toggle_map_aircraft_shapes(st: &mut AppState) {
        st.map_aircraft_shapes = !st.map_aircraft_shapes;
        st.live_status = if st.map_aircraft_shapes {
            "HARITA PLATFORM SEKILLERI ACIK ([▲]Jet [✦]ISR [◈]Tanker/Kargo [❖]Heli [◆]Genis [▴]Dar [▫]Hafif)"
                .to_string()
        } else {
            "HARITA PLATFORM SEKILLERI KAPALI (Klasik +CagriAdi modu)".to_string()
        };
    }

    pub(super) fn toggle_sdr_scanner(st: &mut AppState) {
        st.sdr_scanner_enabled = !st.sdr_scanner_enabled;
        if !st.sdr_scanner_enabled {
            st.sdr_scan_locked = false;
        }
        st.live_status = if st.sdr_scanner_enabled {
            "OTONOM HF BANT TARAYICI ACIK (Esik: >=11dB SNR)".to_string()
        } else {
            "OTONOM HF BANT TARAYICI KAPALI".to_string()
        };
    }

    pub(super) fn toggle_sdr_vox(st: &mut AppState) {
        st.sdr_vox_enabled = !st.sdr_vox_enabled;
        st.live_status = if st.sdr_vox_enabled {
            "AKILLI VOX OTO-KAYIT ACIK (recordings/vox_*.wav)".to_string()
        } else {
            "AKILLI VOX OTO-KAYIT KAPALI".to_string()
        };
    }

    pub(super) fn toggle_sdr_cw(st: &mut AppState) {
        st.sdr_cw_enabled = !st.sdr_cw_enabled;
        if !st.sdr_cw_enabled {
            st.sdr_cw_text.clear();
            st.sdr_cw_wpm = 0;
        }
        st.live_status = if st.sdr_cw_enabled {
            "CW MORZE COZUCU ACIK (600Hz Goertzel 5-40 WPM)".to_string()
        } else {
            "CW MORZE COZUCU KAPALI".to_string()
        };
    }

    pub(super) fn sync_radio_state(audio: &AudioManager, st: &AppState) {
        audio.set_radio_playing(st.is_radio_playing);
    }

    /// Hava filtresi uygular; CENTCOM filtrelerinde mini-haritayi CENTCOM harekat alanina,
    /// Kuresel filtrelerde dunya geneline, TR/EUCOM filtresinde Turkiye/Avrupa'ya odaklar
    pub(super) fn apply_air_filter(st: &mut AppState, idx: usize) {
        st.air_filter = idx.min(5);
        st.air_scroll = 0;
        st.follow_target_id = None;
        match st.air_filter {
            1 | 2 => {
                st.map_center_lat = 26.0;
                st.map_center_lon = 46.0;
                st.map_zoom = 2.0;
                st.live_status = "HAVA: CENTCOM HAREKAT ALANI (Levant / Irak / Korfez / Kizildeniz / Iran)".to_string();
            }
            0 | 3 => {
                st.map_center_lat = 15.0;
                st.map_center_lon = 10.0;
                st.map_zoom = 0.35;
                st.live_status = "HAVA: KURESEL GORUNUM (TUM DUNYA)".to_string();
            }
            4 => {
                st.map_center_lat = 44.0;
                st.map_center_lon = 22.0;
                st.map_zoom = 1.6;
                st.live_status = "HAVA: TURKIYE / EUCOM".to_string();
            }
            _ => {}
        }
    }

    pub(super) fn step_scroll(cur: usize, delta: isize, max_pos: usize) -> usize {
        if delta < 0 {
            cur.min(max_pos).saturating_sub(delta.unsigned_abs())
        } else {
            cur.saturating_add(delta.unsigned_abs()).min(max_pos)
        }
    }

    pub(super) fn filtered_news_indices(st: &AppState) -> Vec<usize> {
        let q_lower = st.search_query.to_lowercase();
        st.news
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                let cat_ok = match st.news_filter {
                    1 => item.category.starts_with("P201"),
                    2 => item.category.starts_with("P202"),
                    3 => item.category.starts_with("P203"),
                    4 => item.category.starts_with("P204"),
                    _ => true,
                };
                let search_ok = q_lower.is_empty()
                    || item.title.to_lowercase().contains(&q_lower)
                    || item.source.to_lowercase().contains(&q_lower);
                cat_ok && search_ok
            })
            .map(|(idx, _)| idx)
            .collect()
    }

    pub(super) fn filtered_acled_indices(st: &AppState) -> Vec<usize> {
        let q_lower = st.search_query.to_lowercase();
        st.acled_events
            .iter()
            .enumerate()
            .filter(|(_, ev)| {
                q_lower.is_empty()
                    || ev.country.to_lowercase().contains(&q_lower)
                    || ev.location.to_lowercase().contains(&q_lower)
                    || ev.event_type.to_lowercase().contains(&q_lower)
                    || ev.actor1.to_lowercase().contains(&q_lower)
                    || ev.notes.to_lowercase().contains(&q_lower)
            })
            .map(|(idx, _)| idx)
            .collect()
    }

    pub(super) fn apply_osint_category(st: &mut AppState, cat: usize) {
        let cat = cat.min(5);
        st.news_filter = cat;
        st.news_scroll = 0;
        if cat == 5 {
            let first_idx = Self::filtered_acled_indices(st).first().copied().unwrap_or(0);
            Self::select_item_for_inspector(st, 52, first_idx);
            st.news_scroll = 0;
        } else {
            let first_idx = Self::filtered_news_indices(st).first().copied().unwrap_or(0);
            st.selected_domain = 1;
            st.selected_row = first_idx;
        }
    }

    pub(super) fn apply_masint_tab(st: &mut AppState, tid: usize) {
        st.masint_tab = tid.min(4);
        st.geo_scroll = 0;
        st.radiation_scroll = 0;
        st.alert_scroll = 0;
        if st.masint_tab == 4 {
            let a_len = st.alerts.len();
            if a_len > 0 {
                let _ = Self::handle_alert_click(st, a_len - 1, false);
            } else if !st.fusion_clusters.is_empty() {
                let _ = Self::focus_fusion_cluster(st, 0, false);
            } else {
                st.selected_domain = 56;
                st.selected_row = 0;
            }
            return;
        }
        let dom = match st.masint_tab {
            1 => 51,
            2 => 53,
            3 => 54,
            _ => 50,
        };
        Self::select_item_for_inspector(st, dom, 0);
    }

    pub(super) fn focus_fusion_cluster(st: &mut AppState, fc_idx: usize, is_double_click: bool) -> bool {
        st.follow_target_id = None;
        Self::select_item_for_inspector(st, 55, fc_idx);
        st.map_zoom = st.map_zoom.max(3.5);
        if let Some(fc) = st.fusion_clusters.get(fc_idx) {
            st.live_status = format!(
                "K-4 FUZYON KUMESI [Skor {}]: {:.2}N, {:.2}E ({})",
                fc.score,
                fc.lat,
                fc.lon,
                fc.domains.join("+")
            );
        }
        is_double_click && Self::open_item_dossier(st, 55, fc_idx)
    }

    pub(super) fn handle_alert_click(
        st: &mut AppState,
        alert_idx: usize,
        is_double_click: bool,
    ) -> Option<SoundEffect> {
        let alert = st.alerts.get(alert_idx).cloned()?;
        let Some(tid) = alert.target_id.as_deref() else {
            st.live_status = format!("ALARM: {}", alert.message);
            return Some(SoundEffect::Click);
        };

        if tid == "SYS:DEFCON" {
            st.selected_domain = 56;
            st.selected_row = alert_idx;
            if is_double_click {
                Self::open_defcon_dialog(st);
            }
            return Some(SoundEffect::Blip);
        }

        if let Some(eq_id) = tid.strip_prefix("MASINT:") {
            let pos_opt = st.earthquakes.iter().position(|q| q.id == eq_id);
            if let Some(pos) = pos_opt {
                st.follow_target_id = None;
                Self::select_item_for_inspector(st, 50, pos);
                st.map_zoom = st.map_zoom.max(4.0);
                if let Some(q) = st.earthquakes.get(pos) {
                    st.live_status = format!(
                        "MASINT PATLAMA ALARMI: M{:.1} (d={:.1}km) {} ({:.2}N, {:.2}E)",
                        q.mag, q.depth_km, q.place, q.lat, q.lon
                    );
                }
                return Some(if is_double_click && Self::open_item_dossier(st, 50, pos) {
                    SoundEffect::Blip
                } else {
                    SoundEffect::Click
                });
            }
        }

        let extracted_id = if let Some(rest) = tid.strip_prefix("AAR:") {
            rest.split(':').next().unwrap_or(rest)
        } else if let Some(rest) = tid.strip_prefix("STS:") {
            rest.split(':').next().unwrap_or(rest)
        } else {
            tid.strip_prefix("ANOM:").unwrap_or(tid)
        };

        if let Some(ev_id) = tid.strip_prefix("ACLED:") {
            let pos_opt = st.acled_events.iter().position(|e| e.event_id == ev_id);
            if let Some(pos) = pos_opt {
                st.follow_target_id = None;
                Self::select_item_for_inspector(st, 52, pos);
                st.map_zoom = st.map_zoom.max(4.0);
                if let Some(ev) = st.acled_events.get(pos) {
                    st.live_status = format!(
                        "ALARM CATISMA: {}, {} ({}) ({:.2}N, {:.2}E)",
                        ev.location, ev.country, ev.event_type, ev.lat, ev.lon
                    );
                }
                return Some(if is_double_click && Self::open_item_dossier(st, 52, pos) {
                    SoundEffect::Blip
                } else {
                    SoundEffect::Click
                });
            }
            st.live_status = format!("ALARM: {}", alert.message);
            return Some(SoundEffect::Click);
        }

        if tid.starts_with("FUZ:") {
            let pos = st
                .fusion_clusters
                .iter()
                .position(|fc| format!("FUZ:{:.1}:{:.1}", fc.lat, fc.lon) == tid)
                .unwrap_or(0);
            return Some(if Self::focus_fusion_cluster(st, pos, is_double_click) {
                SoundEffect::Blip
            } else {
                SoundEffect::Click
            });
        }

        let raw_id = extracted_id.to_string();
        if st.follow_target_id.as_deref() != Some(raw_id.as_str()) {
            st.follow_target_id = None;
        }
        st.selected_target_id = Some(raw_id.clone());

        let found = st
            .air_targets
            .iter()
            .find(|t| t.id == raw_id)
            .or_else(|| st.sea_targets.iter().find(|t| t.id == raw_id))
            .or_else(|| st.targets.iter().find(|t| t.id == raw_id))
            .cloned();

        let Some(t) = found else {
            if let Some(dark) = st.dark_ships.iter().find(|d| d.id == raw_id).cloned() {
                st.map_center_lat = dark.dr_lat;
                st.map_center_lon = dark.dr_lon;
                st.map_zoom = st.map_zoom.max(4.5);
                st.selected_domain = 56;
                st.selected_row = alert_idx;
                st.live_status = format!(
                    "DARK-SHIP DR KONUMU: {} ({}) -> {:.2}N, {:.2}E ({}dk sessiz)",
                    dark.callsign, dark.id, dark.dr_lat, dark.dr_lon, dark.silent_mins
                );
                if is_double_click && Self::open_item_dossier(st, 56, alert_idx) {
                    return Some(SoundEffect::Blip);
                }
                return Some(SoundEffect::Click);
            }
            st.selected_domain = 56;
            st.selected_row = alert_idx;
            st.live_status = format!("ALARM KAYDI: {}", alert.message);
            if is_double_click && Self::open_item_dossier(st, 56, alert_idx) {
                return Some(SoundEffect::Blip);
            }
            return Some(SoundEffect::Click);
        };

        st.map_center_lat = t.geo.lat;
        st.map_center_lon = t.geo.lon;
        st.map_zoom = st.map_zoom.max(4.5);

        if t.domain == Domain::Air {
            st.selected_domain = 20;
            st.ensure_air_indices();
            if let Some(pos) = st
                .cached_air_indices
                .iter()
                .position(|&bi| st.air_targets.get(bi).is_some_and(|x| x.id == raw_id))
            {
                st.air_scroll = pos;
                st.selected_row = pos;
            }
        } else if t.domain == Domain::Sea {
            st.selected_domain = 21;
            st.ensure_sea_indices();
            if let Some(pos) = st
                .cached_sea_indices
                .iter()
                .position(|&bi| st.sea_targets.get(bi).is_some_and(|x| x.id == raw_id))
            {
                st.sea_scroll = pos;
                st.selected_row = pos;
            }
        } else {
            st.selected_domain = 2;
        }

        st.ensure_proximity_indices();
        if let Some(ppos) = st
            .cached_prox_indices
            .iter()
            .position(|&bi| st.targets.get(bi).is_some_and(|x| x.id == raw_id))
        {
            st.target_scroll = ppos;
            if t.domain != Domain::Air && t.domain != Domain::Sea {
                st.selected_row = ppos;
            }
        }

        st.live_status = format!(
            "ALARM HEDEFI SECILDI: {} [{}] ({:.2}N, {:.2}E)",
            t.callsign, t.id, t.geo.lat, t.geo.lon
        );

        if is_double_click {
            if t.domain == Domain::Air {
                Self::open_air_target_dossier(st, &t);
                return Some(SoundEffect::Blip);
            }
            let pos_opt = st.targets.iter().position(|x| x.id == raw_id);
            if let Some(pos) = pos_opt {
                if Self::open_item_dossier(st, 2, pos) {
                    return Some(SoundEffect::Blip);
                }
            }
        }

        Some(SoundEffect::Click)
    }

    pub(super) fn select_item_for_inspector(st: &mut AppState, tile_idx: usize, row_idx: usize) {
        st.selected_domain = tile_idx;
        st.selected_row = row_idx;
        match tile_idx {
            20 => {
                if let Some(t) = st.aircraft_at_sorted(row_idx) {
                    st.selected_target_id = Some(t.id.clone());
                }
                if st.follow_target_id.as_deref() != st.selected_target_id.as_deref() {
                    st.follow_target_id = None;
                }
            }
            21 => {
                if let Some(t) = st.vessel_at_sorted(row_idx) {
                    st.selected_target_id = Some(t.id.clone());
                }
                if st.follow_target_id.as_deref() != st.selected_target_id.as_deref() {
                    st.follow_target_id = None;
                }
            }
            2 => {
                if let Some(t) = st.targets.get(row_idx) {
                    st.selected_target_id = Some(t.id.clone());
                }
                if st.follow_target_id.as_deref() != st.selected_target_id.as_deref() {
                    st.follow_target_id = None;
                }
            }
            1 => {
                // news_scroll filtreli liste konumu olarak korunur
            }
            3 => {
                st.sat_scroll = row_idx;
                if let Some(s) = st.satellites.get(row_idx) {
                    st.follow_target_id = None;
                    st.map_center_lat = s.lat;
                    st.map_center_lon = s.lon;
                }
            }
            4 => {
                st.cyber_scroll = row_idx;
            }
            30 => {
                st.eam_scroll = row_idx;
            }
            50 => {
                st.geo_scroll = row_idx;
                if let Some(q) = st.earthquakes.get(row_idx) {
                    st.follow_target_id = None;
                    st.map_center_lat = q.lat;
                    st.map_center_lon = q.lon;
                }
            }
            51 => {
                st.geo_scroll = row_idx;
                if let Some(f) = st.firms_fires.get(row_idx) {
                    st.follow_target_id = None;
                    st.map_center_lat = f.lat;
                    st.map_center_lon = f.lon;
                }
            }
            52 => {
                if st.active_view == ActiveView::Osint && st.news_filter == 5 {
                    if let Some(fpos) = Self::filtered_acled_indices(st)
                        .iter()
                        .position(|&i| i == row_idx)
                    {
                        st.news_scroll = fpos;
                    }
                } else {
                    st.geo_scroll = row_idx;
                }
                if let Some(ev) = st.acled_events.get(row_idx) {
                    st.follow_target_id = None;
                    st.map_center_lat = ev.lat;
                    st.map_center_lon = ev.lon;
                    st.geoint_layers |= 8 | 16;
                }
            }
            53 => {
                st.radiation_scroll = row_idx;
                if let Some(r) = st.radiation_measurements.get(row_idx) {
                    st.follow_target_id = None;
                    st.map_center_lat = r.lat;
                    st.map_center_lon = r.lon;
                }
            }
            54 => {
                st.geo_scroll = row_idx;
                if let Some(aw) = st.airspace_warnings.get(row_idx) {
                    st.follow_target_id = None;
                    st.map_center_lat = aw.center_lat;
                    st.map_center_lon = aw.center_lon;
                    st.geoint_layers |= 1 << 11;
                }
            }
            55 => {
                if let Some(fc) = st.fusion_clusters.get(row_idx) {
                    st.follow_target_id = None;
                    st.map_center_lat = fc.lat;
                    st.map_center_lon = fc.lon;
                    st.geoint_layers |= 1024;
                }
            }
            _ => {}
        }
    }

    pub(super) fn resolve_ruler_pin(st: &mut AppState) -> (f64, f64, String) {
        if let Some(t) = st.inspected_target() {
            let lbl = if t.callsign.trim().is_empty() {
                t.id
            } else {
                t.callsign
            };
            (t.geo.lat, t.geo.lon, lbl)
        } else {
            (st.map_center_lat, st.map_center_lon, "HARITA MRK".to_string())
        }
    }

    pub(super) fn focus_selected_on_map(st: &mut AppState) {
        if let Some(id) = st.selected_target_id.clone() {
            if let Some(t) = st
                .air_targets
                .iter()
                .find(|t| t.id == id)
                .or_else(|| st.sea_targets.iter().find(|t| t.id == id))
            {
                st.map_center_lat = t.geo.lat;
                st.map_center_lon = t.geo.lon;
                if st.map_zoom < 3.0 {
                    st.map_zoom = 3.5;
                }
                st.live_status = format!("HARITA ODAKLANDI: {} ({})", t.callsign, t.id);
            }
        }
    }

    pub(super) fn toggle_selected_follow(st: &mut AppState) {
        if let Some(id) = st.selected_target_id.clone() {
            if st.follow_target_id.as_deref() == Some(id.as_str()) {
                st.follow_target_id = None;
                st.live_status = "TAKIP KILIDI ACILDI".to_string();
            } else {
                st.follow_target_id = Some(id.clone());
                Self::focus_selected_on_map(st);
                st.live_status = format!("OTOMATIK TAKIP KILIDI: {}", id);
            }
        }
    }

    pub(super) fn toggle_selected_watchlist(st: &mut AppState) {
        if let Some(id) = st.selected_target_id.clone() {
            if st.watchlist.contains(&id) {
                st.watchlist.remove(&id);
                st.live_status = format!("IZLEME LISTESINDEN CIKARILDI: {}", id);
            } else {
                st.watchlist.insert(id.clone());
                st.live_status = format!("IZLEME LISTESINE EKLENDI: {}", id);
            }
            let _ = st.save_session();
        }
    }

    pub(super) fn launch_web_map(st: &mut AppState) {
        let _ = std::process::Command::new("xdg-open")
            .arg("http://127.0.0.1:8090")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        st.live_status = "CANLI WEB HARITA ACILDI: http://127.0.0.1:8090".to_string();
    }

    /// Y-1: Kritik alarmlari Linux masaustu (`notify-send`) ve opsiyonel Webhook uzerinden iletir.
    pub(super) fn dispatch_notifications(
        batch: Vec<PendingNotification>,
        desktop_enabled: bool,
        webhook_url: Option<String>,
    ) {
        for n in batch {
            if desktop_enabled {
                let urgency = match n.severity {
                    ThreatSeverity::Emergency | ThreatSeverity::Critical => "critical",
                    _ => "normal",
                };
                let _ = std::process::Command::new("notify-send")
                    .arg("-u")
                    .arg(urgency)
                    .arg("-a")
                    .arg("Project Sentinel")
                    .arg("-t")
                    .arg("5000")
                    .arg(&n.title)
                    .arg(&n.body)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            }
            if let Some(url) = webhook_url.clone().filter(|u| !u.trim().is_empty()) {
                let title = n.title.clone();
                let body = n.body.clone();
                tokio::spawn(async move {
                    argus_ingest::dispatch_webhook_notification(&url, &title, &body).await;
                });
            }
        }
    }

    /// U-2: `39.92, 32.85` veya `39.92 32.85` bicimindeki koordinat girdisini cozumler.
    pub(super) fn parse_lat_lon(raw: &str) -> Option<(f64, f64)> {
        let cleaned = raw.replace([';', '/'], ",");
        let parts: Vec<&str> = if cleaned.contains(',') {
            cleaned.split(',').map(str::trim).filter(|s| !s.is_empty()).collect()
        } else {
            cleaned.split_whitespace().collect()
        };
        if parts.len() != 2 {
            return None;
        }
        let lat: f64 = parts[0].parse().ok()?;
        let lon: f64 = parts[1].parse().ok()?;
        if (-85.0..=85.0).contains(&lat) && (-180.0..=180.0).contains(&lon) {
            Some((lat, lon))
        } else {
            None
        }
    }

    pub(super) fn apply_geoint_goto(st: &mut AppState) -> bool {
        if let Some((lat, lon)) = Self::parse_lat_lon(&st.geoint_goto_input) {
            st.follow_target_id = None;
            st.map_center_lat = lat;
            st.map_center_lon = lon;
            st.map_zoom = st.map_zoom.max(4.0);
            st.geoint_goto_active = false;
            st.live_status = format!("KOORDINATA GIDILDI: {:.4}N, {:.4}E", lat, lon);
            true
        } else {
            st.live_status = "GECERSIZ KOORDINAT (Ornek: 39.92, 32.85)".to_string();
            false
        }
    }

    pub(super) fn apply_region_preset(st: &mut AppState, p_idx: usize) {
        st.follow_target_id = None;
        match p_idx {
            0 => {
                st.map_center_lat = 39.0;
                st.map_center_lon = 35.0;
                st.map_zoom = 2.2;
            }
            1 => {
                st.map_center_lat = 41.02;
                st.map_center_lon = 29.00;
                st.map_zoom = 8.5;
            }
            2 => {
                st.map_center_lat = 35.5;
                st.map_center_lon = 33.0;
                st.map_zoom = 2.5;
            }
            3 => {
                st.map_center_lat = 43.5;
                st.map_center_lon = 34.0;
                st.map_zoom = 2.2;
            }
            5 => {
                st.map_center_lat = 26.0;
                st.map_center_lon = 46.0;
                st.map_zoom = 2.0;
            }
            _ => {
                st.map_center_lat = 15.0;
                st.map_center_lon = 10.0;
                st.map_zoom = 0.35;
            }
        }
    }

    /// U-1: Komut Paleti (`Ctrl+K`) uzerinden secilen komutu calistirir.
    pub(super) fn execute_palette_command(&self, st: &mut AppState, cmd: PaletteCommand) {
        st.command_palette.is_open = false;
        st.command_palette.query.clear();
        st.command_palette.selected_idx = 0;

        match cmd {
            PaletteCommand::SwitchView(v) => {
                st.set_view(v);
                if v == ActiveView::AirTracks {
                    st.sync_air_selection();
                } else if v == ActiveView::Notepad {
                    st.refresh_notepad_files();
                }
            }
            PaletteCommand::RegionPreset(p_idx) => {
                Self::apply_region_preset(st, p_idx);
            }
            PaletteCommand::ToggleLayer(bit) => {
                if bit < 16 {
                    st.geoint_layers ^= 1u16 << bit;
                    let _ = st.save_session();
                }
            }
            PaletteCommand::TriggerAi(mode) => {
                st.ai_mode = mode;
                Self::spawn_ai_sitrep(&self.state, st, true);
            }
            PaletteCommand::OpenFeedHealth => {
                Self::open_feed_health_dialog(st);
            }
            PaletteCommand::OpenWebMap => {
                Self::launch_web_map(st);
            }
            PaletteCommand::ExportMarkdown => {
                let _ = st.export_intelligence_report();
            }
            PaletteCommand::ExportHtml => {
                let _ = st.export_html_briefing_deck();
            }
            PaletteCommand::ToggleShapes => {
                Self::toggle_map_aircraft_shapes(st);
            }
            PaletteCommand::CycleMapStyle => {
                st.map_style = st.map_style.next();
                st.live_status = format!("HARITA MODU: {}", st.map_style.label());
            }
            PaletteCommand::ToggleDvrPlay => {
                st.dvr.toggle_play();
                st.live_status = if st.dvr.is_playing {
                    format!("DVR OYNATILIYOR ({}x | -{}dk)", st.dvr.play_speed, st.dvr.replay_offset_mins)
                } else {
                    "DVR DURAKLATILDI".to_string()
                };
            }
            PaletteCommand::CycleDvrArchive => {
                if let Some(arch) = st.dvr.cycle_archive_in(std::path::Path::new("recordings")) {
                    st.live_status = format!("DVR ARSIV YUKLENDI: {} ({} kare)", arch, st.dvr.snapshots.len());
                } else {
                    st.live_status = "DVR: CANLI AKIS MODU".to_string();
                }
            }
            PaletteCommand::ToggleTrackHistory => {
                st.dvr.show_track_history = !st.dvr.show_track_history;
                st.live_status = if st.dvr.show_track_history {
                    "DVR GUNLUK ROTA IZI ACIK".to_string()
                } else {
                    "DVR GUNLUK ROTA IZI KAPALI".to_string()
                };
            }
            PaletteCommand::OpenGeoGoTo => {
                st.set_view(ActiveView::Geoint);
                st.geoint_goto_active = true;
                st.geoint_goto_input.clear();
            }
            PaletteCommand::ToggleAudio => {
                st.audio_muted = !st.audio_muted;
                self.audio.set_muted(st.audio_muted);
            }
            PaletteCommand::ToggleSdr => {
                st.is_radio_playing = !st.is_radio_playing;
                Self::sync_radio_state(&self.audio, st);
            }
            PaletteCommand::FocusTarget(tid) => {
                st.selected_target_id = Some(tid.clone());
                Self::focus_selected_on_map(st);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::command_palette::build_palette_entries;
    use argus_core::config::AppConfig;

    #[test]
    fn test_parse_lat_lon_formats() {
        assert_eq!(App::parse_lat_lon("39.92, 32.85"), Some((39.92, 32.85)));
        assert_eq!(App::parse_lat_lon("41.0082 28.9784"), Some((41.0082, 28.9784)));
        assert_eq!(App::parse_lat_lon("-33.86; 151.20"), Some((-33.86, 151.20)));
        assert_eq!(App::parse_lat_lon("95.0, 20.0"), None);
        assert_eq!(App::parse_lat_lon("invalid"), None);
    }

    #[test]
    fn test_command_palette_filters_commands_and_targets() {
        let mut st = AppState::new(AppConfig::default());
        st.upsert_target(TargetTrack {
            id: "ae0411".to_string(),
            callsign: "FORTE11".to_string(),
            domain: Domain::Air,
            geo: argus_core::types::GeoPoint::new(43.1, 32.5, Some(16000.0)),
            speed_kts: Some(320.0),
            heading_deg: Some(90.0),
            vertical_rate_fpm: None,
            squawk: None,
            is_military: true,
            operator_country: None,
            operator_branch: None,
            airframe_or_vessel_class: None,
            origin: None,
            destination: None,
            vessel_type: None,
            info: String::new(),
            last_seen: Utc::now(),
        });

        st.command_palette.query = "delta".to_string();
        let entries = build_palette_entries(&st);
        assert!(entries.iter().any(|e| e.label.contains("Delta")));

        st.command_palette.query = "forte11".to_string();
        let target_entries = build_palette_entries(&st);
        assert!(target_entries.iter().any(|e| e.category == "HEDEF" && e.label.contains("FORTE11")));
    }
}
