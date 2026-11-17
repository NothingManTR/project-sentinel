use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoPoint {
    pub lat: f64,
    pub lon: f64,
    pub alt_m: Option<f64>,
}

impl GeoPoint {
    pub fn new(lat: f64, lon: f64, alt_m: Option<f64>) -> Self {
        Self { lat, lon, alt_m }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Domain {
    Air,
    Sea,
    Space,
    News,
    Cyber,
    Geo,
    Conflict,
}

impl Domain {
    pub fn as_str(&self) -> &'static str {
        match self {
            Domain::Air => "HAVA",
            Domain::Sea => "DENİZ",
            Domain::Space => "UYDU",
            Domain::News => "HABER",
            Domain::Cyber => "SİBER",
            Domain::Geo => "AFET",
            Domain::Conflict => "ÇATIŞMA",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ThreatSeverity {
    Low,
    Advisory,
    Warning,
    Critical,
    Emergency,
}

impl ThreatSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            ThreatSeverity::Low => "BİLGİ",
            ThreatSeverity::Advisory => "İKAZ",
            ThreatSeverity::Warning => "DİKKAT",
            ThreatSeverity::Critical => "KRİTİK",
            ThreatSeverity::Emergency => "ACİL",
        }
    }
}

/// Canlı Takip Edilen Hava veya Deniz Hedefi (ADS-B / AIS)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetTrack {
    pub id: String,
    pub callsign: String,
    pub domain: Domain,
    pub geo: GeoPoint,
    pub speed_kts: Option<f64>,
    pub heading_deg: Option<f64>,
    pub vertical_rate_fpm: Option<f64>,
    pub squawk: Option<String>,
    pub is_military: bool,
    pub operator_country: Option<String>,
    pub operator_branch: Option<String>,
    pub airframe_or_vessel_class: Option<String>,
    pub origin: Option<String>,
    pub destination: Option<String>,
    pub vessel_type: Option<String>,
    pub info: String,
    pub last_seen: DateTime<Utc>,
}

/// Canlı Uydu Yörünge Pozisyonu ve İstasyon Geçiş Verisi (CelesTrak SGP4)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SatellitePass {
    pub norad_id: u32,
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub alt_km: f64,
    pub velocity_kms: f64,
    pub az_deg: f64,
    pub el_deg: f64,
    pub next_pass_secs: u64,
    pub pass_duration_secs: u64,
    #[serde(default)]
    pub ground_track: Vec<(f64, f64)>,
}

/// Shodan & Shodan InternetDB Host İnceleme Raporu
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShodanHostInfo {
    pub ip: String,
    pub hostnames: Vec<String>,
    pub ports: Vec<u16>,
    pub cpes: Vec<String>,
    pub vulns: Vec<String>,
    pub tags: Vec<String>,
    pub isp: Option<String>,
    pub country: Option<String>,
    pub city: Option<String>,
}

/// NASA FIRMS Canlı Termal Sıcak Nokta & Patlama Verisi
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirmsFire {
    pub lat: f64,
    pub lon: f64,
    pub brightness: f64,
    pub confidence: String,
    pub acq_date: String,
    pub acq_time: String,
    pub satellite: String,
}

/// ACLED Canlı Silahlı Çatışma, Füze veya İHA Olayı
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcledEvent {
    pub event_id: String,
    pub event_date: String,
    pub event_type: String,
    pub sub_event_type: String,
    pub actor1: String,
    pub actor2: Option<String>,
    pub location: String,
    pub country: String,
    pub lat: f64,
    pub lon: f64,
    pub fatalities: u32,
    pub notes: String,
}

/// USGS & Kandilli Canlı Deprem Bilgisi
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Earthquake {
    pub id: String,
    pub place: String,
    pub mag: f64,
    pub depth_km: f64,
    pub time: DateTime<Utc>,
    pub lat: f64,
    pub lon: f64,
    pub url: String,
}

/// CISA KEV ve IODA Siber Olayları
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CyberIncident {
    pub id: String,
    pub event_type: String, // "BGP Drop", "Exploit In-Wild", "CVE"
    pub title: String,
    pub target: String,
    pub severity: ThreatSeverity,
    pub cve: Option<String>,
    pub asn: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// RSS ve Kriz Haber Başlığı
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsintNews {
    pub id: String,
    pub source: String,
    pub title: String,
    pub link: String,
    pub published: DateTime<Utc>,
    pub category: String,
    pub severity: ThreatSeverity,
}

/// Safecast Canlı Çevresel Radyasyon / Geiger Sayacı Ölçümü
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadiationMeasurement {
    pub id: u64,
    pub value: f64,
    pub unit: String,
    pub lat: f64,
    pub lon: f64,
    pub captured_at: Option<DateTime<Utc>>,
    pub location_name: Option<String>,
    pub device_id: Option<u64>,
}

/// NOAA Uzay Havası (Güneş Fırtınası)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolarWeather {
    pub kp_index: f64,
    pub flare_class: String,
    pub storm_level: String,
    pub updated: DateTime<Utc>,
}

