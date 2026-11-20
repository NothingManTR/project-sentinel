mod actions;
mod dossiers;
mod keys;
mod mouse;

use crate::audio::AudioManager;
use crate::photo_ui::PhotoRenderCtx;
use crate::ui::render_main_ui;
use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::types::{AircraftMediaStatus, Domain};
use crossterm::event::{self, Event, KeyEventKind, MouseEventKind};
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
        let mut last_dvr_tick = Instant::now();
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

                // Y-4: DVR Otomatik Oynatma (Play/Pause) zamanlayicisi
                if st.dvr.is_playing && last_dvr_tick.elapsed() >= Duration::from_millis(700) {
                    if st.dvr.advance_playback() {
                        needs_redraw = true;
                    }
                    last_dvr_tick = Instant::now();
                }

                // Y-1: Bekleyen Kritik Masaustu (notify-send) & Webhook Bildirimlerini gonder
                if !st.pending_notifications.is_empty() {
                    let desktop_on = st.config.notifications.desktop_enabled;
                    let webhook = st.config.notifications.webhook_url.clone();
                    let batch: Vec<_> = st.pending_notifications.drain(..).collect();
                    Self::dispatch_notifications(batch, desktop_on, webhook);
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

                // T-Dossier açıldıysa anında, Inspector'da seçili uçak sabit kaldıysa 350ms sonra fotoğraf + rota çek (T-1: domain 56 dahil)
                if let Some(d_icao) = st.dialog_aircraft_icao.clone() {
                    Self::spawn_aircraft_photo(&self.state, &mut st, &d_icao);
                    if let Some(t) = st.inspected_target().filter(|t| t.id.eq_ignore_ascii_case(&d_icao)) {
                        Self::spawn_flight_schedule(&self.state, &mut st, &t);
                    }
                } else if !st.is_dialog_open && !matches!(st.selected_domain, 1 | 3 | 4 | 30 | 50 | 51 | 52 | 53 | 54 | 55 | 56) {
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
                        {
                            if !st.aircraft_photo_cache.contains_key(&icao)
                                && !st.inflight_photo_icaos.contains(&icao)
                            {
                                Self::spawn_aircraft_photo(&self.state, &mut st, &icao);
                                needs_redraw = true;
                            }
                            let cs_key = t.callsign.trim().to_uppercase();
                            if !st.flight_schedule_cache.contains_key(&cs_key)
                                && !st.inflight_schedule_keys.contains(&cs_key)
                            {
                                Self::spawn_flight_schedule(&self.state, &mut st, &t);
                            }
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
}
