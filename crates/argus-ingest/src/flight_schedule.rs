use argus_core::types::FlightScheduleInfo;
use std::sync::OnceLock;
use std::time::Duration;

static SCHEDULE_HTTP: OnceLock<reqwest::Client> = OnceLock::new();

fn client() -> &'static reqwest::Client {
    SCHEDULE_HTTP.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(6))
            .user_agent("Project-Sentinel/0.1 (+https://github.com/NothingManTR/project-sentinel)")
            .build()
            .unwrap_or_default()
    })
}

async fn lookup_airport_hexdb(code: &str) -> Option<(String, f64, f64)> {
    let clean = code.trim().to_uppercase();
    if clean.len() < 3 || clean.len() > 4 || !clean.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let kind = if clean.len() == 4 { "icao" } else { "iata" };
    let url = format!("https://hexdb.io/api/v1/airport/{}/{}", kind, clean);
    let resp = client().get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let v: serde_json::Value = resp.json().await.ok()?;
    let lat = v["latitude"].as_f64()?;
    let lon = v["longitude"].as_f64()?;
    let name = v["airport"]
        .as_str()
        .or_else(|| v["region_name"].as_str())
        .unwrap_or(&clean)
        .to_string();
    Some((name, lat, lon))
}

async fn lookup_route_hexdb(callsign: &str) -> Option<(String, String)> {
    let cs = callsign.trim().to_uppercase();
    if cs.len() < 3 || !cs.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let url = format!("https://hexdb.io/api/v1/route/icao/{}", cs);
    let resp = client().get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let v: serde_json::Value = resp.json().await.ok()?;
    let route = v["route"].as_str()?;
    let mut parts = route.split('-');
    let orig = parts.next()?.trim().to_string();
    let dest = parts.next()?.trim().to_string();
    if orig.is_empty() || dest.is_empty() {
        return None;
    }
    Some((orig, dest))
}

pub fn parse_flightaware_json(raw: &str, callsign: &str) -> Option<FlightScheduleInfo> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let first = v["flights"].as_array()?.first()?;
    let orig = &first["origin"];
    let dest = &first["destination"];
    Some(FlightScheduleInfo {
        ident: callsign.to_uppercase(),
        origin_code: orig["code_iata"]
            .as_str()
            .or_else(|| orig["code_icao"].as_str())
            .map(|s| s.to_string()),
        origin_name: orig["name"].as_str().map(|s| s.to_string()),
        dest_code: dest["code_iata"]
            .as_str()
            .or_else(|| dest["code_icao"].as_str())
            .map(|s| s.to_string()),
        dest_name: dest["name"].as_str().map(|s| s.to_string()),
        origin_lat: None,
        origin_lon: None,
        dest_lat: None,
        dest_lon: None,
        status: first["status"].as_str().map(|s| s.to_string()),
        scheduled_off: first["scheduled_off"].as_str().map(|s| s.to_string()),
        estimated_on: first["estimated_on"].as_str().map(|s| s.to_string()),
        source: "FlightAware AeroAPI".to_string(),
    })
}

/// A-2: Opsiyonel FlightAware AeroAPI + ücretsiz hexdb.io rota/meydan çözümleyici
pub async fn fetch_flight_schedule(
    callsign: &str,
    known_route: Option<&str>,
    fa_key: Option<&str>,
) -> Option<FlightScheduleInfo> {
    let cs = callsign.trim().to_uppercase();
    if cs.len() < 3 {
        return None;
    }

    let mut info = if let Some(key) = fa_key.filter(|k| !k.trim().is_empty()) {
        let url = format!("https://aeroapi.flightaware.com/aeroapi/flights/{}", cs);
        match client().get(&url).header("x-apikey", key.trim()).send().await {
            Ok(resp) if resp.status().is_success() => {
                let txt = resp.text().await.unwrap_or_default();
                parse_flightaware_json(&txt, &cs)
            }
            _ => None,
        }
    } else {
        None
    };

    if info.is_none() {
        let pair = if let Some(kr) = known_route {
            let mut sp = kr.split("->");
            match (sp.next(), sp.next()) {
                (Some(o), Some(d)) if !o.trim().is_empty() && !d.trim().is_empty() => {
                    Some((o.trim().to_string(), d.trim().to_string()))
                }
                _ => None,
            }
        } else {
            None
        };
        let (orig, dest) = match pair {
            Some(p) => p,
            None => lookup_route_hexdb(&cs).await?,
        };
        info = Some(FlightScheduleInfo {
            ident: cs,
            origin_code: Some(orig),
            origin_name: None,
            dest_code: Some(dest),
            dest_name: None,
            origin_lat: None,
            origin_lon: None,
            dest_lat: None,
            dest_lon: None,
            status: None,
            scheduled_off: None,
            estimated_on: None,
            source: "hexdb.io".to_string(),
        });
    }

    let mut out = info?;
    let o_code = out.origin_code.clone().unwrap_or_default();
    let d_code = out.dest_code.clone().unwrap_or_default();
    let (orig_ap, dest_ap) = tokio::join!(lookup_airport_hexdb(&o_code), lookup_airport_hexdb(&d_code));
    if let Some((name, lat, lon)) = orig_ap {
        if out.origin_name.is_none() {
            out.origin_name = Some(name);
        }
        out.origin_lat = Some(lat);
        out.origin_lon = Some(lon);
    }
    if let Some((name, lat, lon)) = dest_ap {
        if out.dest_name.is_none() {
            out.dest_name = Some(name);
        }
        out.dest_lat = Some(lat);
        out.dest_lon = Some(lon);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_flightaware_json() {
        let sample = r#"{
          "flights":[{
            "ident":"THY1",
            "status":"En Route / On Time",
            "scheduled_off":"2026-11-17T11:30:00Z",
            "estimated_on":"2026-11-17T21:45:00Z",
            "origin":{"code_iata":"IST","name":"Istanbul Airport"},
            "destination":{"code_iata":"JFK","name":"John F Kennedy Intl"}
          }]
        }"#;
        let info = parse_flightaware_json(sample, "THY1").unwrap();
        assert_eq!(info.origin_code.as_deref(), Some("IST"));
        assert_eq!(info.dest_code.as_deref(), Some("JFK"));
        assert_eq!(info.status.as_deref(), Some("En Route / On Time"));
    }
}
