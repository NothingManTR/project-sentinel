use argus_core::types::AirspaceWarning;
use serde_json::Value;

/// NOAA/AWC International SIGMET JSON verisinden (`coords` polygon) hava sahası tehlike çokgenlerini ayıklar.
pub fn parse_isigmet_json(raw: &str) -> Vec<AirspaceWarning> {
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(raw) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (idx, it) in items.iter().enumerate() {
        let Some(coords_arr) = it["coords"].as_array() else {
            continue;
        };
        let mut poly: Vec<(f64, f64)> = Vec::new();
        let (mut sum_lat, mut sum_lon) = (0.0_f64, 0.0_f64);
        for c in coords_arr {
            if let (Some(lat), Some(lon)) = (c["lat"].as_f64(), c["lon"].as_f64()) {
                if lat.abs() <= 90.0 && lon.abs() <= 180.0 {
                    poly.push((lon, lat));
                    sum_lat += lat;
                    sum_lon += lon;
                }
            }
        }
        if poly.len() < 3 {
            continue;
        }
        if poly.first() != poly.last() {
            if let Some(first) = poly.first().copied() {
                poly.push(first);
            }
        }
        let n = (poly.len().saturating_sub(1)).max(1) as f64;
        let fir = it["firId"]
            .as_str()
            .or_else(|| it["icaoId"].as_str())
            .unwrap_or("FIR")
            .to_string();
        let fir_name = it["firName"].as_str().unwrap_or(&fir);
        let hazard = it["hazard"].as_str().unwrap_or("SIGMET");
        let qual = it["qualifier"].as_str().unwrap_or("");
        let top_fl = it["top"].as_u64().map(|t| (t / 100) as u32);
        let raw_sigmet: String = it["rawSigmet"]
            .as_str()
            .unwrap_or("")
            .replace('\n', " ")
            .chars()
            .take(120)
            .collect();

        out.push(AirspaceWarning {
            id: format!("SIGMET-{}-{}", fir, idx),
            fir_or_zone: fir_name.to_string(),
            kind: "SIGMET".to_string(),
            hazard: if qual.is_empty() {
                hazard.to_string()
            } else {
                format!("{} ({})", hazard, qual)
            },
            top_fl,
            polygon: poly,
            center_lat: sum_lat / n,
            center_lon: sum_lon / n,
            summary: if raw_sigmet.is_empty() {
                format!("{} {} SIGMET sahasi", fir_name, hazard)
            } else {
                raw_sigmet
            },
        });
    }
    out
}

/// Resmî EASA CZIB / ICAO Çatışma Bölgesi Hava Sahası & Askeri NOTAM Koridorları.
pub fn active_conflict_notam_zones() -> Vec<AirspaceWarning> {
    let specs = [
        (
            "NOTAM-UKBV",
            "UKBV / UKFV (Karadeniz K. / Ukrayna)",
            "CATISMA HAVA SAHASI (CZIB)",
            Some(660),
            vec![(29.0, 44.5), (38.5, 44.5), (39.5, 51.5), (23.5, 51.5), (29.0, 44.5)],
            "Yuksek yogunluklu hava savunma ve fuze/IHA harekat koridoru",
        ),
        (
            "NOTAM-LEVANT",
            "LLLL / OLBB / OSTT (Levant / D. Akdeniz)",
            "ASKERI HAREKAT / GNSS BOZUNMA",
            Some(600),
            vec![(33.5, 31.2), (37.8, 31.2), (38.0, 35.8), (34.0, 35.8), (33.5, 31.2)],
            "Hava savunma, BAL/SAM angajmani ve yogun GPS spoofing uyarisi",
        ),
        (
            "NOTAM-OYSC",
            "OYSC / HAAA (Kizildeniz / Babulmendep)",
            "BALISTIK FUZE & IHA KORIDORU",
            Some(500),
            vec![(40.5, 11.5), (47.5, 11.5), (45.0, 17.5), (39.0, 17.5), (40.5, 11.5)],
            "Deniz yolu & hava koridorunda aktif anti-gemi fuze / USV / IHA tehdidi",
        ),
        (
            "NOTAM-OIIX",
            "OIIX / ORBB (Hurmuz / Basra Korfezi)",
            "STRATEJIK HAVA SAHASI UYARISI",
            Some(600),
            vec![(45.5, 24.5), (57.5, 24.5), (57.5, 33.5), (45.5, 33.5), (45.5, 24.5)],
            "Askeri tatbikat, devriye ve yuksek riskli irtifa tahdit bolgesi",
        ),
        (
            "NOTAM-HSSS",
            "HSSS (Sudan / Hartum FIR)",
            "CATISMA HAVA SAHASI KAPALI",
            Some(660),
            vec![(24.0, 10.0), (36.5, 10.0), (36.5, 21.5), (24.0, 21.5), (24.0, 10.0)],
            "Silahli catisma ve aktif SAM tehdidi sebebiyle ucusa kisitli saha",
        ),
    ];

    specs
        .into_iter()
        .map(|(id, fir, hazard, top_fl, poly, summary)| {
            let n = (poly.len().saturating_sub(1)).max(1) as f64;
            let clat = poly.iter().take(poly.len().saturating_sub(1)).map(|p| p.1).sum::<f64>() / n;
            let clon = poly.iter().take(poly.len().saturating_sub(1)).map(|p| p.0).sum::<f64>() / n;
            AirspaceWarning {
                id: id.to_string(),
                fir_or_zone: fir.to_string(),
                kind: "NOTAM-CONFLICT".to_string(),
                hazard: hazard.to_string(),
                top_fl,
                polygon: poly,
                center_lat: clat,
                center_lon: clon,
                summary: summary.to_string(),
            }
        })
        .collect()
}

