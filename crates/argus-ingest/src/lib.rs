pub mod acled;
pub mod aircraft_photo;
pub mod airspace_warnings;
pub mod aisstream;
pub mod celestrak;
pub mod cisa_kev;
pub mod defcon_osint;
pub mod eam_sigint;
pub mod flight_schedule;
pub mod gemini_intel;
pub mod ioda_outages;
pub mod kiwisdr;
pub mod nasa_firms;
pub mod news_wire;
pub mod noaa_space;
pub mod opensky;
pub mod safecast_radiation;
pub mod shodan;
pub mod sigint_dsp;
pub mod subsea_cables;
pub mod usgs_seismic;
pub mod weather_owm;

use argus_core::state::AppState;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Tüm asenkron canlı veri toplayıcılarını Tokio görev havuzunda başlatır
pub fn start_ingestion_workers(state: Arc<RwLock<AppState>>) {
    let cfg = {
        let st = state.read();
        st.config.clone()
    };

    // 0. Otomatik IP Tabanlı İstasyon Konumu Tespiti
    if cfg.station.auto_ip_location {
        let st = Arc::clone(&state);
        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default();
            let mut detected: Option<(f64, f64)> = None;

            if let Ok(resp) = client.get("https://ipwho.is/").send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if json["success"].as_bool() == Some(true) {
                        if let (Some(lat), Some(lon)) =
                            (json["latitude"].as_f64(), json["longitude"].as_f64())
                        {
                            if lat != 0.0 && lon != 0.0 && lat.abs() <= 90.0 && lon.abs() <= 180.0 {
                                detected = Some((lat, lon));
                            }
                        }
                    }
                }
            }

            if detected.is_none() {
                if let Ok(resp) = client
                    .get("http://ip-api.com/json/?fields=status,lat,lon")
                    .send()
                    .await
                {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if json["status"].as_str() == Some("success") {
                            if let (Some(lat), Some(lon)) =
                                (json["lat"].as_f64(), json["lon"].as_f64())
                            {
                                if lat != 0.0
                                    && lon != 0.0
                                    && lat.abs() <= 90.0
                                    && lon.abs() <= 180.0
                                {
                                    detected = Some((lat, lon));
                                }
                            }
                        }
                    }
                }
            }

            if let Some((lat, lon)) = detected {
                let mut lock = st.write();
                let unpanned = (lock.map_center_lat - lock.station_lat).abs() < 0.001
                    && (lock.map_center_lon - lock.station_lon).abs() < 0.001;
                lock.station_lat = lat;
                lock.station_lon = lon;
                if unpanned {
                    lock.map_center_lat = lat;
                    lock.map_center_lon = lon;
                }
            }
        });
    }

    // 1. AISStream WebSocket Görevi (Deniz Trafiği)
    if let Some(key) = &cfg.api_keys.aisstream_key {
        let client = aisstream::AisStreamClient::new(Some(key.clone()));
        let st = Arc::clone(&state);
        tokio::spawn(async move {
            client.run_listener(st).await;
        });
    }

    // 2. OpenSky / ADS-B Görevi (Hava Trafiği - Dinamik İstasyon ve Harita Merkezi)
    {
        let st = Arc::clone(&state);
        tokio::spawn(async move {
            let client = opensky::OpenSkyClient::new();
            loop {
                let (s_lat, s_lon, m_lat, m_lon) = {
                    let lock = st.read();
                    (
                        lock.station_lat,
                        lock.station_lon,
                        lock.map_center_lat,
                        lock.map_center_lon,
                    )
                };
                if let Ok(tracks) = client.fetch_live_flights_at(s_lat, s_lon, m_lat, m_lon).await {
                    let jamming_zones = client.detect_jamming_zones(&tracks);
                    let mut lock = st.write();
                    lock.gps_jamming_zones = jamming_zones;
                    lock.upsert_targets_batch(tracks);
                    lock.prune_stale_targets(900);
                }
                sleep(Duration::from_secs(15)).await;
            }
        });
    }

    // 3. NASA FIRMS Termal Yangın / Patlama Görevi
    if let Some(key) = &cfg.api_keys.nasa_firms_key {
        let client = nasa_firms::NasaFirmsClient::new(Some(key.clone()));
        let st = Arc::clone(&state);
        tokio::spawn(async move {
            loop {
                if let Ok(fires) = client.fetch_recent_fires().await {
                    let mut lock = st.write();
                    for fire in fires {
                        lock.upsert_fire(fire);
                        lock.total_events_received += 1;
                    }
                }
                sleep(Duration::from_secs(60)).await;
            }
        });
    }

    // 4. Çatışma ve Askeri Olaylar Görevi (ACLED birincil + Anahtarsız GDELT v2 15-dk CAMEO yedeği)
    {
        let client = acled::AcledClient::new(cfg.acled.email.clone(), cfg.acled.password.clone());
        let st = Arc::clone(&state);
        tokio::spawn(async move {
            loop {
                if let Ok(events) = client.fetch_recent_conflicts().await {
                    let mut lock = st.write();
                    for ev in events {
                        lock.upsert_acled_event(ev);
                        lock.total_events_received += 1;
                    }
                }
                sleep(Duration::from_secs(180)).await;
            }
        });
    }

    // 5. CelesTrak Uyduları & SGP4 Yörünge Fiziği Görevi
    {
        let st = Arc::clone(&state);
        tokio::spawn(async move {
            let client = celestrak::CelesTrakClient::new();
            loop {
                let (lat, lon) = {
                    let lock = st.read();
                    (lock.station_lat, lock.station_lon)
                };
                if let Ok(sats) = client.fetch_satellites(lat, lon).await {
                    let mut lock = st.write();
                    lock.satellites = sats;
                }
                sleep(Duration::from_secs(3)).await;
            }
        });
    }

    // 6. Haberler & Kriz Wire Görevi
    {
        let st = Arc::clone(&state);
        tokio::spawn(async move {
            let client = news_wire::NewsWireClient::new();
            loop {
                if let Ok(news_items) = client.fetch_latest_news().await {
                    let mut lock = st.write();
                    for item in news_items {
                        lock.upsert_news(item);
                        lock.total_events_received += 1;
                    }
                }
                sleep(Duration::from_secs(30)).await;
            }
        });
    }

    // 7. USGS Canlı Depremler Görevi
    {
        let st = Arc::clone(&state);
        tokio::spawn(async move {
            let client = usgs_seismic::UsgsClient::new();
            loop {
                if let Ok(quakes) = client.fetch_recent_earthquakes().await {
                    let mut lock = st.write();
                    for q in quakes {
                        lock.upsert_earthquake(q);
                        lock.total_events_received += 1;
                    }
                }
                sleep(Duration::from_secs(30)).await;
            }
        });
    }

    // 8. CISA KEV ve IODA Siber Olayları Görevi
    {
        let st = Arc::clone(&state);
        tokio::spawn(async move {
            let cisa = cisa_kev::CisaKevClient::new();
            let ioda = ioda_outages::IodaClient::new();
            loop {
                if let Ok(vulns) = cisa.fetch_exploited_vulns().await {
                    let mut lock = st.write();
                    for v in vulns {
                        lock.upsert_cyber_incident(v);
                        lock.total_events_received += 1;
                    }
                }
                if let Ok(outages) = ioda.fetch_internet_outages().await {
                    let mut lock = st.write();
                    for out in outages {
                        lock.upsert_cyber_incident(out);
                        lock.total_events_received += 1;
                    }
                }
                sleep(Duration::from_secs(60)).await;
            }
        });
    }

    // 9. NOAA Uzay Havası Görevi
    {
        let st = Arc::clone(&state);
        tokio::spawn(async move {
            let client = noaa_space::NoaaSpaceClient::new();
            loop {
                if let Ok(sw) = client.fetch_space_weather().await {
                    let mut lock = st.write();
                    lock.solar_weather = Some(sw);
                }
                sleep(Duration::from_secs(60)).await;
            }
        });
    }

    // 10. SIGINT & HFGCS EAM Görevi + Canlı KiwiSDR Kısa Dalga Ses/Spektrum İstemcisi
    {
        let st = Arc::clone(&state);
        tokio::spawn(async move {
            eam_sigint::start_sigint_worker(st).await;
        });
    }
    {
        let st = Arc::clone(&state);
        tokio::spawn(async move {
            kiwisdr::run_kiwisdr_worker(st).await;
        });
    }

    // 11. Safecast Canlı Çevresel Radyasyon & Nükleer Sensör Görevi
    {
        let st = Arc::clone(&state);
        tokio::spawn(async move {
            let client = safecast_radiation::SafecastClient::new();
            loop {
                if let Ok(measurements) = client.fetch_radiation_measurements().await {
                    let mut lock = st.write();
                    for m in measurements {
                        lock.upsert_radiation_measurement(m);
                        lock.total_events_received += 1;
                    }
                }
                sleep(Duration::from_secs(45)).await;
            }
        });
    }

    // 12. Küresel OSINT DEFCON & PizzINT Hazırlık Durumu Görevi
    {
        let st = Arc::clone(&state);
        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default();
            loop {
                if let Some(defcon) = defcon_osint::fetch_osint_defcon(&client).await {
                    let mut lock = st.write();
                    let prev_lvl = lock.defcon_status.as_ref().map(|d| d.overall_level);
                    let lvl_changed = prev_lvl.is_some_and(|p| p != defcon.overall_level);
                    if defcon.overall_level <= 3 || lvl_changed {
                        let sev = if defcon.overall_level <= 2 {
                            argus_core::types::ThreatSeverity::Emergency
                        } else if defcon.overall_level == 3 {
                            argus_core::types::ThreatSeverity::Critical
                        } else {
                            argus_core::types::ThreatSeverity::Advisory
                        };
                        let msg = if lvl_changed {
                            format!(
                                "[DEFCON] Seviye Degisti: DEFCON {} -> DEFCON {} ({})",
                                prev_lvl.unwrap_or(5),
                                defcon.overall_level,
                                defcon.source
                            )
                        } else {
                            format!(
                                "[DEFCON] Yuksek Hazirlik: DEFCON {} ({})",
                                defcon.overall_level, defcon.source
                            )
                        };
                        let item = argus_core::types::AlertItem {
                            timestamp: chrono::Utc::now(),
                            domain: "DEFCON".to_string(),
                            severity: sev,
                            message: msg,
                            target_id: Some("SYS:DEFCON".to_string()),
                        };
                        lock.alerts
                            .upsert_by(item, |a| a.target_id.as_deref() == Some("SYS:DEFCON"));
                    }
                    lock.defcon_status = Some(defcon);
                    lock.total_events_received = lock.total_events_received.wrapping_add(1);
                }
                sleep(Duration::from_secs(180)).await;
            }
        });
    }

    // 13. K-1: Küresel Denizaltı Fiber Kabloları Katmanı
    {
        let st = Arc::clone(&state);
        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .user_agent("Project-Sentinel/0.1")
                .build()
                .unwrap_or_default();
            loop {
                let cables = subsea_cables::fetch_subsea_cables(&client).await;
                if !cables.is_empty() {
                    let mut lock = st.write();
                    lock.subsea_cables = cables;
                }
                sleep(Duration::from_secs(43200)).await;
            }
        });
    }

    // 14. A-3: OpenWeatherMap Anlık Meteoroloji Gözlemi (Opsiyonel Anahtar Varsa)
    if let Some(owm_key) = cfg.api_keys.openweather_key.clone().filter(|k| !k.trim().is_empty()) {
        let st = Arc::clone(&state);
        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default();
            loop {
                let (slat, slon) = {
                    let r = st.read();
                    (r.station_lat, r.station_lon)
                };
                let (station_wx, points) =
                    weather_owm::fetch_weather_snapshot(&client, &owm_key, slat, slon).await;
                if station_wx.is_some() || !points.is_empty() {
                    let mut lock = st.write();
                    if station_wx.is_some() {
                        lock.station_weather = station_wx;
                    }
                    if !points.is_empty() {
                        lock.weather_points = points;
                    }
                }
                sleep(Duration::from_secs(600)).await;
            }
        });
    }

    // 15. K-4: Çapraz-Alan Füzyon & Hava/Deniz İz Anomalisi Motoru
    {
        let st = Arc::clone(&state);
        tokio::spawn(async move {
            sleep(Duration::from_secs(6)).await;
            loop {
                {
                    let mut lock = st.write();
                    argus_core::fusion::recompute_fusion_and_anomalies(&mut lock);
                }
                sleep(Duration::from_secs(8)).await;
            }
        });
    }

    // 16. N-4: Canlı International SIGMET, Çatışma NOTAM Sahaları ve GPS-JAM Çokgenleri
    {
        let st = Arc::clone(&state);
        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .local_address(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED))
                .timeout(Duration::from_secs(10))
                .user_agent("Project-Sentinel/0.1")
                .build()
                .unwrap_or_default();
            loop {
                let jam_snapshot = st.read().gps_jamming_zones.clone();
                let warnings = airspace_warnings::fetch_airspace_warnings(&client, &jam_snapshot).await;
                if !warnings.is_empty() {
                    let mut lock = st.write();
                    lock.airspace_warnings = warnings;
                }
                sleep(Duration::from_secs(120)).await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_celestrak_satellites_sgp4() {
        let client = celestrak::CelesTrakClient::new();
        let sats = client.fetch_satellites(41.0082, 28.9784).await.expect("Satellites fetch failed");
        assert!(!sats.is_empty(), "Satellites list should not be empty");
        let iss = sats.iter().find(|s| s.norad_id == 25544);
        assert!(iss.is_some(), "ISS should be present in orbital propagation");
        let iss_pass = iss.unwrap();
        assert!(iss_pass.alt_km > 300.0, "ISS altitude must be > 300km");
        println!("Propagated {} satellites. ISS: alt={:.1}km, vel={:.2}km/s, az={:.1}°, el={:.1}°",
            sats.len(), iss_pass.alt_km, iss_pass.velocity_kms, iss_pass.az_deg, iss_pass.el_deg);
    }

    #[tokio::test]
    async fn test_cisa_kev_data() {
        let client = cisa_kev::CisaKevClient::new();
        let vulns = client.fetch_exploited_vulns().await.expect("CISA KEV fetch failed");
        assert!(!vulns.is_empty(), "CISA KEV should return active vulnerabilities");
        println!("Fetched {} CISA KEV vulnerabilities. Sample: {} - {}", vulns.len(), vulns[0].id, vulns[0].title);
    }

    #[tokio::test]
    async fn test_ioda_outages_data() {
        let client = ioda_outages::IodaClient::new();
        let outages = client.fetch_internet_outages().await.expect("IODA fetch failed");
        assert!(!outages.is_empty(), "IODA should return internet outages");
        println!("Fetched {} IODA outages. Sample: {} - {}", outages.len(), outages[0].id, outages[0].title);
    }

    #[tokio::test]
    async fn test_live_aircraft_data() {
        let client = opensky::OpenSkyClient::new();
        let flights = client.fetch_live_flights().await.expect("Live flights fetch failed");
        assert!(!flights.is_empty(), "Should return live flights from global ADS-B feeds");
        let mut theaters: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for f in &flights {
            let key = f.origin.as_deref().unwrap_or("UNKNOWN");
            *theaters.entry(key).or_insert(0) += 1;
        }
        println!(
            "Fetched {} live global flights. Theaters: {:?}. Sample: {} ({})",
            flights.len(),
            theaters,
            flights[0].callsign,
            flights[0].id
        );
        assert!(
            flights.len() > 1000 && theaters.len() >= 4,
            "Expected global coverage across >=4 theaters and >1000 flights, got {} flights across {:?}",
            flights.len(),
            theaters
        );
    }

    #[tokio::test]
    async fn test_aisstream_live() {
        use tokio_tungstenite::connect_async;
        use tokio_tungstenite::tungstenite::Message;
        use futures_util::{SinkExt, StreamExt};

        let cfg = argus_core::config::AppConfig::load_or_default();
        let Some(key) = cfg.api_keys.aisstream_key else {
            return;
        };
        if key.trim().is_empty() || key.contains("YOUR_") {
            return;
        }
        let url = "wss://stream.aisstream.io/v0/stream";
        let (mut ws, _) = connect_async(url).await.expect("WebSocket connect failed");

        let sub_msg = serde_json::json!({
            "APIKey": key,
            "BoundingBoxes": [[[48.0, 15.0], [20.0, 45.0]]],
            "FilterMessageTypes": ["PositionReport", "ShipStaticData"]
        });

        ws.send(Message::Text(sub_msg.to_string())).await.expect("Send failed");

        // Try reading messages with 15s timeout
        println!("Waiting for AISStream message...");
        while let Ok(msg_opt) = tokio::time::timeout(std::time::Duration::from_secs(12), ws.next()).await {
            match msg_opt {
                Some(Ok(Message::Text(txt))) => {
                    println!("AISStream Got: {}", &txt[..txt.len().min(120)]);
                    break;
                }
                Some(Ok(other)) => {
                    println!("AISStream other: {:?}", other);
                }
                Some(Err(e)) => {
                    println!("AISStream read error: {:?}", e);
                    break;
                }
                None => {
                    println!("AISStream stream closed (None)");
                    break;
                }
            }
        }
    }

    #[tokio::test]
    async fn test_safecast_radiation_data() {
        let client = safecast_radiation::SafecastClient::new();
        match client.fetch_radiation_measurements().await {
            Ok(rads) => {
                println!("Fetched {} Safecast radiation measurements.", rads.len());
            }
            Err(e) => {
                eprintln!("Safecast API transient network error: {}", e);
            }
        }
    }
}
