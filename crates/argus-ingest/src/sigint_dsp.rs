use std::collections::VecDeque;
use std::f32::consts::TAU;

/// KiwiSDR "gps":"(lat, lon)" metnini koordinata çevirir
pub fn parse_kiwi_gps(gps_str: &str) -> Option<(f64, f64)> {
    let trimmed = gps_str.trim().trim_start_matches('(').trim_end_matches(')');
    let mut parts = trimmed.split(',');
    let lat = parts.next()?.trim().parse::<f64>().ok()?;
    let lon = parts.next()?.trim().parse::<f64>().ok()?;
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return None;
    }
    if lat == 0.0 && lon == 0.0 {
        return None;
    }
    Some((lat, lon))
}

/// 60-bin DFT şelale satırından ses bandı spektral SNR (dB) değerini hesaplar.
/// compute_dft_waterfall_row ölçeği: (dB + 72.0) * 4.2 => 1 dB ≈ 4.2 birim.
/// Geniş bant atmosferik statik tüm kutuları yükselttiği için medyan ile tepe farkı düşük kalır;
/// gerçek konuşma / taşıyıcı / Morze sinyali dar bant kutularında yüksek tepe oluşturur.
pub fn estimate_spectral_snr_db(wf_row: &[u8]) -> f32 {
    if wf_row.len() < 8 {
        return 0.0;
    }
    let mut sorted = wf_row.to_vec();
    sorted.sort_unstable();
    let median = sorted[sorted.len() / 2] as f32;
    let top_start = sorted.len().saturating_sub(3);
    let top_avg: f32 =
        sorted[top_start..].iter().map(|&v| v as f32).sum::<f32>() / (sorted.len() - top_start) as f32;
    ((top_avg - median) / 4.2).max(0.0)
}

/// Kararlı sinyal aktivite dedektörü (Kısa statik çatırtılarını eler, Hang-Time ile kesmeyi geciktirir)
#[derive(Debug, Clone)]
pub struct SignalActivityDetector {
    pub trigger_snr_db: f32,
    pub sustain_ms: f32,
    pub hang_ms: f32,
    accum_active_ms: f32,
    remaining_hang_ms: f32,
    pub is_active: bool,
}

impl SignalActivityDetector {
    pub fn new(trigger_snr_db: f32, sustain_ms: f32, hang_ms: f32) -> Self {
        Self {
            trigger_snr_db,
            sustain_ms,
            hang_ms,
            accum_active_ms: 0.0,
            remaining_hang_ms: 0.0,
            is_active: false,
        }
    }

    pub fn reset(&mut self) {
        self.accum_active_ms = 0.0;
        self.remaining_hang_ms = 0.0;
        self.is_active = false;
    }

    /// Yeni frame süresi ve ölçülen spektral SNR ile aktivite durumunu günceller
    pub fn update(&mut self, snr_db: f32, dt_ms: f32) -> bool {
        if snr_db >= self.trigger_snr_db {
            self.accum_active_ms += dt_ms;
            if self.accum_active_ms >= self.sustain_ms {
                self.is_active = true;
                self.remaining_hang_ms = self.hang_ms;
            }
        } else {
            self.accum_active_ms = (self.accum_active_ms - dt_ms * 1.5).max(0.0);
            if self.is_active {
                self.remaining_hang_ms -= dt_ms;
                if self.remaining_hang_ms <= 0.0 {
                    self.remaining_hang_ms = 0.0;
                    self.is_active = false;
                }
            }
        }
        self.is_active
    }
}

/// Çoklu Merkez Frekanslı Goertzel + Adaptif WPM Morze (CW) Çözümleyici (5-40 WPM)
#[derive(Debug, Clone)]
pub struct CwDecoder {
    sample_rate: u32,
    block_buf: Vec<f32>,
    noise_floor: f32,
    signal_peak: f32,
    key_down: bool,
    on_ms: f32,
    off_ms: f32,
    dit_ms: f32,
    recent_pulses_ms: VecDeque<f32>,
    current_code: String,
    word_space_emitted: bool,
    pub text_buffer: String,
    pub estimated_wpm: u16,
}

