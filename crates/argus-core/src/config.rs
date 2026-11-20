use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationConfig {
    pub callsign: String,
    pub name: String,
    pub default_lat: f64,
    pub default_lon: f64,
    pub auto_ip_location: bool,
}

impl Default for StationConfig {
    fn default() -> Self {
        Self {
            callsign: "ARGUS-STATION".to_string(),
            name: "Istanbul Tactical OSINT Node".to_string(),
            default_lat: 41.0082,
            default_lon: 28.9784,
            auto_ip_location: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiKeysConfig {
    pub shodan_key: Option<String>,
    pub aisstream_key: Option<String>,
    pub nasa_firms_key: Option<String>,
    pub gemini_key: Option<String>,
    pub gemini_model: Option<String>,
    pub openweather_key: Option<String>,
    pub flightaware_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AcledConfig {
    pub email: Option<String>,
    pub password: Option<String>,
    pub token_url: Option<String>,
    pub api_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub enabled: bool,
    pub volume: f32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            volume: 0.5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsConfig {
    #[serde(default = "default_true")]
    pub desktop_enabled: bool,
    pub webhook_url: Option<String>,
    #[serde(default = "default_notify_cooldown")]
    pub cooldown_secs: u64,
}

fn default_true() -> bool {
    true
}

fn default_notify_cooldown() -> u64 {
    300
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self {
            desktop_enabled: true,
            webhook_url: None,
            cooldown_secs: 300,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub station: StationConfig,
    #[serde(default)]
    pub api_keys: ApiKeysConfig,
    #[serde(default)]
    pub acled: AcledConfig,
    #[serde(default)]
    pub audio: AudioConfig,
    #[serde(default)]
    pub notifications: NotificationsConfig,
}

impl AppConfig {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let mut config: AppConfig = toml::from_str(&content)?;
        config.apply_env_overrides();
        Ok(config)
    }

    pub fn load_or_default() -> Self {
        if let Ok(cfg) = Self::load_from_file("config.toml") {
            return cfg;
        }
        if let Some(home_cfg) = std::env::var("HOME")
            .ok()
            .map(|h| format!("{}/.config/argus-osint/config.toml", h))
        {
            if let Ok(cfg) = Self::load_from_file(home_cfg) {
                return cfg;
            }
        }
        let mut cfg = Self::default();
        cfg.apply_env_overrides();
        cfg
    }

    pub fn apply_env_overrides(&mut self) {
        if let Ok(val) = std::env::var("ARGUS_SHODAN_KEY") {
            if !val.trim().is_empty() {
                self.api_keys.shodan_key = Some(val);
            }
        }
        if let Ok(val) = std::env::var("ARGUS_AISSTREAM_KEY") {
            if !val.trim().is_empty() {
                self.api_keys.aisstream_key = Some(val);
            }
        }
        if let Ok(val) = std::env::var("ARGUS_FIRMS_KEY") {
            if !val.trim().is_empty() {
                self.api_keys.nasa_firms_key = Some(val);
            }
        }
        if let Ok(val) = std::env::var("ARGUS_ACLED_EMAIL") {
            if !val.trim().is_empty() {
                self.acled.email = Some(val);
            }
        }
        if let Ok(val) = std::env::var("ARGUS_ACLED_PASSWORD") {
            if !val.trim().is_empty() {
                self.acled.password = Some(val);
            }
        }
        if let Ok(val) = std::env::var("ARGUS_GEMINI_KEY").or_else(|_| std::env::var("GEMINI_API_KEY")) {
            if !val.trim().is_empty() {
                self.api_keys.gemini_key = Some(val);
            }
        }
        if let Ok(val) = std::env::var("ARGUS_GEMINI_MODEL") {
            if !val.trim().is_empty() {
                self.api_keys.gemini_model = Some(val);
            }
        }
        if let Ok(val) = std::env::var("ARGUS_OPENWEATHER_KEY") {
            if !val.trim().is_empty() {
                self.api_keys.openweather_key = Some(val);
            }
        }
        if let Ok(val) = std::env::var("ARGUS_FLIGHTAWARE_KEY") {
            if !val.trim().is_empty() {
                self.api_keys.flightaware_key = Some(val);
            }
        }
        if let Ok(val) = std::env::var("ARGUS_WEBHOOK_URL") {
            if !val.trim().is_empty() {
                self.notifications.webhook_url = Some(val);
            }
        }
    }
}
