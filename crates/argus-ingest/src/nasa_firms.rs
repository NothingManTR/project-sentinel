use argus_core::types::FirmsFire;

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

    /// VIIRS (NOAA-20 + SNPP Yedek) NRT Termal Yangın / Patlama verilerini çeker
    pub async fn fetch_recent_fires(&self) -> anyhow::Result<Vec<FirmsFire>> {
        let key = match &self.map_key {
            Some(k) if !k.trim().is_empty() => k.trim().to_string(),
            _ => return Ok(Vec::new()),
        };

        // Doğu Akdeniz, Türkiye, Karadeniz, Kafkaslar ve Orta Doğu kapsama alanı
        let mut fires = Vec::new();
        for sensor in ["VIIRS_NOAA20_NRT", "VIIRS_SNPP_NRT"] {
            let url = format!(
                "https://firms.modaps.eosdis.nasa.gov/api/area/csv/{}/{}/15,30,50,48/2",
                key, sensor
            );

            let Ok(resp) = self.client.get(&url).send().await else {
                continue;
            };
            if !resp.status().is_success() {
                continue;
            }
            let Ok(csv_text) = resp.text().await else {
                continue;
            };

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
            if !fires.is_empty() {
                break;
            }
        }

        // En yüksek parlaklık sıcaklığına (Kelvin) göre sıralayıp ilk 600 noktayı tut
        fires.sort_by(|a, b| b.brightness.partial_cmp(&a.brightness).unwrap_or(std::cmp::Ordering::Equal));
        fires.truncate(600);

        Ok(fires)
    }
}