impl CwDecoder {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate: sample_rate.max(8000),
            block_buf: Vec::with_capacity(256),
            noise_floor: 0.002,
            signal_peak: 0.008,
            key_down: false,
            on_ms: 0.0,
            off_ms: 0.0,
            dit_ms: 65.0,
            recent_pulses_ms: VecDeque::with_capacity(24),
            current_code: String::with_capacity(12),
            word_space_emitted: true,
            text_buffer: String::with_capacity(256),
            estimated_wpm: 18,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        let sr = sample_rate.max(8000);
        if self.sample_rate != sr {
            self.sample_rate = sr;
            self.block_buf.clear();
        }
    }

    pub fn clear(&mut self) {
        self.block_buf.clear();
        self.key_down = false;
        self.on_ms = 0.0;
        self.off_ms = 0.0;
        self.current_code.clear();
        self.text_buffer.clear();
        self.recent_pulses_ms.clear();
        self.word_space_emitted = true;
    }

    fn goertzel_mag(block: &[f32], target_hz: f32, sample_rate: f32) -> f32 {
        let n = block.len() as f32;
        let omega = TAU * target_hz / sample_rate;
        let coeff = 2.0 * omega.cos();
        let mut q1 = 0.0f32;
        let mut q2 = 0.0f32;
        for (i, &x) in block.iter().enumerate() {
            // Hafif Hann penceresi sızıntıyı azaltır
            let win = 0.5 * (1.0 - (TAU * i as f32 / n).cos());
            let q0 = coeff * q1 - q2 + x * win;
            q2 = q1;
            q1 = q0;
        }
        let power = (q1 * q1 + q2 * q2 - q1 * q2 * coeff).max(0.0);
        power.sqrt() / (n * 0.5)
    }

    pub fn process_samples(&mut self, samples: &[f32]) {
        // ~8 ms analiz penceresi
        let block_len = ((self.sample_rate as f32 * 0.008).round() as usize).clamp(48, 256);
        let block_ms = (block_len as f32 / self.sample_rate as f32) * 1000.0;

        for &s in samples {
            self.block_buf.push(s);
            if self.block_buf.len() >= block_len {
                self.process_block(block_ms);
                self.block_buf.clear();
            }
        }
    }

    fn process_block(&mut self, block_ms: f32) {
        let sr = self.sample_rate as f32;
        let m1 = Self::goertzel_mag(&self.block_buf, 550.0, sr);
        let m2 = Self::goertzel_mag(&self.block_buf, 600.0, sr);
        let m3 = Self::goertzel_mag(&self.block_buf, 650.0, sr);
        let mag = m1.max(m2).max(m3);

        // Adaptif taban & tepe zarf takibi
        if mag < self.noise_floor {
            self.noise_floor = 0.92 * self.noise_floor + 0.08 * mag;
        } else {
            self.noise_floor = 0.996 * self.noise_floor + 0.004 * mag;
        }
        self.noise_floor = self.noise_floor.max(1e-4);

        if mag > self.signal_peak {
            self.signal_peak = 0.72 * self.signal_peak + 0.28 * mag;
        } else {
            self.signal_peak = (0.992 * self.signal_peak + 0.008 * mag).max(self.noise_floor * 1.4);
        }

        let span = self.signal_peak - self.noise_floor;
        let has_tone = self.signal_peak > self.noise_floor * 2.6 && span > 0.006;
        let next_key_down = if !has_tone {
            false
        } else if self.key_down {
            mag >= self.noise_floor + 0.32 * span
        } else {
            mag >= self.noise_floor + 0.52 * span
        };

        if next_key_down {
            if !self.key_down {
                self.key_down = true;
                self.on_ms = block_ms;
            } else {
                self.on_ms += block_ms;
            }
            self.off_ms = 0.0;
        } else {
            if self.key_down {
                // Key-up: tamamlanan darbeyi değerlendir (18 ms altı statik çatırtısı elenir)
                let pulse = self.on_ms;
                self.key_down = false;
                self.on_ms = 0.0;
                if (18.0..=550.0).contains(&pulse) {
                    self.record_pulse(pulse);
                }
            }
            self.off_ms += block_ms;
            if !self.current_code.is_empty() && self.off_ms >= self.dit_ms * 2.1 {
                self.flush_char();
            }
            if !self.word_space_emitted && self.off_ms >= self.dit_ms * 5.2 {
                self.push_text_char(' ');
                self.word_space_emitted = true;
            }
        }
    }

    fn record_pulse(&mut self, pulse_ms: f32) {
        if self.recent_pulses_ms.len() >= 20 {
            self.recent_pulses_ms.pop_front();
        }
        self.recent_pulses_ms.push_back(pulse_ms);

        // En az 3 darbe görüldüğünde dit/dah kümesinden adaptif hız kestirimi
        if self.recent_pulses_ms.len() >= 3 {
            let mut min_p = f32::MAX;
            let mut max_p = 0.0f32;
            for &p in &self.recent_pulses_ms {
                if p < min_p {
                    min_p = p;
                }
                if p > max_p {
                    max_p = p;
                }
            }
            if max_p >= min_p * 2.05 {
                let split = (min_p + max_p) * 0.45;
                let (mut sum, mut cnt) = (0.0f32, 0usize);
                for &p in &self.recent_pulses_ms {
                    if p <= split {
                        sum += p;
                        cnt += 1;
                    }
                }
                if cnt > 0 {
                    let est = (sum / cnt as f32).clamp(26.0, 220.0);
                    self.dit_ms = 0.65 * self.dit_ms + 0.35 * est;
                }
            }
        }

        self.estimated_wpm = (1200.0 / self.dit_ms).round().clamp(5.0, 45.0) as u16;
        let sym = if pulse_ms >= self.dit_ms * 1.85 { '-' } else { '.' };
        if self.current_code.len() < 8 {
            self.current_code.push(sym);
        }
        self.word_space_emitted = false;
    }

    fn flush_char(&mut self) {
        if let Some(ch) = decode_morse_symbol(&self.current_code) {
            self.push_text_char(ch);
        } else if !self.current_code.is_empty() {
            self.push_text_char('*');
        }
        self.current_code.clear();
    }

    fn push_text_char(&mut self, ch: char) {
        if ch == ' ' && self.text_buffer.ends_with(' ') {
            return;
        }
        self.text_buffer.push(ch);
        if self.text_buffer.len() > 220 {
            let drain_to = self.text_buffer.len() - 200;
            self.text_buffer.drain(..drain_to);
        }
    }
}

