use crate::config::AppConfig;
use crate::geo::haversine_km;
use crate::mouse::HitTester;
use crate::types::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::Path;

/// Bounded RingBuffer (Sabit bellek tavanlı dairesel tampon)
#[derive(Debug, Clone)]
pub struct RingBuffer<T> {
    buffer: VecDeque<T>,
    capacity: usize,
}

impl<T> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, item: T) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(item);
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buffer.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.buffer.iter_mut()
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.buffer.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.buffer.get_mut(index)
    }

    pub fn push_if_room(&mut self, item: T) -> bool {
        if self.buffer.len() < self.capacity {
            self.buffer.push_back(item);
            true
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.buffer.retain(f);
    }

    pub fn upsert_by<F>(&mut self, item: T, is_same: F)
    where
        F: FnMut(&T) -> bool,
    {
        if let Some(idx) = self.buffer.iter().position(is_same) {
            self.buffer[idx] = item;
        } else if self.buffer.len() < self.capacity {
            // Kapasite dolana kadar sona ekle; kapasite dolduğunda pop_front() yapıp
            // mevcut tablo satırlarının indekslerini kaydırma! (Eski kayıtlar prune_stale_targets ile temizlenir)
            self.buffer.push_back(item);
        }
    }
}

impl<T> Default for RingBuffer<T> {
    fn default() -> Self {
        Self::new(200)
    }
}

pub type SdrAudioQueue = std::sync::Arc<parking_lot::Mutex<VecDeque<(u32, Vec<f32>)>>>;

