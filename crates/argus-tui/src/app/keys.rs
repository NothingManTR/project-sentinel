use super::App;
use crate::audio::SoundEffect;
use crate::widgets::command_palette::build_palette_entries;
use argus_core::types::{ActiveView, Domain};
use chrono::Utc;
use crossterm::event::{KeyCode, KeyModifiers};
use std::sync::Arc;

impl App {
    pub(super) async fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        if modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        let mut st = self.state.write();

        // 0. Komut Paleti (Ctrl+K) — herhangi bir ekrandan ac/kapa (U-1)
        if modifiers.contains(KeyModifiers::CONTROL) && matches!(code, KeyCode::Char('k') | KeyCode::Char('K')) {
            st.command_palette.is_open = !st.command_palette.is_open;
            st.command_palette.query.clear();
            st.command_palette.selected_idx = 0;
            self.audio.play(SoundEffect::Click);
            return;
        }

        if st.command_palette.is_open {
            match code {
                KeyCode::Esc => {
                    st.command_palette.is_open = false;
                    st.command_palette.query.clear();
                    self.audio.play(SoundEffect::Click);
                }
                KeyCode::Up => {
                    st.command_palette.selected_idx =
                        st.command_palette.selected_idx.saturating_sub(1);
                }
                KeyCode::Down => {
                    let max_i = build_palette_entries(&st).len().saturating_sub(1);
                    st.command_palette.selected_idx =
                        (st.command_palette.selected_idx + 1).min(max_i);
                }
                KeyCode::Backspace => {
                    st.command_palette.query.pop();
                    st.command_palette.selected_idx = 0;
                }
                KeyCode::Enter => {
                    let entries = build_palette_entries(&st);
                    let idx = st
                        .command_palette
                        .selected_idx
                        .min(entries.len().saturating_sub(1));
                    if let Some(entry) = entries.into_iter().nth(idx) {
                        self.execute_palette_command(&mut st, entry.command);
                        self.audio.play(SoundEffect::Blip);
                    }
                }
                KeyCode::Char(c) if st.command_palette.query.chars().count() < 48 => {
                    st.command_palette.query.push(c);
                    st.command_palette.selected_idx = 0;
                }
                _ => {}
            }
            return;
        }

