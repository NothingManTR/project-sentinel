use argus_core::types::{AcledEvent, EscalationMetric};
use flate2::read::DeflateDecoder;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::time::{Duration, Instant};

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct OAuthTokenResponse {
    access_token: Option<String>,
    expires_in: Option<u64>,
    refresh_token: Option<String>,
}

pub struct AcledClient {
    email: Option<String>,
    password: Option<String>,
    token_url: String,
    api_url: String,
    client: reqwest::Client,
    cached_token: parking_lot::RwLock<Option<(String, Instant, Duration)>>,
    last_gdelt_zip_url: parking_lot::RwLock<Option<String>>,
    cached_gdelt_events: parking_lot::RwLock<Vec<AcledEvent>>,
    cached_gdelt_metrics: parking_lot::RwLock<Vec<EscalationMetric>>,
    prev_gdelt_counts: parking_lot::RwLock<HashMap<String, u32>>,
}

impl AcledClient {
    pub fn new(email: Option<String>, password: Option<String>) -> Self {
        Self {
            email,
            password,
            token_url: "https://acleddata.com/oauth/token".to_string(),
            api_url: "https://acleddata.com/api/acled/read".to_string(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(14))
                .user_agent("Project-Sentinel/0.1 (+https://github.com/NothingManTR/project-sentinel)")
                .build()
                .unwrap_or_default(),
            cached_token: parking_lot::RwLock::new(None),
            last_gdelt_zip_url: parking_lot::RwLock::new(None),
            cached_gdelt_events: parking_lot::RwLock::new(Vec::new()),
            cached_gdelt_metrics: parking_lot::RwLock::new(Vec::new()),
            prev_gdelt_counts: parking_lot::RwLock::new(HashMap::new()),
        }
    }

    async fn get_valid_token(&self) -> Option<String> {
        let (email, pass) = match (&self.email, &self.password) {
            (Some(e), Some(p))
                if !e.trim().is_empty()
                    && !p.trim().is_empty()
                    && !e.contains("example.com")
                    && !p.contains("YOUR_") =>
            {
                (e.trim(), p.trim())
            }
            _ => return None,
        };

        {
            let guard = self.cached_token.read();
            if let Some((token, fetched_at, ttl)) = guard.as_ref() {
                if fetched_at.elapsed() < ttl.saturating_sub(Duration::from_secs(60)) {
                    return Some(token.clone());
                }
            }
        }

        let params = [
            ("username", email),
            ("password", pass),
            ("grant_type", "password"),
            ("client_id", "acled"),
            ("scope", "authenticated"),
        ];

        let resp = self.client.post(&self.token_url).form(&params).send().await.ok()?;
        if resp.status().is_success() {
            let token_resp: OAuthTokenResponse = resp.json().await.ok()?;
            if let Some(token) = token_resp.access_token {
                let ttl = Duration::from_secs(token_resp.expires_in.unwrap_or(86400));
                let mut guard = self.cached_token.write();
                *guard = Some((token.clone(), Instant::now(), ttl));
                return Some(token);
            }
        }
        None
    }

    /// ACLED OAuth API üzerinden çatışma olaylarını çeker; erişim yoksa veya 403/401 dönerse
    /// anahtarsız canlı GDELT v2 15-dakikalık CAMEO çatışma akışına düşer.
    pub async fn fetch_recent_conflicts(&self) -> anyhow::Result<Vec<AcledEvent>> {
        self.fetch_recent_conflicts_with_metrics()
            .await
            .map(|(events, _)| events)
    }

