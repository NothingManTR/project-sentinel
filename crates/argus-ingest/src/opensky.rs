use argus_core::types::{Domain, GeoPoint, TargetTrack};
use chrono::Utc;
use futures_util::future::join_all;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Koordinata göre küresel askeri harekat komutanlığı / bölge (AOR) sınıflandırması
pub fn classify_air_theater(lat: f64, lon: f64) -> &'static str {
    // US CENTCOM AOR: Orta Doğu, Mısır/Sina, Kızıldeniz, Levant, Irak, Suriye, İran, Basra Körfezi, Afganistan/Pakistan
    if ((12.0..=36.8).contains(&lat) && (31.5..=75.0).contains(&lon))
        || ((36.8..=40.0).contains(&lat) && (44.8..=75.0).contains(&lon))
    {
        "CENTCOM"
    } else if (34.5..=72.0).contains(&lat) && (-15.0..=60.0).contains(&lon) {
        "TR/EUCOM"
    } else if (-38.0..34.5).contains(&lat) && (-22.0..=52.0).contains(&lon) {
        "AFRICOM"
    } else if lon > 60.0 || lon < -140.0 {
        "INDOPACOM"
    } else if (15.0..=85.0).contains(&lat) && (-168.0..=-15.0).contains(&lon) {
        "NORTHCOM"
    } else if (-60.0..15.0).contains(&lat) && (-120.0..=-15.0).contains(&lon) {
        "SOUTHCOM"
    } else {
        "KURESEL"
    }
}

