use argus_core::state::AppState;
use argus_core::types::{Domain, GeoPoint, TargetTrack};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use parking_lot::RwLock;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, Clone, Default)]
pub struct VesselStaticMeta {
    pub name: Option<String>,
    pub call_sign: Option<String>,
    pub destination: Option<String>,
    pub ship_type: Option<u64>,
}

#[derive(Debug, Clone, Default)]
struct LastVesselPos {
    lat: f64,
    lon: f64,
    sog: Option<f64>,
    cog: Option<f64>,
    anchored: bool,
}

fn clean_ais_str(val: &serde_json::Value) -> Option<String> {
    val.as_str()
        .map(|s| {
            s.trim_matches(|c: char| c.is_whitespace() || c == '@')
                .to_string()
        })
        .filter(|s| !s.is_empty())
}

fn country_from_mmsi(id: &str) -> &'static str {
    let mid = if id.len() >= 3 { &id[0..3] } else { "" };
    match mid {
        "271" => "Türkiye",
        "232" | "233" | "234" | "235" => "Birleşik Krallık",
        "338" | "366" | "367" | "368" | "369" => "ABD",
        "211" => "Almanya",
        "226" | "227" | "228" => "Fransa",
        "247" => "İtalya",
        "237" | "239" | "240" | "241" => "Yunanistan",
        "273" => "Rusya Federasyonu",
        "272" => "Ukrayna",
        "428" => "İsrail",
        "636" => "Liberya",
        "351" | "352" | "353" | "354" | "355" | "356" | "357" => "Panama",
        "538" => "Marshall Adaları",
        "248" | "249" | "256" => "Malta",
        _ => "Uluslararası Bayrak",
    }
}

pub fn parse_ais_message(
    v: &serde_json::Value,
    static_cache: &mut HashMap<String, VesselStaticMeta>,
) -> Option<TargetTrack> {
    parse_ais_value(v, static_cache, None)
}

