use argus_core::types::RadiationMeasurement;
use chrono::{DateTime, Utc};
use serde::Deserialize;

pub struct SafecastClient {
    client: reqwest::Client,
}

#[derive(Deserialize)]
struct SafecastRawMeasurement {
    id: u64,
    value: Option<f64>,
    unit: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    captured_at: Option<String>,
    location_name: Option<String>,
    device_id: Option<u64>,
}

impl Default for SafecastClient {
    fn default() -> Self {
        Self::new()
    }
}

impl SafecastClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .user_agent("SentinelRadiation/2.0")
                .build()
                .unwrap_or_default(),
        }
    }

    /// Safecast açık veri ağından küresel canlı çevresel radyasyon ölçümlerini çeker
    pub async fn fetch_radiation_measurements(&self) -> anyhow::Result<Vec<RadiationMeasurement>> {
        let url = "https://api.safecast.org/measurements.json?limit=60";
        let resp = self.client.get(url).send().await?;

        if !resp.status().is_success() {
            return Ok(Vec::new());
        }

        let raw_list: Vec<SafecastRawMeasurement> = resp.json().await?;
        let mut results = Vec::new();

        for item in raw_list {
            let unit = item.unit.unwrap_or_default();
            // Yalnızca radyasyon ölçüm birimlerini (cpm veya uSv/h) kabul et
            if unit != "cpm" && unit != "uSv/h" && unit != "usv" {
                continue;
            }

            if let (Some(val), Some(lat), Some(lon)) = (item.value, item.latitude, item.longitude) {
                if lat.abs() <= 90.0 && lon.abs() <= 180.0 && val >= 0.0 {
                    let captured_at = item
                        .captured_at
                        .as_deref()
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc));

                    results.push(RadiationMeasurement {
                        id: item.id,
                        value: val,
                        unit,
                        lat,
                        lon,
                        captured_at,
                        location_name: item.location_name,
                        device_id: item.device_id,
                    });
                }
            }
        }

        Ok(results)
    }
}
