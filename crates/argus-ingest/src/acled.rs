use argus_core::types::AcledEvent;
use serde::Deserialize;
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
}

impl AcledClient {
    pub fn new(email: Option<String>, password: Option<String>) -> Self {
        Self {
            email,
            password,
            token_url: "https://acleddata.com/oauth/token".to_string(),
            api_url: "https://acleddata.com/api/acled/read".to_string(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(12))
                .build()
                .unwrap_or_default(),
            cached_token: parking_lot::RwLock::new(None),
        }
    }

    async fn get_valid_token(&self) -> Option<String> {
        let (email, pass) = match (&self.email, &self.password) {
            (Some(e), Some(p)) if !e.is_empty() && !p.is_empty() => (e, p),
            _ => return None,
        };

        // Önbellekte geçerli token var mı?
        {
            let guard = self.cached_token.read();
            if let Some((token, fetched_at, ttl)) = guard.as_ref() {
                if fetched_at.elapsed() < *ttl - Duration::from_secs(60) {
                    return Some(token.clone());
                }
            }
        }

        // Yeni OAuth token talep et
        let params = [
            ("username", email.as_str()),
            ("password", pass.as_str()),
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

    pub async fn fetch_recent_conflicts(&self) -> anyhow::Result<Vec<AcledEvent>> {
        let token = match self.get_valid_token().await {
            Some(t) => t,
            None => return Ok(Vec::new()),
        };

        let url = format!("{}?limit=30", self.api_url);
        let resp = self.client.get(&url).bearer_auth(&token).send().await?;

        if !resp.status().is_success() {
            return Ok(Vec::new());
        }

        let json: serde_json::Value = resp.json().await?;
        let mut events = Vec::new();

        if let Some(items) = json.get("data").and_then(|d| d.as_array()) {
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
        }

        Ok(events)
    }
}
