use argus_core::geo::haversine_km;
use argus_core::state::AppState;
use std::time::Duration;

const DEFAULT_GEMINI_MODEL: &str = "gemini-3.8-flash";

/// Canlı telemetri durumundan Gemini için yoğunlaştırılmış taktik istihbarat bağlamı hazırlar
pub fn build_tactical_prompt(state: &AppState) -> String {
    let st_lat = state.station_lat;
    let st_lon = state.station_lon;
    let mil_air = state.air_targets.iter().filter(|t| t.is_military).count();
    let mil_sea = state.sea_targets.iter().filter(|t| t.is_military).count();

    let defcon_line = if let Some(d) = &state.defcon_status {
        let cmds: Vec<String> = d
            .commands
            .iter()
            .take(5)
            .map(|(c, l)| format!("{}:D{}", c, l))
            .collect();
        format!(
            "OSINT DEFCON {} ({}) | Komutlar: {}",
            d.overall_level,
            d.source,
            cmds.join(", ")
        )
    } else {
        "OSINT DEFCON verisi henuz alinmadi".to_string()
    };

    let wx_line = if let Some(wx) = &state.station_weather {
        format!(
            "Istasyon Havasi ({}): {:.1}C, Ruzgar {:.0}kt @ {:.0}deg, Bulut %{} - {}",
            wx.name, wx.temp_c, wx.wind_kts, wx.wind_deg, wx.clouds_pct, wx.condition
        )
    } else {
        "Istasyon meteoroloji verisi yok".to_string()
    };

    // En yakın veya anomalili askeri uçuşlar
    let mut mil_list: Vec<(f64, String)> = state
        .air_targets
        .iter()
        .filter(|t| t.is_military || state.track_anomalies.contains_key(&t.id))
        .map(|t| {
            let d = haversine_km(st_lat, st_lon, t.geo.lat, t.geo.lon);
            let anom = state
                .track_anomalies
                .get(&t.id)
                .map(|a| format!(" [{}]", a))
                .unwrap_or_default();
            (
                d,
                format!(
                    "{} ({} / {}) {:.0}km {}{}",
                    t.callsign,
                    t.airframe_or_vessel_class.as_deref().unwrap_or("Mil"),
                    t.operator_branch.as_deref().unwrap_or("-"),
                    d,
                    t.origin.as_deref().unwrap_or("KURESEL"),
                    anom
                ),
            )
        })
        .collect();
    mil_list.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let top_mil: Vec<String> = mil_list.into_iter().take(10).map(|(_, s)| s).collect();

    let fusions: Vec<String> = state
        .fusion_clusters
        .iter()
        .take(5)
        .map(|f| {
            format!(
                "({:.2}N,{:.2}E) Skor:{} [{}]: {}",
                f.lat,
                f.lon,
                f.score,
                f.domains.join("+"),
                f.summary
            )
        })
        .collect();

    let eam_vec: Vec<_> = state.eam_messages.iter().collect();
    let eams: Vec<String> = eam_vec
        .into_iter()
        .rev()
        .take(4)
        .map(|e| format!("{} {:.1}kHz {} ({})", e.timestamp.format("%H:%M"), e.frequency_khz, e.preamble, e.auth_station))
        .collect();

    let quakes: Vec<String> = state
        .earthquakes
        .iter()
        .take(4)
        .map(|q| format!("M{:.1} {}", q.mag, q.place))
        .collect();

    let news: Vec<String> = state
        .news
        .iter()
        .take(6)
        .map(|n| format!("[{}] {}", n.source, n.title))
        .collect();

    let cybers: Vec<String> = state
        .cyber_incidents
        .iter()
        .take(4)
        .map(|c| c.title.clone())
        .collect();

    format!(
        "Sen Project Sentinel C4ISR istihbarat subayisin. Asagidaki CANLI gercek zamanli sensor verilerini analiz et ve Turkce olarak kisa, madde madde net bir TAKTIK DURUM RAPORU (SITREP) hazirla. Veri uydurma; sadece verilen telemetriyi yorumla.\n\n\
        BOLUMLER:\n\
        1. GENEL TEHDIT & DEFCON OZETI (2 cumle)\n\
        2. HAVA / DENIZ / ANOMALI DEGERLENDIRMESI\n\
        3. COKLU-SENSOR FUZYON & JEOSISMIK / SIBER BULGULAR\n\
        4. ONCELIKLI TAKIP ONERILERI (3 madde)\n\n\
        --- CANLI TELEMETRI ---\n\
        Istasyon: {:.4}N, {:.4}E\n\
        {}\n\
        {}\n\
        Toplam Hedef: ADS-B {} (Askeri: {}) | AIS {} (Harp: {}) | Uydu: {} | GPS-JAM Bolgesi: {}\n\
        Yakin/Anomalili Askeri Ucuslar: {}\n\
        Fuzyon Kriz Kumeleri: {}\n\
        Son HFGCS EAM: {}\n\
        Sismik: {}\n\
        Siber/IODA: {}\n\
        OSINT Haberler: {}",
        st_lat,
        st_lon,
        defcon_line,
        wx_line,
        state.air_targets.len(),
        mil_air,
        state.sea_targets.len(),
        mil_sea,
        state.satellites.len(),
        state.gps_jamming_zones.len(),
        if top_mil.is_empty() { "Yok".to_string() } else { top_mil.join(" | ") },
        if fusions.is_empty() { "Yok".to_string() } else { fusions.join(" | ") },
        if eams.is_empty() { "Yok".to_string() } else { eams.join(" | ") },
        if quakes.is_empty() { "Yok".to_string() } else { quakes.join(" | ") },
        if cybers.is_empty() { "Yok".to_string() } else { cybers.join(" | ") },
        if news.is_empty() { "Yok".to_string() } else { news.join(" | ") },
    )
}