    /// ACLED / GDELT v2 üzerinden hem kinetik çatışma olaylarını hem de ülke bazlı EVI tırmanma metriklerini döndürür.
    pub async fn fetch_recent_conflicts_with_metrics(
        &self,
    ) -> anyhow::Result<(Vec<AcledEvent>, Vec<EscalationMetric>)> {
        if let Some(token) = self.get_valid_token().await {
            let url = format!("{}?limit=30", self.api_url);
            if let Ok(resp) = self.client.get(&url).bearer_auth(&token).send().await {
                if resp.status().is_success() {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if let Some(items) = json.get("data").and_then(|d| d.as_array()) {
                            let mut events = Vec::new();
                            for item in items {
                                let event = AcledEvent {
                                    event_id: item["event_id_cnty"].as_str().unwrap_or("").to_string(),
                                    event_date: item["event_date"].as_str().unwrap_or("").to_string(),
                                    event_type: item["event_type"].as_str().unwrap_or("ÇATIŞMA").to_string(),
                                    sub_event_type: item["sub_event_type"].as_str().unwrap_or("").to_string(),
                                    actor1: item["actor1"].as_str().unwrap_or("Bilinmeyen").to_string(),
                                    actor2: item["actor2"].as_str().map(|s| s.to_string()),
                                    location: item["location"].as_str().unwrap_or("").to_string(),
                                    country: item["country"].as_str().unwrap_or("").to_string(),
                                    lat: item["latitude"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0),
                                    lon: item["longitude"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0),
                                    fatalities: item["fatalities"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0),
                                    notes: item["notes"].as_str().unwrap_or("").to_string(),
                                };
                                if event.lat != 0.0 && event.lon != 0.0 {
                                    events.push(event);
                                }
                            }
                            if !events.is_empty() {
                                let metrics = self
                                    .fetch_gdelt_live_conflicts_with_metrics()
                                    .await
                                    .map(|(_, m)| m)
                                    .unwrap_or_default();
                                return Ok((events, metrics));
                            }
                        }
                    }
                }
            }
        }

        self.fetch_gdelt_live_conflicts_with_metrics().await
    }

    pub async fn fetch_gdelt_live_conflicts_with_metrics(
        &self,
    ) -> anyhow::Result<(Vec<AcledEvent>, Vec<EscalationMetric>)> {
        let lu_text = self
            .client
            .get("http://data.gdeltproject.org/gdeltv2/lastupdate.txt")
            .send()
            .await?
            .text()
            .await?;

        let export_url = lu_text
            .lines()
            .filter_map(|l| l.split_whitespace().last())
            .find(|u| u.ends_with(".export.CSV.zip"))
            .ok_or_else(|| anyhow::anyhow!("GDELT lastupdate icinde export.CSV.zip bulunamadi"))?
            .to_string();

        {
            let last_url = self.last_gdelt_zip_url.read();
            if last_url.as_deref() == Some(export_url.as_str()) {
                let cached = self.cached_gdelt_events.read();
                let cached_metrics = self.cached_gdelt_metrics.read();
                if !cached.is_empty() || !cached_metrics.is_empty() {
                    return Ok((cached.clone(), cached_metrics.clone()));
                }
            }
        }

        let zip_bytes = self.client.get(&export_url).send().await?.bytes().await?;
        let csv_bytes = extract_first_zip_entry(&zip_bytes)?;
        let csv_text = String::from_utf8_lossy(&csv_bytes);
        let (events, mut metrics) = parse_gdelt_with_metrics(&csv_text);

        {
            let prev = self.prev_gdelt_counts.read();
            if !prev.is_empty() {
                for m in &mut metrics {
                    if let Some(&old) = prev.get(&m.country_code) {
                        if old > 0 {
                            m.delta_pct = (((m.event_count as i32) - (old as i32)) * 100)
                                / (old as i32);
                        }
                    }
                }
            }
        }

        if !events.is_empty() || !metrics.is_empty() {
            *self.last_gdelt_zip_url.write() = Some(export_url);
            *self.cached_gdelt_events.write() = events.clone();
            *self.cached_gdelt_metrics.write() = metrics.clone();
            if !metrics.is_empty() {
                let mut prev = self.prev_gdelt_counts.write();
                for m in &metrics {
                    prev.insert(m.country_code.clone(), m.event_count);
                }
            }
        }
        Ok((events, metrics))
    }
}