/// Harita Görüntüleme Motoru Modu
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MapRenderStyle {
    HalfBlockTerrain,
    HalfBlockDarkTile,
    HalfBlockSatellite,
    BrailleVector,
}

impl MapRenderStyle {
    pub fn next(self) -> Self {
        match self {
            MapRenderStyle::HalfBlockTerrain => MapRenderStyle::HalfBlockDarkTile,
            MapRenderStyle::HalfBlockDarkTile => MapRenderStyle::HalfBlockSatellite,
            MapRenderStyle::HalfBlockSatellite => MapRenderStyle::BrailleVector,
            MapRenderStyle::BrailleVector => MapRenderStyle::HalfBlockTerrain,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            MapRenderStyle::HalfBlockTerrain => "TOPO RASTER",
            MapRenderStyle::HalfBlockDarkTile => "GECE RASTER",
            MapRenderStyle::HalfBlockSatellite => "ARAZI RASTER",
            MapRenderStyle::BrailleVector => "VEKTOR RADAR",
        }
    }
}

/// Canlı KiwiSDR Kısa Dalga Alıcı Sunucu Bilgisi (rx.linkfanel.net / kiwisdr.com)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdrNodeInfo {
    pub name: String,
    pub url: String,
    pub host_port: String,
    pub location: String,
    pub users: u32,
    pub users_max: u32,
    pub snr: i32,
    #[serde(default)]
    pub lat: Option<f64>,
    #[serde(default)]
    pub lon: Option<f64>,
    #[serde(default)]
    pub grid: String,
}

/// Otonom HF Bant Tarayıcı & VOX Tarafından Yakalanan Yayın Kaydı
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdrScanHit {
    pub timestamp: DateTime<Utc>,
    pub frequency_khz: f64,
    pub mode: String,
    pub snr_db: f32,
    pub rssi_dbm: f32,
    pub host_port: String,
    pub recorded_file: Option<String>,
}

/// Otomatik Tehdit ve İzleme Listesi (Watchlist) Alarm Kaydı
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertItem {
    pub timestamp: DateTime<Utc>,
    pub domain: String,
    pub severity: ThreatSeverity,
    pub message: String,
    pub target_id: Option<String>,
}

/// Sentinel Çalışma Alanı / Ekran Seçimi
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActiveView {
    Splash,
    Dashboard,
    Geoint,
    AirTracks,
    SeaTracks,
    Sigint,
    Osint,
    Cyberint,
    Satint,
    Masint,
    Notepad,
    Calculator,
}

impl ActiveView {
    pub fn title(&self) -> &'static str {
        match self {
            ActiveView::Splash => "SENTINEL-BIOS v4.10.1998 POST INITIALIZATION",
            ActiveView::Dashboard => "DASHBOARD (KONSOL MOZAIK)",
            ActiveView::Geoint => "GEOINT: COGRAFI ISTIHBARAT & VEKTOR HARITA",
            ActiveView::AirTracks => "HAVA TRAFIGI & ASKERI HEDEFLER (ADS-B RADAR)",
            ActiveView::SeaTracks => "DENIZ FILOSU & SAVAS GEMILERI (AIS TAKIP)",
            ActiveView::Sigint => "SIGINT: SINYAL ISTIHBARATI & HFGCS EAM ALICISI",
            ActiveView::Osint => "OSINT: ACIK KAYNAK VE KRIZ HABER BULTENI",
            ActiveView::Cyberint => "CYBERINT: SIBER TEHDIT & SHODAN ALTYAPI KESFI",
            ActiveView::Satint => "SATINT: UYDU YORUNGE & GECIS RADAR SKOBU",
            ActiveView::Masint => "MASINT: JEOFIZIK, DEPREM & TERMAL ANALIZ",
            ActiveView::Notepad => "NOT DEFTERI (NOTEPAD) - [notes.txt]",
            ActiveView::Calculator => "HESAP MAKINESI & TAKTIK BIRIM CEVIRICI",
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            ActiveView::Splash => "BOOT",
            ActiveView::Dashboard => "P000",
            ActiveView::Geoint => "P100",
            ActiveView::AirTracks => "P310",
            ActiveView::SeaTracks => "P320",
            ActiveView::Osint => "P200",
            ActiveView::Sigint => "P300",
            ActiveView::Satint => "P400",
            ActiveView::Cyberint => "P500",
            ActiveView::Masint => "P600",
            ActiveView::Notepad => "APP1",
            ActiveView::Calculator => "APP2",
        }
    }
}