/// Gemini Interactions API yanıtından düz metni ayıklar (`steps` ve `outputs` uyumlu)
pub fn parse_gemini_interactions_response(raw: &str) -> anyhow::Result<String> {
    let v: serde_json::Value = serde_json::from_str(raw)?;
    if let Some(err_msg) = v["error"]["message"].as_str() {
        anyhow::bail!("Gemini API Hatasi: {}", err_msg);
    }

    let mut collected = Vec::new();
    if let Some(steps) = v["steps"].as_array() {
        for step in steps {
            if step["type"].as_str() == Some("model_output") || step["type"].is_null() {
                if let Some(contents) = step["content"].as_array() {
                    for c in contents {
                        if let Some(txt) = c["text"].as_str() {
                            if !txt.trim().is_empty() {
                                collected.push(txt.trim().to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    if collected.is_empty() {
        if let Some(outputs) = v["outputs"].as_array() {
            for o in outputs {
                if let Some(txt) = o["text"].as_str() {
                    if !txt.trim().is_empty() {
                        collected.push(txt.trim().to_string());
                    }
                }
            }
        }
    }
    if collected.is_empty() {
        if let Some(txt) = v["output_text"].as_str() {
            if !txt.trim().is_empty() {
                collected.push(txt.trim().to_string());
            }
        }
    }

    if collected.is_empty() {
        anyhow::bail!("Gemini yanitindan metin cozumlenemedi");
    }
    Ok(collected.join("\n"))
}

pub async fn generate_ai_sitrep(
    api_key: &str,
    model_opt: Option<&str>,
    prompt: &str,
) -> anyhow::Result<String> {
    let key = api_key.trim();
    if key.is_empty() {
        anyhow::bail!("config.toml veya ARGUS_GEMINI_KEY icinde gemini_key tanimlanmamis");
    }
    let model = model_opt
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_GEMINI_MODEL);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(35))
        .user_agent("Project-Sentinel/0.1")
        .build()?;

    let payload = serde_json::json!({
        "model": model,
        "input": prompt,
    });

    let resp = client
        .post("https://generativelanguage.googleapis.com/v1beta/interactions")
        .header("x-goog-api-key", key)
        .json(&payload)
        .send()
        .await?;

    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!("HTTP {}: {}", status, &body[..body.len().min(280)]);
    }
    parse_gemini_interactions_response(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gemini_interactions_response() {
        let sample = r#"{
          "id":"v1_123",
          "model":"gemini-3.8-flash",
          "status":"completed",
          "steps":[{
            "type":"model_output",
            "content":[{"type":"text","text":"1. GENEL OZET: DEFCON 3 seviyesi izleniyor."}]
          }]
        }"#;
        let out = parse_gemini_interactions_response(sample).unwrap();
        assert!(out.contains("DEFCON 3"));
    }
}