/// Canlı ADS-B `NIC < 5` GPS Jamming odaklarını harita üzerinde kapalı sekizgen çokgenlere çevirir.
pub fn jam_zones_to_polygons(zones: &[(f64, f64, f64, String)]) -> Vec<AirspaceWarning> {
    let mut out = Vec::new();
    for (idx, (center_lat, center_lon, radius, lbl)) in zones.iter().enumerate() {
        let r_km = (if *radius <= 5.0 {
            *radius * 111.0 * center_lat.to_radians().cos().abs().max(0.35)
        } else {
            *radius
        })
        .clamp(35.0, 320.0);
        let d_lat = r_km / 111.0;
        let cos_lat = center_lat.to_radians().cos().abs().max(0.2);
        let d_lon = r_km / (111.32 * cos_lat);

        let mut poly = Vec::with_capacity(9);
        for k in 0..=8 {
            let theta = (k as f64) * std::f64::consts::TAU / 8.0;
            let p_lon = (center_lon + d_lon * theta.cos()).clamp(-180.0, 180.0);
            let p_lat = (center_lat + d_lat * theta.sin()).clamp(-85.0, 85.0);
            poly.push((p_lon, p_lat));
        }
        out.push(AirspaceWarning {
            id: format!("JAM-{}", idx),
            fir_or_zone: lbl.clone(),
            kind: "GPS-JAM".to_string(),
            hazard: "GNSS / GPS KARISTIRMA (NIC<5)".to_string(),
            top_fl: Some(450),
            polygon: poly,
            center_lat: *center_lat,
            center_lon: *center_lon,
            summary: format!("{} ({:.2}N, {:.2}E | r={:.0}km)", lbl, center_lat, center_lon, r_km),
        });
    }
    out
}

/// Canlı International SIGMET + Aktif Çatışma NOTAM Sahaları + Canlı ADS-B GPS-JAM Çokgenlerini birleştirir.
pub async fn fetch_airspace_warnings(
    client: &reqwest::Client,
    gps_jam_zones: &[(f64, f64, f64, String)],
) -> Vec<AirspaceWarning> {
    let mut all = active_conflict_notam_zones();
    all.extend(jam_zones_to_polygons(gps_jam_zones));

    if let Ok(resp) = client
        .get("https://aviationweather.gov/api/data/isigmet?format=json")
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(raw) = resp.text().await {
                let mut sigmets = parse_isigmet_json(&raw);
                sigmets.truncate(40);
                all.extend(sigmets);
            }
        }
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_isigmet_json_and_jam_polygons() {
        let sample = r#"[{
            "icaoId": "SEQU",
            "firId": "SEFG",
            "firName": "SEFG GUAYAQUIL",
            "hazard": "VA",
            "qualifier": "ERUPTION",
            "top": 14000,
            "coords": [{"lon": -77.65, "lat": -0.06}, {"lon": -77.67, "lat": -0.37}, {"lon": -77.82, "lat": -0.32}],
            "rawSigmet": "SEFG SIGMET 2 VA"
        }]"#;
        let parsed = parse_isigmet_json(sample);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].kind, "SIGMET");
        assert_eq!(parsed[0].top_fl, Some(140));
        assert_eq!(parsed[0].polygon.len(), 4);
        assert_eq!(parsed[0].polygon.first(), parsed[0].polygon.last());

        let jams = jam_zones_to_polygons(&[(35.0, 34.0, 120.0, "D. Akdeniz Jamming".to_string())]);
        assert_eq!(jams.len(), 1);
        assert_eq!(jams[0].kind, "GPS-JAM");
        assert_eq!(jams[0].polygon.len(), 9);
        assert!(jams[0].summary.contains("r=120km"));

        let deg_jams = jam_zones_to_polygons(&[(35.0, 34.0, 1.2, "NIC Jam".to_string())]);
        assert_eq!(deg_jams.len(), 1);
        assert!(deg_jams[0].summary.contains("r=109km"), "got {}", deg_jams[0].summary);
    }
}
