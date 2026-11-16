use argus_core::state::SdrAudioQueue;
use rodio::{source::SineWave, OutputStream, Sink, Source};
use std::collections::VecDeque;
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum SoundEffect {
    Click,
    Blip,
    Alert,
}

pub enum AudioMessage {
    Play(SoundEffect),
    SetMuted(bool),
    SetRadioPlaying(bool),
    Shutdown,
}

/// KiwiSDR WebSocket (/SND) üzerinden gelen gerçek 16-bit PCM örneklerini
/// kesintisiz (underrun veya klik yapmadan) ses kartına aktaran akış kaynağı.
/// Kuyruk boşaldığında sahte gürültü üretmez, yalnızca 0.0 (sessizlik) döndürür.
pub struct KiwiContinuousSource {
    queue: SdrAudioQueue,
    local_buf: VecDeque<f32>,
    sample_rate: u32,
    prebuffering: bool,
}

impl KiwiContinuousSource {
    pub fn new(queue: SdrAudioQueue) -> Self {
        Self {
            queue,
            local_buf: VecDeque::with_capacity(8192),
            sample_rate: 12000,
            prebuffering: true,
        }
    }
}

impl Iterator for KiwiContinuousSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        {
            let mut q = self.queue.lock();
            while let Some((sr, chunk)) = q.pop_front() {
                if sr >= 8000 {
                    self.sample_rate = sr;
                }
                for s in chunk {
                    if self.local_buf.len() < 24000 {
                        self.local_buf.push_back(s);
                    }
                }
            }
        }

        if self.prebuffering {
            // Ağ jitter'ını emmek için ~150ms (1800 örnek) gerçek veri birikmesini bekle
            if self.local_buf.len() >= 1800 {
                self.prebuffering = false;
            } else {
                return Some(0.0);
            }
        }

        if let Some(sample) = self.local_buf.pop_front() {
            Some(sample.clamp(-0.95, 0.95))
        } else {
            self.prebuffering = true;
            Some(0.0)
        }
    }
}

impl Source for KiwiContinuousSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[derive(Clone)]
pub struct AudioManager {
    sender: Sender<AudioMessage>,
}

impl AudioManager {
    pub fn init(initial_muted: bool, sdr_audio_queue: SdrAudioQueue) -> Self {
        let (tx, rx) = channel::<AudioMessage>();

        thread::Builder::new()
            .name("argus-audio".to_string())
            .spawn(move || {
                let stream_res = OutputStream::try_default();
                let (_stream, stream_handle) = match stream_res {
                    Ok((s, h)) => (Some(s), Some(h)),
                    Err(_) => (None, None),
                };

                let mut muted = initial_muted;
                let mut radio_playing = false;
                let mut radio_sink: Option<Sink> = None;

                loop {
                    if radio_playing && !muted {
                        if let Some(handle) = &stream_handle {
                            if radio_sink.is_none() {
                                if let Ok(sink) = Sink::try_new(handle) {
                                    sink.append(KiwiContinuousSource::new(Arc::clone(
                                        &sdr_audio_queue,
                                    )));
                                    sink.play();
                                    radio_sink = Some(sink);
                                }
                            }
                        }
                    } else {
                        if let Some(sink) = radio_sink.take() {
                            sink.stop();
                        }
                        sdr_audio_queue.lock().clear();
                    }

                    match rx.recv_timeout(Duration::from_millis(40)) {
                        Ok(AudioMessage::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                            if let Some(sink) = radio_sink.take() {
                                sink.stop();
                            }
                            break;
                        }
                        Ok(AudioMessage::SetMuted(m)) => {
                            muted = m;
                        }
                        Ok(AudioMessage::SetRadioPlaying(playing)) => {
                            radio_playing = playing;
                        }
                        Ok(AudioMessage::Play(effect)) => {
                            if muted {
                                continue;
                            }
                            if let Some(handle) = &stream_handle {
                                match effect {
                                    SoundEffect::Click => {
                                        if let Ok(sink) = Sink::try_new(handle) {
                                            let s = SineWave::new(880.0)
                                                .take_duration(Duration::from_millis(12))
                                                .amplify(0.03);
                                            sink.append(s);
                                            sink.detach();
                                        }
                                    }
                                    SoundEffect::Blip => {
                                        if let Ok(sink) = Sink::try_new(handle) {
                                            let s = SineWave::new(1320.0)
                                                .take_duration(Duration::from_millis(25))
                                                .amplify(0.04);
                                            sink.append(s);
                                            sink.detach();
                                        }
                                    }
                                    SoundEffect::Alert => {
                                        if let Ok(sink) = Sink::try_new(handle) {
                                            let s1 = SineWave::new(900.0)
                                                .take_duration(Duration::from_millis(50))
                                                .amplify(0.06);
                                            let s2 = SineWave::new(1200.0)
                                                .take_duration(Duration::from_millis(70))
                                                .amplify(0.06);
                                            sink.append(s1);
                                            sink.append(s2);
                                            sink.detach();
                                        }
                                    }
                                }
                            }
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                    }
                }
            })
            .expect("Failed to spawn audio thread");

        Self { sender: tx }
    }

    pub fn play(&self, effect: SoundEffect) {
        let _ = self.sender.send(AudioMessage::Play(effect));
    }

    pub fn set_muted(&self, muted: bool) {
        let _ = self.sender.send(AudioMessage::SetMuted(muted));
    }

    pub fn set_radio_playing(&self, playing: bool) {
        let _ = self.sender.send(AudioMessage::SetRadioPlaying(playing));
    }

    pub fn shutdown(&self) {
        let _ = self.sender.send(AudioMessage::Shutdown);
    }
}
