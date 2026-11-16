use argus_core::types::{OsintNews, ThreatSeverity};
use chrono::{DateTime, Utc};
use quick_xml::events::Event;
use quick_xml::reader::Reader;

pub struct NewsWireClient {
    client: reqwest::Client,
}

impl Default for NewsWireClient {
    fn default() -> Self {
        Self::new()
    }
}

impl NewsWireClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) ArgusOSINT")
                .build()
                .unwrap_or_default(),
        }
    }

    /// BBC, Reuters ve Defense News açık RSS akışlarından haberleri çeker
    pub async fn fetch_latest_news(&self) -> anyhow::Result<Vec<OsintNews>> {
        let sources = [
            ("BBC WORLD", "https://feeds.bbci.co.uk/news/world/rss.xml"),
            ("DEFENSE NEWS", "https://www.defensenews.com/arc/outboundfeeds/rss/?outputType=xml"),
            ("AL JAZEERA", "https://www.aljazeera.com/xml/rss/all.xml"),
        ];

        let mut all_news = Vec::new();

        for (source_name, url) in sources {
            if let Ok(resp) = self.client.get(url).send().await {
                if resp.status().is_success() {
                    if let Ok(xml_text) = resp.text().await {
                        let mut reader = Reader::from_str(&xml_text);
                        reader.config_mut().trim_text(true);

                        let mut in_item = false;
                        let mut current_tag = String::new();
                        let mut title = String::new();
                        let mut link = String::new();
                        let mut pub_date = String::new();

                        let mut buf = Vec::new();
                        while let Ok(event) = reader.read_event_into(&mut buf) {
                            match event {
                                Event::Start(e) => {
                                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                                    if name == "item" {
                                        in_item = true;
                                        title.clear();
                                        link.clear();
                                        pub_date.clear();
                                    }
                                    current_tag = name;
                                }
                                Event::Text(e) => {
                                    if in_item {
                                        let text = e.unescape().unwrap_or_default().to_string();
                                        if current_tag == "title" && title.is_empty() {
                                            title = text;
                                        } else if current_tag == "link" && link.is_empty() {
                                            link = text;
                                        } else if current_tag == "pubDate" && pub_date.is_empty() {
                                            pub_date = text;
                                        }
                                    }
                                }
                                Event::CData(e) => {
                                    if in_item {
                                        let text = String::from_utf8_lossy(e.as_ref()).trim().to_string();
                                        if current_tag == "title" && title.is_empty() {
                                            title = text;
                                        } else if current_tag == "link" && link.is_empty() {
                                            link = text;
                                        } else if current_tag == "pubDate" && pub_date.is_empty() {
                                            pub_date = text;
                                        }
                                    }
                                }
                                Event::End(e) => {
                                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                                    if name == "item" {
                                        in_item = false;
                                        if !title.is_empty() {
                                            let lower = title.to_lowercase();
                                            let severity = if lower.contains("strike") || lower.contains("missile") || lower.contains("kill") || lower.contains("explosion") || lower.contains("saldırı") {
                                                ThreatSeverity::Critical
                                            } else if lower.contains("warning") || lower.contains("threat") || lower.contains("crash") || lower.contains("sanction") || lower.contains("kriz") {
                                                ThreatSeverity::Warning
                                            } else if lower.contains("defense") || lower.contains("military") || lower.contains("army") || lower.contains("navy") {
                                                ThreatSeverity::Advisory
                                            } else {
                                                ThreatSeverity::Low
                                            };

                                            let category = if [
                                                "cyber", "hack", "cve", "ransomware", "breach",
                                                "data", "telecom",
                                            ]
                                            .iter()
                                            .any(|k| lower.contains(k))
                                            {
                                                "P203 SİBER".to_string()
                                            } else if [
                                                "oil", "gas", "energy", "pipeline", "power",
                                                "nuclear", "grid", "opec",
                                            ]
                                            .iter()
                                            .any(|k| lower.contains(k))
                                            {
                                                "P204 ENERJİ".to_string()
                                            } else if [
                                                "missile", "strike", "military", "war", "army",
                                                "navy", "air force", "defense", "drone", "attack",
                                                "conflict", "nato",
                                            ]
                                            .iter()
                                            .any(|k| lower.contains(k))
                                            {
                                                "P202 ASKERİ/KRİZ".to_string()
                                            } else {
                                                "P201 KÜRESEL".to_string()
                                            };

                                            let published = DateTime::parse_from_rfc2822(pub_date.trim())
                                                .map(|dt| dt.with_timezone(&Utc))
                                                .unwrap_or_else(|_| Utc::now());

                                            all_news.push(OsintNews {
                                                id: format!("{}-{}", source_name, all_news.len()),
                                                source: source_name.to_string(),
                                                title: title.clone(),
                                                link: link.clone(),
                                                published,
                                                category,
                                                severity,
                                            });
                                        }
                                    }
                                    current_tag.clear();
                                }
                                Event::Eof => break,
                                _ => {}
                            }
                            buf.clear();
                        }
                    }
                }
            }
        }

        Ok(all_news)
    }
}
