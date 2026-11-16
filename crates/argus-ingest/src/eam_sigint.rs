use argus_core::state::AppState;
use argus_core::types::EamMessage;
use chrono::{NaiveDateTime, Utc};
use parking_lot::RwLock;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Deserialize)]
struct EamWatchResponse {
    data: Option<Vec<EamWatchItem>>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct EamWatchItem {
    #[serde(default)]
    id: String,
    #[serde(rename = "type", default)]
    msg_type: String,
    #[serde(default)]
    sender: String,
    #[serde(default)]
    time: String,
    #[serde(default)]
    message: String,
}

/// eam.watch üzerinden doğrulanmış gerçek HFGCS EAM mesajlarını çeker
async fn fetch_live_eams(client: &reqwest::Client, current_snr_db: f32) -> Vec<EamMessage> {
    let url = "https://eam.watch/api/messages";
    let resp = match client.get(url).send().await {
        Ok(r) if r.status().is_success() => r,
        _ => return Vec::new(),
    };

    let body = match resp.json::<EamWatchResponse>().await {
        Ok(b) => b,
        _ => return Vec::new(),
    };

    let mut messages = Vec::new();

    if let Some(items) = body.data {
        for item in items {
            let content = item.message.trim().to_string();
            if content.is_empty() {
                continue;
            }

            let timestamp = NaiveDateTime::parse_from_str(&item.time, "%Y-%m-%d %H:%M:%S")
                .map(|ndt| ndt.and_utc())
                .unwrap_or_else(|_| Utc::now());

            let is_flash = item.msg_type.to_uppercase().contains("SKYKING")
                || content.to_uppercase().contains("SKYKING")
                || content.to_uppercase().contains("DO NOT ANSWER");

            let station = if item.sender.is_empty() {
                "HFGCS GLOBAL".to_string()
            } else {
                item.sender.to_uppercase()
            };

            let preamble = if is_flash {
                "SKYKING".to_string()
            } else if !item.msg_type.is_empty() {
                item.msg_type.to_uppercase()
            } else {
                station.clone()
            };

            messages.push(EamMessage {
                timestamp,
                frequency_khz: 11175.0,
                mode: "USB".to_string(),
                preamble,
                length_chars: content.len(),
                content,
                auth_station: station,
                raw_signal_snr_db: current_snr_db,
                is_flash_override: is_flash,
            });
        }
    }

    messages
}

/// Gerçek HFGCS EAM API dinleyicisi (Sahte şelale üretimi kaldırıldı; şelale kiwisdr.rs gerçek DFT'den beslenir)
pub async fn start_sigint_worker(state: Arc<RwLock<AppState>>) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(6))
        .user_agent("SentinelSIGINT/2.0")
        .build()
        .unwrap_or_default();

    loop {
        let current_snr = {
            let st = state.read();
            (st.sdr_rssi_dbm + 120.0).max(0.0)
        };
        let live_eams = fetch_live_eams(&client, current_snr).await;
        if !live_eams.is_empty() {
            let mut st = state.write();
            for msg in live_eams {
                if !st
                    .eam_messages
                    .iter()
                    .any(|m| m.content == msg.content && m.timestamp == msg.timestamp)
                {
                    st.eam_messages.push(msg);
                    st.total_events_received += 1;
                }
            }
        }
        sleep(Duration::from_secs(30)).await;
    }
}