/// Tek dosyalı ZIP arşivinin ilk girdisini harici crate olmadan bellek içinde açar (`Stored=0`, `Deflate=8`).
pub fn extract_first_zip_entry(zip_bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    if zip_bytes.len() < 30 || &zip_bytes[0..4] != b"PK\x03\x04" {
        anyhow::bail!("Gecersiz ZIP basligi");
    }
    let method = u16::from_le_bytes([zip_bytes[8], zip_bytes[9]]);
    let comp_size = u32::from_le_bytes([zip_bytes[18], zip_bytes[19], zip_bytes[20], zip_bytes[21]]) as usize;
    let fn_len = u16::from_le_bytes([zip_bytes[26], zip_bytes[27]]) as usize;
    let ex_len = u16::from_le_bytes([zip_bytes[28], zip_bytes[29]]) as usize;

    let data_start = 30 + fn_len + ex_len;
    if data_start >= zip_bytes.len() {
        anyhow::bail!("Bozuk ZIP veri ofseti");
    }

    match method {
        0 => {
            let end = if comp_size > 0 && data_start + comp_size <= zip_bytes.len() {
                data_start + comp_size
            } else {
                zip_bytes.len()
            };
            Ok(zip_bytes[data_start..end].to_vec())
        }
        8 => {
            let slice = if comp_size > 0 && data_start + comp_size <= zip_bytes.len() {
                &zip_bytes[data_start..data_start + comp_size]
            } else {
                &zip_bytes[data_start..]
            };
            let mut decoder = DeflateDecoder::new(slice);
            let mut out = Vec::with_capacity(512 * 1024);
            decoder.read_to_end(&mut out)?;
            Ok(out)
        }
        other => anyhow::bail!("Desteklenmeyen ZIP sikistirma metodu: {}", other),
    }
}

fn cameo_label(code: &str) -> (&'static str, &'static str) {
    if code.starts_with("195") {
        ("HAVA SALDIRISI", "GDELT CAMEO-195 Hava/Fuze Saldirisi")
    } else if code.starts_with("194") {
        ("TOPÇU/ZIRHLI", "GDELT CAMEO-194 Topcu/Tank Atesi")
    } else if code.starts_with("196") {
        ("ATESKES IHLALI", "GDELT CAMEO-196 Ateskes Ihlali")
    } else if code.starts_with("19") {
        ("SILAHLI CATISMA", "GDELT CAMEO-19x Silahli Catisma")
    } else if code.starts_with("183") {
        ("PATLAMA/IHA", "GDELT CAMEO-183 Bombali/Drone Saldirisi")
    } else if code.starts_with("18") {
        ("SALDIRI", "GDELT CAMEO-18x Fiziksel Saldiri")
    } else if code.starts_with("20") {
        ("KRITIK SIDDET", "GDELT CAMEO-20x Yuksek Siddet")
    } else {
        ("PROTESTO", "GDELT CAMEO-145 Siddetli Protesto")
    }
}

/// GDELT FIPS 10-4 ulke kodunu ISO2 koda ve Turkce ulke adina cevirir.
pub fn fips_to_iso2_name(fips: &str) -> (&'static str, &'static str) {
    match fips.trim().to_ascii_uppercase().as_str() {
        "TU" => ("TR", "Turkiye"),
        "UP" => ("UA", "Ukrayna"),
        "RS" => ("RU", "Rusya"),
        "IS" => ("IL", "Israil"),
        "IR" => ("IR", "Iran"),
        "SY" => ("SY", "Suriye"),
        "IZ" => ("IQ", "Irak"),
        "YM" => ("YE", "Yemen"),
        "LE" => ("LB", "Lubnan"),
        "EG" => ("EG", "Misir"),
        "SU" => ("SD", "Sudan"),
        "TW" => ("TW", "Tayvan"),
        "CH" => ("CN", "Cin"),
        "KS" => ("KR", "G.Kore"),
        "KN" => ("KP", "K.Kore"),
        "PK" => ("PK", "Pakistan"),
        "IN" => ("IN", "Hindistan"),
        "GR" => ("GR", "Yunanistan"),
        "US" => ("US", "ABD"),
        "UK" => ("GB", "Birlesik Krallik"),
        "GM" => ("DE", "Almanya"),
        "FR" => ("FR", "Fransa"),
        "IT" => ("IT", "Italya"),
        "SA" => ("SA", "S.Arabistan"),
        "AE" => ("AE", "BAE"),
        "LY" => ("LY", "Libya"),
        "AF" => ("AF", "Afganistan"),
        "JA" => ("JP", "Japonya"),
        "PL" => ("PL", "Polonya"),
        "RO" => ("RO", "Romanya"),
        "BU" => ("BG", "Bulgaristan"),
        "GG" => ("GE", "Gurcistan"),
        "AJ" => ("AZ", "Azerbaycan"),
        "BY" => ("BY", "Belarus"),
        _ => ("", ""),
    }
}

