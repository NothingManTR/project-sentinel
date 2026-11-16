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

    /// VIIRS SNPP NRT Termal Yangın / Patlama verilerini çeker
    pub async fn fetch_recent_fires(&self) -> anyhow::Result<Vec<FirmsFire>> {
        let key = match &self.map_key {
            Some(k) if !k.trim().is_empty() => k.clone(),
            _ => return Ok(Vec::new()),
        };

        // Doğu Akdeniz, Türkiye, Karadeniz, Kafkaslar ve Orta Doğu kapsama alanı
        let url = format!(
            "https://firms.modaps.eosdis.nasa.gov/api/area/csv/{}/VIIRS_SNPP_NRT/15,30,50,48/1",
            key
        );

        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Ok(Vec::new());
        }

        let csv_text = resp.text().await?;
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

        Ok(fires)
    }
}
