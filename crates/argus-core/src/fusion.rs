use crate::geo::{haversine_km, initial_bearing_deg};
use crate::state::AppState;
use crate::types::{AlertItem, FusionCluster, ThreatSeverity};
use chrono::Utc;
use std::collections::HashMap;

/// K-4: Hava aracının geçmiş izinde dairesel devriye / yakıt ikmal yörüngesi (Orbit/CAP) tespiti
pub fn is_air_orbit_pattern(trail: &[(f64, f64)]) -> bool {
    if trail.len() < 6 {
        return false;
    }
    let mut prev_brg: Option<f64> = None;
    let mut cum_turn = 0.0f64;
    let (mut min_lat, mut max_lat) = (90.0f64, -90.0f64);
    let (mut min_lon, mut max_lon) = (180.0f64, -180.0f64);

    for w in trail.windows(2) {
        let (lon1, lat1) = w[0];
        let (lon2, lat2) = w[1];
        min_lat = min_lat.min(lat1).min(lat2);
        max_lat = max_lat.max(lat1).max(lat2);
        min_lon = min_lon.min(lon1).min(lon2);
        max_lon = max_lon.max(lon1).max(lon2);

        if haversine_km(lat1, lon1, lat2, lon2) < 0.8 {
            continue;
        }
        let brg = initial_bearing_deg(lat1, lon1, lat2, lon2);
        if let Some(pb) = prev_brg {
            let mut delta = (brg - pb + 540.0) % 360.0 - 180.0;
            delta = delta.abs();
            if delta < 135.0 {
                cum_turn += delta;
            }
        }
        prev_brg = Some(brg);
    }

    let diag_km = haversine_km(min_lat, min_lon, max_lat, max_lon);
    cum_turn >= 260.0 && diag_km <= 180.0
}

