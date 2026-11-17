use argus_core::types::AircraftPhotoEntry;
use reqwest::Client;
use serde::Deserialize;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

const APP_USER_AGENT: &str =
    "Project-Sentinel/0.1 (+https://github.com/NothingManTR/project-sentinel)";
const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;

static PHOTO_HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

fn http_client() -> &'static Client {
    PHOTO_HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(5))
            .user_agent(APP_USER_AGENT)
            .build()
            .unwrap_or_default()
    })
}

#[derive(Debug, Deserialize)]
struct PlanespottersResp {
    #[serde(default)]
    photos: Vec<PlanespottersPhoto>,
}

#[derive(Debug, Deserialize)]
struct PlanespottersPhoto {
    thumbnail_large: Option<PlanespottersImg>,
    thumbnail: Option<PlanespottersImg>,
    photographer: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PlanespottersImg {
    src: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct HexDbResp {
    registration: Option<String>,
    #[serde(rename = "Type")]
    aircraft_type: Option<String>,
    registered_owners: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AirportDataResp {
    #[serde(default)]
    data: Vec<AirportDataItem>,
}

#[derive(Debug, Deserialize)]
struct AirportDataItem {
    image: Option<String>,
    photographer: Option<String>,
}

struct FoundPhotoMeta {
    image_url: String,
    photographer: Option<String>,
    source: &'static str,
}

pub fn parse_planespotters_json(raw: &str) -> Option<(String, Option<String>)> {
    let parsed: PlanespottersResp = serde_json::from_str(raw).ok()?;
    let first = parsed.photos.into_iter().next()?;
    let url = first
        .thumbnail_large
        .or(first.thumbnail)
        .map(|i| i.src)
        .filter(|u| u.starts_with("http"))?;
    Some((url, first.photographer))
}

pub fn parse_hexdb_json(raw: &str) -> (Option<String>, Option<String>, Option<String>) {
    let Ok(parsed) = serde_json::from_str::<HexDbResp>(raw) else {
        return (None, None, None);
    };
    let clean = |v: Option<String>| v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    (
        clean(parsed.registration),
        clean(parsed.aircraft_type),
        clean(parsed.registered_owners),
    )
}

async fn query_planespotters(client: &Client, url: &str) -> Option<FoundPhotoMeta> {
    let resp = client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body = resp.text().await.ok()?;
    let (image_url, photographer) = parse_planespotters_json(&body)?;
    Some(FoundPhotoMeta {
        image_url,
        photographer,
        source: "Planespotters.net",
    })
}

async fn query_airport_data(client: &Client, hex: &str) -> Option<FoundPhotoMeta> {
    let url = format!("https://airport-data.com/api/ac_thumb.json?m={}", hex);
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let parsed = resp.json::<AirportDataResp>().await.ok()?;
    let first = parsed.data.into_iter().next()?;
    let image_url = first.image.filter(|u| u.starts_with("http"))?;
    Some(FoundPhotoMeta {
        image_url,
        photographer: first.photographer,
        source: "Airport-Data.com",
    })
}

async fn query_hexdb(client: &Client, hex: &str) -> (Option<String>, Option<String>, Option<String>) {
    let url = format!("https://hexdb.io/api/v1/aircraft/{}", hex);
    let Ok(resp) = client.get(&url).send().await else {
        return (None, None, None);
    };
    if !resp.status().is_success() {
        return (None, None, None);
    }
    let Ok(body) = resp.text().await else {
        return (None, None, None);
    };
    parse_hexdb_json(&body)
}

async fn download_image_bytes(client: &Client, url: &str) -> Option<Vec<u8>> {
    let resp = client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    if let Some(len) = resp.content_length() {
        if len as usize > MAX_IMAGE_BYTES {
            return None;
        }
    }
    let bytes = resp.bytes().await.ok()?;
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return None;
    }
    Some(bytes.to_vec())
}

/// ICAO 24-bit hex kodu üzerinden gerçek uçak fotoğrafını ve tescil bilgisini çeker
pub async fn fetch_aircraft_photo(icao24: &str) -> AircraftPhotoEntry {
    let hex_clean = icao24.trim().to_lowercase();
    let client = http_client();

    if hex_clean.len() < 4 || !hex_clean.chars().all(|c| c.is_ascii_hexdigit()) {
        return AircraftPhotoEntry {
            icao: hex_clean,
            registration: None,
            aircraft_type: None,
            registered_owner: None,
            photographer: None,
            source: "Yok".to_string(),
            image_bytes: None,
            note: "Gecersiz ICAO24 kodu".to_string(),
        };
    }

    let ps_hex_url = format!("https://api.planespotters.net/pub/photos/hex/{}", hex_clean);
    let (ps_hex, (reg, ac_type, owner)) =
        tokio::join!(query_planespotters(client, &ps_hex_url), query_hexdb(client, &hex_clean));

    let mut photo_meta = ps_hex;
    if photo_meta.is_none() {
        if let Some(r) = reg.as_deref() {
            let ps_reg_url = format!("https://api.planespotters.net/pub/photos/reg/{}", r);
            photo_meta = query_planespotters(client, &ps_reg_url).await;
        }
    }
    if photo_meta.is_none() {
        photo_meta = query_airport_data(client, &hex_clean).await;
    }

    if let Some(meta) = photo_meta {
        if let Some(bytes) = download_image_bytes(client, &meta.image_url).await {
            return AircraftPhotoEntry {
                icao: hex_clean,
                registration: reg,
                aircraft_type: ac_type,
                registered_owner: owner,
                photographer: meta.photographer,
                source: meta.source.to_string(),
                image_bytes: Some(Arc::new(bytes)),
                note: "OK".to_string(),
            };
        }
    }

    AircraftPhotoEntry {
        icao: hex_clean,
        registration: reg,
        aircraft_type: ac_type,
        registered_owner: owner,
        photographer: None,
        source: "Planespotters / Airport-Data".to_string(),
        image_bytes: None,
        note: "Kamuya acik spotter fotografi bulunamadi".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_planespotters_and_hexdb() {
        let ps = r#"{"photos":[{"id":"1943688","thumbnail":{"src":"https://t.plnspttrs.net/a_t.jpg"},"thumbnail_large":{"src":"https://t.plnspttrs.net/a_280.jpg"},"photographer":"Mehmed Bekir"}]}"#;
        let (url, photog) = parse_planespotters_json(ps).expect("should parse planespotters");
        assert_eq!(url, "https://t.plnspttrs.net/a_280.jpg");
        assert_eq!(photog.as_deref(), Some("Mehmed Bekir"));

        let hx = r#"{"ModeS":"4BAA81","Registration":"TC-JTA","Type":"A321 231SL","RegisteredOwners":"Turkish Airlines"}"#;
        let (reg, typ, own) = parse_hexdb_json(hx);
        assert_eq!(reg.as_deref(), Some("TC-JTA"));
        assert_eq!(typ.as_deref(), Some("A321 231SL"));
        assert_eq!(own.as_deref(), Some("Turkish Airlines"));
    }
}
