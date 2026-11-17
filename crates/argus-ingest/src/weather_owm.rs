use argus_core::types::WeatherPoint;
use futures_util::future::join_all;

pub fn parse_owm_json(raw: &str, fallback_name: &str) -> Option<WeatherPoint> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let temp_c = v["main"]["temp"].as_f64()?;
    let lat = v["coord"]["lat"].as_f64()?;
    let lon = v["coord"]["lon"].as_f64()?;
    let wind_ms = v["wind"]["speed"].as_f64().unwrap_or(0.0);
    let wind_kts = wind_ms * 1.94384;
    let wind_deg = v["wind"]["deg"].as_f64().unwrap_or(0.0);
    let clouds_pct = v["clouds"]["all"].as_u64().unwrap_or(0).min(100) as u8;
    let visibility_km = v["visibility"].as_f64().unwrap_or(10000.0) / 1000.0;
    let name = v["name"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback_name)
        .to_string();
    let condition = v["weather"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(|w| w["description"].as_str().or_else(|| w["main"].as_str()))
        .unwrap_or("Acik")
        .to_string();

    Some(WeatherPoint {
        name,
        lat,
        lon,
        temp_c,
        wind_kts,
        wind_deg,
        clouds_pct,
        visibility_km,
        condition,
    })
}

async fn fetch_point(
    client: &reqwest::Client,
    api_key: &str,
    name: &str,
    lat: f64,
    lon: f64,
) -> Option<WeatherPoint> {
    let url = format!(
        "https://api.openweathermap.org/data/2.5/weather?lat={:.4}&lon={:.4}&appid={}&units=metric&lang=tr",
        lat, lon, api_key
    );
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body = resp.text().await.ok()?;
    parse_owm_json(&body, name)
}

/// A-3: İstasyon ve stratejik harekat merkezleri için canlı OpenWeatherMap meteoroloji gözlemi
pub async fn fetch_weather_snapshot(
    client: &reqwest::Client,
    api_key: &str,
    station_lat: f64,
    station_lon: f64,
) -> (Option<WeatherPoint>, Vec<WeatherPoint>) {
    let key = api_key.trim();
    if key.is_empty() {
        return (None, Vec::new());
    }

    let hubs = [
        ("ISTASYON", station_lat, station_lon),
        ("Ankara", 39.9334, 32.8597),
        ("Atina", 37.9838, 23.7275),
        ("Kahire", 30.0444, 31.2357),
        ("Riyad", 24.7136, 46.6753),
        ("Tahran", 35.6892, 51.3890),
        ("Londra", 51.5072, -0.1276),
        ("Washington", 38.9072, -77.0369),
        ("Tokyo", 35.6762, 139.6503),
    ];

    let results = join_all(
        hubs.iter()
            .map(|(n, lat, lon)| fetch_point(client, key, n, *lat, *lon)),
    )
    .await;

    let mut station = None;
    let mut points = Vec::new();
    for (idx, res) in results.into_iter().enumerate() {
        if let Some(wp) = res {
            if idx == 0 {
                station = Some(wp.clone());
            } else {
                points.push(wp);
            }
        }
    }
    (station, points)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_owm_json() {
        let raw = r#"{
          "coord":{"lon":28.97,"lat":41.01},
          "weather":[{"main":"Clouds","description":"parcali bulutlu"}],
          "main":{"temp":16.4},
          "visibility":10000,
          "wind":{"speed":5.14,"deg":45},
          "clouds":{"all":40},
          "name":"Istanbul"
        }"#;
        let wp = parse_owm_json(raw, "ISTASYON").unwrap();
        assert_eq!(wp.name, "Istanbul");
        assert!((wp.temp_c - 16.4).abs() < 0.01);
        assert!((wp.wind_kts - 10.0).abs() < 0.2);
        assert_eq!(wp.clouds_pct, 40);
        assert_eq!(wp.condition, "parcali bulutlu");
    }
}