/// K-4: Hava/Deniz iz anomalileri ve çoklu sensör füzyon kümelerini hesaplar
pub fn recompute_fusion_and_anomalies(state: &mut AppState) {
    let mut anomalies: HashMap<String, String> = HashMap::new();

    for t in state.air_targets.iter() {
        if !t.is_military {
            continue;
        }
        if let Some(trail) = state.target_trails.get(&t.id) {
            if is_air_orbit_pattern(trail) {
                anomalies.insert(t.id.clone(), "ORBIT / REFUEL-CAP".to_string());
            }
        }
    }

    for s in state.sea_targets.iter() {
        let spd = s.speed_kts.unwrap_or(0.0);
        if (0.2..=2.2).contains(&spd) {
            if let Some(trail) = state.target_trails.get(&s.id) {
                if trail.len() >= 4 {
                    anomalies.insert(s.id.clone(), "LOITERING".to_string());
                }
            }
        } else if spd > 65.0 {
            anomalies.insert(s.id.clone(), "AIS ANOMALI (>65kt)".to_string());
        }
    }

    state.track_anomalies = anomalies;

    // Çoklu sensör coğrafi füzyon (160 km yarıçap içinde >= 2 bağımsız disiplin çakışması)
    let mut signals: Vec<(f64, f64, &'static str, String)> = Vec::new();
    for t in state.air_targets.iter() {
        if t.is_military && state.track_anomalies.contains_key(&t.id) {
            signals.push((t.geo.lat, t.geo.lon, "AIR-ORBIT", format!("{} Orbit", t.callsign)));
        }
    }
    for (jlat, jlon, _, lbl) in state.gps_jamming_zones.iter().take(40) {
        signals.push((*jlat, *jlon, "EW-JAM", lbl.clone()));
    }
    for q in state.earthquakes.iter() {
        if q.mag >= 4.5 {
            signals.push((q.lat, q.lon, "SISMIK", format!("M{:.1} {}", q.mag, q.place)));
        }
    }
    for f in state.firms_fires.iter().take(60) {
        if f.brightness >= 335.0 {
            signals.push((f.lat, f.lon, "TERMAL", format!("{:.0}K", f.brightness)));
        }
    }
    for a in state.acled_events.iter().take(40) {
        signals.push((a.lat, a.lon, "CATISMA", a.event_type.clone()));
    }

    let mut visited = vec![false; signals.len()];
    let mut clusters = Vec::new();

    for i in 0..signals.len() {
        if visited[i] {
            continue;
        }
        visited[i] = true;
        let (lat0, lon0, dom0, note0) = &signals[i];
        let mut doms = vec![dom0.to_string()];
        let mut notes = vec![note0.clone()];
        let mut sum_lat = *lat0;
        let mut sum_lon = *lon0;
        let mut count = 1usize;

        for j in (i + 1)..signals.len() {
            if visited[j] {
                continue;
            }
            if haversine_km(*lat0, *lon0, signals[j].0, signals[j].1) <= 160.0 {
                visited[j] = true;
                sum_lat += signals[j].0;
                sum_lon += signals[j].1;
                count += 1;
                let d_str = signals[j].2.to_string();
                if !doms.contains(&d_str) {
                    doms.push(d_str);
                }
                if notes.len() < 3 {
                    notes.push(signals[j].3.clone());
                }
            }
        }

        if doms.len() >= 2 {
            clusters.push(FusionCluster {
                lat: sum_lat / count as f64,
                lon: sum_lon / count as f64,
                score: (doms.len() as u32) * 35 + (count as u32) * 5,
                summary: notes.join(" + "),
                domains: doms,
            });
        }
    }

    clusters.sort_by_key(|a| std::cmp::Reverse(a.score));
    clusters.truncate(16);

    // E-3: Yüksek skorlu K-4 füzyon kümeleri ve askeri anomali yörüngeleri için otomatik taktik alarmlar
    for (tid, tag) in &state.track_anomalies {
        if tag.contains("ORBIT") || tag.contains("REFUEL") || tag.contains("LOW-FAST") {
            let cs = state
                .air_targets
                .iter()
                .find(|t| &t.id == tid)
                .map(|t| t.callsign.clone())
                .unwrap_or_else(|| tid.clone());
            let aid = format!("ANOM:{}", tid);
            let item = AlertItem {
                timestamp: Utc::now(),
                domain: "HAVA-ANOMALI".to_string(),
                severity: ThreatSeverity::Warning,
                message: format!("[ANOMALI] {} ({}): {}", cs, tid, tag),
                target_id: Some(aid.clone()),
            };
            state
                .alerts
                .upsert_by(item, |a| a.target_id.as_deref() == Some(aid.as_str()));
        }
    }

    for fc in &clusters {
        if fc.score >= 70 {
            let sev = if fc.score >= 100 {
                ThreatSeverity::Critical
            } else {
                ThreatSeverity::Warning
            };
            let fid = format!("FUZ:{:.1}:{:.1}", fc.lat, fc.lon);
            let item = AlertItem {
                timestamp: Utc::now(),
                domain: "FUZYON".to_string(),
                severity: sev,
                message: format!(
                    "[FUZYON SKOR:{}] ({}) {}",
                    fc.score,
                    fc.domains.join("+"),
                    fc.summary
                ),
                target_id: Some(fid.clone()),
            };
            state
                .alerts
                .upsert_by(item, |a| a.target_id.as_deref() == Some(fid.as_str()));
        }
    }

    state.fusion_clusters = clusters;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_air_orbit_detection() {
        // Gerçekçi dairesel tanker/AWACS yörüngesi (10 nokta, ~30 km yarıçap)
        let center_lat = 35.0;
        let center_lon = 33.0;
        let mut circle = Vec::new();
        for k in 0..10 {
            let ang = (k as f64) * std::f64::consts::TAU / 9.0;
            circle.push((center_lon + 0.32 * ang.cos(), center_lat + 0.28 * ang.sin()));
        }
        assert!(is_air_orbit_pattern(&circle));

        // Düz transit uçuş: yörünge sayılmamalı
        let straight = vec![
            (28.0, 41.0),
            (28.5, 41.1),
            (29.0, 41.2),
            (29.5, 41.3),
            (30.0, 41.4),
            (30.5, 41.5),
        ];
        assert!(!is_air_orbit_pattern(&straight));
    }

    #[test]
    fn test_auto_alerts_on_conflict_and_fusion_cluster() {
        let mut st = AppState::new(crate::config::AppConfig::default());
        st.station_lat = 41.0;
        st.station_lon = 29.0;

        // Istasyon yakininda (<800 km) GDELT catisma olayi eklendiginde otomatik alarm uretilmeli
        st.upsert_acled_event(crate::types::AcledEvent {
            event_id: "GDELT-1".to_string(),
            event_date: "2026-11-18".to_string(),
            event_type: "Battles".to_string(),
            sub_event_type: "CAMEO 190".to_string(),
            actor1: "ALPHA".to_string(),
            actor2: Some("BETA".to_string()),
            location: "Marmara".to_string(),
            country: "Turkey".to_string(),
            lat: 40.5,
            lon: 29.2,
            fatalities: 1,
            notes: "Test catisma".to_string(),
        });
        assert!(
            st.alerts.iter().any(|a| a.domain == "CATISMA"),
            "Yakin/zayiatli GDELT olayi otomatik CATISMA alarmi uretmeli"
        );

        // Ayni 160km icinde >=2 bagimsiz disiplin (CATISMA + SISMIK) -> Fuzyon skoru >= 70 -> FUZYON alarmi
        st.upsert_earthquake(crate::types::Earthquake {
            id: "USGS-1".to_string(),
            place: "Marmara Test".to_string(),
            mag: 5.0,
            depth_km: 10.0,
            time: Utc::now(),
            lat: 40.6,
            lon: 29.3,
            url: String::new(),
        });
        recompute_fusion_and_anomalies(&mut st);
        assert!(
            !st.fusion_clusters.is_empty() && st.fusion_clusters[0].score >= 70,
            "En az >=70 skorlu fuzyon kumesi bekleniyordu"
        );
        assert!(
            st.alerts.iter().any(|a| a.domain == "FUZYON"),
            "Skor >=70 fuzyon kumesi otomatik FUZYON alarmi uretmeli"
        );
    }
}
