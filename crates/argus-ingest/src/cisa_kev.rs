use argus_core::types::{CyberIncident, ThreatSeverity};
use chrono::Utc;

pub struct CisaKevClient {
    client: reqwest::Client,
}

impl Default for CisaKevClient {
    fn default() -> Self {
        Self::new()
    }
}

impl CisaKevClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(12))
                .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .build()
                .unwrap_or_default(),
        }
    }

    /// Aktif sömürülen CISA KEV zafiyetlerini çeker (GitHub Mirror & CISA resmi kaynağı)
    pub async fn fetch_exploited_vulns(&self) -> anyhow::Result<Vec<CyberIncident>> {
        let urls = [
            "https://raw.githubusercontent.com/cisagov/kev-data/main/known_exploited_vulnerabilities.json",
            "https://www.cisa.gov/sites/default/files/feeds/known_exploited_vulnerabilities.json",
        ];

        for url in urls {
            if let Ok(resp) = self.client.get(url).send().await {
                if resp.status().is_success() {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        let mut incidents = Vec::new();
                        if let Some(vulns) = json.get("vulnerabilities").and_then(|v| v.as_array()) {
                            for item in vulns.iter().rev().take(40) {
                                let cve = item["cveID"].as_str().unwrap_or("").to_string();
                                let product = item["product"].as_str().unwrap_or("Sistem").to_string();
                                let vendor = item["vendorProject"].as_str().unwrap_or("Vendor").to_string();
                                let name = item["vulnerabilityName"].as_str().unwrap_or("Zafiyet").to_string();

                                incidents.push(CyberIncident {
                                    id: cve.clone(),
                                    event_type: "CISA-KEV".to_string(),
                                    title: format!("{} / {}", vendor, product),
                                    target: name,
                                    severity: ThreatSeverity::Critical,
                                    cve: Some(cve),
                                    asn: None,
                                    timestamp: Utc::now(),
                                });
                            }
                        }
                        if !incidents.is_empty() {
                            return Ok(incidents);
                        }
                    }
                }
            }
        }

        Ok(Vec::new())
    }
}