        // 1. Eğer diyalog açıksa (AI soru + çok-turlu takip sorusu dahil) etkileşimleri yönet
        if st.is_dialog_open {
            if st.dialog_title.contains("GEMINI AI") && st.ai_prompt_input_active {
                match code {
                    KeyCode::Esc => {
                        st.ai_prompt_input_active = false;
                        self.audio.play(SoundEffect::Click);
                    }
                    KeyCode::Enter => {
                        st.ai_mode = 3;
                        st.ai_prompt_input_active = false;
                        Self::spawn_ai_sitrep(&self.state, &mut st, true);
                        self.audio.play(SoundEffect::Blip);
                    }
                    KeyCode::Backspace => {
                        st.ai_custom_prompt.pop();
                    }
                    KeyCode::Char(c) if st.ai_custom_prompt.chars().count() < 160 => {
                        st.ai_custom_prompt.push(c);
                    }
                    _ => {}
                }
                return;
            }

            if st.dialog_title.contains("GEMINI AI") && st.ai_followup_input_active {
                match code {
                    KeyCode::Esc => {
                        st.ai_followup_input_active = false;
                        self.audio.play(SoundEffect::Click);
                    }
                    KeyCode::Enter => {
                        let q = std::mem::take(&mut st.ai_followup_input);
                        st.ai_followup_input_active = false;
                        Self::spawn_ai_followup(&self.state, &mut st, q);
                        self.audio.play(SoundEffect::Blip);
                    }
                    KeyCode::Backspace => {
                        st.ai_followup_input.pop();
                    }
                    KeyCode::Char(c) if st.ai_followup_input.chars().count() < 180 => {
                        st.ai_followup_input.push(c);
                    }
                    _ => {}
                }
                return;
            }

            let is_ai_dialog = st.dialog_title.contains("GEMINI AI");
            match code {
                KeyCode::Esc | KeyCode::Enter => {
                    st.ai_prompt_input_active = false;
                    st.ai_followup_input_active = false;
                    st.close_dialog();
                    self.audio.play(SoundEffect::Click);
                }
                KeyCode::Char('s') if modifiers.contains(KeyModifiers::CONTROL) => {
                    if let Some(path) = st.save_dialog_to_markdown() {
                        st.dialog_content.push(format!("[*] DOSYAYA KAYDEDILDI: {}", path));
                    }
                    self.audio.play(SoundEffect::Blip);
                }
                KeyCode::Char('k') | KeyCode::Char('K') => {
                    if let Some(path) = st.save_dialog_to_markdown() {
                        st.dialog_content.push(format!("[*] DOSYAYA KAYDEDILDI: {}", path));
                    }
                    self.audio.play(SoundEffect::Blip);
                }
                KeyCode::Char('h') | KeyCode::Char('H') => {
                    if let Some(path) = st.export_html_briefing_deck() {
                        st.dialog_content.push(format!("[*] HTML BRIFING KAYDEDILDI: {}", path));
                    }
                    self.audio.play(SoundEffect::Blip);
                }
                KeyCode::Char('e') | KeyCode::Char('E') => {
                    Self::append_dialog_to_notepad(&mut st);
                    self.audio.play(SoundEffect::Blip);
                }
                KeyCode::Char(c) if is_ai_dialog && (('1'..='3').contains(&c) || c == '5' || c == '6') => {
                    let m = match c {
                        '5' => 4,
                        '6' => 5,
                        _ => (c as usize) - ('1' as usize),
                    };
                    st.ai_mode = m;
                    Self::spawn_ai_sitrep(&self.state, &mut st, true);
                    self.audio.play(SoundEffect::Blip);
                }
                KeyCode::Char('4') if is_ai_dialog => {
                    st.ai_mode = 3;
                    st.ai_followup_input_active = false;
                    st.ai_prompt_input_active = true;
                    self.audio.play(SoundEffect::Click);
                }
                KeyCode::Char('f') | KeyCode::Char('F') if is_ai_dialog => {
                    st.ai_prompt_input_active = false;
                    st.ai_followup_input_active = true;
                    self.audio.play(SoundEffect::Click);
                }
                KeyCode::Char('r') | KeyCode::Char('R') if is_ai_dialog => {
                    Self::spawn_ai_sitrep(&self.state, &mut st, true);
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

        // 2. GEOINT Koordinata Git (Go-To Lat,Lon) giris modu (U-2)
        if st.geoint_goto_active {
            match code {
                KeyCode::Esc => {
                    st.geoint_goto_active = false;
                    self.audio.play(SoundEffect::Click);
                }
                KeyCode::Enter => {
                    if Self::apply_geoint_goto(&mut st) {
                        self.audio.play(SoundEffect::Blip);
                    } else {
                        self.audio.play(SoundEffect::Click);
                    }
                }
                KeyCode::Backspace => {
                    st.geoint_goto_input.pop();
                }
                KeyCode::Char(c)
                    if (c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | ' ' | ';' | '/'))
                        && st.geoint_goto_input.len() < 28 =>
                {
                    st.geoint_goto_input.push(c);
                }
                _ => {}
            }
            return;
        }

        // 2.5 Canlı Arama Çubuğu Modu (U-3)
        if st.is_search_active {
            match code {
                KeyCode::Esc => {
                    st.search_query.clear();
                    st.is_search_active = false;
                    if st.active_view == ActiveView::AirTracks {
                        st.air_scroll = 0;
                        st.sync_air_selection();
                    } else if st.active_view == ActiveView::Osint {
                        let cat = st.news_filter;
                        Self::apply_osint_category(&mut st, cat);
                    }
                    self.audio.play(SoundEffect::Click);
                }
                KeyCode::Enter => {
                    st.is_search_active = false;
                    if st.active_view == ActiveView::AirTracks {
                        st.air_scroll = 0;
                        st.sync_air_selection();
                    } else if st.active_view == ActiveView::Osint {
                        let cat = st.news_filter;
                        Self::apply_osint_category(&mut st, cat);
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

        if (code == KeyCode::Char('/')
            && st.active_view != ActiveView::Calculator
            && (st.active_view != ActiveView::Notepad || st.notepad.focus_sidebar))
            || (modifiers.contains(KeyModifiers::CONTROL) && matches!(code, KeyCode::Char('f') | KeyCode::Char('F')))
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
                if modifiers.contains(KeyModifiers::CONTROL) {
                    match code {
                        KeyCode::Char('s') | KeyCode::Char('S') => {
                            Self::run_notepad_action(&mut st, 0);
                            self.audio.play(SoundEffect::Blip);
                            return;
                        }
                        KeyCode::Char('t') | KeyCode::Char('T') => {
                            Self::run_notepad_action(&mut st, 2);
                            self.audio.play(SoundEffect::Blip);
                            return;
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') => {
                            Self::run_notepad_action(&mut st, 3);
                            self.audio.play(SoundEffect::Blip);
                            return;
                        }
                        KeyCode::Char('h') | KeyCode::Char('H') => {
                            Self::run_notepad_action(&mut st, 4);
                            self.audio.play(SoundEffect::Blip);
                            return;
                        }
                        KeyCode::Char('d') | KeyCode::Char('D') => {
                            Self::run_notepad_action(&mut st, 5);
                            self.audio.play(SoundEffect::Blip);
                            return;
                        }
                        _ => {}
                    }
                }
                match code {
                    KeyCode::Esc => {
                        st.set_view(ActiveView::Dashboard);
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Tab | KeyCode::BackTab => {
                        st.notepad.focus_sidebar = !st.notepad.focus_sidebar;
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Char('A') => {
                        Self::spawn_ai_sitrep(&self.state, &mut st, false);
                        self.audio.play(SoundEffect::Blip);
                        return;
                    }
                    KeyCode::Delete if st.notepad.focus_sidebar => {
                        Self::run_notepad_action(&mut st, 5);
                        self.audio.play(SoundEffect::Blip);
                        return;
                    }
                    KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown | KeyCode::Home | KeyCode::End => {
                        if st.notepad.focus_sidebar {
                            let max = st.notepad.files.len().saturating_sub(1) as isize;
                            let delta: isize = match code {
                                KeyCode::Up => -1,
                                KeyCode::Down => 1,
                                KeyCode::PageUp => -8,
                                KeyCode::PageDown => 8,
                                KeyCode::Home => isize::MIN / 2,
                                _ => isize::MAX / 2,
                            };
                            let next = (st.notepad.selected_file_idx as isize)
                                .saturating_add(delta)
                                .clamp(0, max) as usize;
                            st.select_notepad_file(next);
                        } else {
                            let step: u16 = match code {
                                KeyCode::Up | KeyCode::Down => 1,
                                _ => 10,
                            };
                            match code {
                                KeyCode::Up | KeyCode::PageUp => st.notepad.scroll = st.notepad.scroll.saturating_sub(step),
                                KeyCode::Home => st.notepad.scroll = 0,
                                KeyCode::End => st.notepad.scroll = st.notepad.content.lines().count().min(u16::MAX as usize) as u16,
                                _ => st.notepad.scroll = st.notepad.scroll.saturating_add(step),
                            }
                        }
                        return;
                    }
                    KeyCode::Enter if st.notepad.focus_sidebar => {
                        let idx = st.notepad.selected_file_idx;
                        st.select_notepad_file(idx);
                        st.notepad.focus_sidebar = false;
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Char(c) if !st.notepad.focus_sidebar && !st.notepad.active_file.ends_with(".html") => {
                        st.notepad.content.push(c);
                        return;
                    }
                    KeyCode::Enter if !st.notepad.focus_sidebar && !st.notepad.active_file.ends_with(".html") => {
                        st.notepad.content.push('\n');
                        return;
                    }
                    KeyCode::Backspace if !st.notepad.focus_sidebar && !st.notepad.active_file.ends_with(".html") => {
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
                    KeyCode::Char(c) if ('0'..='5').contains(&c) => {
                        let cat = (c as usize) - ('0' as usize);
                        Self::apply_osint_category(&mut st, cat);
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown => {
                        let delta: isize = match code {
                            KeyCode::Up => -1,
                            KeyCode::Down => 1,
                            KeyCode::PageUp => -10,
                            _ => 10,
                        };
                        if st.news_filter == 5 {
                            let filtered = Self::filtered_acled_indices(&st);
                            let max_pos = filtered.len().saturating_sub(1);
                            let next_pos = Self::step_scroll(st.news_scroll, delta, max_pos);
                            let orig_idx = filtered.get(next_pos).copied().unwrap_or(0);
                            Self::select_item_for_inspector(&mut st, 52, orig_idx);
                            st.news_scroll = next_pos;
                        } else {
                            let filtered = Self::filtered_news_indices(&st);
                            let max_pos = filtered.len().saturating_sub(1);
                            let next_pos = Self::step_scroll(st.news_scroll, delta, max_pos);
                            st.news_scroll = next_pos;
                            let orig_idx = filtered.get(next_pos).copied().unwrap_or(0);
                            Self::select_item_for_inspector(&mut st, 1, orig_idx);
                        }
                        return;
                    }
                    KeyCode::Enter => {
                        if st.news_filter == 5 {
                            let filtered = Self::filtered_acled_indices(&st);
                            if let Some(&idx) = filtered.get(st.news_scroll.min(filtered.len().saturating_sub(1))) {
                                if Self::open_item_dossier(&mut st, 52, idx) {
                                    self.audio.play(SoundEffect::Blip);
                                }
                            }
                        } else {
                            let filtered = Self::filtered_news_indices(&st);
                            if let Some(&orig_idx) = filtered.get(st.news_scroll.min(filtered.len().saturating_sub(1))) {
                                if Self::open_item_dossier(&mut st, 1, orig_idx) {
                                    self.audio.play(SoundEffect::Blip);
                                }
                            }
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
                    KeyCode::Char('t') | KeyCode::Char('T') => {
                        Self::toggle_sdr_scanner(&mut st);
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Char('x') | KeyCode::Char('X') => {
                        Self::toggle_sdr_vox(&mut st);
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Char('c') | KeyCode::Char('C') => {
                        Self::toggle_sdr_cw(&mut st);
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
                    KeyCode::Char(c) if ('1'..='5').contains(&c) => {
                        let tab = (c as usize) - ('1' as usize);
                        Self::apply_masint_tab(&mut st, tab);
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown => {
                        let delta: isize = match code {
                            KeyCode::Up => -1,
                            KeyCode::Down => 1,
                            KeyCode::PageUp => -10,
                            _ => 10,
                        };
                        if st.masint_tab == 4 {
                            let a_len = st.alerts.len();
                            let f_len = st.fusion_clusters.len();
                            let total = a_len + f_len;
                            let max_pos = total.saturating_sub(1);
                            st.alert_scroll = Self::step_scroll(st.alert_scroll, delta, max_pos);
                            if st.alert_scroll < a_len {
                                let orig_idx = a_len.saturating_sub(1 + st.alert_scroll);
                                let _ = Self::handle_alert_click(&mut st, orig_idx, false);
                            } else if f_len > 0 {
                                let fc_idx = (st.alert_scroll - a_len).min(f_len - 1);
                                let _ = Self::focus_fusion_cluster(&mut st, fc_idx, false);
                            }
                            return;
                        }
                        let (dom, max_pos) = match st.masint_tab {
                            1 => (51, st.firms_fires.len().saturating_sub(1)),
                            2 => (53, st.radiation_measurements.len().saturating_sub(1)),
                            3 => (54, st.airspace_warnings.len().saturating_sub(1)),
                            _ => (50, st.earthquakes.len().saturating_sub(1)),
                        };
                        let idx = if st.masint_tab == 2 {
                            st.radiation_scroll = Self::step_scroll(st.radiation_scroll, delta, max_pos);
                            st.radiation_scroll
                        } else {
                            st.geo_scroll = Self::step_scroll(st.geo_scroll, delta, max_pos);
                            st.geo_scroll
                        };
                        Self::select_item_for_inspector(&mut st, dom, idx);
                        return;
                    }
                    KeyCode::Enter => {
                        if st.masint_tab == 4 {
                            let a_len = st.alerts.len();
                            let f_len = st.fusion_clusters.len();
                            let total = a_len + f_len;
                            if total > 0 {
                                let scroll = st.alert_scroll.min(total - 1);
                                if scroll < a_len {
                                    let orig_idx = a_len.saturating_sub(1 + scroll);
                                    if let Some(sfx) = Self::handle_alert_click(&mut st, orig_idx, true) {
                                        self.audio.play(sfx);
                                    }
                                } else {
                                    let fc_idx = scroll - a_len;
                                    if Self::focus_fusion_cluster(&mut st, fc_idx, true) {
                                        self.audio.play(SoundEffect::Blip);
                                    }
                                }
                            }
                            return;
                        }
                        let (dom, scroll, len) = match st.masint_tab {
                            1 => (51, st.geo_scroll, st.firms_fires.len()),
                            2 => (53, st.radiation_scroll, st.radiation_measurements.len()),
                            3 => (54, st.geo_scroll, st.airspace_warnings.len()),
                            _ => (50, st.geo_scroll, st.earthquakes.len()),
                        };
                        if len > 0 {
                            let idx = scroll.min(len.saturating_sub(1));
                            if Self::open_item_dossier(&mut st, dom, idx) {
                                self.audio.play(SoundEffect::Blip);
                            }
                        }
                        return;
                    }
                    _ => {}
                }
            }
            ActiveView::Satint => {
                match code {
                    KeyCode::Esc => {
                        st.set_view(ActiveView::Dashboard);
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Up => {
                        st.sat_scroll = st.sat_scroll.saturating_sub(1);
                        st.selected_domain = 3;
                        st.selected_row = st.sat_scroll;
                        return;
                    }
                    KeyCode::Down => {
                        let max_pos = st.satellites.len().saturating_sub(1);
                        st.sat_scroll = (st.sat_scroll + 1).min(max_pos);
                        st.selected_domain = 3;
                        st.selected_row = st.sat_scroll;
                        return;
                    }
                    KeyCode::PageUp => {
                        st.sat_scroll = st.sat_scroll.saturating_sub(10);
                        st.selected_domain = 3;
                        st.selected_row = st.sat_scroll;
                        return;
                    }
                    KeyCode::PageDown => {
                        let max_pos = st.satellites.len().saturating_sub(1);
                        st.sat_scroll = (st.sat_scroll + 10).min(max_pos);
                        st.selected_domain = 3;
                        st.selected_row = st.sat_scroll;
                        return;
                    }
                    KeyCode::Enter => {
                        let idx = st.sat_scroll;
                        if Self::open_item_dossier(&mut st, 3, idx) {
                            self.audio.play(SoundEffect::Blip);
                        }
                        return;
                    }
                    _ => {}
                }
            }
            ActiveView::Cyberint => {
                match code {
                    KeyCode::Esc => {
                        st.set_view(ActiveView::Dashboard);
                        self.audio.play(SoundEffect::Click);
                        return;
                    }
                    KeyCode::Up => {
                        st.cyber_scroll = st.cyber_scroll.saturating_sub(1);
                        return;
                    }
                    KeyCode::Down => {
                        let max_pos = st.cyber_incidents.len().saturating_sub(1);
                        st.cyber_scroll = (st.cyber_scroll + 1).min(max_pos);
                        return;
                    }
                    KeyCode::PageUp => {
                        st.cyber_scroll = st.cyber_scroll.saturating_sub(10);
                        return;
                    }
                    KeyCode::PageDown => {
                        let max_pos = st.cyber_incidents.len().saturating_sub(1);
                        st.cyber_scroll = (st.cyber_scroll + 10).min(max_pos);
                        return;
                    }
                    KeyCode::Char('o') if modifiers.contains(KeyModifiers::CONTROL) => {
                        let idx = st.cyber_scroll;
                        if Self::open_item_dossier(&mut st, 4, idx) {
                            self.audio.play(SoundEffect::Blip);
                        }
                        return;
                    }
                    KeyCode::Enter => {
                        let target = st.shodan_input_ip.trim().to_string();
                        if !target.is_empty() {
                            let api_key = st.config.api_keys.shodan_key.clone();
                            st.live_status = format!("HIBRIT KESIF TARANIYOR: {}...", target);
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
                                        lock.live_status = format!("KESIF TAMAMLANDI: {}", resolved_ip);
                                        lock.open_dialog(
                                            format!("Hibrit Host Recon: {} ({})", target, resolved_ip),
                                            vec![
                                                format!("=== HIBRIT SIBER, RIPE BGP & CANLI PORT KESIF RAPORU: {} ===", target),
                                                format!("Cozumlenen Hedef IP  : {}", resolved_ip),
                                                format!("Host / Alan Adlari   : {}", hostnames_str),
                                                format!("ISP / Operator (ASN) : {}", isp_str),
                                                format!("Cografi Konum        : {}", loc_str),
                                                format!("Acik Portlar         : {}", ports_str),
                                                format!("BGP / Ag Etiketleri  : {}", tags_str),
                                                format!("Servis Banner / CPE  : {}", cpes_str),
                                                format!("Zafiyetler (CVE)     : {}", vulns_str),
                                            ],
                                        );
                                    }
                                    Err(e) => {
                                        let mut lock = state_clone.write();
                                        lock.live_status = format!("KESIF HATA: {}", target);
                                        lock.open_dialog(
                                            format!("Host Sorgu Hatasi: {}", target),
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
            KeyCode::Esc if st.active_view == ActiveView::Dashboard && st.zoomed_tile.is_some() => {
                st.zoomed_tile = None;
                self.audio.play(SoundEffect::Click);
            }
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
            KeyCode::Char('p') | KeyCode::Char('P') => {
                Self::toggle_map_aircraft_shapes(&mut st);
                self.audio.play(SoundEffect::Click);
            }
            KeyCode::Char('h') | KeyCode::Char('H') => {
                st.set_view(ActiveView::AirTracks);
                st.sync_air_selection();
                self.audio.play(SoundEffect::Click);
            }
            KeyCode::Char('D') => {
                st.set_view(ActiveView::SeaTracks);
                self.audio.play(SoundEffect::Click);
            }
            KeyCode::Char('d') if !matches!(st.active_view, ActiveView::Dashboard | ActiveView::Geoint) => {
                st.set_view(ActiveView::SeaTracks);
                self.audio.play(SoundEffect::Click);
            }
            KeyCode::Tab => {
                st.active_tile = (st.active_tile + 1) % 7;
                if st.zoomed_tile.is_some() {
                    st.zoomed_tile = Some(st.active_tile);
                }
                self.audio.play(SoundEffect::Click);
            }
            KeyCode::BackTab => {
                st.active_tile = if st.active_tile == 0 { 6 } else { st.active_tile - 1 };
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
            KeyCode::Char('7') => { st.active_tile = 6; if st.zoomed_tile.is_some() { st.zoomed_tile = Some(6); } self.audio.play(SoundEffect::Click); }
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
                if st.active_view == ActiveView::Geoint {
                    st.geoint_goto_active = true;
                    st.geoint_goto_input.clear();
                    self.audio.play(SoundEffect::Click);
                } else {
                    Self::focus_selected_on_map(&mut st);
                    self.audio.play(SoundEffect::Blip);
                }
            }
            KeyCode::Char('W') => {
                Self::launch_web_map(&mut st);
                self.audio.play(SoundEffect::Blip);
            }
            KeyCode::Char('e') | KeyCode::Char('E') if modifiers.contains(KeyModifiers::CONTROL) => {
                let _ = st.export_intelligence_report();
                self.audio.play(SoundEffect::Blip);
            }
            KeyCode::Char('A') => {
                Self::spawn_ai_sitrep(&self.state, &mut st, false);
                self.audio.play(SoundEffect::Blip);
            }
            KeyCode::Char('a') if modifiers.contains(KeyModifiers::CONTROL) => {
                Self::spawn_ai_sitrep(&self.state, &mut st, false);
                self.audio.play(SoundEffect::Blip);
            }
            KeyCode::Char('+') | KeyCode::Char('=')
                if matches!(st.active_view, ActiveView::Dashboard | ActiveView::Geoint) =>
            {
                st.map_zoom = (st.map_zoom * 1.25).min(50.0);
            }
            KeyCode::Char('-') | KeyCode::Char('_')
                if matches!(st.active_view, ActiveView::Dashboard | ActiveView::Geoint) =>
            {
                st.map_zoom = (st.map_zoom / 1.25).max(0.25);
            }
            KeyCode::Left | KeyCode::Char('a')
                if matches!(st.active_view, ActiveView::Dashboard | ActiveView::Geoint) =>
            {
                st.map_center_lon = (st.map_center_lon - 5.0 / st.map_zoom).clamp(-180.0, 180.0);
            }
            KeyCode::Right | KeyCode::Char('d')
                if matches!(st.active_view, ActiveView::Dashboard | ActiveView::Geoint) =>
            {
                st.map_center_lon = (st.map_center_lon + 5.0 / st.map_zoom).clamp(-180.0, 180.0);
            }
            KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown
                if st.active_view == ActiveView::Dashboard && st.active_tile == 6 =>
            {
                let delta: isize = match code {
                    KeyCode::Up => -1,
                    KeyCode::Down => 1,
                    KeyCode::PageUp => -8,
                    _ => 8,
                };
                let a_len = st.alerts.len();
                let f_len = st.fusion_clusters.len();
                let max_pos = (a_len + f_len).saturating_sub(1);
                st.alert_scroll = Self::step_scroll(st.alert_scroll, delta, max_pos);
                if st.alert_scroll < a_len {
                    let orig_idx = a_len.saturating_sub(1 + st.alert_scroll);
                    let _ = Self::handle_alert_click(&mut st, orig_idx, false);
                } else if f_len > 0 {
                    let fc_idx = (st.alert_scroll - a_len).min(f_len - 1);
                    let _ = Self::focus_fusion_cluster(&mut st, fc_idx, false);
                }
            }
            KeyCode::Up | KeyCode::Char('w')
                if matches!(st.active_view, ActiveView::Dashboard | ActiveView::Geoint) =>
            {
                st.map_center_lat = (st.map_center_lat + 2.5 / st.map_zoom).clamp(-85.0, 85.0);
            }
            KeyCode::Down | KeyCode::Char('s')
                if matches!(st.active_view, ActiveView::Dashboard | ActiveView::Geoint) =>
            {
                st.map_center_lat = (st.map_center_lat - 2.5 / st.map_zoom).clamp(-85.0, 85.0);
            }
            KeyCode::Char('o') | KeyCode::Char('O') => {
                let dom = st.selected_domain;
                let row = st.selected_row;
                if matches!(dom, 1 | 3 | 4 | 30 | 50 | 51 | 52 | 53 | 54 | 55 | 56) {
                    if Self::open_item_dossier(&mut st, dom, row) {
                        self.audio.play(SoundEffect::Blip);
                    }
                } else if let Some(t) = st.inspected_target() {
                    if t.domain == Domain::Air {
                        Self::open_air_target_dossier(&mut st, &t);
                        self.audio.play(SoundEffect::Blip);
                    } else {
                        let pos_opt = st.targets.iter().position(|x| x.id == t.id);
                        if let Some(pos) = pos_opt {
                            if Self::open_item_dossier(&mut st, 2, pos) {
                                self.audio.play(SoundEffect::Blip);
                            }
                        }
                    }
                }
            }
            KeyCode::Enter => {
                let active = st.active_tile;
                if active == 6 {
                    let a_len = st.alerts.len();
                    let f_len = st.fusion_clusters.len();
                    let total = a_len + f_len;
                    if total > 0 {
                        let scroll = st.alert_scroll.min(total - 1);
                        if scroll < a_len {
                            let orig_idx = a_len.saturating_sub(1 + scroll);
                            if let Some(sfx) = Self::handle_alert_click(&mut st, orig_idx, true) {
                                self.audio.play(sfx);
                            }
                        } else {
                            let fc_idx = scroll - a_len;
                            if Self::focus_fusion_cluster(&mut st, fc_idx, true) {
                                self.audio.play(SoundEffect::Blip);
                            }
                        }
                    }
                } else {
                    let (dom, row_idx) = match active {
                        1 => (1, st.news_scroll),
                        2 => (2, st.target_scroll),
                        3 => (3, st.sat_scroll),
                        4 => (30, st.eam_scroll),
                        5 => (5, st.geo_scroll),
                        _ => (active, 0),
                    };
                    if Self::open_item_dossier(&mut st, dom, row_idx) {
                        self.audio.play(SoundEffect::Blip);
                    }
                }
            }
            _ => {}
        }
    }

}