pub fn decode_morse_symbol(code: &str) -> Option<char> {
    match code {
        ".-" => Some('A'),
        "-..." => Some('B'),
        "-.-." => Some('C'),
        "-.." => Some('D'),
        "." => Some('E'),
        "..-." => Some('F'),
        "--." => Some('G'),
        "...." => Some('H'),
        ".." => Some('I'),
        ".---" => Some('J'),
        "-.-" => Some('K'),
        ".-.." => Some('L'),
        "--" => Some('M'),
        "-." => Some('N'),
        "---" => Some('O'),
        ".--." => Some('P'),
        "--.-" => Some('Q'),
        ".-." => Some('R'),
        "..." => Some('S'),
        "-" => Some('T'),
        "..-" => Some('U'),
        "...-" => Some('V'),
        ".--" => Some('W'),
        "-..-" => Some('X'),
        "-.--" => Some('Y'),
        "--.." => Some('Z'),
        "-----" => Some('0'),
        ".----" => Some('1'),
        "..---" => Some('2'),
        "...--" => Some('3'),
        "....-" => Some('4'),
        "....." => Some('5'),
        "-...." => Some('6'),
        "--..." => Some('7'),
        "---.." => Some('8'),
        "----." => Some('9'),
        "-..-." => Some('/'),
        "..--.." => Some('?'),
        ".-.-.-" => Some('.'),
        "--..--" => Some(','),
        "-...-" => Some('='),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn morse_for(ch: char) -> &'static str {
        match ch {
            'A' => ".-",
            'C' => "-.-.",
            'D' => "-..",
            'E' => ".",
            'G' => "--.",
            'O' => "---",
            'Q' => "--.-",
            'R' => ".-.",
            'S' => "...",
            'U' => "..-",
            '3' => "...--",
            '7' => "--...",
            _ => "",
        }
    }

    fn synth_morse_audio(text: &str, wpm: f32, sample_rate: u32, tone_hz: f32) -> Vec<f32> {
        let dit_s = 1.2 / wpm;
        let sr = sample_rate as f32;
        let mut out = Vec::new();
        let mut phase = 0.0f32;
        let dphi = TAU * tone_hz / sr;

        let push_dur = |buf: &mut Vec<f32>, ph: &mut f32, secs: f32, tone: bool| {
            let n = (secs * sr).round() as usize;
            for i in 0..n {
                // Hafif pseudo-gürültü ile gerçekçi test
                let noise = ((i as f32 * 13.7).sin()) * 0.015;
                let s = if tone { 0.45 * ph.sin() + noise } else { noise };
                buf.push(s);
                *ph = (*ph + dphi) % TAU;
            }
        };

        push_dur(&mut out, &mut phase, 0.15, false);
        for ch in text.chars() {
            if ch == ' ' {
                push_dur(&mut out, &mut phase, dit_s * 4.5, false);
                continue;
            }
            let pat = morse_for(ch);
            for (idx, sym) in pat.chars().enumerate() {
                let dur = if sym == '-' { dit_s * 3.0 } else { dit_s };
                push_dur(&mut out, &mut phase, dur, true);
                if idx + 1 < pat.len() {
                    push_dur(&mut out, &mut phase, dit_s, false);
                }
            }
            push_dur(&mut out, &mut phase, dit_s * 3.0, false);
        }
        push_dur(&mut out, &mut phase, 0.35, false);
        out
    }

    #[test]
    fn test_cw_decoder_synthetic_18_and_24_wpm() {
        for &(msg, wpm, hz) in &[("CQ DE ARGUS 73", 18.0, 600.0), ("SOS CQ", 24.0, 630.0)] {
            let pcm = synth_morse_audio(msg, wpm, 12000, hz);
            let mut dec = CwDecoder::new(12000);
            for chunk in pcm.chunks(256) {
                dec.process_samples(chunk);
            }
            let decoded = dec.text_buffer.trim();
            assert_eq!(decoded, msg, "Decoded mismatch for {} WPM", wpm);
            assert!(
                (dec.estimated_wpm as f32 - wpm).abs() <= 4.0,
                "WPM estimate {} too far from {}",
                dec.estimated_wpm,
                wpm
            );
        }
    }

    #[test]
    fn test_spectral_snr_and_activity_detector() {
        // Geniş bant gürültü: tüm kutular ~80 => düşük spektral SNR
        let noise_row = vec![80u8; 60];
        assert!(estimate_spectral_snr_db(&noise_row) < 1.0);

        // Dar bant taşıyıcı/konuşma piki: taban ~70, 4 kutu ~145 => (145-70)/4.2 = 17.8 dB
        let mut sig_row = vec![70u8; 60];
        sig_row[15] = 145;
        sig_row[16] = 152;
        sig_row[17] = 142;
        assert!(estimate_spectral_snr_db(&sig_row) > 15.0);

        let mut det = SignalActivityDetector::new(11.0, 300.0, 1000.0);
        assert!(!det.update(18.0, 150.0)); // henüz 300ms dolmadı (kısa paraziti eler)
        assert!(det.update(18.0, 200.0));  // 350ms => kilitlendi
        assert!(det.update(4.0, 600.0));   // hang-time içinde hala aktif
        assert!(!det.update(4.0, 600.0));  // hang-time bitti => kapandı
    }

    #[test]
    fn test_parse_kiwi_gps() {
        let (lat, lon) = parse_kiwi_gps("(-38.152073, 176.070718)").unwrap();
        assert!((lat - (-38.152073)).abs() < 1e-6);
        assert!((lon - 176.070718).abs() < 1e-6);
        assert!(parse_kiwi_gps("(0.0, 0.0)").is_none());
    }
}
