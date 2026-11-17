use crate::audio::{AudioManager, SoundEffect};
use crate::photo_ui::PhotoRenderCtx;
use crate::ui::render_main_ui;
use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::types::{ActiveView, AircraftMediaStatus, Domain, TargetTrack};
use chrono::Utc;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use parking_lot::RwLock;
use ratatui::backend::Backend;
use ratatui::Terminal;
use ratatui_image::picker::Picker;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct App {
    pub state: Arc<RwLock<AppState>>,
    pub audio: AudioManager,
    pub photo_ctx: PhotoRenderCtx,
    pub should_quit: bool,
    last_click_time: Instant,
    last_click_target: Option<HitTarget>,
}

impl App {
    pub fn new(state: Arc<RwLock<AppState>>, audio: AudioManager, picker: Picker) -> Self {
        Self {
            state,
            audio,
            photo_ctx: PhotoRenderCtx::new(picker),
            should_quit: false,
            last_click_time: Instant::now(),
            last_click_target: None,
        }
    }

    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> anyhow::Result<()> {
        let poll_wait = Duration::from_millis(50);
        let mut needs_redraw = true;
        let mut last_redraw = Instant::now();
        let mut last_events_ver = u64::MAX;
        let mut last_sdr_status = String::new();
        let mut last_wf_len = usize::MAX;
        let mut last_sat_len = usize::MAX;

        loop {
            if self.should_quit {
                break;
            }

            {
                let mut st = self.state.write();
                if st.is_splash_active {
                    st.advance_splash();
                    needs_redraw = true;
                }
                if st.total_events_received != last_events_ver
                    || st.waterfall_data.len() != last_wf_len
                    || st.satellites.len() != last_sat_len
                    || st.sdr_status != last_sdr_status
                {
                    last_events_ver = st.total_events_received;
                    last_wf_len = st.waterfall_data.len();
                    last_sat_len = st.satellites.len();
                    last_sdr_status.clone_from(&st.sdr_status);
                    needs_redraw = true;
                }

                // T-Dossier açıldıysa anında, Inspector'da seçili uçak sabit kaldıysa 350ms sonra fotoğraf çek
                if let Some(d_icao) = st.dialog_aircraft_icao.clone() {
                    Self::spawn_aircraft_photo(&self.state, &mut st, &d_icao);
                } else if !st.is_dialog_open && !matches!(st.selected_domain, 3 | 30 | 50) {
                    if let Some(t) = st.inspected_target().filter(|t| t.domain == Domain::Air) {
                        let icao = t.id.to_lowercase();
                        if self.photo_ctx.last_inspector_icao.as_deref() != Some(&icao) {
                            self.photo_ctx.last_inspector_icao = Some(icao.clone());
                            self.photo_ctx.inspector_changed_at = Instant::now();
                            if let Some(cached) = st.aircraft_photo_cache.get(&icao).cloned() {
                                if cached.image_bytes.is_some() {
                                    st.aircraft_media_status = AircraftMediaStatus::Ready(cached);
                                } else {
                                    st.aircraft_media_status = AircraftMediaStatus::NotFound {
                                        icao,
                                        registration: cached.registration,
                                        reason: cached.note,
                                    };
                                }
                                needs_redraw = true;
                            }
                        } else if self.photo_ctx.inspector_changed_at.elapsed()
                            >= Duration::from_millis(350)
                            && !st.aircraft_photo_cache.contains_key(&icao)
                            && !st.inflight_photo_icaos.contains(&icao)
                        {
                            Self::spawn_aircraft_photo(&self.state, &mut st, &icao);
                            needs_redraw = true;
                        }
                    }
                }
            }

            if needs_redraw || last_redraw.elapsed() >= Duration::from_millis(500) {
                let clear_before_draw = {
                    let mut st = self.state.write();
                    std::mem::take(&mut st.force_terminal_clear)
                };
                if clear_before_draw {
                    terminal.clear()?;
                }
                terminal.draw(|frame| {
                    let mut lock = self.state.write();
                    render_main_ui(frame, &mut lock, &mut self.photo_ctx);
                })?;
                needs_redraw = self.state.read().force_terminal_clear;
                last_redraw = Instant::now();
            }

            // Ilk olayi kisa sure bekle, ardindan birikmis tum olaylari tek seferde bosalt (0 gecikme)
            if event::poll(poll_wait)? {
                loop {
                    match event::read()? {
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            self.handle_key(key.code, key.modifiers).await;
                            needs_redraw = true;
                        }
                        Event::Mouse(mouse) => {
                            if !matches!(mouse.kind, MouseEventKind::Moved) {
                                self.handle_mouse(mouse).await;
                                needs_redraw = true;
                            }
                        }
                        Event::Resize(_, _) => {
                            needs_redraw = true;
                        }
                        _ => {}
                    }
                    if self.should_quit || !event::poll(Duration::ZERO)? {
                        break;
                    }
                }
            }
        }

