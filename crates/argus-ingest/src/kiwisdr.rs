use crate::sigint_dsp::{
    estimate_spectral_snr_db, parse_kiwi_gps, CwDecoder, SignalActivityDetector,
};
use argus_core::state::AppState;
use argus_core::types::{SdrNodeInfo, SdrScanHit};
use futures_util::{SinkExt, StreamExt};
use parking_lot::RwLock;
use std::f32::consts::TAU;
use std::fs::{self, File};
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

const IMA_INDEX_TABLE: [i32; 16] = [
    -1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8,
];

const IMA_STEP_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408,
    449, 494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066,
    2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630,
    9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794,
    32767,
];

#[derive(Default)]
pub struct ImaAdpcmDecoder {
    predictor: i32,
    step_index: i32,
}

impl ImaAdpcmDecoder {
    pub fn decode_nibble(&mut self, nibble: u8) -> f32 {
        let step = IMA_STEP_TABLE[self.step_index as usize];
        let mut diff = step >> 3;
        if (nibble & 1) != 0 {
            diff += step >> 2;
        }
        if (nibble & 2) != 0 {
            diff += step >> 1;
        }
        if (nibble & 4) != 0 {
            diff += step;
        }
        if (nibble & 8) != 0 {
            self.predictor = (self.predictor - diff).clamp(-32768, 32767);
        } else {
            self.predictor = (self.predictor + diff).clamp(-32768, 32767);
        }
        self.step_index = (self.step_index + IMA_INDEX_TABLE[(nibble & 0x0F) as usize]).clamp(0, 88);
        self.predictor as f32 / 32768.0
    }

    pub fn decode_block(&mut self, bytes: &[u8]) -> Vec<f32> {
        let mut out = Vec::with_capacity(bytes.len() * 2);
        for &b in bytes {
            out.push(self.decode_nibble(b & 0x0F));
            out.push(self.decode_nibble(b >> 4));
        }
        out
    }
}

/// KiwiSDR ikili "SND" paketini çözer: (flags, seq, rssi_dbm, f32_pcm_samples)
pub fn parse_kiwi_snd_frame(
    bin: &[u8],
    adpcm: &mut ImaAdpcmDecoder,
) -> Option<(u8, u32, f32, Vec<f32>)> {
    let idx = bin.windows(3).position(|w| w == b"SND")?;
    let frame = &bin[idx..];
    if frame.len() < 12 {
        return None;
    }

    let flags = frame[3];
    let seq = u32::from_le_bytes([frame[4], frame[5], frame[6], frame[7]]);
    let smeter = u16::from_be_bytes([frame[8], frame[9]]);
    let rssi_dbm = (smeter as f32) * 0.1 - 127.0;
    let payload = &frame[10..];

    let is_compressed = (flags & 0x10) != 0;
    let samples = if is_compressed {
        adpcm.decode_block(payload)
    } else {
        let mut pcm = Vec::with_capacity(payload.len() / 2);
        for chunk in payload.as_chunks::<2>().0 {
            let val = i16::from_be_bytes(*chunk);
            pcm.push((val as f32 / 32768.0) * 1.8);
        }
        pcm
    };

    if samples.is_empty() {
        None
    } else {
        Some((flags, seq, rssi_dbm, samples))
    }
}

/// Gerçek PCM ses örneklerinden 60-bin Ayrık Fourier Dönüşümü (DFT) şelale satırı hesaplar
pub fn compute_dft_waterfall_row(samples: &[f32], num_bins: usize) -> Vec<u8> {
    let n = samples.len().min(256);
    if n < 16 {
        return vec![0; num_bins];
    }
    let slice = &samples[..n];
    let mut row = Vec::with_capacity(num_bins);

    // 300 Hz .. 3300 Hz ses bandını 60 frekans kutusuna (bin) eşle
    for b in 0..num_bins {
        let k = 2.0 + (b as f32 / num_bins as f32) * ((n as f32) * 0.32);
        let omega = TAU * k / (n as f32);
        let mut re = 0.0f32;
        let mut im = 0.0f32;

        for (i, &s) in slice.iter().enumerate() {
            // Hann penceresi
            let w = 0.5 * (1.0 - (TAU * i as f32 / (n as f32)).cos());
            let ws = s * w;
            let angle = omega * (i as f32);
            re += ws * angle.cos();
            im -= ws * angle.sin();
        }

        let mag = ((re * re + im * im).sqrt() / (n as f32)).max(1e-7);
        let db = 20.0 * mag.log10(); // tipik aralık: -75 dB .. -10 dB
        let scaled = (((db + 72.0) * 4.2).clamp(0.0, 255.0)) as u8;
        row.push(scaled);
    }

    row
}

