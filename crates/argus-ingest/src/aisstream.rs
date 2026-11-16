use argus_core::state::AppState;
use argus_core::types::{Domain, GeoPoint, TargetTrack};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use parking_lot::RwLock;
use serde_json::json;
use std::sync::Arc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

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

                    if ws_stream.send(Message::Text(sub_msg.to_string())).await.is_err() {
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
                                let meta = &v["MetaData"];
                                let mmsi = meta["MMSI"]
                                    .as_u64()
                                    .map(|m| m.to_string())
                                    .or_else(|| meta["MMSI_String"].as_str().map(|s| s.to_string()))
                                    .or_else(|| v["Message"]["PositionReport"]["UserID"].as_u64().map(|m| m.to_string()));

                                let ship_name = meta["ShipName"].as_str().unwrap_or("VESSEL").trim().to_string();
                                let lat = meta["latitude"]
                                    .as_f64()
                                    .or_else(|| v["Message"]["PositionReport"]["Latitude"].as_f64());
                                let lon = meta["longitude"]
                                    .as_f64()
                                    .or_else(|| v["Message"]["PositionReport"]["Longitude"].as_f64());

                                let sog = v["Message"]["PositionReport"]["Sog"].as_f64();
                                let cog = v["Message"]["PositionReport"]["Cog"].as_f64();

                                if let (Some(id), Some(lat), Some(lon)) = (mmsi, lat, lon) {
                                    if lat != 0.0 && lon != 0.0 && lat.abs() <= 90.0 && lon.abs() <= 180.0 {
                                        let name = if ship_name.is_empty() { format!("MMSI:{}", id) } else { ship_name.clone() };
                                        let name_upper = name.to_uppercase();

                                        let mid = if id.len() >= 3 { &id[0..3] } else { "" };
                                        let country = match mid {
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
                                        };

                                        let mut is_military = false;
                                        let mut operator_branch = None;
                                        let mut vessel_class = None;
                                        let mut vtype = "Ticari Gemi".to_string();

                                        if name_upper.starts_with("TCG ") || name_upper.starts_with("TCG-") || name_upper.starts_with("TCG") {
                                            is_military = true;
                                            operator_branch = Some("Türk Deniz Kuvvetleri".to_string());
                                            vessel_class = Some("DzKK Harp Gemisi (Fırkateyn/Korvet)".to_string());
                                            vtype = "Savaş Gemisi".to_string();
                                        } else if name_upper.starts_with("TCSG ") || name_upper.starts_with("TCSG-") || name_upper.starts_with("TCSG") {
                                            is_military = false;
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
                                        } else if name_upper.contains("PILOT") || name_upper.contains("KILAVUZ") {
                                            vtype = "Kılavuz / Liman Hizmet".to_string();
                                            vessel_class = Some("Kılavuz Kaptan Servis Botu".to_string());
                                        } else if name_upper.contains("TUG") || name_upper.contains("ROMORKOR") {
                                            vtype = "Römorkör".to_string();
                                            vessel_class = Some("Liman / Açık Deniz Çekici".to_string());
                                        } else if name_upper.contains("TANKER") || name_upper.contains("OIL") || name_upper.contains("CHEM") {
                                            vtype = "Tanker (Petrol/Kimyasal)".to_string();
                                            vessel_class = Some("Sıvı Yakıt / Kimyasal Taşıyıcı".to_string());
                                        } else if name_upper.contains("CONTAINER") || name_upper.contains("MSC") || name_upper.contains("MAERSK") || name_upper.contains("CMA") {
                                            vtype = "Kargo / Konteyner".to_string();
                                            vessel_class = Some("Konteyner Hat Gemisi".to_string());
                                        }

                                        let track = TargetTrack {
                                            id: id.clone(),
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
                                            destination: None,
                                            vessel_type: Some(vtype),
                                            info: format!("MMSI: {} | Rota: {:.1}° | SOG: {:.1}kt | {}", id, cog.unwrap_or(0.0), sog.unwrap_or(0.0), country),
                                            last_seen: Utc::now(),
                                        };

                                        let mut st = state.write();
                                        st.upsert_target(track);
                                        st.total_events_received += 1;
                                    }
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
