use argus_core::types::FirmsFire;
use std::collections::HashSet;

pub struct NasaFirmsClient {
    map_key: Option<String>,
    client: reqwest::Client,
}

impl NasaFirmsClient {
    pub fn new(map_key: Option<String>) -> Self {
        Self {
            map_key,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(12))
                .build()
                .unwrap_or_default(),
        }
    }

    async fn fetch_sensor_fires(&self, key: &str, sensor: &str) -> Vec<FirmsFire> {
        let url = format!(
            "https://firms.modaps.eosdis.nasa.gov/api/area/csv/{}/{}/15,30,50,48/2",
            key, sensor
        );

        let Ok(resp) = self.client.get(&url).send().await else {
            return Vec::new();
        };
        if !resp.status().is_success() {
            return Vec::new();
        }
        let Ok(csv_text) = resp.text().await else {
            return Vec::new();
        };
        parse_firms_csv(&csv_text)
    }

    /// VIIRS (NOAA-20 + SNPP) NRT Termal Yangın / Patlama verilerini birleştirerek çeker
    pub async fn fetch_recent_fires(&self) -> anyhow::Result<Vec<FirmsFire>> {
        let key = match &self.map_key {
            Some(k) if !k.trim().is_empty() => k.trim().to_string(),
            _ => return Ok(Vec::new()),
        };

        // Doğu Akdeniz, Türkiye, Karadeniz, Kafkaslar ve Orta Doğu kapsama alanı
        let (noaa_fires, snpp_fires) = tokio::join!(
            self.fetch_sensor_fires(&key, "VIIRS_NOAA20_NRT"),
            self.fetch_sensor_fires(&key, "VIIRS_SNPP_NRT"),
        );

        let mut combined = Vec::with_capacity(noaa_fires.len() + snpp_fires.len());
        combined.extend(noaa_fires);
        combined.extend(snpp_fires);

        Ok(merge_and_dedup_fires(combined))
    }
}

pub fn parse_firms_csv(csv_text: &str) -> Vec<FirmsFire> {
    let mut fires = Vec::new();
    for (idx, line) in csv_text.lines().enumerate() {
        if idx == 0 || line.trim().is_empty() {
            continue; // Başlık satırı
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 10 {
            if let (Ok(lat), Ok(lon), Ok(bright)) = (
                parts[0].trim().parse::<f64>(),
                parts[1].trim().parse::<f64>(),
                parts[2].trim().parse::<f64>(),
            ) {
                fires.push(FirmsFire {
                    lat,
                    lon,
                    brightness: bright,
                    confidence: parts[9].trim().to_string(),
                    acq_date: parts[5].trim().to_string(),
                    acq_time: parts[6].trim().to_string(),
                    satellite: parts[7].trim().to_string(),
                });
            }
        }
    }
    fires
}

pub fn merge_and_dedup_fires(mut fires: Vec<FirmsFire>) -> Vec<FirmsFire> {
    // En yüksek parlaklık sıcaklığına (Kelvin) göre sırala ki tekilleştirmede en sıcak ölçüm kalsın
    fires.sort_by(|a, b| {
        b.brightness
            .partial_cmp(&a.brightness)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut seen_coords: HashSet<(i64, i64)> = HashSet::with_capacity(fires.len());
    fires.retain(|f| {
        let key = (
            (f.lat * 1000.0).round() as i64,
            (f.lon * 1000.0).round() as i64,
        );
        seen_coords.insert(key)
    });
    fires.truncate(600);
    fires
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_and_dedup_fires_caps_at_600_and_keeps_hottest() {
        let mut raw = vec![
            FirmsFire {
                lat: 36.5001,
                lon: 35.5001,
                brightness: 330.0,
                confidence: "n".to_string(),
                acq_date: "2026-11-18".to_string(),
                acq_time: "0100".to_string(),
                satellite: "N20".to_string(),
            },
            FirmsFire {
                lat: 36.5002,
                lon: 35.5002,
                brightness: 395.5,
                confidence: "h".to_string(),
                acq_date: "2026-11-18".to_string(),
                acq_time: "0145".to_string(),
                satellite: "SNPP".to_string(),
            },
        ];
        for i in 0..700 {
            raw.push(FirmsFire {
                lat: 30.0 + (i as f64) * 0.01,
                lon: 30.0 + (i as f64) * 0.01,
                brightness: 310.0 + (i as f64) * 0.05,
                confidence: "n".to_string(),
                acq_date: "2026-11-18".to_string(),
                acq_time: "0200".to_string(),
                satellite: if i % 2 == 0 { "N20" } else { "SNPP" }.to_string(),
            });
        }
        let merged = merge_and_dedup_fires(raw);
        assert_eq!(merged.len(), 600);
        assert!((merged[0].brightness - 395.5).abs() < 0.01);
        assert_eq!(merged[0].satellite, "SNPP");
    }
}