/// Canlı EAM (Emergency Action Message) Formatı
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EamMessage {
    pub timestamp: DateTime<Utc>,
    pub frequency_khz: f64,
    pub mode: String, // "USB", "AM"
    pub preamble: String, // "M4Z", "A1B"
    pub length_chars: usize,
    pub content: String,
    pub auth_station: String, // "ANDREWS", "OFFUTT", "PUERTO RICO", "MAINS"
    pub raw_signal_snr_db: f32,
    pub is_flash_override: bool, // SkyKing / Flash broadcast
}

/// Telsiz / SDR İstasyon Tanımı
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadioPreset {
    pub name: String,
    pub frequency_khz: f64,
    pub mode: String,
    pub description: String,
    pub stream_url: Option<String>,
}

/// Win98 Not Defteri Durumu
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotepadState {
    pub content: String,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub status_message: String,
}

impl Default for NotepadState {
    fn default() -> Self {
        Self {
            content: String::new(),
            cursor_line: 0,
            cursor_col: 0,
            status_message: "Hazır".to_string(),
        }
    }
}

/// Win98 Hesap Makinesi ve Birim Çevirici Durumu
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalcState {
    pub display: String,
    pub current_value: f64,
    pub pending_op: Option<char>,
    pub clear_on_next: bool,
    pub history: Vec<String>,
    // Birim çevirici girdisi ve çıktısı
    pub conv_mode: usize, // 0: Knots->km/h, 1: NM->km, 2: Ft->m, 3: MHz->m (Dalga boyu)
    pub conv_input: String,
    pub conv_result: String,
}

impl Default for CalcState {
    fn default() -> Self {
        Self {
            display: "0".to_string(),
            current_value: 0.0,
            pending_op: None,
            clear_on_next: false,
            history: Vec::new(),
            conv_mode: 0,
            conv_input: "100".to_string(),
            conv_result: "185.20 km/h".to_string(),
        }
    }
}

/// İnternetten çekilen canlı uçak fotoğrafı ve tescil özeti
#[derive(Debug, Clone)]
pub struct AircraftPhotoEntry {
    pub icao: String,
    pub registration: Option<String>,
    pub aircraft_type: Option<String>,
    pub registered_owner: Option<String>,
    pub photographer: Option<String>,
    pub source: String,
    pub image_bytes: Option<std::sync::Arc<Vec<u8>>>,
    pub note: String,
}

#[derive(Debug, Clone, Default)]
pub enum AircraftMediaStatus {
    #[default]
    Idle,
    Loading {
        icao: String,
    },
    Ready(AircraftPhotoEntry),
    NotFound {
        icao: String,
        registration: Option<String>,
        reason: String,
    },
}

/// Küresel OSINT DEFCON & Muharip Komutanlık Hazırlık Durumu (defconlevel.com + PizzINT)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefconStatus {
    pub overall_level: u8,
    pub source: String,
    pub commands: Vec<(String, u8)>,
    pub doughcon_index: Option<i64>,
    pub updated_at: DateTime<Utc>,
}

/// K-1: Küresel Denizaltı Fiber Kablo Hattı (TeleGeography Submarine Cable Map)
#[derive(Debug, Clone)]
pub struct SubseaCable {
    pub id: String,
    pub name: String,
    pub color_hex: String,
    pub lines: Vec<Vec<(f64, f64)>>, // (lon, lat) segmentleri
}

/// A-3: OpenWeatherMap Anlık Hava & Rüzgar Gözlemi
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherPoint {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub temp_c: f64,
    pub wind_kts: f64,
    pub wind_deg: f64,
    pub clouds_pct: u8,
    pub visibility_km: f64,
    pub condition: String,
}

/// A-2: Resmi Uçuş Programı & Rota Özeti (FlightAware AeroAPI / hexdb.io)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightScheduleInfo {
    pub ident: String,
    pub origin_code: Option<String>,
    pub origin_name: Option<String>,
    pub dest_code: Option<String>,
    pub dest_name: Option<String>,
    pub origin_lat: Option<f64>,
    pub origin_lon: Option<f64>,
    pub dest_lat: Option<f64>,
    pub dest_lon: Option<f64>,
    pub status: Option<String>,
    pub scheduled_off: Option<String>,
    pub estimated_on: Option<String>,
    pub source: String,
}

/// K-4: Çoklu-Sensör Çapraz Korelasyon Kriz Kümesi
#[derive(Debug, Clone)]
pub struct FusionCluster {
    pub lat: f64,
    pub lon: f64,
    pub score: u32,
    pub domains: Vec<String>,
    pub summary: String,
}