/// rx.linkfanel.net/kiwisdr_com.js üzerinden aktif ve boş kanalı olan tüm 0-30 MHz KiwiSDR alıcılarını çeker
pub async fn fetch_public_kiwisdr_nodes(client: &reqwest::Client) -> Vec<SdrNodeInfo> {
    let mut nodes = Vec::new();
    let url = "http://rx.linkfanel.net/kiwisdr_com.js";

    if let Ok(resp) = client.get(url).send().await {
        if resp.status().is_success() {
            if let Ok(text) = resp.text().await {
                if let (Some(start), Some(end)) = (text.find('['), text.rfind(']')) {
                    if start < end {
                        let inner = text[start + 1..end].trim().trim_end_matches(',');
                        let clean_json = format!("[{}]", inner);
                        if let Ok(arr) =
                            serde_json::from_str::<Vec<serde_json::Value>>(&clean_json)
                        {
                            for item in arr {
                                let status = item["status"].as_str().unwrap_or("");
                                let offline = item["offline"].as_str().unwrap_or("yes");
                                let bands = item["bands"].as_str().unwrap_or("");
                                let users: u32 = item["users"]
                                    .as_str()
                                    .and_then(|s| s.parse().ok())
                                    .unwrap_or(99);
                                let users_max: u32 = item["users_max"]
                                    .as_str()
                                    .and_then(|s| s.parse().ok())
                                    .unwrap_or(0);
                                let raw_url = item["url"].as_str().unwrap_or("").trim();

                                // Aktif, çevrimiçi, en az 1 boş kanalı olan ve HF (3-30 MHz) kapsayan tüm alıcılar
                                let covers_hf = bands.contains("30000000")
                                    || bands.contains("29999")
                                    || bands.contains("32000000")
                                    || bands.is_empty();
                                if status == "active"
                                    && offline == "no"
                                    && users < users_max
                                    && covers_hf
                                    && raw_url.starts_with("http://")
                                {
                                    let host_port = raw_url
                                        .trim_start_matches("http://")
                                        .trim_end_matches('/')
                                        .to_string();
                                    if host_port.is_empty() || host_port.contains('/') {
                                        continue;
                                    }
                                    if nodes.iter().any(|n: &SdrNodeInfo| n.host_port == host_port)
                                    {
                                        continue;
                                    }
                                    let snr_str = item["snr"].as_str().unwrap_or("0");
                                    let snr: i32 = snr_str
                                        .split(',')
                                        .next()
                                        .and_then(|s| s.parse().ok())
                                        .unwrap_or(12);
                                    if snr < 5 {
                                        continue;
                                    }

                                    let name: String = item["name"]
                                        .as_str()
                                        .unwrap_or(&host_port)
                                        .chars()
                                        .filter(|c| c.is_ascii() && !c.is_control())
                                        .take(42)
                                        .collect();
                                    let loc_raw: String = item["loc"]
                                        .as_str()
                                        .unwrap_or("-")
                                        .chars()
                                        .filter(|c| c.is_ascii() && !c.is_control())
                                        .take(38)
                                        .collect();
                                    let location = if loc_raw.len() <= 2 && !name.is_empty() {
                                        name.clone()
                                    } else {
                                        loc_raw
                                    };

                                    let gps_coords = item["gps"]
                                        .as_str()
                                        .and_then(parse_kiwi_gps);
                                    let grid: String = item["grid"]
                                        .as_str()
                                        .unwrap_or("")
                                        .chars()
                                        .filter(|c| c.is_ascii_alphanumeric())
                                        .take(8)
                                        .collect();

                                    nodes.push(SdrNodeInfo {
                                        name,
                                        url: raw_url.to_string(),
                                        host_port,
                                        location,
                                        users,
                                        users_max,
                                        snr,
                                        lat: gps_coords.map(|c| c.0),
                                        lon: gps_coords.map(|c| c.1),
                                        grid,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Yüksek SNR ve boş kanal kapasitesine göre sırala, 300 küresel alıcıya kadar tut
    nodes.sort_by(|a, b| {
        let free_a = a.users_max.saturating_sub(a.users) > 1;
        let free_b = b.users_max.saturating_sub(b.users) > 1;
        free_b
            .cmp(&free_a)
            .then_with(|| b.snr.cmp(&a.snr))
    });
    nodes.truncate(300);
    nodes
}

fn build_tune_cmd(freq_khz: f64, mode: &str) -> String {
    let m = mode.to_lowercase();
    let (mod_str, lo, hi) = match m.as_str() {
        "am" => ("am", -4000, 4000),
        "lsb" => ("lsb", -2700, -300),
        "cw" => ("cw", 300, 900),
        _ => ("usb", 300, 2700),
    };
    format!(
        "SET mod={} low_cut={} high_cut={} freq={:.3}",
        mod_str, lo, hi, freq_khz
    )
}

struct WavRecorder {
    writer: BufWriter<File>,
    sample_count: u32,
    path: String,
}

impl WavRecorder {
    fn create(prefix: &str, freq_khz: f64, sample_rate: u32) -> std::io::Result<Self> {
        fs::create_dir_all("recordings")?;
        let stamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let path = format!("recordings/{}_{:.0}kHz_{}.wav", prefix, freq_khz, stamp);
        let file = File::create(&path)?;
        let mut writer = BufWriter::new(file);
        // 44-byte RIFF/PCM header placeholder (16-bit mono)
        writer.write_all(b"RIFF")?;
        writer.write_all(&0u32.to_le_bytes())?;
        writer.write_all(b"WAVEfmt ")?;
        writer.write_all(&16u32.to_le_bytes())?;
        writer.write_all(&1u16.to_le_bytes())?; // PCM
        writer.write_all(&1u16.to_le_bytes())?; // Mono
        writer.write_all(&sample_rate.to_le_bytes())?;
        writer.write_all(&(sample_rate * 2).to_le_bytes())?;
        writer.write_all(&2u16.to_le_bytes())?;
        writer.write_all(&16u16.to_le_bytes())?;
        writer.write_all(b"data")?;
        writer.write_all(&0u32.to_le_bytes())?;
        Ok(Self {
            writer,
            sample_count: 0,
            path,
        })
    }

    fn write_samples(&mut self, samples: &[f32]) {
        for &s in samples {
            let pcm = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
            if self.writer.write_all(&pcm.to_le_bytes()).is_ok() {
                self.sample_count = self.sample_count.saturating_add(1);
            }
        }
    }

    fn finish(mut self) {
        let data_bytes = self.sample_count.saturating_mul(2);
        let riff_size = data_bytes.saturating_add(36);
        let _ = self.writer.flush();
        let file = self.writer.get_mut();
        let _ = file.seek(SeekFrom::Start(4));
        let _ = file.write_all(&riff_size.to_le_bytes());
        let _ = file.seek(SeekFrom::Start(40));
        let _ = file.write_all(&data_bytes.to_le_bytes());
        let _ = file.flush();
    }
}

/// Gerçek KiwiSDR WebSocket İstemci Görevi
pub async fn run_kiwisdr_worker(state: Arc<RwLock<AppState>>) {
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent("ArgusOSINT/2.0")
        .build()
        .unwrap_or_default();

    // Başlangıçta canlı KiwiSDR alıcı dizinini çek
    let initial_nodes = fetch_public_kiwisdr_nodes(&http_client).await;
    if !initial_nodes.is_empty() {
        let mut st = state.write();
        st.sdr_nodes = initial_nodes;
        st.sdr_status = format!(
            "HAZIR: {} Canli KiwiSDR HF Alicisi Bulundu (Dinlemek icin S)",
            st.sdr_nodes.len()
        );
    }

    let mut last_dir_refresh = Instant::now();

    loop {
        let (playing, freq_khz, active_idx, auto_failover, node_opt) = {
            let st = state.read();
            let node = st.sdr_nodes.get(st.active_sdr_idx).cloned();
            (
                st.is_radio_playing,
                st.current_frequency_khz,
                st.active_sdr_idx,
                st.sdr_auto_failover,
                node,
            )
        };

        if !playing {
            // Boştayken her 5 dakikada bir canlı alıcı dizinini tazele (seçili alıcıyı koru)
            if last_dir_refresh.elapsed() > Duration::from_secs(300) {
                last_dir_refresh = Instant::now();
                let nodes = fetch_public_kiwisdr_nodes(&http_client).await;
                if !nodes.is_empty() {
                    let mut st = state.write();
                    let sel_host = st.sdr_nodes.get(st.active_sdr_idx).map(|n| n.host_port.clone());
                    let cur_host = st.sdr_nodes.get(st.sdr_scroll).map(|n| n.host_port.clone());
                    st.sdr_nodes = nodes;
                    st.active_sdr_idx = sel_host
                        .and_then(|h| st.sdr_nodes.iter().position(|n| n.host_port == h))
                        .unwrap_or(0);
                    st.sdr_scroll = cur_host
                        .and_then(|h| st.sdr_nodes.iter().position(|n| n.host_port == h))
                        .unwrap_or(st.active_sdr_idx);
                }
            }
            sleep(Duration::from_millis(200)).await;
            continue;
        }

        // Eğer alıcı listesi boşsa yeniden sorgula
        let target_node = match node_opt {
            Some(n) => n,
            None => {
                if last_dir_refresh.elapsed() > Duration::from_secs(10) {
                    last_dir_refresh = Instant::now();
                    let nodes = fetch_public_kiwisdr_nodes(&http_client).await;
                    let mut st = state.write();
                    st.sdr_nodes = nodes;
                }
                sleep(Duration::from_secs(1)).await;
                continue;
            }
        };

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let ws_url = format!("ws://{}/kiwi/{}/SND", target_node.host_port, ts);

        let connect_timeout = if auto_failover {
            Duration::from_secs(6)
        } else {
            Duration::from_secs(15)
        };

        {
            let mut st = state.write();
            st.sdr_status = if auto_failover {
                format!("BAGLANIYOR: {} ({:.1} kHz)...", target_node.host_port, freq_khz)
            } else {
                format!(
                    "BAGLANIYOR [KILITLI {}sn]: {} ({:.1} kHz)...",
                    connect_timeout.as_secs(),
                    target_node.host_port,
                    freq_khz
                )
            };
        }

        let conn_res = tokio::time::timeout(connect_timeout, connect_async(&ws_url)).await;
        let (mut ws_stream, _) = match conn_res {
            Ok(Ok(pair)) => pair,
            _ => {
                let failover_now = {
                    let mut st = state.write();
                    let fo = st.sdr_auto_failover;
                    if fo {
                        if !st.sdr_nodes.is_empty() {
                            st.active_sdr_idx = (active_idx + 1) % st.sdr_nodes.len();
                        }
                        st.sdr_status = format!(
                            "ALICI DOLU/YANIT YOK ({}), SIRADAKI DENENIYOR...",
                            target_node.host_port
                        );
                    } else {
                        st.sdr_status = format!(
                            "YANIT YOK/ZAMAN ASIMI [KILITLI]: {} - Tekrar deneniyor...",
                            target_node.host_port
                        );
                    }
                    fo
                };
                sleep(Duration::from_millis(if failover_now { 600 } else { 2500 })).await;
                continue;
            }
        };

        // 1. KiwiSDR Yetkilendirme El Sıkışması
        if ws_stream
            .send(Message::Text("SET auth t=kiwi p=#".to_string()))
            .await
            .is_err()
        {
            sleep(Duration::from_millis(1000)).await;
            continue;
        }

        let mut configured = false;
        let mut sample_rate: u32 = 12000;
        let mut tuned_freq = freq_khz;
        let mut tuned_mode = {
            let st = state.read();
            st.sdr_mode.clone()
        };
        let mut last_keepalive = Instant::now();
        let mut last_snd_received = Instant::now();
        let mut adpcm = ImaAdpcmDecoder::default();
        let mut read_timed_out_or_errored = false;
        let mut recorder: Option<WavRecorder> = None;
        let mut scan_detector = SignalActivityDetector::new(11.0, 350.0, 4000.0);
        let mut vox_detector = SignalActivityDetector::new(11.0, 400.0, 5000.0);
        let mut cw_decoder = CwDecoder::new(sample_rate);
        let mut scan_step_started = Instant::now();
        let mut scan_preset_idx: usize = 0;
        let mut scan_hit_logged_for_step = false;
        let mut prev_scan_enabled = false;
        let mut prev_cw_enabled = false;
        let mut latest_snr_db: f32 = 0.0;

        loop {
            let (
                still_playing,
                req_freq,
                req_idx,
                cur_auto_failover,
                req_mode,
                want_rec,
                scan_enabled,
                vox_enabled,
                cw_enabled,
            ) = {
                let st = state.read();
                (
                    st.is_radio_playing,
                    st.current_frequency_khz,
                    st.active_sdr_idx,
                    st.sdr_auto_failover,
                    st.sdr_mode.clone(),
                    st.sdr_recording,
                    st.sdr_scanner_enabled,
                    st.sdr_vox_enabled,
                    st.sdr_cw_enabled,
                )
            };

            if scan_enabled && !prev_scan_enabled {
                scan_step_started = Instant::now();
                scan_hit_logged_for_step = false;
                scan_detector.reset();
            } else if !scan_enabled && prev_scan_enabled {
                scan_detector.reset();
                state.write().sdr_scan_locked = false;
            }
            prev_scan_enabled = scan_enabled;

            if !cw_enabled && prev_cw_enabled {
                cw_decoder.clear();
            }
            prev_cw_enabled = cw_enabled;

            let need_recording = want_rec || (vox_enabled && vox_detector.is_active);
            if need_recording && configured && recorder.is_none() {
                let prefix = if !want_rec && vox_enabled { "vox" } else { "kiwi" };
                if let Ok(rec) = WavRecorder::create(prefix, tuned_freq, sample_rate) {
                    let mut st = state.write();
                    st.sdr_rec_path = rec.path.clone();
                    recorder = Some(rec);
                }
            } else if !need_recording {
                if let Some(rec) = recorder.take() {
                    let finished_path = rec.path.clone();
                    rec.finish();
                    let mut st = state.write();
                    st.sdr_rec_path = format!("KAYDEDILDI: {}", finished_path);
                }
            }

            if !still_playing || req_idx != active_idx {
                if let Some(rec) = recorder.take() {
                    let finished_path = rec.path.clone();
                    rec.finish();
                    let mut st = state.write();
                    st.sdr_recording = false;
                    st.sdr_rec_path = format!("KAYDEDILDI: {}", finished_path);
                }
                let _ = ws_stream.close(None).await;
                let mut st = state.write();
                st.sdr_audio_queue.lock().clear();
                if !still_playing {
                    st.sdr_status = "DURDURULDU (S ile Canli Dinlemeyi Baslatin)".to_string();
                }
                break;
            }

            let snd_grace = if cur_auto_failover {
                Duration::from_secs(4)
            } else {
                Duration::from_secs(25)
            };

            if last_snd_received.elapsed() > snd_grace {
                let _ = ws_stream.close(None).await;
                let mut st = state.write();
                if cur_auto_failover {
                    if !st.sdr_nodes.is_empty() {
                        st.active_sdr_idx = (active_idx + 1) % st.sdr_nodes.len();
                    }
                    st.sdr_status = format!(
                        "SES AKISI YOK ({}), SIRADAKI ALICIYA GECILIYOR...",
                        target_node.host_port
                    );
                } else {
                    st.sdr_status = format!(
                        "SES AKISI BEKLENDI ({}sn) [KILITLI]: {} - Yeniden baglaniliyor...",
                        snd_grace.as_secs(),
                        target_node.host_port
                    );
                    read_timed_out_or_errored = true;
                }
                break;
            }

            let ws_read_timeout = if cur_auto_failover {
                Duration::from_secs(5)
            } else {
                Duration::from_secs(12)
            };
            let msg_res = match tokio::time::timeout(ws_read_timeout, ws_stream.next()).await {
                Ok(Some(res)) => res,
                _ => {
                    read_timed_out_or_errored = true;
                    break;
                }
            };

            // Kullanıcı frekansı veya modu değiştirdiyse açık WebSocket üzerinden anında yeniden ayarla
            if configured && ((req_freq - tuned_freq).abs() > 0.05 || req_mode != tuned_mode) {
                tuned_freq = req_freq;
                tuned_mode = req_mode;
                let cmd = build_tune_cmd(tuned_freq, &tuned_mode);
                let _ = ws_stream.send(Message::Text(cmd)).await;
                let mut st = state.write();
                st.sdr_audio_queue.lock().clear();
                st.sdr_status = format!(
                    "CANLI: {} | {:.1} kHz {} ({} Hz PCM)",
                    target_node.host_port, tuned_freq, tuned_mode, sample_rate
                );
            }

            // Periyodik Keepalive
            if last_keepalive.elapsed() >= Duration::from_secs(4) {
                last_keepalive = Instant::now();
                if ws_stream
                    .send(Message::Text("SET keepalive".to_string()))
                    .await
                    .is_err()
                {
                    break;
                }
            }

            let bin_data = match msg_res {
                Ok(Message::Binary(b)) => b,
                Ok(Message::Text(t)) => t.into_bytes(),
                Err(_) => break,
                _ => continue,
            };

            // Sunucu MSG paketlerinde hata/doluluk/şifre kontrolü
            if bin_data.starts_with(b"MSG") {
                let text_lossy = String::from_utf8_lossy(&bin_data);
                if text_lossy.contains("too_busy=")
                    || text_lossy.contains("badp=1")
                    || text_lossy.contains(" redirect=")
                    || text_lossy.contains("down=")
                {
                    let mut st = state.write();
                    if st.sdr_auto_failover {
                        if !st.sdr_nodes.is_empty() {
                            st.active_sdr_idx = (active_idx + 1) % st.sdr_nodes.len();
                        }
                    } else {
                        let reason = if text_lossy.contains("too_busy=") {
                            "KANAL DOLU (too_busy)"
                        } else if text_lossy.contains("badp=1") {
                            "SIFRELI (badp=1)"
                        } else {
                            "KAPALI / YONLENDIRME"
                        };
                        st.sdr_status = format!(
                            "{} [KILITLI]: {} - Ayni alici tekrar denenecek...",
                            reason, target_node.host_port
                        );
                        read_timed_out_or_errored = true;
                    }
                    break;
                }

                if !configured
                    && (text_lossy.contains("sample_rate=") || text_lossy.contains("audio_rate="))
                {
                    for part in text_lossy.split_whitespace() {
                        if let Some(val_str) = part
                            .strip_prefix("sample_rate=")
                            .or_else(|| part.strip_prefix("audio_rate="))
                        {
                            let clean: String = val_str
                                .chars()
                                .take_while(|c| c.is_ascii_digit() || *c == '.')
                                .collect();
                            if let Ok(sr_f) = clean.parse::<f64>() {
                                if (11500.0..=12500.0).contains(&sr_f) {
                                    sample_rate = 12000;
                                } else if (19500.0..=21000.0).contains(&sr_f) {
                                    sample_rate = 20250;
                                } else if (8000.0..=48000.0).contains(&sr_f) {
                                    sample_rate = sr_f.round() as u32;
                                }
                            }
                        }
                    }

                    configured = true;
                    let init_cmds = [
                        format!("SET AR OK in={} out=44100", sample_rate),
                        "SET ident_user=argus_osint".to_string(),
                        "SET compression=0".to_string(),
                        build_tune_cmd(tuned_freq, &tuned_mode),
                        "SET agc=1 hang=0 thresh=-100 slope=6 decay=1000 manGain=50".to_string(),
                        "SET squelch=0 max=0".to_string(),
                        "SET gen=0 mix=-1".to_string(),
                        "SET keepalive".to_string(),
                    ];
                    for cmd in init_cmds {
                        let _ = ws_stream.send(Message::Text(cmd)).await;
                    }

                    let mut st = state.write();
                    st.sdr_connected_host = target_node.host_port.clone();
                    st.sdr_sample_rate = sample_rate;
                    st.sdr_status = format!(
                        "BAGLANDI: {} ({}) | {:.1} kHz {}",
                        target_node.host_port, target_node.location, tuned_freq, tuned_mode
                    );
                }
            }

            // Gerçek "SND" ses çerçevesini çöz
            if let Some((_flags, seq, rssi_dbm, samples)) =
                parse_kiwi_snd_frame(&bin_data, &mut adpcm)
            {
                last_snd_received = Instant::now();
                let dt_ms = (samples.len() as f32 / sample_rate.max(8000) as f32) * 1000.0;

                if let Some(rec) = recorder.as_mut() {
                    rec.write_samples(&samples);
                }
                if cw_enabled {
                    cw_decoder.set_sample_rate(sample_rate);
                    cw_decoder.process_samples(&samples);
                }
                let wf_row = if seq.is_multiple_of(3) {
                    let row = compute_dft_waterfall_row(&samples, 60);
                    latest_snr_db = estimate_spectral_snr_db(&row);
                    Some(row)
                } else {
                    None
                };

                // Kanal değişimi sonrası 500ms AGC oturmasını bekle (parazit kilitlenmelerini önler)
                let settled = scan_step_started.elapsed() >= Duration::from_millis(500);
                if settled {
                    if scan_enabled {
                        scan_detector.update(latest_snr_db, dt_ms);
                    }
                    if vox_enabled {
                        vox_detector.update(latest_snr_db, dt_ms);
                    } else if vox_detector.is_active {
                        vox_detector.reset();
                    }
                }

                let mut st = state.write();
                st.sdr_rssi_dbm = rssi_dbm;
                st.sdr_snr_db = latest_snr_db;

                if cw_enabled {
                    st.sdr_cw_text.clone_from(&cw_decoder.text_buffer);
                    st.sdr_cw_wpm = cw_decoder.estimated_wpm;
                }

                if scan_enabled {
                    st.sdr_scan_locked = scan_detector.is_active;
                    if scan_detector.is_active {
                        if !scan_hit_logged_for_step {
                            scan_hit_logged_for_step = true;
                            if st.sdr_scan_hits.len() >= 24 {
                                st.sdr_scan_hits.pop_front();
                            }
                            st.sdr_scan_hits.push_back(SdrScanHit {
                                timestamp: chrono::Utc::now(),
                                frequency_khz: tuned_freq,
                                mode: tuned_mode.clone(),
                                snr_db: latest_snr_db,
                                rssi_dbm,
                                host_port: target_node.host_port.clone(),
                                recorded_file: recorder.as_ref().map(|r| r.path.clone()),
                            });
                        }
                    } else if scan_step_started.elapsed() >= Duration::from_millis(2300)
                        && !st.radio_presets.is_empty()
                    {
                        scan_preset_idx = (scan_preset_idx + 1) % st.radio_presets.len();
                        let (nf, nm) = {
                            let p = &st.radio_presets[scan_preset_idx];
                            (p.frequency_khz, p.mode.clone())
                        };
                        st.current_frequency_khz = nf;
                        st.sdr_mode = nm;
                        scan_step_started = Instant::now();
                        scan_hit_logged_for_step = false;
                        scan_detector.reset();
                        vox_detector.reset();
                    }
                } else {
                    st.sdr_scan_locked = false;
                }

                if seq.is_multiple_of(15) {
                    st.sdr_status = if scan_enabled && scan_detector.is_active {
                        format!(
                            "TARAMA KILIDI: {:.1} kHz {} (+{:.1}dB SNR | {:.1}dBm)",
                            tuned_freq, tuned_mode, latest_snr_db, rssi_dbm
                        )
                    } else if scan_enabled {
                        format!(
                            "TARANIYOR: {:.1} kHz {} (+{:.1}dB SNR | {})",
                            tuned_freq, tuned_mode, latest_snr_db, target_node.host_port
                        )
                    } else {
                        format!(
                            "CANLI SES AKISI: {} ({}) | {:.1} kHz {} | {:.1} dBm (+{:.1}dB)",
                            target_node.host_port,
                            target_node.location,
                            tuned_freq,
                            tuned_mode,
                            rssi_dbm,
                            latest_snr_db
                        )
                    };
                }
                if let Some(row) = wf_row {
                    if st.waterfall_data.len() >= 16 {
                        st.waterfall_data.remove(0);
                    }
                    st.waterfall_data.push(row);
                }

                let mut q = st.sdr_audio_queue.lock();
                if q.len() >= 24 {
                    q.pop_front();
                }
                q.push_back((sample_rate, samples));
            }
        }

        if let Some(rec) = recorder.take() {
            let finished_path = rec.path.clone();
            rec.finish();
            let mut st = state.write();
            st.sdr_recording = false;
            st.sdr_rec_path = format!("KAYDEDILDI: {}", finished_path);
        }

        if read_timed_out_or_errored {
            sleep(Duration::from_millis(2500)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kiwi_snd_pcm_decode_and_dft() {
        // Yapay olmayan protokol testi: 10 bayt SND başlığı + 64 adet 16-bit Big-Endian örnek
        let mut packet = Vec::new();
        packet.extend_from_slice(b"SND");
        packet.push(0x05); // flags: sıkıştırmasız PCM
        packet.extend_from_slice(&1u32.to_le_bytes()); // seq = 1
        packet.extend_from_slice(&250u16.to_be_bytes()); // smeter = 250 -> -102.0 dBm
        for i in 0..64 {
            let val = ((i as f32 * 0.4).sin() * 8000.0) as i16;
            packet.extend_from_slice(&val.to_be_bytes());
        }

        let mut adpcm = ImaAdpcmDecoder::default();
        let (flags, seq, rssi, samples) =
            parse_kiwi_snd_frame(&packet, &mut adpcm).expect("SND frame should parse");
        assert_eq!(flags, 0x05);
        assert_eq!(seq, 1);
        assert!((rssi - (-102.0)).abs() < 0.01);
        assert_eq!(samples.len(), 64);

        let wf = compute_dft_waterfall_row(&samples, 60);
        assert_eq!(wf.len(), 60);
        assert!(wf.iter().any(|&v| v > 0));
    }

    #[tokio::test]
    async fn test_live_kiwisdr_directory_and_snd_stream() {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .user_agent("ArgusOSINT/2.0")
            .build()
            .unwrap();
        let nodes = fetch_public_kiwisdr_nodes(&client).await;
        assert!(
            !nodes.is_empty(),
            "Should parse active HF KiwiSDR nodes from rx.linkfanel.net"
        );
        println!("Found {} active KiwiSDR nodes. Top: {} ({})", nodes.len(), nodes[0].host_port, nodes[0].location);

        let state = Arc::new(RwLock::new(AppState::new(
            argus_core::config::AppConfig::default(),
        )));
        {
            let mut st = state.write();
            st.sdr_nodes = nodes;
            st.is_radio_playing = true;
            st.current_frequency_khz = 11175.0;
        }

        let st_worker = Arc::clone(&state);
        let handle = tokio::spawn(async move {
            run_kiwisdr_worker(st_worker).await;
        });

        let mut received_samples = 0usize;
        for _ in 0..30 {
            sleep(Duration::from_millis(250)).await;
            let st = state.read();
            let q_len: usize = st.sdr_audio_queue.lock().iter().map(|(_, v)| v.len()).sum();
            if q_len > 0 {
                received_samples = q_len;
                println!(
                    "Live KiwiSDR audio streaming verified! Host: {}, RSSI: {:.1} dBm, Queued PCM samples: {}",
                    st.sdr_connected_host, st.sdr_rssi_dbm, received_samples
                );
                break;
            }
        }
        handle.abort();
        assert!(
            received_samples > 0,
            "Should receive real live SND PCM audio samples from KiwiSDR"
        );
    }
}