/// Merkezi Operasyonel Durum (Thread-safe AppState)
#[derive(Debug, Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub station_lat: f64,
    pub station_lon: f64,
    
    // Sentinel Çalışma Alanı / Ekran Seçimi (DASHBOARD, GEOINT, SIGINT, OSINT, vb.)
    pub active_view: ActiveView,
    pub previous_view: ActiveView,

    // Pencere & Mozaik Yönetimi (Dashboard içindeki paneller)
    pub active_tile: usize,            // 0: P100 Harita, 1: P200 Haber, 2: P300 Hava/Deniz, 3: P400 Uydu, 4: P500 Siber, 5: P600 Afet
    pub zoomed_tile: Option<usize>,    // None: Mozaik (Split-Tile), Some(i): Tam Ekran Zoom
    pub minimized_tiles: [bool; 6],
    pub is_start_menu_open: bool,
    
    // Win98 Diyalog Kutusu (Dossier Popup)
    pub is_dialog_open: bool,
    pub dialog_title: String,
    pub dialog_content: Vec<String>,
    pub dialog_scroll: u16,
    pub dialog_aircraft_icao: Option<String>,

    // Canlı Uçak Fotoğrafı & Önbellek (Inspector + Dossier ortak)
    pub aircraft_media_status: AircraftMediaStatus,
    pub aircraft_photo_cache: HashMap<String, AircraftPhotoEntry>,
    pub aircraft_photo_order: VecDeque<String>,
    pub inflight_photo_icaos: HashSet<String>,
    pub pending_photo_slot: Option<(String, (u16, u16, u16, u16))>,
    pub force_terminal_clear: bool,
    
    // Harita Görünüm Parametreleri
    pub map_center_lon: f64,
    pub map_center_lat: f64,
    pub map_zoom: f64,                 // 1.0 = Dünya Geneli, >1.0 = Yakınlaşma (Zoom In)
    pub map_high_contrast: bool,
    pub map_style: MapRenderStyle,
    pub map_aircraft_shapes: bool,
    pub geoint_layers: u16,            // Bitmask: 0..6 klasik, 7=SDR, 8=Kablo(K-1), 9=HavaDurumu(A-3), 10=Fuzyon(K-4)
    pub selected_target_id: Option<String>,
    pub follow_target_id: Option<String>,
    pub target_trails: HashMap<String, Vec<(f64, f64)>>,
    pub gps_jamming_zones: Vec<(f64, f64, f64, String)>,
    pub defcon_status: Option<DefconStatus>,
    pub subsea_cables: Vec<SubseaCable>,
    pub weather_points: Vec<WeatherPoint>,
    pub station_weather: Option<WeatherPoint>,
    pub flight_schedule_cache: HashMap<String, FlightScheduleInfo>,
    pub inflight_schedule_keys: HashSet<String>,
    pub track_anomalies: HashMap<String, String>,
    pub fusion_clusters: Vec<FusionCluster>,
    pub ai_analysis_running: bool,
    pub ai_last_report: Option<(chrono::DateTime<chrono::Utc>, String)>,
    
    // Canlı Veri Akışları (Ring Buffers)
    pub targets: RingBuffer<TargetTrack>,
    pub air_targets: RingBuffer<TargetTrack>,
    pub sea_targets: RingBuffer<TargetTrack>,
    pub satellites: Vec<SatellitePass>,
    pub news: RingBuffer<OsintNews>,
    pub cyber_incidents: RingBuffer<CyberIncident>,
    pub earthquakes: RingBuffer<Earthquake>,
    pub firms_fires: RingBuffer<FirmsFire>,
    pub acled_events: RingBuffer<AcledEvent>,
    pub radiation_measurements: RingBuffer<RadiationMeasurement>,
    pub solar_weather: Option<SolarWeather>,
    
    // SIGINT, Canlı KiwiSDR Alıcı ve EAM Akışı
    pub eam_messages: RingBuffer<EamMessage>,
    pub radio_presets: Vec<RadioPreset>,
    pub current_frequency_khz: f64,
    pub sdr_mode: String,
    pub sdr_freq_input_active: bool,
    pub sdr_freq_input: String,
    pub sdr_recording: bool,
    pub sdr_rec_path: String,
    pub is_radio_playing: bool,
    pub waterfall_data: Vec<Vec<u8>>,
    pub sdr_nodes: Vec<SdrNodeInfo>,
    pub active_sdr_idx: usize,
    pub sdr_auto_failover: bool,
    pub sdr_connected_host: String,
    pub sdr_rssi_dbm: f32,
    pub sdr_sample_rate: u32,
    pub sdr_status: String,
    pub sdr_audio_queue: SdrAudioQueue,
    // Opsiyonel MEGA-C SIGINT DSP modulleri (varsayilan hepsi kapali)
    pub sdr_scanner_enabled: bool,
    pub sdr_vox_enabled: bool,
    pub sdr_cw_enabled: bool,
    pub sdr_snr_db: f32,
    pub sdr_scan_locked: bool,
    pub sdr_scan_hits: VecDeque<SdrScanHit>,
    pub sdr_cw_text: String,
    pub sdr_cw_wpm: u16,

    // Master-Detail Sağ Denetçi Paneli, İzleme Listesi ve Otomatik Alarmlar
    pub show_inspector: bool,
    pub selected_domain: usize,
    pub selected_row: usize,
    pub watchlist: HashSet<String>,
    pub alerts: RingBuffer<AlertItem>,
    
    // Shodan Önbelleği (IP -> ShodanHostInfo)
    pub shodan_recon_cache: HashMap<String, ShodanHostInfo>,
    pub shodan_input_ip: String,
    
    // Win98 Donatılar: Not Defteri ve Hesap Makinesi
    pub notepad: NotepadState,
    pub calculator: CalcState,

    // Liste ve Tablo Kaydırma Konumları
    pub news_scroll: usize,
    pub target_scroll: usize,
    pub air_scroll: usize,
    pub sea_scroll: usize,
    pub sat_scroll: usize,
    pub cyber_scroll: usize,
    pub geo_scroll: usize,
    pub eam_scroll: usize,
    pub sdr_scroll: usize,
    pub sigint_focus: usize,           // 0: KiwiSDR Alıcı Listesi (Varsayılan), 1: EAM Tablosu
    pub masint_tab: usize,             // 0: Depremler (USGS/Kandilli/EMSC), 1: FIRMS Termal, 2: Safecast CBRN
    pub radiation_scroll: usize,
    
    // Filtreler (Hava Küresel/CENTCOM/Askeri ve Deniz Seçimi)
    pub air_filter: usize,             // 0: Küresel Tümü, 1: CENTCOM Tümü, 2: CENTCOM Askeri, 3: Küresel Askeri, 4: TR/EUCOM, 5: 7700 Acil
    pub sea_filter: usize,             // 0: Tümü, 1: Savaş Gemisi, 2: Sahil Güvenlik, 3: Ticari
    pub news_filter: usize,            // 0: All, 1: P201 Küresel, 2: P202 Askeri/Kriz, 3: P203 Siber, 4: P204 Enerji
    pub air_sort_col: usize,           // 0: Varsayılan, 1: Çağrı Adı, 2: Sınıf, 3: İrtifa, 4: Hız
    pub air_sort_asc: bool,
    pub sea_sort_col: usize,           // 0: Varsayılan, 1: Gemi Adı, 2: Sınıf, 3: Hız
    pub sea_sort_asc: bool,
    
    // Win98 BIOS / POST Açılış Ekranı
    pub is_splash_active: bool,
    pub splash_progress: u8,
    pub splash_ticks: u32,
    pub splash_started_at: std::time::Instant,

    // Durum ve Kontroller
    pub audio_muted: bool,
    pub search_query: String,
    pub is_search_active: bool,
    pub total_events_received: u64,
    pub live_status: String,

    // Sirali hedef indeks onbellegi (15000+ hedefte her kare yeniden siralamayi onler)
    pub air_version: u64,
    pub sea_version: u64,
    pub targets_version: u64,
    cached_air_key: (u64, usize, usize, bool),
    pub cached_air_indices: Vec<usize>,
    cached_sea_key: (u64, usize, usize, bool),
    pub cached_sea_indices: Vec<usize>,
    cached_prox_key: u64,
    pub cached_prox_indices: Vec<usize>,
    
    // Fare Hit Tester
    pub hit_tester: HitTester,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let lat = config.station.default_lat;
        let lon = config.station.default_lon;
        let audio_muted = !config.audio.enabled;

        let mut state = Self {
            config,
            station_lat: lat,
            station_lon: lon,
            active_view: ActiveView::Dashboard,
            previous_view: ActiveView::Dashboard,
            active_tile: 0,
            zoomed_tile: None,
            minimized_tiles: [false; 6],
            is_start_menu_open: false,
            is_dialog_open: false,
            dialog_title: String::new(),
            dialog_content: Vec::new(),
            dialog_scroll: 0,
            dialog_aircraft_icao: None,
            aircraft_media_status: AircraftMediaStatus::Idle,
            aircraft_photo_cache: HashMap::new(),
            aircraft_photo_order: VecDeque::new(),
            inflight_photo_icaos: HashSet::new(),
            pending_photo_slot: None,
            force_terminal_clear: false,
            map_center_lon: lon,
            map_center_lat: lat,
            map_zoom: 1.0,
            map_high_contrast: true,
            map_style: MapRenderStyle::HalfBlockTerrain,
            map_aircraft_shapes: false,
            geoint_layers: 0b0000_0100_0111_1111, // Klasik katmanlar + K-4 Fuzyon (bit 10) acik; SDR/Kablo/HavaDur opt-in
            selected_target_id: None,
            follow_target_id: None,
            target_trails: HashMap::new(),
            gps_jamming_zones: Vec::new(),
            defcon_status: None,
            subsea_cables: Vec::new(),
            weather_points: Vec::new(),
            station_weather: None,
            flight_schedule_cache: HashMap::new(),
            inflight_schedule_keys: HashSet::new(),
            track_anomalies: HashMap::new(),
            fusion_clusters: Vec::new(),
            ai_analysis_running: false,
            ai_last_report: None,
            targets: RingBuffer::new(30000),
            air_targets: RingBuffer::new(22000),
            sea_targets: RingBuffer::new(10000),
            satellites: Vec::with_capacity(64),
            news: RingBuffer::new(200),
            cyber_incidents: RingBuffer::new(200),
            earthquakes: RingBuffer::new(100),
            firms_fires: RingBuffer::new(300),
            acled_events: RingBuffer::new(150),
            radiation_measurements: RingBuffer::new(100),
            solar_weather: None,
            eam_messages: RingBuffer::new(100),
            radio_presets: vec![
                RadioPreset {
                    name: "HFGCS Küresel 1".to_string(),
                    frequency_khz: 11175.0,
                    mode: "USB".to_string(),
                    description: "USAF HFGCS Birincil Küresel EAM ve Komuta Frekansı".to_string(),
                    stream_url: Some("http://websdr.ewi.utwente.nl:8901/".to_string()),
                },
                RadioPreset {
                    name: "HFGCS Küresel 2".to_string(),
                    frequency_khz: 8992.0,
                    mode: "USB".to_string(),
                    description: "USAF HFGCS İkincil Küresel Taktik Frekansı".to_string(),
                    stream_url: None,
                },
                RadioPreset {
                    name: "HFGCS CENTCOM".to_string(),
                    frequency_khz: 6739.0,
                    mode: "USB".to_string(),
                    description: "USAF HFGCS / CENTCOM Orta Bant Komuta Frekansı".to_string(),
                    stream_url: None,
                },
                RadioPreset {
                    name: "HFGCS Gündüz".to_string(),
                    frequency_khz: 13200.0,
                    mode: "USB".to_string(),
                    description: "USAF HFGCS / EUCOM-CENTCOM Gündüz Hava-Yer Kanalı".to_string(),
                    stream_url: None,
                },
                RadioPreset {
                    name: "HFGCS Andrews".to_string(),
                    frequency_khz: 4724.0,
                    mode: "USB".to_string(),
                    description: "HFGCS Gece / Düşük Bant Yayın Kanalı".to_string(),
                    stream_url: None,
                },
                RadioPreset {
                    name: "HFGCS Offutt".to_string(),
                    frequency_khz: 15016.0,
                    mode: "USB".to_string(),
                    description: "USSTRATCOM Yüksek Bant Gündüz Kanalı".to_string(),
                    stream_url: None,
                },
                RadioPreset {
                    name: "UVB-76 Buzzer".to_string(),
                    frequency_khz: 4625.0,
                    mode: "AM".to_string(),
                    description: "Rusya Batı Askeri Bölgesi Taktik Çağrı İstasyonu (MDZhB)".to_string(),
                    stream_url: None,
                },
                RadioPreset {
                    name: "The Pip".to_string(),
                    frequency_khz: 5448.0,
                    mode: "USB".to_string(),
                    description: "Güney Askeri Bölgesi Periyodik Marker (Rostov-on-Don)".to_string(),
                    stream_url: None,
                },
                RadioPreset {
                    name: "Shannon VOLMET".to_string(),
                    frequency_khz: 5505.0,
                    mode: "USB".to_string(),
                    description: "Kuzey Atlantik Havacılık ve Meteoroloji Telsiz Yayını".to_string(),
                    stream_url: None,
                },
            ],
            current_frequency_khz: 11175.0,
            sdr_mode: "USB".to_string(),
            sdr_freq_input_active: false,
            sdr_freq_input: String::new(),
            sdr_recording: false,
            sdr_rec_path: String::new(),
            is_radio_playing: false,
            waterfall_data: Vec::new(),
            sdr_nodes: Vec::new(),
            active_sdr_idx: 0,
            sdr_auto_failover: true,
            sdr_connected_host: String::new(),
            sdr_rssi_dbm: -120.0,
            sdr_sample_rate: 12000,
            sdr_status: "BEKLEMEDE (S ile Canli KiwiSDR Baglantisi Baslatin)".to_string(),
            sdr_audio_queue: std::sync::Arc::new(parking_lot::Mutex::new(VecDeque::with_capacity(32))),
            sdr_scanner_enabled: false,
            sdr_vox_enabled: false,
            sdr_cw_enabled: false,
            sdr_snr_db: 0.0,
            sdr_scan_locked: false,
            sdr_scan_hits: VecDeque::with_capacity(24),
            sdr_cw_text: String::new(),
            sdr_cw_wpm: 0,
            show_inspector: true,
            selected_domain: 20,
            selected_row: 0,
            watchlist: HashSet::new(),
            alerts: RingBuffer::new(100),
            shodan_recon_cache: HashMap::new(),
            shodan_input_ip: "1.1.1.1".to_string(),
            notepad: NotepadState::default(),
            calculator: CalcState::default(),
            news_scroll: 0,
            target_scroll: 0,
            air_scroll: 0,
            sea_scroll: 0,
            sat_scroll: 0,
            cyber_scroll: 0,
            geo_scroll: 0,
            eam_scroll: 0,
            sdr_scroll: 0,
            sigint_focus: 0,
            masint_tab: 0,
            radiation_scroll: 0,
            air_filter: 0,
            sea_filter: 0,
            news_filter: 0,
            air_sort_col: 0,
            air_sort_asc: false,
            sea_sort_col: 0,
            sea_sort_asc: false,
            is_splash_active: false,
            splash_progress: 100,
            splash_ticks: 0,
            splash_started_at: std::time::Instant::now(),
            audio_muted,
            search_query: String::new(),
            is_search_active: false,
            total_events_received: 0,
            live_status: "SENTINEL AKTIF".to_string(),
            air_version: 1,
            sea_version: 1,
            targets_version: 1,
            cached_air_key: (0, 0, 0, false),
            cached_air_indices: Vec::new(),
            cached_sea_key: (0, 0, 0, false),
            cached_sea_indices: Vec::new(),
            cached_prox_key: 0,
            cached_prox_indices: Vec::new(),
            hit_tester: HitTester::new(),
        };

        state.load_notepad();
        state
    }

    pub fn set_view(&mut self, view: ActiveView) {
        if self.active_view != view {
            self.previous_view = self.active_view;
            self.active_view = view;
        }
    }

    pub fn advance_splash(&mut self) {
        if self.is_splash_active {
            self.splash_ticks += 1;
            let elapsed_ms = self.splash_started_at.elapsed().as_millis();
            let time_prog = ((elapsed_ms * 100) / 2500).min(100) as u8;
            // 25 ticks @ 100ms = 2.5 sn sonra otomatik Dashboard'a geç
            let tick_prog = (self.splash_ticks * 4).min(100) as u8;
            self.splash_progress = time_prog.max(tick_prog);
            if self.splash_progress >= 100 {
                self.dismiss_splash();
            }
        }
    }

    pub fn dismiss_splash(&mut self) {
        self.is_splash_active = false;
        self.splash_progress = 100;
        if self.active_view == ActiveView::Splash {
            self.active_view = ActiveView::Dashboard;
        }
    }

    pub fn is_centcom_track(t: &TargetTrack) -> bool {
        t.origin.as_deref() == Some("CENTCOM")
            || ((12.0..=36.8).contains(&t.geo.lat) && (31.5..=75.0).contains(&t.geo.lon))
            || ((36.8..=40.0).contains(&t.geo.lat) && (44.8..=75.0).contains(&t.geo.lon))
    }

    pub fn aircraft_sorted_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = self
            .air_targets
            .iter()
            .enumerate()
            .filter_map(|(idx, t)| {
                let keep = match self.air_filter {
                    1 => Self::is_centcom_track(t),
                    2 => Self::is_centcom_track(t) && t.is_military,
                    3 => t.is_military,
                    4 => t.origin.as_deref() == Some("TR/EUCOM"),
                    5 => matches!(t.squawk.as_deref(), Some("7700") | Some("7600") | Some("7500")),
                    _ => true,
                };
                if keep { Some(idx) } else { None }
            })
            .collect();

        let targets = &self.air_targets;
        match self.air_sort_col {
            1 => indices.sort_by(|&ia, &ib| {
                let a = targets.get(ia).unwrap();
                let b = targets.get(ib).unwrap();
                let cmp = a.callsign.cmp(&b.callsign);
                if self.air_sort_asc { cmp } else { cmp.reverse() }
            }),
            2 => indices.sort_by(|&ia, &ib| {
                let a = targets.get(ia).unwrap();
                let b = targets.get(ib).unwrap();
                let cmp = a.is_military.cmp(&b.is_military);
                if self.air_sort_asc { cmp } else { cmp.reverse() }
            }),
            3 => indices.sort_by(|&ia, &ib| {
                let alt_a = targets.get(ia).and_then(|t| t.geo.alt_m).unwrap_or(-1.0);
                let alt_b = targets.get(ib).and_then(|t| t.geo.alt_m).unwrap_or(-1.0);
                let cmp = alt_a.partial_cmp(&alt_b).unwrap_or(std::cmp::Ordering::Equal);
                if self.air_sort_asc { cmp } else { cmp.reverse() }
            }),
            4 => indices.sort_by(|&ia, &ib| {
                let spd_a = targets.get(ia).and_then(|t| t.speed_kts).unwrap_or(-1.0);
                let spd_b = targets.get(ib).and_then(|t| t.speed_kts).unwrap_or(-1.0);
                let cmp = spd_a.partial_cmp(&spd_b).unwrap_or(std::cmp::Ordering::Equal);
                if self.air_sort_asc { cmp } else { cmp.reverse() }
            }),
            5 => {
                let st_lat = self.station_lat;
                let st_lon = self.station_lon;
                indices.sort_by(|&ia, &ib| {
                    let a = targets.get(ia).unwrap();
                    let b = targets.get(ib).unwrap();
                    let da = haversine_km(st_lat, st_lon, a.geo.lat, a.geo.lon);
                    let db = haversine_km(st_lat, st_lon, b.geo.lat, b.geo.lon);
                    let cmp = da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal);
                    if self.air_sort_asc { cmp } else { cmp.reverse() }
                });
            }
            _ => {
                indices.sort_by(|&ia, &ib| {
                    let a = targets.get(ia).unwrap();
                    let b = targets.get(ib).unwrap();
                    b.is_military
                        .cmp(&a.is_military)
                        .then_with(|| Self::is_centcom_track(b).cmp(&Self::is_centcom_track(a)))
                });
            }
        }
        indices
    }

    pub fn ensure_air_indices(&mut self) -> &[usize] {
        let key = (self.air_version, self.air_filter, self.air_sort_col, self.air_sort_asc);
        if self.cached_air_key != key {
            self.cached_air_indices = self.aircraft_sorted_indices();
            self.cached_air_key = key;
        }
        &self.cached_air_indices
    }

    pub fn aircraft_at_sorted(&mut self, pos: usize) -> Option<&TargetTrack> {
        let idx = *self.ensure_air_indices().get(pos)?;
        self.air_targets.get(idx)
    }

    pub fn aircraft_at_visible(&mut self, pos: usize) -> Option<&TargetTrack> {
        let q = self.search_query.to_lowercase();
        self.ensure_air_indices();
        if q.is_empty() {
            let idx = *self.cached_air_indices.get(pos)?;
            return self.air_targets.get(idx);
        }
        let mut match_idx = 0usize;
        for &buf_idx in &self.cached_air_indices {
            if let Some(t) = self.air_targets.get(buf_idx) {
                if t.callsign.to_lowercase().contains(&q)
                    || t.id.to_lowercase().contains(&q)
                    || t.operator_country.as_deref().unwrap_or("").to_lowercase().contains(&q)
                    || t.operator_branch.as_deref().unwrap_or("").to_lowercase().contains(&q)
                    || t.info.to_lowercase().contains(&q)
                {
                    if match_idx == pos {
                        return Some(t);
                    }
                    match_idx += 1;
                }
            }
        }
        None
    }

    pub fn aircraft_list(&self) -> Vec<TargetTrack> {
        self.aircraft_sorted_indices()
            .into_iter()
            .filter_map(|idx| self.air_targets.get(idx).cloned())
            .collect()
    }

    pub fn ensure_proximity_indices(&mut self) -> &[usize] {
        if self.cached_prox_key != self.targets_version {
            let st_lat = self.station_lat;
            let st_lon = self.station_lon;
            let mut scored: Vec<(usize, bool, f64)> = self
                .targets
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let d = haversine_km(st_lat, st_lon, t.geo.lat, t.geo.lon);
                    (i, t.is_military, d)
                })
                .collect();
            scored.sort_by(|a, b| {
                b.1.cmp(&a.1)
                    .then_with(|| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
            });
            self.cached_prox_indices = scored.into_iter().map(|(i, _, _)| i).collect();
            self.cached_prox_key = self.targets_version;
        }
        &self.cached_prox_indices
    }

    fn evaluate_target_alert(&self, track: &TargetTrack) -> Option<(ThreatSeverity, String)> {
        let id = &track.id;
        let dist_km = haversine_km(self.station_lat, self.station_lon, track.geo.lat, track.geo.lon);
        let is_emerg = matches!(track.squawk.as_deref(), Some("7700") | Some("7600") | Some("7500"));
        if is_emerg {
            return Some((
                ThreatSeverity::Emergency,
                format!(
                    "ACIL SQUAWK {}: {} ({}) - {:.0} km",
                    track.squawk.as_deref().unwrap_or(""),
                    track.callsign,
                    id,
                    dist_km
                ),
            ));
        }
        if self.watchlist.contains(id) {
            return Some((
                ThreatSeverity::Critical,
                format!("IZLEME LISTESI TESPITI: {} ({}) - {:.0} km", track.callsign, id, dist_km),
            ));
        }
        if track.is_military && dist_km <= 180.0 {
            return Some((
                ThreatSeverity::Critical,
                format!("YAKIN ASKERI HEDEF: {} ({}) - {:.0} km", track.callsign, id, dist_km),
            ));
        }
        if track.is_military {
            const CORRIDORS: &[(&str, f64, f64, f64)] = &[
                ("Istanbul Bogazi", 41.12, 29.07, 45.0),
                ("Canakkale Bogazi", 40.15, 26.40, 45.0),
                ("Hurmuz Bogazi", 26.56, 56.25, 90.0),
                ("Bab el-Mandep", 12.60, 43.35, 90.0),
                ("Suveys Kanali", 30.58, 32.33, 65.0),
            ];
            for &(cname, clat, clon, radius_km) in CORRIDORS {
                let d_corr = haversine_km(clat, clon, track.geo.lat, track.geo.lon);
                if d_corr <= radius_km {
                    return Some((
                        ThreatSeverity::Warning,
                        format!(
                            "GEOFENCE [{}]: {} ({}) - {:.0} km",
                            cname, track.callsign, id, d_corr
                        ),
                    ));
                }
            }
        }
        None
    }

    pub fn upsert_targets_batch(&mut self, tracks: Vec<TargetTrack>) {
        if tracks.is_empty() {
            return;
        }
        let mut air_idx: HashMap<String, usize> = self
            .air_targets
            .iter()
            .enumerate()
            .map(|(i, t)| (t.id.clone(), i))
            .collect();
        let mut target_idx: HashMap<String, usize> = self
            .targets
            .iter()
            .enumerate()
            .map(|(i, t)| (t.id.clone(), i))
            .collect();
        let mut alert_idx: HashMap<String, usize> = self
            .alerts
            .iter()
            .enumerate()
            .filter_map(|(i, a)| a.target_id.as_ref().map(|tid| (tid.clone(), i)))
            .collect();

        for track in tracks {
            let id = track.id.clone();
            let pt = (track.geo.lon, track.geo.lat);
            let trail = self.target_trails.entry(id.clone()).or_default();
            let should_push = match trail.last() {
                Some(&(last_lon, last_lat)) => {
                    (last_lon - pt.0).abs() > 0.0005 || (last_lat - pt.1).abs() > 0.0005
                }
                None => true,
            };
            if should_push {
                if trail.len() >= 15 {
                    trail.remove(0);
                }
                trail.push(pt);
            }
            if self.follow_target_id.as_deref() == Some(id.as_str()) {
                self.map_center_lat = track.geo.lat;
                self.map_center_lon = track.geo.lon;
            }

            if let Some((sev, msg)) = self.evaluate_target_alert(&track) {
                let item = AlertItem {
                    timestamp: chrono::Utc::now(),
                    domain: track.domain.as_str().to_string(),
                    severity: sev,
                    message: msg,
                    target_id: Some(id.clone()),
                };
                if let Some(&idx) = alert_idx.get(&id) {
                    if let Some(slot) = self.alerts.get_mut(idx) {
                        *slot = item;
                    }
                } else {
                    let new_idx = self.alerts.len();
                    if self.alerts.push_if_room(item) {
                        alert_idx.insert(id.clone(), new_idx);
                    }
                }
            }

            if track.domain == Domain::Air {
                if let Some(&idx) = air_idx.get(&id) {
                    if let Some(slot) = self.air_targets.get_mut(idx) {
                        *slot = track.clone();
                    }
                } else {
                    let new_idx = self.air_targets.len();
                    if self.air_targets.push_if_room(track.clone()) {
                        air_idx.insert(id.clone(), new_idx);
                    }
                }
            }

            if let Some(&idx) = target_idx.get(&id) {
                if let Some(slot) = self.targets.get_mut(idx) {
                    *slot = track;
                }
            } else {
                let new_idx = self.targets.len();
                if self.targets.push_if_room(track) {
                    target_idx.insert(id, new_idx);
                }
            }
            self.total_events_received += 1;
        }
        self.air_version = self.air_version.wrapping_add(1);
        self.targets_version = self.targets_version.wrapping_add(1);
    }

    pub fn upsert_target(&mut self, mut track: TargetTrack) {
        let id = track.id.clone();

        // Eğer hedef zaten listedeyse ve yeni pakette çağrı adı boş/jenerik geldiyse eski gerçek adını koru
        if track.domain == Domain::Sea
            && (track.callsign == "VESSEL" || track.callsign.starts_with("MMSI:"))
        {
            if let Some(existing) = self.sea_targets.iter().find(|t| t.id == id) {
                if existing.callsign != "VESSEL" && !existing.callsign.starts_with("MMSI:") {
                    track.callsign = existing.callsign.clone();
                    if track.vessel_type.as_deref() == Some("Ticari Gemi") {
                        track.vessel_type = existing.vessel_type.clone();
                        track.airframe_or_vessel_class = existing.airframe_or_vessel_class.clone();
                        track.is_military = existing.is_military;
                    }
                }
            }
        }
        let pt = (track.geo.lon, track.geo.lat);
        let trail = self.target_trails.entry(id.clone()).or_default();
        let should_push = match trail.last() {
            Some(&(last_lon, last_lat)) => {
                (last_lon - pt.0).abs() > 0.0005 || (last_lat - pt.1).abs() > 0.0005
            }
            None => true,
        };
        if should_push {
            if trail.len() >= 15 {
                trail.remove(0);
            }
            trail.push(pt);
        }
        if self.follow_target_id.as_deref() == Some(id.as_str()) {
            self.map_center_lat = track.geo.lat;
            self.map_center_lon = track.geo.lon;
        }

        if let Some((sev, msg)) = self.evaluate_target_alert(&track) {
            self.alerts.upsert_by(
                AlertItem {
                    timestamp: chrono::Utc::now(),
                    domain: track.domain.as_str().to_string(),
                    severity: sev,
                    message: msg,
                    target_id: Some(id.clone()),
                },
                |a| a.target_id.as_deref() == Some(id.as_str()),
            );
        }

        match track.domain {
            Domain::Air => {
                self.air_targets.upsert_by(track.clone(), |t| t.id == id);
                self.air_version = self.air_version.wrapping_add(1);
            }
            Domain::Sea => {
                self.sea_targets.upsert_by(track.clone(), |t| t.id == id);
                self.sea_version = self.sea_version.wrapping_add(1);
            }
            _ => {}
        }
        self.targets.upsert_by(track, |t| t.id == id);
        self.targets_version = self.targets_version.wrapping_add(1);
    }

    pub fn prune_stale_targets(&mut self, max_age_secs: i64) {
        let now = chrono::Utc::now();
        let targets_before = self.targets.len();
        let air_before = self.air_targets.len();
        let sea_before = self.sea_targets.len();
        self.targets
            .retain(|t| (now - t.last_seen).num_seconds() <= max_age_secs);
        self.air_targets
            .retain(|t| (now - t.last_seen).num_seconds() <= max_age_secs);
        self.sea_targets
            .retain(|t| (now - t.last_seen).num_seconds() <= max_age_secs);
        if self.targets.len() != targets_before {
            self.targets_version = self.targets_version.wrapping_add(1);
        }
        if self.air_targets.len() != air_before {
            self.air_version = self.air_version.wrapping_add(1);
        }
        if self.sea_targets.len() != sea_before {
            self.sea_version = self.sea_version.wrapping_add(1);
        }
        let active_ids: std::collections::HashSet<&str> =
            self.targets.iter().map(|t| t.id.as_str()).collect();
        self.target_trails
            .retain(|id, _| active_ids.contains(id.as_str()));
    }

    pub fn upsert_cyber_incident(&mut self, inc: CyberIncident) {
        let id = inc.id.clone();
        self.cyber_incidents.upsert_by(inc, |c| c.id == id);
    }

    pub fn vessel_sorted_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = self
            .sea_targets
            .iter()
            .enumerate()
            .filter_map(|(idx, t)| {
                let keep = match self.sea_filter {
                    1 => t.is_military || t.vessel_type.as_deref() == Some("Warship") || t.vessel_type.as_deref() == Some("Savaş Gemisi") || t.callsign.starts_with("TCG") || t.callsign.starts_with("USS") || t.callsign.starts_with("HMS"),
                    2 => t.vessel_type.as_deref() == Some("Coast Guard") || t.vessel_type.as_deref() == Some("Sahil Güvenlik") || t.callsign.starts_with("TCSG") || t.vessel_type.as_deref() == Some("Law Enforcement") || t.vessel_type.as_deref() == Some("SAR"),
                    3 => !t.is_military && !t.callsign.starts_with("TCG") && !t.callsign.starts_with("USS") && !t.callsign.starts_with("HMS") && !t.callsign.starts_with("TCSG") && t.vessel_type.as_deref() != Some("Coast Guard") && t.vessel_type.as_deref() != Some("Sahil Güvenlik") && t.vessel_type.as_deref() != Some("Warship") && t.vessel_type.as_deref() != Some("Savaş Gemisi"),
                    _ => true,
                };
                if keep { Some(idx) } else { None }
            })
            .collect();

        let targets = &self.sea_targets;
        match self.sea_sort_col {
            1 => indices.sort_by(|&ia, &ib| {
                let a = targets.get(ia).unwrap();
                let b = targets.get(ib).unwrap();
                let cmp = a.callsign.cmp(&b.callsign);
                if self.sea_sort_asc { cmp } else { cmp.reverse() }
            }),
            2 => indices.sort_by(|&ia, &ib| {
                let a = targets.get(ia).unwrap();
                let b = targets.get(ib).unwrap();
                let cmp = a.is_military.cmp(&b.is_military);
                if self.sea_sort_asc { cmp } else { cmp.reverse() }
            }),
            3 => indices.sort_by(|&ia, &ib| {
                let spd_a = targets.get(ia).and_then(|t| t.speed_kts).unwrap_or(-1.0);
                let spd_b = targets.get(ib).and_then(|t| t.speed_kts).unwrap_or(-1.0);
                let cmp = spd_a.partial_cmp(&spd_b).unwrap_or(std::cmp::Ordering::Equal);
                if self.sea_sort_asc { cmp } else { cmp.reverse() }
            }),
            4 => {
                let st_lat = self.station_lat;
                let st_lon = self.station_lon;
                indices.sort_by(|&ia, &ib| {
                    let a = targets.get(ia).unwrap();
                    let b = targets.get(ib).unwrap();
                    let da = haversine_km(st_lat, st_lon, a.geo.lat, a.geo.lon);
                    let db = haversine_km(st_lat, st_lon, b.geo.lat, b.geo.lon);
                    let cmp = da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal);
                    if self.sea_sort_asc { cmp } else { cmp.reverse() }
                });
            }
            _ => {}
        }
        indices
    }

    pub fn ensure_sea_indices(&mut self) -> &[usize] {
        let key = (self.sea_version, self.sea_filter, self.sea_sort_col, self.sea_sort_asc);
        if self.cached_sea_key != key {
            self.cached_sea_indices = self.vessel_sorted_indices();
            self.cached_sea_key = key;
        }
        &self.cached_sea_indices
    }

    pub fn vessel_at_sorted(&mut self, pos: usize) -> Option<&TargetTrack> {
        let idx = *self.ensure_sea_indices().get(pos)?;
        self.sea_targets.get(idx)
    }

    pub fn vessel_list(&self) -> Vec<TargetTrack> {
        self.vessel_sorted_indices()
            .into_iter()
            .filter_map(|idx| self.sea_targets.get(idx).cloned())
            .collect()
    }

    pub fn upsert_radiation_measurement(&mut self, rad: RadiationMeasurement) {
        let id = rad.id;
        self.radiation_measurements.upsert_by(rad, |r| r.id == id);
    }

    pub fn upsert_earthquake(&mut self, q: Earthquake) {
        let id = q.id.clone();
        self.earthquakes.upsert_by(q, |item| item.id == id);
    }

    pub fn upsert_news(&mut self, item: OsintNews) {
        let id = item.id.clone();
        let title = item.title.clone();
        self.news.upsert_by(item, |n| n.id == id || n.title == title);
    }

    pub fn upsert_fire(&mut self, f: FirmsFire) {
        let lat = f.lat;
        let lon = f.lon;
        let d = f.acq_date.clone();
        let t = f.acq_time.clone();
        self.firms_fires.upsert_by(f, |item| {
            (item.lat - lat).abs() < 0.001 && (item.lon - lon).abs() < 0.001 && item.acq_date == d && item.acq_time == t
        });
    }

    pub fn upsert_acled_event(&mut self, ev: AcledEvent) {
        let id = ev.event_id.clone();
        self.acled_events.upsert_by(ev, |item| item.event_id == id);
    }

    pub fn toggle_zoom(&mut self, tile_idx: usize) {
        if self.zoomed_tile == Some(tile_idx) {
            self.zoomed_tile = None;
        } else {
            self.zoomed_tile = Some(tile_idx);
            self.active_tile = tile_idx;
        }
    }

    pub fn toggle_minimize(&mut self, tile_idx: usize) {
        if tile_idx < 6 {
            self.minimized_tiles[tile_idx] = !self.minimized_tiles[tile_idx];
            if self.zoomed_tile == Some(tile_idx) && self.minimized_tiles[tile_idx] {
                self.zoomed_tile = None;
            }
        }
    }

    pub fn open_dialog(&mut self, title: impl Into<String>, lines: Vec<String>) {
        self.dialog_title = title.into();
        self.dialog_content = lines;
        self.dialog_scroll = 0;
        self.dialog_aircraft_icao = None;
        self.is_dialog_open = true;
    }

    pub fn close_dialog(&mut self) {
        self.is_dialog_open = false;
        self.dialog_content.clear();
        if self.dialog_aircraft_icao.take().is_some() {
            self.force_terminal_clear = true;
        }
    }

    pub fn store_aircraft_photo(&mut self, entry: AircraftPhotoEntry) {
        let key = entry.icao.to_lowercase();
        if !self.aircraft_photo_cache.contains_key(&key) {
            if self.aircraft_photo_order.len() >= 48 {
                if let Some(old_k) = self.aircraft_photo_order.pop_front() {
                    self.aircraft_photo_cache.remove(&old_k);
                }
            }
            self.aircraft_photo_order.push_back(key.clone());
        }
        self.aircraft_photo_cache.insert(key, entry);
    }

    pub fn sync_air_selection(&mut self) {
        self.selected_domain = 20;
        self.selected_row = self.air_scroll;
        self.selected_target_id = self.aircraft_at_visible(self.air_scroll).map(|t| t.id.clone());
    }

    pub fn inspected_target(&mut self) -> Option<TargetTrack> {
        self.selected_target_id
            .as_ref()
            .and_then(|id| {
                self.air_targets
                    .iter()
                    .find(|t| &t.id == id)
                    .or_else(|| self.sea_targets.iter().find(|t| &t.id == id))
                    .or_else(|| self.targets.iter().find(|t| &t.id == id))
            })
            .cloned()
            .or_else(|| match self.selected_domain {
                20 => self.aircraft_at_sorted(self.selected_row).cloned(),
                21 => self.vessel_at_sorted(self.selected_row).cloned(),
                _ => self.aircraft_at_sorted(0).cloned(),
            })
    }

    pub fn load_notepad(&mut self) {
        let note_path = Path::new("notes.txt");
        if note_path.exists() {
            if let Ok(content) = fs::read_to_string(note_path) {
                self.notepad.content = content;
                self.notepad.status_message = "notes.txt yüklendi".to_string();
                return;
            }
        }
        self.notepad.content = "# PROJECT SENTINEL - Taktik Operasyon Not Defteri\n# [F5: Zaman Damgasi | Ctrl+S: Kaydet]\n".to_string();
    }

    pub fn save_notepad(&mut self) {
        if fs::write("notes.txt", &self.notepad.content).is_ok() {
            self.notepad.status_message = "notes.txt başarıyla kaydedildi".to_string();
        } else {
            self.notepad.status_message = "Hata: Dosyaya yazılamadı".to_string();
        }
    }

    pub fn export_intelligence_report(&mut self) -> Option<String> {
        let now = chrono::Utc::now();
        let filename = format!("sentinel_report_{}.md", now.format("%Y%m%d_%H%M%S"));
        let mut out = String::new();
        out.push_str(&format!(
            "# PROJECT SENTINEL - DURUMSAL FARKINDALIK RAPORU ({} UTC)\n\n",
            now.format("%Y-%m-%d %H:%M:%S")
        ));
        out.push_str(&format!(
            "- Istasyon Konumu: {:.4} N, {:.4} E\n",
            self.station_lat, self.station_lon
        ));
        out.push_str(&format!(
            "- Aktif Hava Hedefleri: {} | Deniz Hedefleri: {} | Uydular: {} | Depremler: {}\n\n",
            self.air_targets.len(),
            self.sea_targets.len(),
            self.satellites.len(),
            self.earthquakes.len()
        ));

        out.push_str("## 1. AKTIF ALARMLAR VE IZLEME LISTESI\n");
        for a in self.alerts.iter() {
            out.push_str(&format!(
                "- [{}] [{}] {}: {}\n",
                a.timestamp.format("%H:%M:%S"),
                a.severity.as_str(),
                a.domain,
                a.message
            ));
        }

        out.push_str("\n## 2. ASKERI VE KRITIK HAVA HEDEFLERI\n");
        for t in self.air_targets.iter().filter(|t| t.is_military) {
            let d = haversine_km(self.station_lat, self.station_lon, t.geo.lat, t.geo.lon);
            out.push_str(&format!(
                "- {} (ICAO:{}) | {:.3}N, {:.3}E | Mesafe: {:.1} km | {}\n",
                t.callsign, t.id, t.geo.lat, t.geo.lon, d, t.info
            ));
        }

        out.push_str("\n## 3. HFGCS EAM MESAJLARI\n");
        for m in self.eam_messages.iter() {
            out.push_str(&format!(
                "- [{}] {} ({:.1} kHz): {}\n",
                m.timestamp.format("%H:%M:%S"),
                m.preamble,
                m.frequency_khz,
                m.content
            ));
        }

        if fs::write(&filename, &out).is_ok() {
            self.live_status = format!("RAPOR DISA AKTARILDI: {}", filename);
            Some(filename)
        } else {
            None
        }
    }

    pub fn calc_input_char(&mut self, ch: char) {
        if ch.is_ascii_digit() || ch == '.' {
            if self.calculator.clear_on_next || self.calculator.display == "0" {
                self.calculator.display = ch.to_string();
                self.calculator.clear_on_next = false;
            } else {
                if ch == '.' && self.calculator.display.contains('.') {
                    return;
                }
                self.calculator.display.push(ch);
            }
        } else if ch == '+' || ch == '-' || ch == '*' || ch == '/' {
            if let Ok(val) = self.calculator.display.parse::<f64>() {
                self.calculator.current_value = val;
                self.calculator.pending_op = Some(ch);
                self.calculator.clear_on_next = true;
            }
        } else if ch == '=' || ch == '\n' {
            if let (Some(op), Ok(val)) = (self.calculator.pending_op, self.calculator.display.parse::<f64>()) {
                let res = match op {
                    '+' => self.calculator.current_value + val,
                    '-' => self.calculator.current_value - val,
                    '*' => self.calculator.current_value * val,
                    '/' => if val != 0.0 { self.calculator.current_value / val } else { 0.0 },
                    _ => val,
                };
                let entry = format!("{:.2} {} {:.2} = {:.2}", self.calculator.current_value, op, val, res);
                self.calculator.history.push(entry);
                self.calculator.display = format!("{:.2}", res);
                self.calculator.pending_op = None;
                self.calculator.clear_on_next = true;
            }
        } else if ch == 'c' || ch == 'C' {
            self.calculator.display = "0".to_string();
            self.calculator.current_value = 0.0;
            self.calculator.pending_op = None;
            self.calculator.clear_on_next = false;
        }

        if self.calculator.display.parse::<f64>().is_ok() {
            self.calculator.conv_input = self.calculator.display.clone();
            self.calc_convert();
        }
    }

    pub fn calc_convert(&mut self) {
        if let Ok(val) = self.calculator.conv_input.parse::<f64>() {
            self.calculator.conv_result = match self.calculator.conv_mode {
                0 => format!("{:.2} km/h ({:.2} mph)", val * 1.852, val * 1.15078),
                1 => format!("{:.2} km", val * 1.852),
                2 => format!("{:.2} m (FL{:.0})", val * 0.3048, val / 100.0),
                3 => format!("{:.2} m (Dalga Boyu)", 299.792458 / val),
                _ => "-".to_string(),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_bounded() {
        let mut rb = RingBuffer::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        assert_eq!(rb.len(), 3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        let items: Vec<_> = rb.iter().copied().collect();
        assert_eq!(items, vec![2, 3, 4]);
    }

    #[test]
    fn test_zoom_toggle() {
        let cfg = AppConfig::default();
        let mut state = AppState::new(cfg);
        assert_eq!(state.zoomed_tile, None);
        state.toggle_zoom(2);
        assert_eq!(state.zoomed_tile, Some(2));
        state.toggle_zoom(2);
        assert_eq!(state.zoomed_tile, None);
    }

    #[test]
    fn test_calculator_basic() {
        let cfg = AppConfig::default();
        let mut state = AppState::new(cfg);
        state.calc_input_char('1');
        state.calc_input_char('2');
        state.calc_input_char('+');
        state.calc_input_char('8');
        state.calc_input_char('=');
        assert_eq!(state.calculator.display, "20.00");
        assert_eq!(state.calculator.conv_input, "20.00");
        assert!(state.calculator.conv_result.contains("37.04 km/h"));
    }

    fn dummy_track(id: &str, domain: Domain, lat: f64, lon: f64, age_secs: i64) -> TargetTrack {
        TargetTrack {
            id: id.to_string(),
            callsign: id.to_string(),
            domain,
            geo: GeoPoint::new(lat, lon, Some(1000.0)),
            speed_kts: Some(250.0),
            heading_deg: Some(90.0),
            vertical_rate_fpm: None,
            squawk: None,
            is_military: false,
            operator_country: None,
            operator_branch: None,
            airframe_or_vessel_class: None,
            origin: None,
            destination: None,
            vessel_type: None,
            info: String::new(),
            last_seen: chrono::Utc::now() - chrono::Duration::seconds(age_secs),
        }
    }

    #[test]
    fn test_target_trails_and_follow() {
        let cfg = AppConfig::default();
        let mut state = AppState::new(cfg);
        state.follow_target_id = Some("AIR1".to_string());

        // Initial point
        state.upsert_target(dummy_track("AIR1", Domain::Air, 41.0, 29.0, 0));
        assert_eq!(state.target_trails.get("AIR1").unwrap().len(), 1);
        assert_eq!(state.map_center_lat, 41.0);
        assert_eq!(state.map_center_lon, 29.0);

        // Tiny delta (<= 0.0005) should not push
        state.upsert_target(dummy_track("AIR1", Domain::Air, 41.0002, 29.0002, 0));
        assert_eq!(state.target_trails.get("AIR1").unwrap().len(), 1);

        // Push 16 distinct points -> capped at 15
        for i in 1..=16 {
            let lat = 41.0 + (i as f64) * 0.01;
            let lon = 29.0 + (i as f64) * 0.01;
            state.upsert_target(dummy_track("AIR1", Domain::Air, lat, lon, 0));
        }
        let trail = state.target_trails.get("AIR1").unwrap();
        assert_eq!(trail.len(), 15);
    }

    #[test]
    fn test_prune_stale_targets() {
        let cfg = AppConfig::default();
        let mut state = AppState::new(cfg);
        state.upsert_target(dummy_track("FRESH_AIR", Domain::Air, 41.0, 29.0, 10));
        state.upsert_target(dummy_track("STALE_AIR", Domain::Air, 41.5, 29.5, 400));
        state.upsert_target(dummy_track("STALE_SEA", Domain::Sea, 40.5, 28.5, 500));

        assert_eq!(state.targets.len(), 3);
        assert_eq!(state.air_targets.len(), 2);
        assert_eq!(state.sea_targets.len(), 1);
        assert_eq!(state.target_trails.len(), 3);

        state.prune_stale_targets(300);

        assert_eq!(state.targets.len(), 1);
        assert_eq!(state.air_targets.len(), 1);
        assert_eq!(state.sea_targets.len(), 0);
        assert!(state.target_trails.contains_key("FRESH_AIR"));
        assert!(!state.target_trails.contains_key("STALE_AIR"));
        assert!(!state.target_trails.contains_key("STALE_SEA"));
    }
}