struct CountryEviAcc {
    name: &'static str,
    event_count: u32,
    mat_count: u32,
    weighted_sum: f64,
}

/// Tek CSV taramasinda hem askeri/kinetik catisma olaylarini (maks 35) hem de
/// QuadClass 3/4 olaylarindan ulke bazli EVI metriklerini (azalan sira, top 10) ayiklar.
pub fn parse_gdelt_with_metrics(raw_csv: &str) -> (Vec<AcledEvent>, Vec<EscalationMetric>) {
    let mut events = Vec::new();
    let mut seen_keys: HashSet<String> = HashSet::new();
    let mut country_stats: HashMap<&'static str, CountryEviAcc> = HashMap::new();

    let mil_types = ["MIL", "REB", "INS", "SEP", "UAF", "TER", "MOD"];
    let kinetic_kw = [
        "airstrike",
        "missile",
        "drone",
        "shelling",
        "artillery",
        "bombard",
        "incursion",
        "ceasefire",
        "troop",
        "battlefield",
        "insurgent",
        "sabotage",
        "military-strike",
        "naval",
        "warship",
        "frontline",
    ];

    for line in raw_csv.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 61 {
            continue;
        }
        let quad = cols[29].trim();
        if quad != "3" && quad != "4" {
            continue;
        }

        let goldstein: f64 = cols[30].trim().parse().unwrap_or(0.0);
        let fips = cols[53].trim();
        let (iso2, cname) = fips_to_iso2_name(fips);
        if !iso2.is_empty() {
            let acc = country_stats.entry(iso2).or_insert(CountryEviAcc {
                name: cname,
                event_count: 0,
                mat_count: 0,
                weighted_sum: 0.0,
            });
            acc.event_count += 1;
            if quad == "4" {
                acc.mat_count += 1;
                acc.weighted_sum += 3.0 + (-goldstein).clamp(0.0, 10.0) * 0.4;
            } else {
                acc.weighted_sum += 1.0 + (-goldstein).clamp(0.0, 10.0) * 0.2;
            }
        }

        // Sadece QuadClass=4 (Maddi Çatışma) ve koordinatlı olaylar (events maks 35, ama EVI taramasi kesilmez)
        if quad != "4" || events.len() >= 35 {
            continue;
        }
        let root = cols[28];
        let code = cols[26];
        let is_conflict_root = matches!(root, "18" | "19" | "20") || code == "145";
        if !is_conflict_root {
            continue;
        }

        let (Ok(lat), Ok(lon)) = (cols[56].trim().parse::<f64>(), cols[57].trim().parse::<f64>()) else {
            continue;
        };
        if lat == 0.0 && lon == 0.0 {
            continue;
        }

        let a1_type = cols[12].trim();
        let a2_type = cols[22].trim();
        let url = cols[60].trim();
        let url_lower = url.to_ascii_lowercase();

        let has_mil_actor = mil_types.contains(&a1_type) || mil_types.contains(&a2_type);
        let has_kinetic_url = kinetic_kw.iter().any(|k| url_lower.contains(k));

        if !(has_kinetic_url || (has_mil_actor && goldstein <= -7.0)) {
            continue;
        }

        let actor1_raw = if !cols[6].trim().is_empty() {
            cols[6].trim()
        } else {
            cols[5].trim()
        };
        let actor2_raw = if !cols[16].trim().is_empty() {
            cols[16].trim()
        } else {
            cols[15].trim()
        };
        let actor1 = if actor1_raw.is_empty() {
            "Silahli Unsur".to_string()
        } else {
            actor1_raw.to_string()
        };
        let actor2 = if actor2_raw.is_empty() {
            None
        } else {
            Some(actor2_raw.to_string())
        };

        let location = cols[52].trim();
        let dedup_key = format!("{:.2}:{:.2}:{}", lat, lon, code);
        if !seen_keys.insert(dedup_key) {
            continue;
        }

        let raw_date = cols[1].trim();
        let event_date = if raw_date.len() == 8 {
            format!("{}-{}-{}", &raw_date[0..4], &raw_date[4..6], &raw_date[6..8])
        } else {
            raw_date.to_string()
        };
        let country = if !iso2.is_empty() {
            iso2.to_string()
        } else if !fips.is_empty() {
            fips.to_string()
        } else {
            "-".to_string()
        };

        let (ev_type, sub_type) = cameo_label(code);
        let mentions: u32 = cols[31].trim().parse().unwrap_or(1);

        events.push(AcledEvent {
            event_id: format!("GDELT-{}", cols[0].trim()),
            event_date,
            event_type: ev_type.to_string(),
            sub_event_type: format!("{} (Goldstein:{:.1}, Kaynak:{})", sub_type, goldstein, mentions),
            actor1,
            actor2,
            location: if location.is_empty() { country.clone() } else { location.to_string() },
            country,
            lat,
            lon,
            fatalities: 0,
            notes: format!("GDELT v2 Kaynak: {}", url),
        });
    }

    let mut metrics: Vec<EscalationMetric> = country_stats
        .into_iter()
        .map(|(code, st)| {
            let evi_score = (100.0 * (1.0 - (-st.weighted_sum / 90.0).exp()))
                .round()
                .clamp(1.0, 100.0) as u32;
            let mat_ratio_pct = (st.mat_count as f64 / st.event_count.max(1) as f64) * 100.0;
            let delta_pct = (mat_ratio_pct - 30.0).round() as i32;
            EscalationMetric {
                country_code: code.to_string(),
                country_name: st.name.to_string(),
                evi_score,
                delta_pct,
                event_count: st.event_count,
            }
        })
        .collect();

    metrics.sort_by(|a, b| {
        b.evi_score
            .cmp(&a.evi_score)
            .then_with(|| b.event_count.cmp(&a.event_count))
            .then_with(|| a.country_code.cmp(&b.country_code))
    });
    metrics.truncate(10);

    (events, metrics)
}