fn parse_ais_value(
    v: &serde_json::Value,
    static_cache: &mut HashMap<String, VesselStaticMeta>,
    mut pos_cache: Option<&mut HashMap<String, LastVesselPos>>,
) -> Option<TargetTrack> {
    let meta = &v["MetaData"];
    let pos_rep = &v["Message"]["PositionReport"];
    let static_data = &v["Message"]["ShipStaticData"];

    let id = meta["MMSI"]
        .as_u64()
        .map(|m| m.to_string())
        .or_else(|| {
            meta["MMSI_String"]
                .as_str()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .or_else(|| pos_rep["UserID"].as_u64().map(|m| m.to_string()))
        .or_else(|| static_data["UserID"].as_u64().map(|m| m.to_string()))?;

    if static_data.is_object() {
        if static_cache.len() > 25_000 {
            static_cache.clear();
        }
        let entry = static_cache.entry(id.clone()).or_default();
        if let Some(n) =
            clean_ais_str(&static_data["Name"]).filter(|s| !s.eq_ignore_ascii_case("VESSEL"))
        {
            entry.name = Some(n);
        }
        if let Some(cs) = clean_ais_str(&static_data["CallSign"]) {
            entry.call_sign = Some(cs);
        }
        if let Some(dest) = clean_ais_str(&static_data["Destination"]) {
            entry.destination = Some(dest);
        }
        if let Some(tp) = static_data["Type"].as_u64() {
            entry.ship_type = Some(tp);
        }
    }

    let cached_static = static_cache.get(&id).cloned().unwrap_or_default();

    let has_pos_report = pos_rep.is_object();
    let raw_lat = meta["latitude"]
        .as_f64()
        .or_else(|| pos_rep["Latitude"].as_f64());
    let raw_lon = meta["longitude"]
        .as_f64()
        .or_else(|| pos_rep["Longitude"].as_f64());
    let valid_coords = match (raw_lat, raw_lon) {
        (Some(la), Some(lo))
            if la != 0.0 && lo != 0.0 && la.abs() <= 90.0 && lo.abs() <= 180.0 =>
        {
            Some((la, lo))
        }
        _ => None,
    };

    let (lat, lon, sog, cog, is_anchored) = if has_pos_report {
        let (la, lo) = valid_coords?;
        let sog = pos_rep["Sog"].as_f64();
        let cog = pos_rep["Cog"].as_f64();
        let nav_status = pos_rep["NavigationalStatus"].as_u64();
        let anchored = matches!(nav_status, Some(1) | Some(5));
        if let Some(pc) = pos_cache.as_deref_mut() {
            if pc.len() > 25_000 {
                pc.clear();
            }
            pc.insert(
                id.clone(),
                LastVesselPos {
                    lat: la,
                    lon: lo,
                    sog,
                    cog,
                    anchored,
                },
            );
        }
        (la, lo, sog, cog, anchored)
    } else if let Some((la, lo)) = valid_coords {
        let prev = pos_cache.as_deref().and_then(|pc| pc.get(&id));
        (
            la,
            lo,
            prev.and_then(|p| p.sog),
            prev.and_then(|p| p.cog),
            prev.is_some_and(|p| p.anchored),
        )
    } else {
        let prev = pos_cache.as_deref().and_then(|pc| pc.get(&id))?;
        (prev.lat, prev.lon, prev.sog, prev.cog, prev.anchored)
    };

    let meta_name =
        clean_ais_str(&meta["ShipName"]).filter(|s| !s.eq_ignore_ascii_case("VESSEL"));
    let name = cached_static
        .name
        .clone()
        .or(meta_name)
        .or_else(|| cached_static.call_sign.clone())
        .unwrap_or_else(|| format!("MMSI:{}", id));
    let name_upper = name.to_uppercase();
    let country = country_from_mmsi(&id);

    let mut is_military = false;
    let mut operator_branch = None;
    let mut vessel_class = None;
    let mut vtype = "Ticari Gemi".to_string();

    if name_upper.starts_with("TCG ")
        || name_upper.starts_with("TCG-")
        || name_upper.starts_with("TCG")
    {
        is_military = true;
        operator_branch = Some("Türk Deniz Kuvvetleri".to_string());
        vessel_class = Some("DzKK Harp Gemisi (Fırkateyn/Korvet)".to_string());
        vtype = "Savaş Gemisi".to_string();
    } else if name_upper.starts_with("TCSG ")
        || name_upper.starts_with("TCSG-")
        || name_upper.starts_with("TCSG")
    {
        operator_branch = Some("Sahil Güvenlik Komutanlığı".to_string());
        vessel_class = Some("SG Arama Kurtarma / Karakol Botu".to_string());
        vtype = "Sahil Güvenlik".to_string();
    } else if name_upper.starts_with("USS ") {
        is_military = true;
        operator_branch = Some("US Navy 6th Fleet".to_string());
        vessel_class = Some("Muharip Harp Gemisi".to_string());
        vtype = "Savaş Gemisi".to_string();
    } else if name_upper.starts_with("HMS ") {
        is_military = true;
        operator_branch = Some("Royal Navy Task Group".to_string());
        vessel_class = Some("Kraliyet Donanması Harp Gemisi".to_string());
        vtype = "Savaş Gemisi".to_string();
    }

    match cached_static.ship_type {
        Some(35) => {
            is_military = true;
            if operator_branch.is_none() {
                vtype = "Savaş Gemisi".to_string();
                vessel_class = Some("Askeri / Savaş Gemisi (AIS Type 35)".to_string());
            }
        }
        Some(55) if !is_military && operator_branch.is_none() => {
            vtype = "Kablo/Özel Gemisi".to_string();
            vessel_class = Some("Kablo / Özel Görev Gemisi (AIS Type 55)".to_string());
        }
        Some(80..=89) if !is_military && operator_branch.is_none() => {
            vtype = "Tanker (Petrol/Kimyasal)".to_string();
            vessel_class = Some("Sıvı Yakıt / Kimyasal Tanker (AIS Type 80-89)".to_string());
        }
        Some(70..=79) if !is_military && operator_branch.is_none() => {
            vtype = "Kargo / Konteyner".to_string();
            vessel_class = Some("Kargo / Yük Gemisi (AIS Type 70-79)".to_string());
        }
        _ if operator_branch.is_none() => {
            if name_upper.contains("PILOT") || name_upper.contains("KILAVUZ") {
                vtype = "Kılavuz / Liman Hizmet".to_string();
                vessel_class = Some("Kılavuz Kaptan Servis Botu".to_string());
            } else if name_upper.contains("TUG") || name_upper.contains("ROMORKOR") {
                vtype = "Römorkör".to_string();
                vessel_class = Some("Liman / Açık Deniz Çekici".to_string());
            } else if name_upper.contains("TANKER")
                || name_upper.contains("OIL")
                || name_upper.contains("CHEM")
            {
                vtype = "Tanker (Petrol/Kimyasal)".to_string();
                vessel_class = Some("Sıvı Yakıt / Kimyasal Taşıyıcı".to_string());
            } else if name_upper.contains("CONTAINER")
                || name_upper.contains("MSC")
                || name_upper.contains("MAERSK")
                || name_upper.contains("CMA")
            {
                vtype = "Kargo / Konteyner".to_string();
                vessel_class = Some("Konteyner Hat Gemisi".to_string());
            }
        }
        _ => {}
    }

    let mut info = format!(
        "MMSI: {} | Rota: {:.1}° | SOG: {:.1}kt | {}",
        id,
        cog.unwrap_or(0.0),
        sog.unwrap_or(0.0),
        country
    );
    if let Some(ref cs) = cached_static.call_sign {
        info.push_str(&format!(" | CS: {}", cs));
    }
    if let Some(ref dest) = cached_static.destination {
        info.push_str(&format!(" | Dest: {}", dest));
    }
    if is_anchored {
        info.push_str(" [DEMIRLI]");
    }

    Some(TargetTrack {
        id,
        callsign: name,
        domain: Domain::Sea,
        geo: GeoPoint::new(lat, lon, Some(0.0)),
        speed_kts: sog,
        heading_deg: cog,
        vertical_rate_fpm: None,
        squawk: None,
        is_military,
        operator_country: Some(country.to_string()),
        operator_branch,
        airframe_or_vessel_class: vessel_class,
        origin: None,
        destination: cached_static.destination,
        vessel_type: Some(vtype),
        info,
        last_seen: Utc::now(),
    })
}

pub struct AisStreamClient {
    api_key: Option<String>,
}

impl AisStreamClient {
    pub fn new(api_key: Option<String>) -> Self {
        Self { api_key }
    }

    pub async fn run_listener(&self, state: Arc<RwLock<AppState>>) {
        let key = match &self.api_key {
            Some(k) if !k.trim().is_empty() => k.clone(),
            _ => return,
        };

        let mut static_cache: HashMap<String, VesselStaticMeta> = HashMap::new();
        let mut pos_cache: HashMap<String, LastVesselPos> = HashMap::new();

        let url = "wss://stream.aisstream.io/v0/stream";
        loop {
            match connect_async(url).await {
                Ok((mut ws_stream, _)) => {
                    // AISStream Küresel BoundingBoxes: Tüm Dünya Okyanusları ve Stratejik Boğazlar
                    // (İstanbul/Çanakkale, Süveyş/Bab el-Mandep, Hürmüz, Malakka, Tayvan, Panama, Avrupa)
                    let sub_msg = json!({
                        "APIKey": key,
                        "BoundingBoxes": [
                            [[90.0, -180.0], [-90.0, 180.0]]
                        ],
                        "FilterMessageTypes": ["PositionReport", "ShipStaticData"]
                    });

                    if ws_stream
                        .send(Message::Text(sub_msg.to_string()))
                        .await
                        .is_err()
                    {
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                        continue;
                    }

                    while let Some(msg_result) = ws_stream.next().await {
                        let txt_opt = match msg_result {
                            Ok(Message::Text(txt)) => Some(txt),
                            Ok(Message::Binary(bin)) => String::from_utf8(bin).ok(),
                            _ => None,
                        };

                        if let Some(txt) = txt_opt {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
                                if let Some(track) =
                                    parse_ais_value(&v, &mut static_cache, Some(&mut pos_cache))
                                {
                                    let mut st = state.write();
                                    st.upsert_target(track);
                                    st.total_events_received += 1;
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ship_static_data_and_anchored_status() {
        let mut cache = HashMap::new();
        let static_msg = json!({
            "MessageType": "ShipStaticData",
            "MetaData": {
                "MMSI": 271000444,
                "ShipName": "VESSEL",
                "latitude": 40.95,
                "longitude": 28.88
            },
            "Message": {
                "ShipStaticData": {
                    "UserID": 271000444,
                    "Name": "TURK KORVET   ",
                    "CallSign": "TB444 ",
                    "Destination": "AKSAZ@@",
                    "Type": 35
                }
            }
        });

        let t1 = parse_ais_message(&static_msg, &mut cache).expect("static msg should parse");
        assert!(t1.is_military);
        assert_eq!(t1.callsign, "TURK KORVET");
        assert_eq!(t1.destination.as_deref(), Some("AKSAZ"));
        assert_eq!(t1.vessel_type.as_deref(), Some("Savaş Gemisi"));
        assert!(t1.info.contains("CS: TB444"));

        let pos_msg = json!({
            "MessageType": "PositionReport",
            "MetaData": {
                "MMSI": 271000444,
                "latitude": 40.96,
                "longitude": 28.89
            },
            "Message": {
                "PositionReport": {
                    "UserID": 271000444,
                    "Latitude": 40.96,
                    "Longitude": 28.89,
                    "Sog": 0.2,
                    "Cog": 120.0,
                    "NavigationalStatus": 1
                }
            }
        });

        let t2 = parse_ais_message(&pos_msg, &mut cache).expect("pos msg should parse");
        assert!(t2.is_military);
        assert_eq!(t2.callsign, "TURK KORVET");
        assert_eq!(t2.destination.as_deref(), Some("AKSAZ"));
        assert!(t2.info.contains(" [DEMIRLI]"));

        let cable_msg = json!({
            "MetaData": { "MMSI": 248001122, "latitude": 35.1, "longitude": 25.2 },
            "Message": { "ShipStaticData": { "Name": "CS RELIANCE", "Type": 55 } }
        });
        let t_cable = parse_ais_message(&cable_msg, &mut cache).unwrap();
        assert!(!t_cable.is_military);
        assert!(t_cable.vessel_type.as_deref().unwrap().contains("Kablo"));

        let tanker_msg = json!({
            "MetaData": { "MMSI": 636001122, "latitude": 26.5, "longitude": 56.2 },
            "Message": { "ShipStaticData": { "Name": "NORDIC BREEZE", "Type": 80 } }
        });
        let t_tanker = parse_ais_message(&tanker_msg, &mut cache).unwrap();
        assert!(t_tanker.vessel_type.as_deref().unwrap().contains("Tanker"));

        let cargo_msg = json!({
            "MetaData": { "MMSI": 351001122, "latitude": 30.5, "longitude": 32.3 },
            "Message": { "ShipStaticData": { "Name": "BLUE STAR", "Type": 71 } }
        });
        let t_cargo = parse_ais_message(&cargo_msg, &mut cache).unwrap();
        assert!(t_cargo.vessel_type.as_deref().unwrap().contains("Kargo"));
    }
}
