use argus_core::types::SolarWeather;
use chrono::Utc;

pub struct NoaaSpaceClient {
    client: reqwest::Client,
}

impl Default for NoaaSpaceClient {
    fn default() -> Self {
        Self::new()
    }
}

impl NoaaSpaceClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(8))
                .build()
                .unwrap_or_default(),
        }
    }

    /// NOAA SWPC Planetary K-Index ve GOES X-Ray Flare akışını canlı sorgular
    pub async fn fetch_space_weather(&self) -> anyhow::Result<SolarWeather> {
        let kp_url = "https://services.swpc.noaa.gov/products/noaa-planetary-k-index.json";
        let resp = self.client.get(kp_url).send().await?;

        let mut kp = 0.0;
        if resp.status().is_success() {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(arr) = json.as_array() {
                    if let Some(last_row) = arr.last() {
                        if let Some(val_str) = last_row.get(1).and_then(|v| v.as_str()) {
                            if let Ok(parsed_kp) = val_str.parse::<f64>() {
                                kp = parsed_kp;
                            }
                        }
                    }
                }
            }
        }

        // Gerçek GOES X-Ray Flare sınıfını çek
        let mut flare_class = "N/A".to_string();
        let xray_url = "https://services.swpc.noaa.gov/json/goes/primary/xray-flares-latest.json";
        if let Ok(x_resp) = self.client.get(xray_url).send().await {
            if x_resp.status().is_success() {
                if let Ok(x_json) = x_resp.json::<serde_json::Value>().await {
                    if let Some(first) = x_json.as_array().and_then(|a| a.first()) {
                        let curr = first["current_class"].as_str().unwrap_or("");
                        let max_c = first["max_class"].as_str().unwrap_or("");
                        if !curr.is_empty() && !max_c.is_empty() {
                            flare_class = format!("{} (Max:{})", curr, max_c);
                        } else if !curr.is_empty() {
                            flare_class = curr.to_string();
                        }
                    }
                }
            }
        }

        let storm_level = if kp >= 9.0 {
            "G5 (ASIRI FIRTINA)".to_string()
        } else if kp >= 8.0 {
            "G4 (SIDDETLI FIRTINA)".to_string()
        } else if kp >= 7.0 {
            "G3 (GUCLU FIRTINA)".to_string()
        } else if kp >= 6.0 {
            "G2 (ORTA FIRTINA)".to_string()
        } else if kp >= 5.0 {
            "G1 (KUCUK FIRTINA)".to_string()
        } else {
            "G0 (SAKIN/NORMAL)".to_string()
        };

        Ok(SolarWeather {
            kp_index: kp,
            flare_class,
            storm_level,
            updated: Utc::now(),
        })
    }
}