        self.audio.shutdown();
        Ok(())
    }

    async fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        if modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        let mut st = self.state.write();

        // 0. Eğer Splash / BIOS açılış ekranı aktifse herhangi bir tuşla geç
        if st.is_splash_active || st.active_view == ActiveView::Splash {
            st.dismiss_splash();
            self.audio.play(SoundEffect::Click);
            return;
        }

        // 1. Eğer diyalog açıksa Esc veya Enter ile kapat, yön tuşlarıyla kaydır, E ile Notepad'e aktar
        if st.is_dialog_open {
            match code {
                KeyCode::Esc | KeyCode::Enter => {
                    st.close_dialog();
                    self.audio.play(SoundEffect::Click);
                }
                KeyCode::Char('e') | KeyCode::Char('E') => {
                    let stamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
                    let exported = format!(
                        "\n--- [{}] {} ---\n{}\n",
                        stamp,
                        st.dialog_title,
                        st.dialog_content.join("\n")
                    );
                    st.notepad.content.push_str(&exported);
                    st.save_notepad();
                    st.dialog_content
                        .push("[*] RAPOR notes.txt DOSYASINA AKTARILDI".to_string());
                    self.audio.play(SoundEffect::Blip);
                }
                KeyCode::Up => st.dialog_scroll = st.dialog_scroll.saturating_sub(1),
                KeyCode::Down => st.dialog_scroll = st.dialog_scroll.saturating_add(1),
                KeyCode::PageUp => st.dialog_scroll = st.dialog_scroll.saturating_sub(6),
                KeyCode::PageDown => st.dialog_scroll = st.dialog_scroll.saturating_add(6),
                _ => {}
            }
            return;
        }

        // 2. Eğer başlat menüsü açıksa Esc ile kapat
        if st.is_start_menu_open && code == KeyCode::Esc {
            st.is_start_menu_open = false;
            self.audio.play(SoundEffect::Click);
            return;
        }

        // 2.5 Canlı Arama Çubuğu Modu
        if st.is_search_active {
            match code {
                KeyCode::Esc => {
                    st.search_query.clear();
                    st.is_search_active = false;
                    if st.active_view == ActiveView::AirTracks {
                        st.air_scroll = 0;
                        st.sync_air_selection();
                    }
                    self.audio.play(SoundEffect::Click);
                }
                KeyCode::Enter => {
                    st.is_search_active = false;
                    if st.active_view == ActiveView::AirTracks {
                        st.air_scroll = 0;
                        st.sync_air_selection();
                    }
                    self.audio.play(SoundEffect::Blip);
                }
                KeyCode::Backspace => {
                    st.search_query.pop();
                }
                KeyCode::Char(c) => {
                    st.search_query.push(c);
                }
                _ => {}
            }
            return;
        }

        if code == KeyCode::Char('/')
            && st.active_view != ActiveView::Notepad
            && st.active_view != ActiveView::Calculator
        {
            st.is_search_active = true;
            self.audio.play(SoundEffect::Click);
            return;
        }

        // 3. F-Tuşları ile Çalışma Alanı Değiştirme (Global Kısayollar)
        match code {
            KeyCode::F(1) => { st.set_view(ActiveView::Dashboard); self.audio.play(SoundEffect::Click); return; }
            KeyCode::F(2) => { st.set_view(ActiveView::Geoint); self.audio.play(SoundEffect::Click); return; }
            KeyCode::F(3) => {
                st.set_view(ActiveView::AirTracks);
                st.sync_air_selection();
                self.audio.play(SoundEffect::Click);
                return;
            }
            KeyCode::F(4) => { st.set_view(ActiveView::SeaTracks); self.audio.play(SoundEffect::Click); return; }
            KeyCode::F(5) => {
                if st.active_view == ActiveView::Notepad {
                    let stamp = format!("\n[{}] ", Utc::now().format("%H:%M %d/%m/%Y"));
                    st.notepad.content.push_str(&stamp);
                    self.audio.play(SoundEffect::Click);
                    return;
                } else {
                    st.set_view(ActiveView::Sigint);
                    self.audio.play(SoundEffect::Click);
                    return;
                }
            }
            KeyCode::F(6) => { st.set_view(ActiveView::Osint); self.audio.play(SoundEffect::Click); return; }
            KeyCode::F(7) => { st.set_view(ActiveView::Cyberint); self.audio.play(SoundEffect::Click); return; }
            KeyCode::F(8) => { st.set_view(ActiveView::Satint); self.audio.play(SoundEffect::Click); return; }
            KeyCode::F(9) => { st.set_view(ActiveView::Masint); self.audio.play(SoundEffect::Click); return; }
            KeyCode::F(10) => { st.set_view(ActiveView::Notepad); self.audio.play(SoundEffect::Click); return; }
            KeyCode::F(11) => { st.set_view(ActiveView::Calculator); self.audio.play(SoundEffect::Click); return; }
            KeyCode::F(12) | KeyCode::Char('?') => {
                Self::open_help_dialog(&mut st);
                self.audio.play(SoundEffect::Blip);
                return;
            }
            _ => {}
        }

        // 4. Özel Çalışma Alanı Girişleri (AirTracks, SeaTracks, Notepad, Calculator, Cyberint, Sigint)
        match st.active_view {
            ActiveView::AirTracks => {
                match code {
                    KeyCode::Esc => {
                        st.set_view(ActiveView::Dashboard);
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Char(c) if ('1'..='6').contains(&c) => {
                        Self::apply_air_filter(&mut st, (c as usize) - ('1' as usize));
                        st.sync_air_selection();
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Up => {
                        st.air_scroll = st.air_scroll.saturating_sub(1);
                        st.sync_air_selection();
                        return;
                    }
                    KeyCode::Down => {
                        st.air_scroll = st.air_scroll.saturating_add(1);
                        st.sync_air_selection();
                        return;
                    }
                    KeyCode::PageUp => {
                        st.air_scroll = st.air_scroll.saturating_sub(10);
                        st.sync_air_selection();
                        return;
                    }
                    KeyCode::PageDown => {
                        st.air_scroll = st.air_scroll.saturating_add(10);
                        st.sync_air_selection();
                        return;
                    }
                    KeyCode::Enter => {
                        let idx = st.air_scroll;
                        if let Some(t) = st.aircraft_at_visible(idx).cloned() {
                            Self::open_air_target_dossier(&mut st, &t);
                            self.audio.play(SoundEffect::Blip);
                        }
                        return;
                    }
                    _ => {}
                }
            }
            ActiveView::SeaTracks => {
                match code {
                    KeyCode::Esc => {
                        st.set_view(ActiveView::Dashboard);
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Char('1') => { st.sea_filter = 0; st.sea_scroll = 0; self.audio.play(SoundEffect::Click); return; }
                    KeyCode::Char('2') => { st.sea_filter = 1; st.sea_scroll = 0; self.audio.play(SoundEffect::Click); return; }
                    KeyCode::Char('3') => { st.sea_filter = 2; st.sea_scroll = 0; self.audio.play(SoundEffect::Click); return; }
                    KeyCode::Char('4') => { st.sea_filter = 3; st.sea_scroll = 0; self.audio.play(SoundEffect::Click); return; }
                    KeyCode::Up => { st.sea_scroll = st.sea_scroll.saturating_sub(1); return; }
                    KeyCode::Down => { st.sea_scroll = st.sea_scroll.saturating_add(1); return; }
                    KeyCode::PageUp => { st.sea_scroll = st.sea_scroll.saturating_sub(10); return; }
                    KeyCode::PageDown => { st.sea_scroll = st.sea_scroll.saturating_add(10); return; }
                    KeyCode::Enter => {
                        let idx = st.sea_scroll;
                        if Self::open_item_dossier(&mut st, 21, idx) {
                            self.audio.play(SoundEffect::Blip);
                        }
                        return;
                    }
                    _ => {}
                }
            }
            ActiveView::Notepad => {
                if modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('s') {
                    st.save_notepad();
                    self.audio.play(SoundEffect::Blip);
                    return;
                }
                match code {
                    KeyCode::Esc => {
                        st.set_view(ActiveView::Dashboard);
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Char(c) => {
                        st.notepad.content.push(c);
                        return;
                    }
                    KeyCode::Enter => {
                        st.notepad.content.push('\n');
                        return;
                    }
                    KeyCode::Backspace => {
                        st.notepad.content.pop();
                        return;
                    }
                    _ => {}
                }
            }
            ActiveView::Calculator => {
                match code {
                    KeyCode::Esc => {
                        st.set_view(ActiveView::Dashboard);
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Tab => {
                        st.calculator.conv_mode = (st.calculator.conv_mode + 1) % 4;
                        st.calc_convert();
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Char(c) if c.is_ascii_digit() || c == '.' || c == '+' || c == '-' || c == '*' || c == '/' || c == 'c' || c == 'C' => {
                        st.calc_input_char(c);
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Enter => {
                        st.calc_input_char('=');
                        self.audio.play(SoundEffect::Blip);
                        return;
                    }
                    _ => {}
                }
            }
            ActiveView::Osint => {
                match code {
                    KeyCode::Esc => {
                        st.set_view(ActiveView::Dashboard);
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Char(c) if ('0'..='4').contains(&c) => {
                        st.news_filter = (c as usize) - ('0' as usize);
                        st.news_scroll = 0;
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Up => {
                        st.news_scroll = st.news_scroll.saturating_sub(1);
                        return;
                    }
                    KeyCode::Down => {
                        st.news_scroll = st.news_scroll.saturating_add(1);
                        return;
                    }
                    KeyCode::PageUp => {
                        st.news_scroll = st.news_scroll.saturating_sub(10);
                        return;
                    }
                    KeyCode::PageDown => {
                        st.news_scroll = st.news_scroll.saturating_add(10);
                        return;
                    }
                    KeyCode::Enter => {
                        let idx = st.news_scroll;
                        if Self::open_item_dossier(&mut st, 1, idx) {
                            self.audio.play(SoundEffect::Blip);
                        }
                        return;
                    }
                    _ => {}
                }
            }
            ActiveView::Sigint => {
                if st.sdr_freq_input_active {
                    match code {
                        KeyCode::Esc => {
                            st.sdr_freq_input_active = false;
                            st.sdr_freq_input.clear();
                            self.audio.play(SoundEffect::Click);
                        }
                        KeyCode::Enter => {
                            if let Ok(freq) = st.sdr_freq_input.trim().parse::<f64>() {
                                if (100.0..=30000.0).contains(&freq) {
                                    st.current_frequency_khz = freq;
                                    st.live_status = format!("SDR FREKANS: {:.1} kHz {}", freq, st.sdr_mode);
                                    self.audio.play(SoundEffect::Blip);
                                }
                            }
                            st.sdr_freq_input_active = false;
                            st.sdr_freq_input.clear();
                        }
                        KeyCode::Backspace => {
                            st.sdr_freq_input.pop();
                        }
                        KeyCode::Char(c)
                            if (c.is_ascii_digit() || c == '.') && st.sdr_freq_input.len() < 8 =>
                        {
                            st.sdr_freq_input.push(c);
                        }
                        _ => {}
                    }
                    return;
                }
                match code {
                    KeyCode::Esc => {
                        st.set_view(ActiveView::Dashboard);
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Char('f') | KeyCode::Char('F') => {
                        st.sdr_freq_input_active = true;
                        st.sdr_freq_input.clear();
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Char('u') | KeyCode::Char('U') => {
                        st.sdr_mode = match st.sdr_mode.as_str() {
                            "USB" => "LSB",
                            "LSB" => "AM",
                            "AM" => "CW",
                            _ => "USB",
                        }
                        .to_string();
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Char('v') | KeyCode::Char('V') => {
                        st.sdr_recording = !st.sdr_recording;
                        if !st.sdr_recording && !st.is_radio_playing {
                            st.sdr_rec_path.clear();
                        }
                        self.audio.play(SoundEffect::Blip);
                        return;
                    }
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        st.is_radio_playing = !st.is_radio_playing;
                        Self::sync_radio_state(&self.audio, &st);
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Char('l') | KeyCode::Char('L') => {
                        st.sdr_auto_failover = !st.sdr_auto_failover;
                        st.sdr_status = if st.sdr_auto_failover {
                            "OTO-GECIS ACIK: Yanit vermeyen alicidan siradakine gecilir".to_string()
                        } else {
                            "ALICI KILITLI: Secili alicida uzun sureli baglanti denenecek".to_string()
                        };
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Char(d) if ('1'..='9').contains(&d) => {
                        let idx = (d as usize) - ('1' as usize);
                        if let Some((freq, mode)) = st.radio_presets.get(idx).map(|p| (p.frequency_khz, p.mode.clone())) {
                            st.current_frequency_khz = freq;
                            st.sdr_mode = mode;
                            Self::sync_radio_state(&self.audio, &st);
                            self.audio.play(SoundEffect::Click);
                        }
                        return;
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') => {
                        if !st.sdr_nodes.is_empty() {
                            st.active_sdr_idx = (st.active_sdr_idx + 1) % st.sdr_nodes.len();
                            st.sdr_scroll = st.active_sdr_idx;
                            self.audio.play(SoundEffect::Click);
                        }
                        return;
                    }
                    KeyCode::Tab | KeyCode::BackTab => {
                        st.sigint_focus = if st.sigint_focus == 0 { 1 } else { 0 };
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown | KeyCode::Home | KeyCode::End => {
                        let step: isize = match code {
                            KeyCode::Up => -1,
                            KeyCode::Down => 1,
                            KeyCode::PageUp => -10,
                            KeyCode::PageDown => 10,
                            KeyCode::Home => isize::MIN / 2,
                            _ => isize::MAX / 2,
                        };
                        if st.sigint_focus == 0 {
                            let max = st.sdr_nodes.len().saturating_sub(1) as isize;
                            st.sdr_scroll = (st.sdr_scroll as isize).saturating_add(step).clamp(0, max) as usize;
                        } else {
                            let max = st.eam_messages.len().saturating_sub(1) as isize;
                            st.eam_scroll = (st.eam_scroll as isize).saturating_add(step).clamp(0, max) as usize;
                            st.selected_domain = 30;
                            st.selected_row = st.eam_scroll;
                        }
                        return;
                    }
                    KeyCode::Enter => {
                        if st.sigint_focus == 0 {
                            let q_lower = st.search_query.to_lowercase();
                            let target_idx = st
                                .sdr_nodes
                                .iter()
                                .enumerate()
                                .filter(|(_, n)| {
                                    q_lower.is_empty()
                                        || n.host_port.to_lowercase().contains(&q_lower)
                                        || n.location.to_lowercase().contains(&q_lower)
                                        || n.name.to_lowercase().contains(&q_lower)
                                })
                                .nth(st.sdr_scroll)
                                .map(|(i, _)| i);
                            if let Some(real_idx) = target_idx {
                                st.active_sdr_idx = real_idx;
                                if !st.is_radio_playing {
                                    st.is_radio_playing = true;
                                    Self::sync_radio_state(&self.audio, &st);
                                }
                                self.audio.play(SoundEffect::Blip);
                            }
                        } else {
                            let idx = st.eam_scroll;
                            if Self::open_item_dossier(&mut st, 30, idx) {
                                self.audio.play(SoundEffect::Blip);
                            }
                        }
                        return;
                    }
                    _ => {}
                }
            }
            ActiveView::Masint => {
                match code {
                    KeyCode::Esc => {
                        st.set_view(ActiveView::Dashboard);
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Char('1') => { st.masint_tab = 0; st.geo_scroll = 0; self.audio.play(SoundEffect::Click); return; }
                    KeyCode::Char('2') => { st.masint_tab = 1; st.geo_scroll = 0; self.audio.play(SoundEffect::Click); return; }
                    KeyCode::Char('3') => { st.masint_tab = 2; st.radiation_scroll = 0; self.audio.play(SoundEffect::Click); return; }
                    KeyCode::Up => {
                        if st.masint_tab == 2 {
                            st.radiation_scroll = st.radiation_scroll.saturating_sub(1);
                        } else {
                            st.geo_scroll = st.geo_scroll.saturating_sub(1);
                        }
                        return;
                    }
                    KeyCode::Down => {
                        if st.masint_tab == 2 {
                            st.radiation_scroll = st.radiation_scroll.saturating_add(1);
                        } else {
                            st.geo_scroll = st.geo_scroll.saturating_add(1);
                        }
                        return;
                    }
                    KeyCode::Enter => {
                        let (dom, idx) = match st.masint_tab {
                            1 => (51, st.geo_scroll),
                            2 => (53, st.radiation_scroll),
                            _ => (50, st.geo_scroll),
                        };
                        if Self::open_item_dossier(&mut st, dom, idx) {
                            self.audio.play(SoundEffect::Blip);
                        }
                        return;
                    }
                    _ => {}
                }
            }
            ActiveView::Cyberint => {
                match code {
                    KeyCode::Enter => {
                        let target = st.shodan_input_ip.trim().to_string();
                        if !target.is_empty() {
                            let api_key = st.config.api_keys.shodan_key.clone();
                            st.live_status = format!("SHODAN TARANIYOR: {}...", target);
                            self.audio.play(SoundEffect::Blip);
                            drop(st);
                            let state_clone = Arc::clone(&self.state);
                            tokio::spawn(async move {
                                let client = argus_ingest::shodan::ShodanClient::new(api_key);
                                match client.lookup_host(&target).await {
                                    Ok(info) => {
                                        let mut lock = state_clone.write();
                                        let ports_str = if info.ports.is_empty() {
                                            "Acik port tespit edilmedi".to_string()
                                        } else {
                                            info.ports
                                                .iter()
                                                .map(|p| p.to_string())
                                                .collect::<Vec<_>>()
                                                .join(", ")
                                        };
                                        let hostnames_str = if info.hostnames.is_empty() {
                                            "-".to_string()
                                        } else {
                                            info.hostnames.join(", ")
                                        };
                                        let cpes_str = if info.cpes.is_empty() {
                                            "Tespit edilmedi".to_string()
                                        } else {
                                            info.cpes.join(", ")
                                        };
                                        let vulns_str = if info.vulns.is_empty() {
                                            "Bilinen kritik zafiyet (CVE) tespit edilmedi".to_string()
                                        } else {
                                            info.vulns.join(", ")
                                        };
                                        let tags_str = if info.tags.is_empty() {
                                            "-".to_string()
                                        } else {
                                            info.tags.join(", ")
                                        };
                                        let isp_str = info.isp.clone().unwrap_or_else(|| "Bilinmiyor".to_string());
                                        let loc_str = format!(
                                            "{}, {}",
                                            info.city.as_deref().unwrap_or("-"),
                                            info.country.as_deref().unwrap_or("-")
                                        );
                                        let resolved_ip = info.ip.clone();
                                        lock.shodan_recon_cache.insert(target.clone(), info.clone());
                                        lock.shodan_recon_cache.insert(resolved_ip.clone(), info);
                                        lock.live_status = format!("SHODAN TAMAMLANDI: {}", resolved_ip);
                                        lock.open_dialog(
                                            format!("Shodan Host Recon: {} ({})", target, resolved_ip),
                                            vec![
                                                format!("=== SHODAN INTERNETDB & GEOIP KESIF RAPORU: {} ===", target),
                                                format!("Cozumlenen Hedef IP  : {}", resolved_ip),
                                                format!("Host / Alan Adlari   : {}", hostnames_str),
                                                format!("ISP / Operator (ASN) : {}", isp_str),
                                                format!("Cografi Konum        : {}", loc_str),
                                                format!("Acik Portlar         : {}", ports_str),
                                                format!("Sistem Etiketleri    : {}", tags_str),
                                                format!("CPE Servisleri       : {}", cpes_str),
                                                format!("Zafiyetler (CVE)     : {}", vulns_str),
                                            ],
                                        );
                                    }
                                    Err(e) => {
                                        let mut lock = state_clone.write();
                                        lock.live_status = format!("SHODAN HATA: {}", target);
                                        lock.open_dialog(
                                            format!("Shodan Sorgu Hatasi: {}", target),
                                            vec![
                                                format!("Hedef sorgulanamadi: {}", target),
                                                format!("Hata detayi: {}", e),
                                            ],
                                        );
                                    }
                                }
                            });
                        }
                        return;
                    }
                    KeyCode::Backspace => {
                        st.shodan_input_ip.pop();
                        return;
                    }
                    KeyCode::Char(c) if c.is_ascii_alphanumeric() || c == '.' || c == '-' => {
                        st.shodan_input_ip.push(c);
                        return;
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        // 5. Genel Gezinme ve Kontroller
        match code {
            KeyCode::Char('q') | KeyCode::Char('Q') => self.should_quit = true,
            KeyCode::Char('f') | KeyCode::Char('F')
                if matches!(
                    st.active_view,
                    ActiveView::AirTracks | ActiveView::SeaTracks | ActiveView::Dashboard | ActiveView::Geoint
                ) =>
            {
                let air_pos = st.air_scroll;
                let sea_pos = st.sea_scroll;
                let target = match st.active_view {
                    ActiveView::AirTracks => st.aircraft_at_sorted(air_pos).cloned(),
                    ActiveView::SeaTracks => st.vessel_at_sorted(sea_pos).cloned(),
                    _ => st
                        .targets
                        .get(st.target_scroll)
                        .or_else(|| st.targets.get(0))
                        .cloned(),
                };
                if let Some(t) = target {
                    if st.follow_target_id.as_deref() == Some(t.id.as_str()) {
                        st.follow_target_id = None;
                        st.live_status = "TAKIP KILIDI ACILDI".to_string();
                    } else {
                        st.follow_target_id = Some(t.id.clone());
                        st.selected_target_id = Some(t.id.clone());
                        st.map_center_lat = t.geo.lat;
                        st.map_center_lon = t.geo.lon;
                        st.live_status = format!("TAKIP EDILIYOR: {} ({})", t.callsign, t.id);
                    }
                } else if st.follow_target_id.is_some() {
                    st.follow_target_id = None;
                    st.live_status = "TAKIP KILIDI ACILDI".to_string();
                }
                self.audio.play(SoundEffect::Blip);
            }
            KeyCode::Char('h') | KeyCode::Char('H') => {
                st.set_view(ActiveView::AirTracks);
                st.sync_air_selection();
                self.audio.play(SoundEffect::Click);
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                st.set_view(ActiveView::SeaTracks);
                self.audio.play(SoundEffect::Click);
            }
            KeyCode::Tab => {
                st.active_tile = (st.active_tile + 1) % 6;
                if st.zoomed_tile.is_some() {
                    st.zoomed_tile = Some(st.active_tile);
                }
                self.audio.play(SoundEffect::Click);
            }
            KeyCode::BackTab => {
                st.active_tile = if st.active_tile == 0 { 5 } else { st.active_tile - 1 };
                if st.zoomed_tile.is_some() {
                    st.zoomed_tile = Some(st.active_tile);
                }
                self.audio.play(SoundEffect::Click);
            }
            KeyCode::Char('1') => { st.active_tile = 0; if st.zoomed_tile.is_some() { st.zoomed_tile = Some(0); } self.audio.play(SoundEffect::Click); }
            KeyCode::Char('2') => { st.active_tile = 1; if st.zoomed_tile.is_some() { st.zoomed_tile = Some(1); } self.audio.play(SoundEffect::Click); }
            KeyCode::Char('3') => { st.active_tile = 2; if st.zoomed_tile.is_some() { st.zoomed_tile = Some(2); } self.audio.play(SoundEffect::Click); }
            KeyCode::Char('4') => { st.active_tile = 3; if st.zoomed_tile.is_some() { st.zoomed_tile = Some(3); } self.audio.play(SoundEffect::Click); }
            KeyCode::Char('5') => { st.active_tile = 4; if st.zoomed_tile.is_some() { st.zoomed_tile = Some(4); } self.audio.play(SoundEffect::Click); }
            KeyCode::Char('6') => { st.active_tile = 5; if st.zoomed_tile.is_some() { st.zoomed_tile = Some(5); } self.audio.play(SoundEffect::Click); }
            KeyCode::Char('z') | KeyCode::Char('Z') | KeyCode::Char(' ') => {
                let active = st.active_tile;
                st.toggle_zoom(active);
                self.audio.play(SoundEffect::Click);
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                st.audio_muted = !st.audio_muted;
                self.audio.set_muted(st.audio_muted);
                self.audio.play(SoundEffect::Click);
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                st.is_radio_playing = !st.is_radio_playing;
                Self::sync_radio_state(&self.audio, &st);
                self.audio.play(SoundEffect::Click);
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                st.map_high_contrast = !st.map_high_contrast;
                self.audio.play(SoundEffect::Click);
            }
            KeyCode::Char('v') | KeyCode::Char('V') => {
                st.map_style = st.map_style.next();
                st.live_status = format!("HARITA MODU: {}", st.map_style.label());
                self.audio.play(SoundEffect::Click);
            }
            KeyCode::Char('i') | KeyCode::Char('I') => {
                st.show_inspector = !st.show_inspector;
                self.audio.play(SoundEffect::Click);
            }
            KeyCode::Char('b') | KeyCode::Char('B') => {
                Self::toggle_selected_watchlist(&mut st);
                self.audio.play(SoundEffect::Blip);
            }
            KeyCode::Char('g') | KeyCode::Char('G') => {
                Self::focus_selected_on_map(&mut st);
                self.audio.play(SoundEffect::Blip);
            }
            KeyCode::Char('w') | KeyCode::Char('W') if st.active_view != ActiveView::Geoint => {
                Self::launch_web_map(&mut st);
                self.audio.play(SoundEffect::Blip);
            }
            KeyCode::Char('e') | KeyCode::Char('E') if modifiers.contains(KeyModifiers::CONTROL) => {
                let _ = st.export_intelligence_report();
                self.audio.play(SoundEffect::Blip);
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                st.map_zoom = (st.map_zoom * 1.25).min(50.0);
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                st.map_zoom = (st.map_zoom / 1.25).max(0.25);
            }
            KeyCode::Left | KeyCode::Char('a') => {
                st.map_center_lon = (st.map_center_lon - 5.0 / st.map_zoom).clamp(-180.0, 180.0);
            }
            KeyCode::Right | KeyCode::Char('s') if st.active_view != ActiveView::Sigint => {
                st.map_center_lon = (st.map_center_lon + 5.0 / st.map_zoom).clamp(-180.0, 180.0);
            }
            KeyCode::Up | KeyCode::Char('w') => {
                st.map_center_lat = (st.map_center_lat + 2.5 / st.map_zoom).clamp(-85.0, 85.0);
            }
            KeyCode::Down if st.active_view != ActiveView::Sigint => {
                st.map_center_lat = (st.map_center_lat - 2.5 / st.map_zoom).clamp(-85.0, 85.0);
            }
            KeyCode::Char('o') | KeyCode::Char('O') => {
                if let Some(t) = st.inspected_target().filter(|t| t.domain == Domain::Air) {
                    Self::open_air_target_dossier(&mut st, &t);
                    self.audio.play(SoundEffect::Blip);
                }
            }
            KeyCode::Enter => {
                let active = st.active_tile;
                let row_idx = match active {
                    1 => st.news_scroll,
                    2 => st.target_scroll,
                    3 => st.sat_scroll,
                    4 => st.cyber_scroll,
                    5 => st.geo_scroll,
                    _ => 0,
                };
                if Self::open_item_dossier(&mut st, active, row_idx) {
                    self.audio.play(SoundEffect::Blip);
                }
            }
            _ => {}
        }
    }

    async fn handle_mouse(&mut self, mouse: MouseEvent) {
        let col = mouse.column;
        let row = mouse.row;

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let mut st = self.state.write();

                // 0. Eğer Splash / BIOS açılış ekranı aktifse tıklama ile geç
                if st.is_splash_active || st.active_view == ActiveView::Splash {
                    st.dismiss_splash();
                    self.audio.play(SoundEffect::Click);
                    return;
                }

                let target = st.hit_tester.hit_test(col, row);

                if st.is_start_menu_open
                    && !matches!(target, Some(HitTarget::StartButton) | Some(HitTarget::StartMenuItem(_)))
                {
                    st.is_start_menu_open = false;
                }

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
                        HitTarget::WindowHeader(idx) | HitTarget::WindowBody(idx) => {
                            st.active_tile = idx;
                            if is_double_click {
                                st.toggle_zoom(idx);
                            }
                            self.audio.play(SoundEffect::Click);
                        }
                        HitTarget::WindowMaximize(idx) => {
                            st.toggle_zoom(idx);
                        }
                        HitTarget::WindowClose(idx) => {
                            st.toggle_minimize(idx);
                        }
                        HitTarget::StartButton => {
                            st.is_start_menu_open = !st.is_start_menu_open;
                        }
                        HitTarget::StartMenuItem(idx) => {
                            st.is_start_menu_open = false;
                            match idx {
                                0 => st.set_view(ActiveView::Dashboard),
                                1 => st.set_view(ActiveView::Geoint),
                                2 => st.set_view(ActiveView::AirTracks),
                                3 => st.set_view(ActiveView::SeaTracks),
                                4 => st.set_view(ActiveView::Sigint),
                                5 => st.set_view(ActiveView::Osint),
                                6 => st.set_view(ActiveView::Cyberint),
                                7 => st.set_view(ActiveView::Satint),
                                8 => st.set_view(ActiveView::Masint),
                                10 => st.set_view(ActiveView::Notepad),
                                11 => st.set_view(ActiveView::Calculator),
                                12 => {
                                    st.audio_muted = !st.audio_muted;
                                    self.audio.set_muted(st.audio_muted);
                                }
                                13 => st.map_high_contrast = !st.map_high_contrast,
                                14 => {
                                    st.open_dialog("PROJECT SENTINEL Istasyon Bilgisi", vec![
                                        "=== PROJECT SENTINEL: STRATEJİK İSTİHBARAT İSTASYONU ===".to_string(),
                                        "İstasyon Düzeyi      : Taktik & Stratejik Multi-INT Terminali".to_string(),
                                        "Entegre İstihbarat   : GEOINT, AIRINT, SEAINT, SIGINT, OSINT, CYBERINT, SATINT, MASINT".to_string(),
                                        "Canlı Veri Beslemeleri: OpenSky (ADS-B), AISStream (AIS), NASA FIRMS, USGS, CelesTrak, Shodan".to_string(),
                                        "Sinyal Dinleme       : HFGCS Küresel Komuta EAM Frekansları (11175 kHz USB)".to_string(),
                                        "Mimari Standart      : Retro Windows 98 POST/BIOS & Ceefax Teletext".to_string(),
                                    ]);
                                    self.audio.play(SoundEffect::Blip);
                                }
                                15 => self.should_quit = true,
                                _ => {}
                            }
                        }
                        HitTarget::TaskbarViewTab(view) => {
                            st.set_view(view);
                        }
                        HitTarget::TaskbarWindowTab(idx) => {
                            if st.minimized_tiles[idx] {
                                st.minimized_tiles[idx] = false;
                            }
                            st.active_tile = idx;
                            if st.zoomed_tile.is_some() {
                                st.zoomed_tile = Some(idx);
                            }
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
                            st.close_dialog();
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
                            st.news_filter = cat;
                            st.news_scroll = 0;
                            self.audio.play(SoundEffect::Click);
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
                        HitTarget::MasintTab(tid) => {
                            st.masint_tab = tid.min(2);
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
                        HitTarget::GenericButton(999) => {
                            st.dismiss_splash();
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
                    st.radiation_scroll = st.radiation_scroll.saturating_sub(2);
                    st.geo_scroll = st.geo_scroll.saturating_sub(2);
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
                } else if st.active_view == ActiveView::Geoint || st.active_tile == 0 || st.zoomed_tile == Some(0) {
                    st.map_zoom = (st.map_zoom * 1.25).min(50.0);
                } else {
                    match st.active_tile {
                        1 => st.news_scroll = st.news_scroll.saturating_sub(2),
                        2 => st.target_scroll = st.target_scroll.saturating_sub(2),
                        3 => st.sat_scroll = st.sat_scroll.saturating_sub(2),
                        4 => st.cyber_scroll = st.cyber_scroll.saturating_sub(2),
                        5 => st.geo_scroll = st.geo_scroll.saturating_sub(2),
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
                    st.radiation_scroll = st.radiation_scroll.saturating_add(2);
                    st.geo_scroll = st.geo_scroll.saturating_add(2);
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
                } else if st.active_view == ActiveView::Geoint || st.active_tile == 0 || st.zoomed_tile == Some(0) {
                    st.map_zoom = (st.map_zoom / 1.25).max(0.25);
                } else {
                    match st.active_tile {
                        1 => st.news_scroll = st.news_scroll.saturating_add(2),
                        2 => st.target_scroll = st.target_scroll.saturating_add(2),
                        3 => st.sat_scroll = st.sat_scroll.saturating_add(2),
                        4 => st.cyber_scroll = st.cyber_scroll.saturating_add(2),
                        5 => st.geo_scroll = st.geo_scroll.saturating_add(2),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn spawn_aircraft_photo(state_arc: &Arc<RwLock<AppState>>, st: &mut AppState, icao: &str) {
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

    fn open_air_target_dossier(st: &mut AppState, t: &TargetTrack) {
        st.selected_target_id = Some(t.id.clone());
        let alt_m = t.geo.alt_m.unwrap_or(0.0);
        let alt_ft = alt_m * 3.28084;
        let alt_fl = alt_ft / 100.0;
        let spd_kts = t.speed_kts.unwrap_or(0.0);
        let sqwk = t.squawk.as_deref().unwrap_or("----");

        st.open_dialog(
            format!("T-DOSSIER: {} [{}]", t.callsign, t.id),
            vec![
                format!("=== HAVA HEDEFI VE TRANSONDER TELEMETRISI: {} ===", t.callsign),
                format!("ICAO 24-Bit Hex Kodu : {}", t.id),
                format!("Telsiz Cagri Kodu     : {}", t.callsign),
                format!("Hedef Siniflandirma   : {}", if t.is_military { "[ASKERI GOREV PLATFORMU]" } else { "[SIVIL TICARI HAVACILIK]" }),
                format!("Kuvvet / Birlik       : {}", t.operator_branch.as_deref().unwrap_or("Sivil Hava Yollari")),
                format!("Platform / Govde Tipi : {}", t.airframe_or_vessel_class.as_deref().unwrap_or("Standart Yolcu/Kargo Govdesi")),
                format!("Tescil / Bayrak Ulke  : {}", t.operator_country.as_deref().unwrap_or("Bilinmeyen Ulke")),
                format!("Anlik Konum (GPS)     : {:.4}° N, {:.4}° E", t.geo.lat, t.geo.lon),
                format!("Barometrik Irtifa     : FL{:.0} ({:.0} metre / {:.0} feet)", alt_fl, alt_m, alt_ft),
                format!("Yer Hizi (Ground Spd) : {:.0} Knot ({:.0} km/h)", spd_kts, spd_kts * 1.852),
                format!("Dikey Tirmanis Hizi   : {:+0.0} ft/min", t.vertical_rate_fpm.unwrap_or(0.0)),
                format!("Ucus Rotasi (Track)   : {:.1}°", t.heading_deg.unwrap_or(0.0)),
                format!("Mode-S Squawk Kodu    : {}", sqwk),
                format!("Squawk Durum Analizi  : {}", match sqwk { "7700" => "[!] ACIL DURUM (EMERGENCY DECLARED)", "7600" => "[!] TELSİZ KAYBI (RADIO FAILURE)", "7500" => "[!] UCAK KACIRILMA (HIJACK ALARM)", _ => "Standart IFR/VFR Radar Kontrol" }),
                "--- STRATEJIK GOREV VE TEHDIT DEGERLENDIRMESI ---".to_string(),
                format!("Taktik Misyon Analizi : {}", if t.is_military { "Elektronik Harp (EW), Taktik Nakliye veya Muharip Hava Devriyesi (CAP)" } else { "Ticari Tarifeli Sefer / Uluslararasi Sivil Ucus" }),
                "Sinyal Bütünlüğü      : ADS-B 1090MHz Mode-S Dogrulandi (CRC OK)".to_string(),
                format!("Son Gorulme (ZULU)    : {}", t.last_seen.format("%Y-%m-%d %H:%M:%S UTC")),
            ],
        );
        st.dialog_aircraft_icao = Some(t.id.clone());
    }

    /// Derin, teknik ve GitHub-star seviyesinde çok bölümlü istihbarat dosyası açar
    fn open_item_dossier(st: &mut AppState, tile_idx: usize, row_idx: usize) -> bool {
        // 1. AirTracks Ekranından Tıklanan Uçak (tile_idx == 20)
        if tile_idx == 20 {
            if let Some(t) = st.aircraft_at_sorted(row_idx).cloned() {
                Self::open_air_target_dossier(st, &t);
                return true;
            }
        }

        // 2. SeaTracks Ekranından Tıklanan Gemi (tile_idx == 21)
        if tile_idx == 21 {
            if let Some(t) = st.vessel_at_sorted(row_idx).cloned() {
                st.selected_target_id = Some(t.id.clone());
                st.open_dialog(
                    format!("M-DOSSIER: {} [MMSI:{}]", t.callsign, t.id),
                    vec![
                        format!("=== DENIZ HARBI & AIS FILO TAKIP DOSYASI: {} ===", t.callsign),
                        format!("MMSI Tanim Numarasi   : {}", t.id),
                        format!("Gemi / Birlik Adi     : {}", t.callsign),
                        format!("Taktik Sinif          : {}", t.vessel_type.as_deref().unwrap_or("Bilinmeyen")),
                        format!("Komuta / Donanma      : {}", t.operator_branch.as_deref().unwrap_or("Ticari Denizcilik")),
                        format!("Gemi Govde Sinifi     : {}", t.airframe_or_vessel_class.as_deref().unwrap_or("Ticari Yuk/Kargo")),
                        format!("Bayrak Devleti (MID)  : {}", t.operator_country.as_deref().unwrap_or("Bilinmeyen")),
                        format!("Anlik Konum (WGS-84)  : {:.4}° N, {:.4}° E", t.geo.lat, t.geo.lon),
                        format!("Seyir Sürati (SOG)    : {:.1} Knot ({:.1} km/h)", t.speed_kts.unwrap_or(0.0), t.speed_kts.unwrap_or(0.0) * 1.852),
                        format!("Rotalanan Seyir (COG) : {:.1}°", t.heading_deg.unwrap_or(0.0)),
                        "--- BOGAZLAR VE STRATEJIK GECIS KORIDORU ---".to_string(),
                        format!("Bogaz / Koridor Rejimi: {}", if t.is_military { "Montro Bogazlar Sozlesmesi Md. 10-18 Kapsaminda Harp Gemisi Gecis Statüsü" } else { "Uluslararasi Ticari Gecis ve Trafik Ayrim Duzeni (TSS)" }),
                        "AIS Verici Tipi      : Class-A Taktik Deniz Transponderi (VHF 161.975 / 162.025 MHz)".to_string(),
                        format!("Son Rapor Zamani      : {}", t.last_seen.format("%Y-%m-%d %H:%M:%S UTC")),
                    ],
                );
                return true;
            }
        }

        // 3. Genel Paneller (Dashboard P100 - P600)
        match tile_idx {
            1 => {
                if let Some(news) = st.news.get(row_idx) {
                    st.open_dialog(
                        format!("N-DOSSIER: {}", news.source),
                        vec![
                            format!("=== ACIK KAYNAK VE KRIZ HABER BULTENI: {} ===", news.source),
                            format!("Haber Basligi         : {}", news.title),
                            format!("Istihbarat Kaynagi    : {}", news.source),
                            format!("Tehdit Onem Derecesi  : {}", news.severity.as_str()),
                            format!("Yayinlanma Zamani     : {}", news.published.format("%Y-%m-%d %H:%M:%S UTC")),
                            format!("Baglanti URI          : {}", news.link),
                            "--- KRIZ ANALIZI VE ONEMLI BASLIKLAR ---".to_string(),
                            "Bulten Ozeti          : Bolgesel jeopolitik riskler, jeostratejik askeri hareketlilik ve kriz gostergeleri izlenmektedir.".to_string(),
                        ],
                    );
                    return true;
                }
            }
            2 => {
                if let Some(t) = st.targets.get(row_idx).cloned() {
                    if t.domain == Domain::Air {
                        Self::open_air_target_dossier(st, &t);
                        return true;
                    }
                    let domain_name = match t.domain {
                        argus_core::types::Domain::Air => "HAVA (ADS-B)",
                        argus_core::types::Domain::Sea => "DENİZ (AIS)",
                        _ => "DİĞER",
                    };
                    st.open_dialog(
                        format!("TARGET-DOSSIER: {}", t.callsign),
                        vec![
                            format!("=== TAKTİK HEDEF DOSYASI: {} ===", t.callsign),
                            format!("Hedef Tanımlayıcı     : {}", t.id),
                            format!("Çağrı / Birlik Adı    : {}", t.callsign),
                            format!("İstihbarat Alanı      : {}", domain_name),
                            format!("Askeri Durum          : {}", if t.is_military { "[ASKERİ HEDEF]" } else { "[SİVİL / TİCARİ]" }),
                            format!("Kuvvet / Operatör     : {}", t.operator_branch.as_deref().unwrap_or("Belirtilmemiş")),
                            format!("Platform Tipi         : {}", t.airframe_or_vessel_class.as_deref().unwrap_or("Standart Platform")),
                            format!("Konum Koordinatları   : {:.4}° N, {:.4}° E", t.geo.lat, t.geo.lon),
                            format!("Hız (Speed)           : {:.1} Knot", t.speed_kts.unwrap_or(0.0)),
                            format!("Rota (Heading)        : {:.1}°", t.heading_deg.unwrap_or(0.0)),
                            format!("Squawk Kodu           : {}", t.squawk.as_deref().unwrap_or("----")),
                            format!("Detay Bilgisi         : {}", t.info),
                            format!("Son Görülme Zamanı    : {}", t.last_seen.format("%Y-%m-%d %H:%M:%S UTC")),
                        ],
                    );
                    return true;
                }
            }
            3 => {
                if let Some(sat) = st.satellites.get(row_idx) {
                    st.open_dialog(
                        format!("S-DOSSIER: {}", sat.name),
                        vec![
                            format!("=== YORUNGE VE UYDU TELEMETRISI: {} ===", sat.name),
                            format!("NORAD Katalog No      : {}", sat.norad_id),
                            format!("Yorunge Konumu        : {:.4}° N, {:.4}° E", sat.lat, sat.lon),
                            format!("Irtifa (Apogee/Perig) : {:.1} km", sat.alt_km),
                            format!("Yorunge Hizi (Vel)    : {:.2} km/s", sat.velocity_kms),
                            format!("Yer Istasyonu Azimut  : {:.1}°", sat.az_deg),
                            format!("Yukselis Acisi (Elev) : {:.1}°", sat.el_deg),
                            format!("Sonraki Gecis Zamani  : T-{} saniye", sat.next_pass_secs),
                            format!("Gecis Suresi (Pass)   : {} saniye", sat.pass_duration_secs),
                            "Propagasyon Modeli   : SGP4 / SDP4 CelesTrak TLE Matematiksel Modelleyici".to_string(),
                        ],
                    );
                    return true;
                }
            }
            30 => {
                let mut eams: Vec<_> = st.eam_messages.iter().collect();
                eams.reverse();
                if let Some(msg) = eams.get(row_idx) {
                    st.open_dialog(
                        "E-DOSSIER: HFGCS EAM / SKYKING MESAJ ANALIZI".to_string(),
                        vec![
                            format!("=== USAF HFGCS ACIL EYLEM MESAJI (EAM): {} ===", msg.preamble),
                            format!("Yakalama Zamani (UTC) : {}", msg.timestamp.format("%Y-%m-%d %H:%M:%S UTC")),
                            format!("Tasiyici Frekans      : {:.1} kHz ({})", msg.frequency_khz, msg.mode),
                            format!("Komuta Istasyonu      : {}", msg.auth_station),
                            format!("Mesaj Preamble Kodu   : {}", msg.preamble),
                            format!("Mesaj Uzunlugu        : {} Karakter", msg.length_chars),
                            format!("Sinyal / Gurultu (SNR): {:+.1} dB", msg.raw_signal_snr_db),
                            format!(
                                "Aciliyet Onceligi     : {}",
                                if msg.is_flash_override {
                                    "[!] FLASH OVERRIDE / SKYKING (YUKSEK ONCELIKLI KOMUTA)"
                                } else {
                                    "STANDART EAM YAYINI"
                                }
                            ),
                            "--- KRIPTOLU FONETIK KOD BLOKLARI ---".to_string(),
                            msg.content.clone(),
                        ],
                    );
                    return true;
                }
            }
            4 => {
                if let Some(inc) = st.cyber_incidents.get(row_idx) {
                    st.open_dialog(
                        format!("C-DOSSIER: {}", inc.title),
                        vec![
                            format!("=== SIBER TEHDIT VE OLAY RAPORU: {} ===", inc.title),
                            format!("Olay ID               : {}", inc.id),
                            format!("Olay Turu             : {}", inc.event_type),
                            format!("Baslik                : {}", inc.title),
                            format!("Hedef / Sektor        : {}", inc.target),
                            format!("Tehdit Seviyesi       : {}", inc.severity.as_str()),
                            format!("CVE Zafiyet Kodu      : {}", inc.cve.as_deref().unwrap_or("N/A")),
                            format!("ASN / Ag Bilgisi      : {}", inc.asn.as_deref().unwrap_or("N/A")),
                            format!("Tespit Zamani         : {}", inc.timestamp.format("%Y-%m-%d %H:%M:%S UTC")),
                            "--- TAVSIYE VE AKSIYON ---".to_string(),
                            "Cozum Onerisi         : Ilgili yazilim yamalarini ivedilikle uygulayin ve acik portlari guvenlik duvari ile izole edin.".to_string(),
                        ],
                    );
                    return true;
                }
            }
            50 => {
                if let Some(q) = st.earthquakes.get(row_idx) {
                    st.open_dialog(
                        format!("S-DOSSIER: M{:.1} - {}", q.mag, q.place),
                        vec![
                            format!("=== SİSMOLOJİK AFET ANALİZ DOSYASI: M {:.1} ===", q.mag),
                            format!("Deprem Büyüklüğü      : M {:.1} (Moment Magnitüd)", q.mag),
                            format!("Merkez Üssü Konumu    : {}", q.place),
                            format!("Odak Derinliği        : {:.1} km ({})", q.depth_km, if q.depth_km < 30.0 { "Sığ Odaklı - Yüksek Yüzey Sarsıntısı" } else { "Derin Odaklı" }),
                            format!("Koordinatlar          : {:.4}° N, {:.4}° E", q.lat, q.lon),
                            format!("Meydana Geliş Zamanı  : {}", q.time.format("%Y-%m-%d %H:%M:%S UTC")),
                            "--- JEOFİZİK TEHDİT VE LEVHA TEKTONİĞİ ---".to_string(),
                            format!("Fay ve Tektonik Kuşak : {}", if q.place.contains("Turkey") || q.place.contains("Turk") || q.place.contains("Greece") { "Anadolu Levhası / Ege Graben Sistemi / Alp-Himalaya Kuşağı" } else { "Küresel Sismik Kuşak / Pasifik Ateş Çemberi" }),
                            format!("Tahmini Mercalli (MMI): {}", match q.mag { m if m >= 7.0 => "MMI VIII-X (Yıkıcı / Çok Şiddetli)", m if m >= 5.5 => "MMI VI-VII (Kuvvetli)", _ => "MMI III-V (Hafif / Orta)" }),
                            format!("Tsunami Riski         : {}", if q.depth_km < 35.0 && q.mag >= 6.5 { "[!] TSUNAMİ DİKKAT VE ERKEN UYARI DEĞERLENDİRMESİ" } else { "Tsunami Riski Bulunmamaktadır" }),
                            format!("USGS Doğrulama URI    : {}", q.url),
                        ],
                    );
                    return true;
                }
            }
            51 => {
                if let Some(fire) = st.firms_fires.get(row_idx) {
                    st.open_dialog(
                        format!("F-DOSSIER: TERMAL [{:.2}N, {:.2}E]", fire.lat, fire.lon),
                        vec![
                            "=== NASA FIRMS TERMAL RADYASYON VE YANGIN ANALİZİ ===".to_string(),
                            format!("Koordinatlar          : {:.4}° N, {:.4}° E", fire.lat, fire.lon),
                            format!("Parlaklık Sıcaklığı   : {:.1} Kelvin ({:.1} °C)", fire.brightness, fire.brightness - 273.15),
                            format!("Tespit Tarihi / Saat  : {} {} UTC", fire.acq_date, fire.acq_time),
                            format!("Güvenilirlik Oranı    : %{}", fire.confidence),
                            "Algılayan Uydu Sensörü: NASA MODIS / VIIRS (Suomi-NPP / NOAA-20)".to_string(),
                            "Termal Anomali Sınıfı : Endüstriyel Parlama, Geniş Ölçekli Orman Yangını veya Patlama".to_string(),
                        ],
                    );
                    return true;
                }
            }
            52 => {
                if let Some(acled) = st.acled_events.get(row_idx) {
                    st.open_dialog(
                        format!("C-DOSSIER: {} [{}]", acled.event_type, acled.country),
                        vec![
                            format!("=== ACLED SİLAHLI ÇATIŞMA VE RİSK DOSYASI: {} ===", acled.event_type),
                            format!("Olay Yeri ve Ülke     : {}, {}", acled.location, acled.country),
                            format!("Olay Tarihi           : {}", acled.event_date),
                            format!("Taktik Olay Türü      : {} / {}", acled.event_type, acled.sub_event_type),
                            format!("Doğrulanmış Zayiat    : {} Kişi (Fatalities)", acled.fatalities),
                            format!("Birincil Taraf Aktörü : {}", acled.actor1),
                            format!("Karşı Taraf Aktörü    : {}", acled.actor2.as_deref().unwrap_or("Belirtilmedi")),
                            "--- OLAY İSTİHBARAT NOTU ---".to_string(),
                            format!("Olay Detayı           : {}", acled.notes),
                        ],
                    );
                    return true;
                }
            }
            53 => {
                if let Some(rad) = st.radiation_measurements.get(row_idx) {
                    let usvh = if rad.unit == "cpm" {
                        rad.value / 334.0
                    } else {
                        rad.value
                    };
                    let (status, advice) = if usvh >= 1.0 {
                        ("[!] KRİTİK ALARM (CBRN TEHLİKE)", "İvedi radyolojik tahliye ve KBRN müdahale protokolü tavsiye edilir.")
                    } else if usvh >= 0.3 {
                        ("[!] YÜKSELMİŞ FON (ELEVATED)", "Doğal fon üzerinde radyasyon artışı. Anomali izlemeye alındı.")
                    } else {
                        ("NORMAL FON (BACKGROUND SAFE)", "Güvenli doğal arka plan seviyesi. Herhangi bir radyolojik sızıntı tespit edilmedi.")
                    };

                    st.open_dialog(
                        format!("R-DOSSIER: SAFECAST SENSOR #{}", rad.id),
                        vec![
                            format!("=== SAFECAST ÇEVRESEL RADYASYON VE GEIGER VERİSİ #{} ===", rad.id),
                            format!("Sensör / İstasyon ID  : {}", rad.id),
                            format!("Ölçüm Cihaz Numarası  : {}", rad.device_id.map(|d| d.to_string()).unwrap_or_else(|| "Mobil Geiger-Müller".to_string())),
                            format!("Radyasyon Doz Hızı    : {:.4} µSv/h (Mikrosievert/saat)", usvh),
                            format!("Sensör Sayım Hızı     : {:.1} {} (Counts Per Minute)", rad.value, rad.unit),
                            format!("Radyolojik Durum      : {}", status),
                            format!("İstasyon / Bölge Adı  : {}", rad.location_name.as_deref().unwrap_or("Sabit / Mobil Çevresel İstasyon")),
                            format!("Koordinatlar (WGS-84) : {:.5}° N, {:.5}° E", rad.lat, rad.lon),
                            format!("Kayıt Zamanı (ZULU)   : {}", rad.captured_at.map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string()).unwrap_or_else(|| "N/A".to_string())),
                            "--- RADYOLOJİK GÜVENLİK VE CBRN RAPORU ---".to_string(),
                            format!("Taktik Değerlendirme  : {}", advice),
                            "Veri Kaynağı          : Safecast Uluslararası Açık Radyasyon Ölçüm Ağı (API v1)".to_string(),
                        ],
                    );
                    return true;
                }
            }
            5 => {
                let q_len = st.earthquakes.len();
                let f_len = st.firms_fires.len();

                if row_idx < q_len {
                    if let Some(q) = st.earthquakes.get(row_idx) {
                        st.open_dialog(
                            format!("S-DOSSIER: M{:.1} - {}", q.mag, q.place),
                            vec![
                                format!("=== SISMOLOJIK AFET ANALIZ DOSYASI: M {:.1} ===", q.mag),
                                format!("Deprem Buyuklugu      : M {:.1} (Moment Magnitud)", q.mag),
                                format!("Merkez Ussu Konumu    : {}", q.place),
                                format!("Odak Derinligi        : {:.1} km ({})", q.depth_km, if q.depth_km < 30.0 { "Sig Odakli - Yuksek Yuzey Sarsintisi" } else { "Derin Odakli" }),
                                format!("Koordinatlar          : {:.4}° N, {:.4}° E", q.lat, q.lon),
                                format!("Meydana Gelis Zamani  : {}", q.time.format("%Y-%m-%d %H:%M:%S UTC")),
                                "--- JEOFISIK TEHDIT VE LEVHA TEKTONIGI ---".to_string(),
                                format!("Fay ve Tektonik Kusak : {}", if q.place.contains("Turkey") || q.place.contains("Turk") || q.place.contains("Greece") { "Anadolu Levhasi / Ege Graben Sistemi / Alp-Himalaya Kusagi" } else { "Kuresel Sismik Kusak / Pasifik Ates Cemberi" }),
                                format!("Tahmini Mercalli (MMI): {}", match q.mag { m if m >= 7.0 => "MMI VIII-X (Yikici / Cok Siddetli)", m if m >= 5.5 => "MMI VI-VII (Kuvvetli)", _ => "MMI III-V (Hafif / Orta)" }),
                                format!("Tsunami Riski         : {}", if q.depth_km < 35.0 && q.mag >= 6.5 { "[!] TSUNAMI DIKKAT VE ERKEN UYARI DEGERLENDIRMESI" } else { "Tsunami Riski Bulunmamaktadir" }),
                                format!("USGS Dogrulama URI    : {}", q.url),
                            ],
                        );
                        return true;
                    }
                } else if row_idx < q_len + f_len {
                    if let Some(fire) = st.firms_fires.get(row_idx - q_len) {
                        st.open_dialog(
                            format!("F-DOSSIER: TERMAL [{:.2}N, {:.2}E]", fire.lat, fire.lon),
                            vec![
                                "=== NASA FIRMS TERMAL RADYASYON VE YANGIN ANALIZI ===".to_string(),
                                format!("Koordinatlar          : {:.4}° N, {:.4}° E", fire.lat, fire.lon),
                                format!("Parlaklik Sicakligi   : {:.1} Kelvin ({:.1} °C)", fire.brightness, fire.brightness - 273.15),
                                format!("Tespit Tarihi / Saat  : {} {} UTC", fire.acq_date, fire.acq_time),
                                format!("Guvenilirlik Orani    : %{}", fire.confidence),
                                "Algilayan Uydu Sensoru: NASA MODIS / VIIRS (Suomi-NPP / NOAA-20)".to_string(),
                                "Termal Anomali Sinifi : Endustriyel Parlama, Genis Olcekli Orman Yangini veya Patlama".to_string(),
                            ],
                        );
                        return true;
                    }
                } else if let Some(acled) = st.acled_events.get(row_idx - q_len - f_len) {
                    st.open_dialog(
                        format!("C-DOSSIER: {} [{}]", acled.event_type, acled.country),
                        vec![
                            format!("=== ACLED SILAHLI CATISMA VE RISK DOSYASI: {} ===", acled.event_type),
                            format!("Olay Yeri ve Ulke     : {}, {}", acled.location, acled.country),
                            format!("Olay Tarihi           : {}", acled.event_date),
                            format!("Taktik Olay Turu      : {} / {}", acled.event_type, acled.sub_event_type),
                            format!("Dogrulanmis Zayiat    : {} Kisi (Fatalities)", acled.fatalities),
                            format!("Birincil Taraf Aktoru : {}", acled.actor1),
                            format!("Karsi Taraf Aktoru    : {}", acled.actor2.as_deref().unwrap_or("Belirtilmedi")),
                            "--- OLAY ISTIHBARAT NOTU ---".to_string(),
                            format!("Olay Detayi           : {}", acled.notes),
                        ],
                    );
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    fn sync_radio_state(audio: &AudioManager, st: &AppState) {
        audio.set_radio_playing(st.is_radio_playing);
    }

    /// Hava filtresi uygular; CENTCOM filtrelerinde mini-haritayi CENTCOM harekat alanina,
    /// Kuresel filtrelerde dunya geneline, TR/EUCOM filtresinde Turkiye/Avrupa'ya odaklar
    fn apply_air_filter(st: &mut AppState, idx: usize) {
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

    fn select_item_for_inspector(st: &mut AppState, tile_idx: usize, row_idx: usize) {
        st.selected_domain = tile_idx;
        st.selected_row = row_idx;
        match tile_idx {
            20 => {
                if let Some(t) = st.aircraft_at_sorted(row_idx) {
                    st.selected_target_id = Some(t.id.clone());
                }
            }
            21 => {
                if let Some(t) = st.vessel_at_sorted(row_idx) {
                    st.selected_target_id = Some(t.id.clone());
                }
            }
            2 => {
                if let Some(t) = st.targets.get(row_idx) {
                    st.selected_target_id = Some(t.id.clone());
                }
            }
            _ => {}
        }
    }

    fn focus_selected_on_map(st: &mut AppState) {
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

    fn toggle_selected_follow(st: &mut AppState) {
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

    fn toggle_selected_watchlist(st: &mut AppState) {
        if let Some(id) = st.selected_target_id.clone() {
            if st.watchlist.contains(&id) {
                st.watchlist.remove(&id);
                st.live_status = format!("IZLEME LISTESINDEN CIKARILDI: {}", id);
            } else {
                st.watchlist.insert(id.clone());
                st.live_status = format!("IZLEME LISTESINE EKLENDI: {}", id);
            }
        }
    }

    fn launch_web_map(st: &mut AppState) {
        let _ = std::process::Command::new("xdg-open")
            .arg("http://127.0.0.1:8090")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        st.live_status = "CANLI WEB HARITA ACILDI: http://127.0.0.1:8090".to_string();
    }

    fn open_help_dialog(st: &mut AppState) {
        st.open_dialog(
            "PROJECT SENTINEL: KULLANICI KILAVUZU & KISAYOLLAR (F12 / ?)",
            vec![
                "=== 1. CALISMA ALANLARI (F-TUSLARI VEYA ALT GOREV CUBUGU) ===".to_string(),
                "> [F1] GENEL     : 6 Panelli Mozaik Komuta Konsolu (Z/Bosluk: Tam Ekran)".to_string(),
                "> [F2] HARITA    : Taktik Harita, Katmanlar & Istasyon Telemetrisi".to_string(),
                "> [F3] HAVA (H)  : Canli ADS-B Hava Sahasi Radari, Mesafe & Kerteriz".to_string(),
                "> [F4] DENIZ (D) : Canli AIS Deniz Filosu, Savas Gemileri & Bogazlar".to_string(),
                "> [F5] SIGINT    : Canli KiwiSDR WebSocket (/SND) HF Alicisi & EAM".to_string(),
                "> [F6] OSINT     : Kriz & Istihbarat Haber Bulteni".to_string(),
                "> [F7] SIBER     : Shodan InternetDB Host Tarayici, CISA KEV & IODA".to_string(),
                "> [F8] UYDU      : CelesTrak GP SGP4 Uydu Polar Radar Skobu & Gecisler".to_string(),
                "> [F9] AFET      : USGS/Kandilli Deprem, NASA FIRMS Termal & Safecast".to_string(),
                "> [F10/F11]      : Taktik Not Defteri (notes.txt) / Hesap Makinesi".to_string(),
                "--- 2. SAG DENETCI PANELI VE HARITA KONTROLLERI ---".to_string(),
                "> [Tek Tik]      : Tablo veya haritadaki hedefi Sag Denetci Panelinde inceler".to_string(),
                "> [Cift Tik/Ent] : Hedefin tam ekran dosyasini acar".to_string(),
                "> [I] Tusu       : Sag Kalici Denetci Panelini goster/gizle".to_string(),
                "> [W] veya [WEB] : http://127.0.0.1:8090 uzerinde canli Leaflet haritasini acar".to_string(),
                "> [G / F / B]    : Haritada Odakla (G) | Takip Kilidi (F) | Izleme Listesi (B)".to_string(),
                "--- 3. GERCEK KIWISDR KISA DALGA RADYO KONTROLLERI ---".to_string(),
                "> [S] veya Tikla : SIGINT ekraninda canli KiwiSDR WebSocket ses akisini baslat/durdur".to_string(),
                "> [1-7] Tuslari  : Frekans sec (11175 USB, 8992 USB, 4625 UVB-76, 5448 Pip)".to_string(),
                "> [N] veya Tikla : Aktif 0-30 MHz KiwiSDR anten sunucusunu degistir".to_string(),
                "> [Ctrl+E]       : Anlik istihbarat raporunu Markdown olarak disa aktar".to_string(),
            ],
        );
    }
}