/// Çağrı kodu ve ICAO 24-bit adresine göre askeri kuvvet, filo ve platform çıkarımı
#[allow(clippy::too_many_arguments)]
fn enrich_aircraft_heuristics(
    callsign: &str,
    icao: &str,
    is_mil_flag: bool,
    own_op: &str,
    desc: &str,
    ac_type: &str,
    fallback_country: Option<String>,
) -> (bool, Option<String>, Option<String>, Option<String>) {
    let callsign_upper = callsign.to_uppercase();
    let icao_lower = icao.to_lowercase();
    let mut is_military = is_mil_flag;
    let mut operator_branch = if !own_op.is_empty() {
        Some(own_op.to_string())
    } else {
        None
    };
    let mut airframe_class = if !desc.is_empty() {
        Some(desc.to_string())
    } else if !ac_type.is_empty() {
        Some(ac_type.to_string())
    } else {
        None
    };
    let mut operator_country = fallback_country;

    if callsign_upper.starts_with("FORTE") {
        is_military = true;
        operator_branch = Some("USAF 319th RW / ISR".to_string());
        airframe_class = Some("RQ-4B Global Hawk (HALE ISR)".to_string());
        operator_country = Some("United States".to_string());
    } else if callsign_upper.starts_with("LAGR")
        || callsign_upper.starts_with("NCHO")
        || callsign_upper.starts_with("GOLD")
        || callsign_upper.starts_with("QUID")
        || callsign_upper.starts_with("PACK")
        || callsign_upper.starts_with("ETHYL")
    {
        is_military = true;
        operator_branch = Some("USAF / CENTCOM Tanker Task Force".to_string());
        if airframe_class.is_none() {
            airframe_class = Some("KC-135R Stratotanker / KC-46A Pegasus".to_string());
        }
        operator_country = Some("United States".to_string());
    } else if callsign_upper.starts_with("RCH")
        || callsign_upper.starts_with("REACH")
        || callsign_upper.starts_with("CMB")
    {
        is_military = true;
        operator_branch = Some("USAF Air Mobility Command".to_string());
        if airframe_class.is_none() {
            airframe_class = Some("C-17A Globemaster III / C-5M".to_string());
        }
        operator_country = Some("United States".to_string());
    } else if callsign_upper.starts_with("JAKE")
        || callsign_upper.starts_with("HOMER")
        || callsign_upper.starts_with("RIVER")
        || callsign_upper.starts_with("OLIVE")
        || callsign_upper.starts_with("COBRA")
    {
        is_military = true;
        operator_branch = Some("USAF 55th Wing (SIGINT/ISR)".to_string());
        if airframe_class.is_none() {
            airframe_class = Some("RC-135V/W Rivet Joint (SIGINT)".to_string());
        }
        operator_country = Some("United States".to_string());
    } else if callsign_upper.starts_with("TUAF")
        || callsign_upper.starts_with("HVKK")
        || callsign_upper.starts_with("TURAF")
        || (callsign_upper.starts_with("TRK") && is_mil_flag)
    {
        is_military = true;
        operator_branch = Some("Turk Hava Kuvvetleri".to_string());
        if airframe_class.is_none() {
            airframe_class = Some("HvKK Taktik / ISR Filo".to_string());
        }
        operator_country = Some("Turkiye".to_string());
    } else if callsign_upper.starts_with("NATO") {
        is_military = true;
        operator_branch = Some("NATO NAEW&CF E-3A".to_string());
        if airframe_class.is_none() {
            airframe_class = Some("Boeing E-3A Sentry (AWACS)".to_string());
        }
        operator_country = Some("NATO".to_string());
    } else if callsign_upper.starts_with("RRR")
        || callsign_upper.starts_with("ASCOT")
        || callsign_upper.starts_with("TARTN")
    {
        is_military = true;
        operator_branch = Some("Royal Air Force (RAF)".to_string());
        if airframe_class.is_none() {
            airframe_class = Some("RAF A330 MRTT / RC-135W / C-17".to_string());
        }
        operator_country = Some("United Kingdom".to_string());
    } else if callsign_upper.starts_with("CNV")
        || callsign_upper.starts_with("NAVY")
        || callsign_upper.starts_with("VV")
    {
        is_military = true;
        operator_branch = Some("US Navy (5th/6th Fleet Aviation)".to_string());
        if airframe_class.is_none() {
            airframe_class = Some("P-8A Poseidon / E-2D / C-40A".to_string());
        }
        operator_country = Some("United States".to_string());
    } else if callsign_upper.starts_with("RSAF") {
        is_military = true;
        operator_branch = Some("Royal Saudi Air Force".to_string());
        operator_country = Some("Saudi Arabia".to_string());
    } else if callsign_upper.starts_with("IAF") {
        is_military = true;
        operator_branch = Some("Israeli Air Force".to_string());
        operator_country = Some("Israel".to_string());
    } else if callsign_upper.starts_with("QAF") || callsign_upper.starts_with("LHOB") {
        is_military = true;
        operator_branch = Some("Qatar Emiri Air Force".to_string());
        operator_country = Some("Qatar".to_string());
    } else if callsign_upper.starts_with("UAF")
        || (callsign_upper.starts_with("UAE") && is_mil_flag)
    {
        is_military = true;
        operator_branch = Some("UAE Air Force".to_string());
        operator_country = Some("UAE".to_string());
    } else if icao_lower.starts_with("ae") || (icao_lower.starts_with("ad") && is_mil_flag) {
        is_military = true;
        if operator_branch.is_none() {
            operator_branch = Some("USAF / USCENTCOM / DoD".to_string());
        }
        operator_country = Some("United States".to_string());
    } else if icao_lower.starts_with("43c") {
        is_military = true;
        if operator_branch.is_none() {
            operator_branch = Some("UK RAF / MoD".to_string());
        }
        operator_country = Some("United Kingdom".to_string());
    } else if icao_lower.starts_with("3f") {
        is_military = true;
        if operator_branch.is_none() {
            operator_branch = Some("German Air Force (Luftwaffe)".to_string());
        }
        operator_country = Some("Germany".to_string());
    } else if icao_lower.starts_with("3b") {
        is_military = true;
        if operator_branch.is_none() {
            operator_branch = Some("French Air & Space Force".to_string());
        }
        operator_country = Some("France".to_string());
    } else if icao_lower.starts_with("738") && is_mil_flag {
        is_military = true;
        if operator_branch.is_none() {
            operator_branch = Some("Israeli Air Force (IAF)".to_string());
        }
        operator_country = Some("Israel".to_string());
    } else if icao_lower.starts_with("710") && is_mil_flag {
        is_military = true;
        if operator_branch.is_none() {
            operator_branch = Some("Royal Saudi Air Force".to_string());
        }
        operator_country = Some("Saudi Arabia".to_string());
    } else if icao_lower.starts_with("4b8") && is_mil_flag {
        is_military = true;
        if operator_branch.is_none() {
            operator_branch = Some("Turk Hava Kuvvetleri".to_string());
        }
        operator_country = Some("Turkiye".to_string());
    }

    (is_military, operator_country, operator_branch, airframe_class)
}

