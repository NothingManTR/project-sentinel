use argus_core::types::Earthquake;
use chrono::{DateTime, NaiveDateTime, Utc};

pub struct UsgsClient {
    client: reqwest::Client,
}

impl Default for UsgsClient {
    fn default() -> Self {
        Self::new()
    }
}

impl UsgsClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Küresel son M4.5+ depremleri (USGS) ve bölgesel M3.0+ depremleri (Kandilli) çeker
    pub async fn fetch_recent_earthquakes(&self) -> anyhow::Result<Vec<Earthquake>> {
        let mut quakes = Vec::new();

        // 1. USGS 4.5+ Günlük Küresel Akış
        let url = "https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/4.5_day.geojson";
        if let Ok(resp) = self.client.get(url).send().await {
            if resp.status().is_success() {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(features) = json.get("features").and_then(|f| f.as_array()) {
                        for item in features.iter().take(50) {
                            let id = item["id"].as_str().unwrap_or("").to_string();
                            let props = &item["properties"];
                            let place = props["place"].as_str().unwrap_or("Bilinmeyen").to_string();
                            let mag = props["mag"].as_f64().unwrap_or(0.0);
                            let time_ms = props["time"].as_i64().unwrap_or(0);
                            let time = DateTime::<Utc>::from_timestamp_millis(time_ms).unwrap_or_else(Utc::now);
                            let url = props["url"].as_str().unwrap_or("").to_string();

                            let coords = item["geometry"]["coordinates"].as_array();
                            let lon = coords.and_then(|c| c.first()).and_then(|v| v.as_f64()).unwrap_or(0.0);
                            let lat = coords.and_then(|c| c.get(1)).and_then(|v| v.as_f64()).unwrap_or(0.0);
                            let depth = coords.and_then(|c| c.get(2)).and_then(|v| v.as_f64()).unwrap_or(0.0);

                            if lat != 0.0 && lon != 0.0 {
                                quakes.push(Earthquake {
                                    id,
                                    place,
                                    mag,
                                    depth_km: depth,
                                    time,
                                    lat,
                                    lon,
                                    url,
                                });
                            }
                        }
                    }
                }
            }
        }

        // 2. Kandilli Rasathanesi Bölgesel Depremler (M >= 3.0)
        let kandilli_url = "https://api.orhanaydogdu.com.tr/deprem/kandilli/live?limit=25";
        if let Ok(resp) = self
            .client
            .get(kandilli_url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(results) = json.get("result").and_then(|r| r.as_array()) {
                        for item in results {
                            let mag = item["mag"]
                                .as_f64()
                                .or_else(|| item["mag"].as_str().and_then(|s| s.parse().ok()))
                                .unwrap_or(0.0);
                            if mag < 3.0 {
                                continue;
                            }

                            let depth = item["depth"]
                                .as_f64()
                                .or_else(|| item["depth"].as_str().and_then(|s| s.parse().ok()))
                                .unwrap_or(0.0);
                            let place = item["title"]
                                .as_str()
                                .unwrap_or("Türkiye / Bölgesel")
                                .to_string();
                            let raw_id = item["earthquake_id"]
                                .as_str()
                                .or_else(|| item["_id"].as_str())
                                .unwrap_or("");

                            let k_coords = item["geojson"]["coordinates"].as_array();
                            let lon = k_coords
                                .and_then(|c| c.first())
                                .and_then(|v| v.as_f64())
                                .or_else(|| item["lng"].as_f64())
                                .unwrap_or(0.0);
                            let lat = k_coords
                                .and_then(|c| c.get(1))
                                .and_then(|v| v.as_f64())
                                .or_else(|| item["lat"].as_f64())
                                .unwrap_or(0.0);

                            let time = item["date_time"]
                                .as_str()
                                .or_else(|| item["date"].as_str())
                                .and_then(|s| {
                                    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                                        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y.%m.%d %H:%M:%S"))
                                        .ok()
                                })
                                .map(|ndt| ndt.and_utc())
                                .or_else(|| {
                                    item["created_at"].as_i64().and_then(|ts| {
                                        if ts > 1_000_000_000_000 {
                                            DateTime::<Utc>::from_timestamp_millis(ts)
                                        } else {
                                            DateTime::<Utc>::from_timestamp(ts, 0)
                                        }
                                    })
                                })
                                .unwrap_or_else(Utc::now);

                            if lat != 0.0 && lon != 0.0 {
                                let id_suffix = if raw_id.is_empty() {
                                    format!("{:.2}-{:.2}", lat, lon)
                                } else {
                                    raw_id.to_string()
                                };
                                quakes.push(Earthquake {
                                    id: format!("KANDILLI-{}", id_suffix),
                                    place,
                                    mag,
                                    depth_km: depth,
                                    time,
                                    lat,
                                    lon,
                                    url: "http://www.koeri.boun.edu.tr".to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(quakes)
    }
}
