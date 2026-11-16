use argus_core::types::{CyberIncident, ThreatSeverity};
use chrono::Utc;

pub struct IodaClient {
    client: reqwest::Client,
}

impl Default for IodaClient {
    fn default() -> Self {
        Self::new()
    }
}

impl IodaClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(12))
                .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .build()
                .unwrap_or_default(),
        }
    }

    /// IODA Küresel BGP ve İnternet Trafik Kesintilerini çeker
    pub async fn fetch_internet_outages(&self) -> anyhow::Result<Vec<CyberIncident>> {
        let now = Utc::now().timestamp();
        let from = now - 86400; // Son 24 saat

        let url = format!(
            "https://api.ioda.inetintel.cc.gatech.edu/v2/outages/alerts?from={}&until={}&limit=30",
            from, now
        );

        let resp = self.client.get(&url).send().await?;

        if !resp.status().is_success() {
            return Ok(Vec::new());
        }

        let json: serde_json::Value = resp.json().await?;
        let mut incidents = Vec::new();

        if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
            for item in data.iter().take(30) {
                let entity = &item["entity"];
                let entity_name = entity["name"].as_str().unwrap_or("Bilinmeyen").to_string();
                let entity_code = entity["code"].as_str().unwrap_or("AS0").to_string();
                let entity_type = entity["type"].as_str().unwrap_or("ASN").to_string();
                let level = item["level"].as_str().unwrap_or("warning");
                let condition = item["condition"].as_str().unwrap_or("< normal");
                let datasource = item["datasource"].as_str().unwrap_or("BGP");

                let severity = match level {
                    "critical" => ThreatSeverity::Critical,
                    "warning" => ThreatSeverity::Warning,
                    _ => ThreatSeverity::Advisory,
                };

                incidents.push(CyberIncident {
                    id: format!("IODA-{}", entity_code),
                    event_type: "BGP-OUTAGE".to_string(),
                    title: format!("{} ({})", entity_name, entity_type),
                    target: format!("{} Trafik Anomali: {}", datasource, condition),
                    severity,
                    cve: None,
                    asn: Some(entity_code),
                    timestamp: Utc::now(),
                });
            }
        }

        Ok(incidents)
    }
}
