use argus_core::types::SubseaCable;

/// TeleGeography Submarine Cable Map GeoJSON verisini parse eder
pub fn parse_subsea_cables_geojson(raw: &str) -> Vec<SubseaCable> {
    let Ok(val) = serde_json::from_str::<serde_json::Value>(raw) else {
        return Vec::new();
    };
    let Some(features) = val["features"].as_array() else {
        return Vec::new();
    };

    let mut cables = Vec::with_capacity(features.len());
    for feat in features {
        let props = &feat["properties"];
        let id = props["id"].as_str().unwrap_or("").to_string();
        let name = props["name"].as_str().unwrap_or("Denizalti Kablo").to_string();
        let color_hex = props["color"].as_str().unwrap_or("#6aaa80").to_string();
        let geom = &feat["geometry"];
        let gtype = geom["type"].as_str().unwrap_or("");
        let mut lines: Vec<Vec<(f64, f64)>> = Vec::new();

        let extract_ring = |arr: &[serde_json::Value]| -> Vec<(f64, f64)> {
            let step = if arr.len() > 24 { arr.len() / 20 } else { 1 }.max(1);
            let mut pts = Vec::with_capacity(arr.len() / step + 2);
            for (i, pt) in arr.iter().enumerate() {
                if i == 0 || i + 1 == arr.len() || i % step == 0 {
                    if let (Some(lon), Some(lat)) = (pt[0].as_f64(), pt[1].as_f64()) {
                        pts.push((lon, lat));
                    }
                }
            }
            pts
        };

        if gtype == "LineString" {
            if let Some(coords) = geom["coordinates"].as_array() {
                let pts = extract_ring(coords);
                if pts.len() >= 2 {
                    lines.push(pts);
                }
            }
        } else if gtype == "MultiLineString" {
            if let Some(parts) = geom["coordinates"].as_array() {
                for part in parts {
                    if let Some(coords) = part.as_array() {
                        let pts = extract_ring(coords);
                        if pts.len() >= 2 {
                            lines.push(pts);
                        }
                    }
                }
            }
        }

        if !lines.is_empty() {
            cables.push(SubseaCable {
                id,
                name,
                color_hex,
                lines,
            });
        }
    }
    cables
}

pub async fn fetch_subsea_cables(client: &reqwest::Client) -> Vec<SubseaCable> {
    let url = "https://www.submarinecablemap.com/api/v3/cable/cable-geo.json";
    let Ok(resp) = client.get(url).send().await else {
        return Vec::new();
    };
    if !resp.status().is_success() {
        return Vec::new();
    }
    let Ok(text) = resp.text().await else {
        return Vec::new();
    };
    parse_subsea_cables_geojson(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_subsea_cables_geojson() {
        let sample = r##"{
          "type":"FeatureCollection",
          "features":[{
            "type":"Feature",
            "properties":{"id":"med-1","name":"Med Nautilus","color":"#38bdf8"},
            "geometry":{"type":"MultiLineString","coordinates":[[[28.9,41.0],[24.0,37.9],[15.0,37.5]]]}
          }]
        }"##;
        let cables = parse_subsea_cables_geojson(sample);
        assert_eq!(cables.len(), 1);
        assert_eq!(cables[0].name, "Med Nautilus");
        assert_eq!(cables[0].lines[0].len(), 3);
    }
}