pub struct OpenSkyClient {
    client: reqwest::Client,
    global_cache: Mutex<(Option<Instant>, Vec<TargetTrack>)>,
}

impl Default for OpenSkyClient {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenSkyClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(12))
                .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .build()
                .unwrap_or_default(),
            global_cache: Mutex::new((None, Vec::new())),
        }
    }

    /// Tüm Dünya genelindeki canlı uçuşları (OpenSky küresel + adsb.fi / adsb.lol askeri + stratejik bölgeler) çeker
    pub async fn fetch_live_flights(&self) -> anyhow::Result<Vec<TargetTrack>> {
        self.fetch_live_flights_at(41.0082, 28.9784, 38.5, 32.0).await
    }

    /// Küresel Ölçekli (Tüm Dünya 10000+ canlı sivil, askeri & okyanus ötesi uçuş) + CENTCOM + TR/EUCOM
    pub async fn fetch_live_flights_at(
        &self,
        station_lat: f64,
        station_lon: f64,
        map_lat: f64,
        map_lon: f64,
    ) -> anyhow::Result<Vec<TargetTrack>> {
        let mut tracks = Vec::with_capacity(14000);
        let mut seen_ids: HashSet<String> = HashSet::with_capacity(14000);

        // 1. Yüksek Detaylı Askeri Akışlar ve Stratejik Bölge Sektörleri (Filtresiz ADS-B)
        let mut sectors = vec![
            // CENTCOM HAREKAT ALANI (Orta Doğu, Körfez, Kızıldeniz, Levant)
            (33.50, 35.60),
            (33.30, 44.30),
            (25.50, 51.20),
            (25.30, 56.00),
            (27.50, 35.00),
            (15.50, 42.50),
            (24.70, 46.70),
            (32.50, 51.50),
            // TÜRKİYE & EUCOM
            (station_lat, station_lon),
            (39.20, 36.50),
            (37.80, 23.50),
            (44.20, 33.50),
            (51.50, 18.50),
            (50.50, 6.00),
        ];

        if sectors
            .iter()
            .all(|(slat, slon)| (map_lat - slat).hypot(map_lon - slon) > 3.5)
        {
            sectors.push((map_lat, map_lon));
        }

        let mut adsb_urls = Vec::with_capacity(sectors.len() + 3);
        adsb_urls.push("https://opendata.adsb.fi/api/v2/mil".to_string());
        adsb_urls.push("https://api.adsb.lol/v2/mil".to_string());
        adsb_urls.push("https://api.adsb.lol/v2/ladd".to_string());
        for (lat, lon) in sectors {
            adsb_urls.push(format!(
                "https://opendata.adsb.fi/api/v2/lat/{:.4}/lon/{:.4}/dist/250",
                lat, lon
            ));
        }

        // 2. Tam Küresel Bölünmüş Mozaik Ağı (20 Bölge - Amerika, Avrupa, Afrika, Asya, Okyanusya, Okyanuslar + Uydu ADS-B)
        let fr24_boxes: [(i32, i32, i32, i32); 20] = [
            (70, 20, -170, -110), // Kuzey Amerika Batı & Alaska
            (70, 20, -110, -90),  // Kuzey Amerika Orta
            (70, 20, -90, -78),   // Kuzey Amerika Doğu-İç
            (70, 20, -78, -50),   // Kuzey Amerika Atlantik Kıyısı
            (25, 5, -120, -55),   // Orta Amerika & Karayipler
            (12, -60, -90, -30),  // Güney Amerika
            (70, 35, -15, 7),     // Batı Avrupa & İngiltere
            (70, 35, 7, 24),      // Orta Avrupa & Akdeniz
            (70, 35, 24, 60),     // Türkiye, Balkanlar, Karadeniz, Doğu Avrupa
            (37, 10, 28, 46),     // CENTCOM Batı (Levant, Mısır, Kızıldeniz, Irak, Suriye)
            (39, 10, 46, 75),     // CENTCOM Doğu (Körfez, Suudi Arabistan, İran, Pakistan)
            (35, -40, -25, 52),   // Afrika Kıtası
            (38, -10, 68, 95),    // Güney Asya & Hint Okyanusu
            (28, -15, 95, 130),   // Güneydoğu Asya & Güney Çin Denizi
            (55, 28, 95, 122),    // Kuzey Çin, Moğolistan & Kore
            (55, 28, 122, 150),   // Japonya & Doğu Denizi
            (0, -55, 110, 180),   // Avustralya, Yeni Zelanda & Güney Pasifik
            (70, -55, -60, -15),  // Kuzey & Güney Atlantik Okyanus Koridoru
            (75, -55, 150, 180),  // Pasifik Doğu & Bering
            (75, 50, 60, 150),    // Kuzey Asya & Sibirya Koridoru
        ];

        let fr24_urls: Vec<String> = fr24_boxes
            .iter()
            .map(|(n, s, w, e)| {
                format!(
                    "https://data-cloud.flightradar24.com/zones/fcgi/feed.js?faa=1&satellite=1&mlat=1&flarm=1&adsb=1&gnd=0&air=1&vehicles=0&estimated=0&maxage=14400&gliders=0&stats=0&bounds={},{},{},{}",
                    n, s, w, e
                )
            })
            .collect();

        let client = &self.client;
        let adsb_fut = join_all(adsb_urls.into_iter().map(|url| async move {
            if let Ok(resp) = client.get(&url).send().await {
                if resp.status().is_success() {
                    return resp.json::<serde_json::Value>().await.ok();
                }
            }
            None
        }));

        let fr24_fut = join_all(fr24_urls.into_iter().map(|url| async move {
            if let Ok(resp) = client.get(&url).send().await {
                if resp.status().is_success() {
                    return resp.json::<serde_json::Value>().await.ok();
                }
            }
            None
        }));

        let (adsb_results, fr24_results) = tokio::join!(adsb_fut, fr24_fut);

        for json in adsb_results.into_iter().flatten() {
            Self::parse_adsbfi_json(&json, &mut tracks, &mut seen_ids);
        }
        for json in fr24_results.into_iter().flatten() {
            Self::parse_fr24_json(&json, &mut tracks, &mut seen_ids);
        }

        // 3. OpenSky Network (/api/states/all - Tam Küresel Destek / Önbellekli 90s)
        let need_refresh = {
            let cache = self.global_cache.lock().await;
            cache
                .0
                .map(|t| t.elapsed() >= Duration::from_secs(90))
                .unwrap_or(true)
        };

        if need_refresh {
            if let Ok(resp) = self
                .client
                .get("https://opensky-network.org/api/states/all")
                .send()
                .await
            {
                if resp.status().is_success() {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        let mut global_list = Vec::with_capacity(8000);
                        let mut global_seen = HashSet::with_capacity(8000);
                        Self::parse_opensky_json(&json, &mut global_list, &mut global_seen);
                        if !global_list.is_empty() {
                            let mut cache = self.global_cache.lock().await;
                            *cache = (Some(Instant::now()), global_list);
                        }
                    }
                }
            }
        }

        let now = Utc::now();
        {
            let cache = self.global_cache.lock().await;
            for t in &cache.1 {
                if seen_ids.insert(t.id.clone()) {
                    let mut cloned = t.clone();
                    cloned.last_seen = now;
                    tracks.push(cloned);
                }
            }
        }

        Ok(tracks)
    }

    fn parse_fr24_json(
        json: &serde_json::Value,
        tracks: &mut Vec<TargetTrack>,
        seen_ids: &mut HashSet<String>,
    ) {
        let Some(obj) = json.as_object() else {
            return;
        };
        let now = Utc::now();
        for (key, val) in obj {
            let Some(arr) = val.as_array() else {
                continue;
            };
            if arr.len() < 17 {
                continue;
            }
            let on_ground = arr.get(14).and_then(|v| v.as_i64()).unwrap_or(0) != 0;
            if on_ground {
                continue;
            }

            let raw_icao = arr.first().and_then(|v| v.as_str()).unwrap_or("").trim();
            let icao = if raw_icao.is_empty() {
                key.to_lowercase()
            } else {
                raw_icao.to_lowercase()
            };
            if icao.is_empty() || !seen_ids.insert(icao.clone()) {
                continue;
            }

            let Some(lat) = arr.get(1).and_then(|v| v.as_f64()) else { continue };
            let Some(lon) = arr.get(2).and_then(|v| v.as_f64()) else { continue };
            if (lat == 0.0 && lon == 0.0) || lat.abs() > 89.0 || lon.abs() > 180.0 {
                continue;
            }

            let heading_deg = arr.get(3).and_then(|v| v.as_f64());
            let alt_ft = arr.get(4).and_then(|v| v.as_f64());
            let alt_m = alt_ft.map(|ft| ft * 0.3048);
            let speed_kts = arr.get(5).and_then(|v| v.as_f64());
            let squawk = arr
                .get(6)
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let ac_type = arr.get(8).and_then(|v| v.as_str()).unwrap_or("").trim();
            let reg = arr.get(9).and_then(|v| v.as_str()).unwrap_or("").trim();
            let orig_iata = arr.get(11).and_then(|v| v.as_str()).unwrap_or("").trim();
            let dest_iata = arr.get(12).and_then(|v| v.as_str()).unwrap_or("").trim();
            let vertical_rate_fpm = arr.get(15).and_then(|v| v.as_f64());
            let cs = arr.get(16).and_then(|v| v.as_str()).unwrap_or("").trim();
            let flight_no = arr.get(13).and_then(|v| v.as_str()).unwrap_or("").trim();
            let airline = arr.get(18).and_then(|v| v.as_str()).unwrap_or("").trim();

            let callsign = if !cs.is_empty() {
                cs.to_string()
            } else if !flight_no.is_empty() {
                flight_no.to_string()
            } else if !reg.is_empty() {
                reg.to_string()
            } else {
                icao.clone()
            };

            let (is_military, operator_country, operator_branch, airframe_class) =
                enrich_aircraft_heuristics(&callsign, &icao, false, airline, "", ac_type, None);

            let theater = classify_air_theater(lat, lon);
            let sqwk_str = squawk.clone().unwrap_or_else(|| "----".to_string());
            let route_str = if !orig_iata.is_empty() && !dest_iata.is_empty() {
                Some(format!("{}->{}", orig_iata, dest_iata))
            } else {
                None
            };
            let mut info = if let Some(ref r) = route_str {
                format!(
                    "[{}] {} | FL{:.0} | SQWK: {}",
                    theater,
                    r,
                    alt_ft.unwrap_or(0.0) / 100.0,
                    sqwk_str
                )
            } else {
                format!(
                    "[{}] FL{:.0} | SQWK: {}",
                    theater,
                    alt_ft.unwrap_or(0.0) / 100.0,
                    sqwk_str
                )
            };
            if is_military
                && alt_ft.is_some_and(|a| a < 3000.0)
                && speed_kts.is_some_and(|s| s > 350.0)
            {
                info.push_str(" [LOW-FAST]");
            }

            tracks.push(TargetTrack {
                id: icao,
                callsign,
                domain: Domain::Air,
                geo: GeoPoint::new(lat, lon, alt_m),
                speed_kts,
                heading_deg,
                vertical_rate_fpm,
                squawk,
                is_military,
                operator_country,
                operator_branch,
                airframe_or_vessel_class: airframe_class,
                origin: Some(theater.to_string()),
                destination: route_str,
                vessel_type: Some(if is_military {
                    format!("ASKERI ({})", theater)
                } else {
                    format!("SIVIL ({})", theater)
                }),
                info,
                last_seen: now,
            });
        }
    }

    fn parse_adsbfi_json(
        json: &serde_json::Value,
        tracks: &mut Vec<TargetTrack>,
        seen_ids: &mut HashSet<String>,
    ) {
        let list = json
            .get("aircraft")
            .or_else(|| json.get("ac"))
            .and_then(|a| a.as_array());
        if let Some(aircraft) = list {
            for item in aircraft.iter().take(4000) {
                let icao = item["hex"].as_str().unwrap_or("").trim().to_lowercase();
                if icao.is_empty() || !seen_ids.insert(icao.clone()) {
                    continue;
                }

                let flight = item["flight"].as_str().unwrap_or("").trim().to_string();
                let callsign = if flight.is_empty() { icao.clone() } else { flight };
                let lat = item["lat"].as_f64();
                let lon = item["lon"].as_f64();

                if let (Some(lat), Some(lon)) = (lat, lon) {
                    if lat == 0.0 && lon == 0.0 {
                        continue;
                    }

                    let alt_baro = item["alt_baro"].as_f64();
                    let alt_geom = item["alt_geom"].as_f64();
                    let alt_ft = if let Some(ft) = alt_baro {
                        Some(ft)
                    } else if item["alt_baro"].as_str() == Some("ground") {
                        Some(0.0)
                    } else {
                        alt_geom
                    };
                    let alt_m = alt_ft.map(|ft| ft * 0.3048);

                    let speed_kts = item["gs"].as_f64();
                    let heading_deg = item["track"].as_f64();
                    let vertical_rate_fpm = item["baro_rate"].as_f64();
                    let squawk = item["squawk"].as_str().map(|s| s.to_string());

                    let db_flags = item["dbFlags"].as_u64().unwrap_or(0);
                    let is_mil_flag = (db_flags & 1) != 0;
                    let desc = item["desc"].as_str().unwrap_or("");
                    let own_op = item["ownOp"].as_str().unwrap_or("");
                    let ac_type = item["t"].as_str().unwrap_or("");

                    let (is_military, operator_country, operator_branch, airframe_class) =
                        enrich_aircraft_heuristics(
                            &callsign,
                            &icao,
                            is_mil_flag,
                            own_op,
                            desc,
                            ac_type,
                            None,
                        );

                    let theater = classify_air_theater(lat, lon);
                    let squawk_str = squawk.clone().unwrap_or_else(|| "----".to_string());
                    let low_nav_integrity = item["nic"].as_u64().is_some_and(|n| n < 5)
                        || item["nac_p"].as_u64().is_some_and(|n| n < 5);
                    let mut info = format!(
                        "[{}] FL{:.0} | SQWK: {}",
                        theater,
                        alt_ft.unwrap_or(0.0) / 100.0,
                        squawk_str
                    );
                    if low_nav_integrity {
                        info.push_str(" | [EW:GPS-JAM]");
                    }
                    if is_military
                        && alt_ft.is_some_and(|a| a < 3000.0)
                        && speed_kts.is_some_and(|s| s > 350.0)
                    {
                        info.push_str(" [LOW-FAST]");
                    }
                    if let (Some(geom), Some(baro)) = (alt_geom, alt_baro) {
                        if (geom - baro).abs() > 1500.0 {
                            info.push_str(" [ALT-SPOOF]");
                        }
                    }

                    tracks.push(TargetTrack {
                        id: icao,
                        callsign,
                        domain: Domain::Air,
                        geo: GeoPoint::new(lat, lon, alt_m),
                        speed_kts,
                        heading_deg,
                        vertical_rate_fpm,
                        squawk,
                        is_military,
                        operator_country,
                        operator_branch,
                        airframe_or_vessel_class: airframe_class,
                        origin: Some(theater.to_string()),
                        destination: None,
                        vessel_type: Some(if is_military {
                            format!("ASKERI ({})", theater)
                        } else {
                            format!("SIVIL ({})", theater)
                        }),
                        info,
                        last_seen: Utc::now(),
                    });
                }
            }
        }
    }

    /// Yalnızca gerçek ADS-B paketlerinde NIC < 5 veya NACp < 5 raporlayan uçaklardan GPS karıştırma bölgelerini tespit eder
    pub fn detect_jamming_zones(&self, tracks: &[TargetTrack]) -> Vec<(f64, f64, f64, String)> {
        let mut zones: Vec<(f64, f64, f64, String)> = Vec::new();
        for t in tracks.iter().filter(|t| t.info.contains("[EW:GPS-JAM]")) {
            let already_covered = zones
                .iter()
                .any(|(zlat, zlon, _, _)| (zlat - t.geo.lat).hypot(zlon - t.geo.lon) < 2.0);
            if !already_covered {
                zones.push((
                    t.geo.lat,
                    t.geo.lon,
                    1.2,
                    format!("GPS-JAM (NIC<5: {})", t.callsign),
                ));
            }
        }
        zones
    }

    fn parse_opensky_json(
        json: &serde_json::Value,
        tracks: &mut Vec<TargetTrack>,
        seen_ids: &mut HashSet<String>,
    ) {
        if let Some(states) = json.get("states").and_then(|s| s.as_array()) {
            for state in states {
                if let Some(arr) = state.as_array() {
                    let icao = arr
                        .first()
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_lowercase();
                    if icao.is_empty() || !seen_ids.insert(icao.clone()) {
                        continue;
                    }
                    let callsign = arr
                        .get(1)
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    let country = arr
                        .get(2)
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let lon = arr.get(5).and_then(|v| v.as_f64());
                    let lat = arr.get(6).and_then(|v| v.as_f64());
                    let alt_baro_m = arr.get(7).and_then(|v| v.as_f64());
                    let alt_geom_m = arr.get(13).and_then(|v| v.as_f64());
                    let alt_m = alt_geom_m.or(alt_baro_m);
                    let on_ground = arr.get(8).and_then(|v| v.as_bool()).unwrap_or(false);
                    let vel_ms = arr.get(9).and_then(|v| v.as_f64());
                    let track_deg = arr.get(10).and_then(|v| v.as_f64());
                    let vert_ms = arr.get(11).and_then(|v| v.as_f64());
                    let squawk = arr.get(14).and_then(|v| v.as_str()).map(|s| s.to_string());

                    if let (Some(lat), Some(lon)) = (lat, lon) {
                        if !on_ground && lat != 0.0 && lon != 0.0 && lat.abs() <= 89.0 && lon.abs() <= 180.0 {
                            let speed_kts = vel_ms.map(|v| v * 1.94384);
                            let alt_ft = alt_m.map(|m| (m * 3.28084).round());
                            let vertical_rate_fpm = vert_ms.map(|v| v * 196.85);
                            let squawk_str = squawk.clone().unwrap_or_else(|| "----".to_string());
                            let name = if callsign.is_empty() {
                                icao.clone()
                            } else {
                                callsign.clone()
                            };

                            let fallback_country = if country.is_empty() {
                                None
                            } else {
                                Some(country.clone())
                            };
                            let (is_military, operator_country, operator_branch, airframe_class) =
                                enrich_aircraft_heuristics(
                                    &name,
                                    &icao,
                                    false,
                                    "",
                                    "",
                                    "",
                                    fallback_country,
                                );
                            let theater = classify_air_theater(lat, lon);
                            let mut info = format!(
                                "[{}] FL{:.0} | SQWK: {}",
                                theater,
                                alt_ft.unwrap_or(0.0) / 100.0,
                                squawk_str
                            );
                            if is_military
                                && alt_ft.is_some_and(|a| a < 3000.0)
                                && speed_kts.is_some_and(|s| s > 350.0)
                            {
                                info.push_str(" [LOW-FAST]");
                            }
                            if let (Some(geom_m), Some(baro_m)) = (alt_geom_m, alt_baro_m) {
                                if ((geom_m - baro_m) * 3.28084).abs() > 1500.0 {
                                    info.push_str(" [ALT-SPOOF]");
                                }
                            }

                            tracks.push(TargetTrack {
                                id: icao,
                                callsign: name,
                                domain: Domain::Air,
                                geo: GeoPoint::new(lat, lon, alt_m),
                                speed_kts,
                                heading_deg: track_deg,
                                vertical_rate_fpm,
                                squawk,
                                is_military,
                                operator_country,
                                operator_branch: operator_branch.or_else(|| {
                                    if country.is_empty() {
                                        None
                                    } else {
                                        Some(country.clone())
                                    }
                                }),
                                airframe_or_vessel_class: airframe_class,
                                origin: Some(theater.to_string()),
                                destination: None,
                                vessel_type: Some(if is_military {
                                    format!("ASKERI ({})", theater)
                                } else {
                                    format!("SIVIL ({})", theater)
                                }),
                                info,
                                last_seen: Utc::now(),
                            });
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_low_fast_and_alt_spoof_tags() {
        let json = serde_json::json!({
            "ac": [
                {
                    "hex": "ae1234",
                    "flight": "RCH999 ",
                    "lat": 36.0,
                    "lon": 36.0,
                    "alt_baro": 1800.0,
                    "alt_geom": 4200.0,
                    "gs": 420.0,
                    "dbFlags": 1
                },
                {
                    "hex": "4b8001",
                    "flight": "THY101",
                    "lat": 41.0,
                    "lon": 29.0,
                    "alt_baro": 2000.0,
                    "alt_geom": 2100.0,
                    "gs": 390.0,
                    "dbFlags": 0
                }
            ]
        });
        let mut tracks = Vec::new();
        let mut seen = HashSet::new();
        OpenSkyClient::parse_adsbfi_json(&json, &mut tracks, &mut seen);
        assert_eq!(tracks.len(), 2);
        assert!(tracks[0].info.contains(" [LOW-FAST]"));
        assert!(tracks[0].info.contains(" [ALT-SPOOF]"));
        assert!(!tracks[1].info.contains("[LOW-FAST]"));
        assert!(!tracks[1].info.contains("[ALT-SPOOF]"));
    }
}