/// GDELT v2 61-sütunlu TSV çıktısından yalnızca askeri/kinetik çatışma olaylarını `AcledEvent` olarak ayıklar.
pub fn parse_gdelt_export_csv(csv_text: &str) -> Vec<AcledEvent> {
    parse_gdelt_with_metrics(csv_text).0
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::DeflateEncoder;
    use flate2::Compression;
    use std::io::Write;

    #[test]
    fn test_extract_first_zip_entry_deflate() {
        let payload = b"hello_gdelt_conflict_csv";
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
        enc.write_all(payload).unwrap();
        let comp = enc.finish().unwrap();

        let mut zip = Vec::new();
        zip.extend_from_slice(b"PK\x03\x04");
        zip.extend_from_slice(&20u16.to_le_bytes()); // version
        zip.extend_from_slice(&0u16.to_le_bytes());  // flags
        zip.extend_from_slice(&8u16.to_le_bytes());  // method deflate
        zip.extend_from_slice(&0u16.to_le_bytes());  // mtime
        zip.extend_from_slice(&0u16.to_le_bytes());  // mdate
        zip.extend_from_slice(&0u32.to_le_bytes());  // crc
        zip.extend_from_slice(&(comp.len() as u32).to_le_bytes());
        zip.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        zip.extend_from_slice(&5u16.to_le_bytes()); // fnlen
        zip.extend_from_slice(&0u16.to_le_bytes()); // exlen
        zip.extend_from_slice(b"a.csv");
        zip.extend_from_slice(&comp);

        let out = extract_first_zip_entry(&zip).unwrap();
        assert_eq!(out, payload);
    }

    #[test]
    fn test_parse_gdelt_export_csv_filters_kinetic_conflicts() {
        let mut mil_cols = vec![""; 61];
        mil_cols[0] = "123456";
        mil_cols[1] = "20261117";
        mil_cols[6] = "ARMED FORCES";
        mil_cols[12] = "MIL";
        mil_cols[16] = "MILITIA";
        mil_cols[22] = "REB";
        mil_cols[26] = "195";
        mil_cols[28] = "19";
        mil_cols[29] = "4";
        mil_cols[30] = "-10.0";
        mil_cols[31] = "12";
        mil_cols[52] = "Latakia, Syria";
        mil_cols[53] = "SY";
        mil_cols[56] = "35.5317";
        mil_cols[57] = "35.7901";
        mil_cols[60] = "https://example.org/news/airstrike-reported-near-port";

        // Sivil siyasi haber (QuadClass=4 olsa da MIL/kinetik URL yok)
        let mut civ_cols = vec![""; 61];
        civ_cols[0] = "999999";
        civ_cols[1] = "20261117";
        civ_cols[6] = "SENATE";
        civ_cols[12] = "GOV";
        civ_cols[26] = "190";
        civ_cols[28] = "19";
        civ_cols[29] = "4";
        civ_cols[30] = "-4.0";
        civ_cols[52] = "Washington, United States";
        civ_cols[53] = "US";
        civ_cols[56] = "38.8951";
        civ_cols[57] = "-77.0364";
        civ_cols[60] = "https://example.org/politics/budget-debate";

        let sample = format!("{}\n{}\n", mil_cols.join("\t"), civ_cols.join("\t"));
        let events = parse_gdelt_export_csv(&sample);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_id, "GDELT-123456");
        assert_eq!(events[0].event_type, "HAVA SALDIRISI");
        assert_eq!(events[0].country, "SY");
    }

    #[test]
    fn test_fips_to_iso2_name_and_gdelt_evi_past_35_cutoff() {
        assert_eq!(fips_to_iso2_name("TU"), ("TR", "Turkiye"));
        assert_eq!(fips_to_iso2_name("UP"), ("UA", "Ukrayna"));
        assert_eq!(fips_to_iso2_name("RS"), ("RU", "Rusya"));
        assert_eq!(fips_to_iso2_name("IS"), ("IL", "Israil"));
        assert_eq!(fips_to_iso2_name("UK"), ("GB", "Birlesik Krallik"));
        assert_eq!(fips_to_iso2_name("CH"), ("CN", "Cin"));

        let mut lines = Vec::new();
        for i in 0..40 {
            let mut cols = vec![String::new(); 61];
            cols[0] = (1000 + i).to_string();
            cols[1] = "20261118".to_string();
            cols[6] = "FORCE".to_string();
            cols[12] = "MIL".to_string();
            cols[26] = "195".to_string();
            cols[28] = "19".to_string();
            cols[29] = "4".to_string();
            cols[30] = "-10.0".to_string();
            cols[31] = "5".to_string();
            cols[52] = "Zone".to_string();
            cols[53] = "UP".to_string();
            cols[56] = format!("{:.2}", 34.0 + (i as f64) * 0.1);
            cols[57] = "35.0".to_string();
            cols[60] = "https://example.org/airstrike".to_string();
            lines.push(cols.join("\t"));
        }
        // 35 kesmesinden sonra gelen TU QuadClass 3 olaylari yine de EVI'ye girmeli
        for i in 0..5 {
            let mut cols = vec![String::new(); 61];
            cols[0] = (2000 + i).to_string();
            cols[1] = "20261118".to_string();
            cols[26] = "130".to_string();
            cols[28] = "13".to_string();
            cols[29] = "3".to_string();
            cols[30] = "-4.0".to_string();
            cols[53] = "TU".to_string();
            lines.push(cols.join("\t"));
        }

        let csv = lines.join("\n");
        let (events, metrics) = parse_gdelt_with_metrics(&csv);
        assert_eq!(events.len(), 35);
        assert_eq!(metrics.len(), 2);
        assert_eq!(metrics[0].country_code, "UA");
        assert_eq!(metrics[0].event_count, 40);
        assert_eq!(metrics[1].country_code, "TR");
        assert_eq!(metrics[1].event_count, 5);
        assert!(metrics[0].evi_score > metrics[1].evi_score);
    }

    #[tokio::test]
    async fn test_live_gdelt_conflict_fallback() {
        let client = AcledClient::new(None, None);
        let events = client
            .fetch_recent_conflicts()
            .await
            .expect("GDELT live conflict fallback failed");
        assert!(!events.is_empty(), "Expected live GDELT CAMEO conflict events");
        println!(
            "Fetched {} live GDELT conflict events. Sample: [{}] {} ({}, {})",
            events.len(),
            events[0].event_type,
            events[0].actor1,
            events[0].location,
            events[0].event_date
        );
    }
}
