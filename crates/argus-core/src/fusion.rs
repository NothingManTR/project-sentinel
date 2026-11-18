use crate::geo::{cross_track_distance_km, haversine_km, initial_bearing_deg};
use crate::state::AppState;
use crate::types::{AarPair, AlertItem, FusionCluster, ReconWindow, TargetTrack, ThreatSeverity};
use chrono::Utc;
use std::collections::HashMap;

const EARTH_RADIUS_KM: f64 = 6371.0088;

fn is_tanker_aircraft(t: &TargetTrack) -> bool {
    let cs = t.callsign.to_uppercase();
    if ["LAGR", "NCHO", "GOLD", "QUID", "PACK", "ETHYL", "MMF"]
        .iter()
        .any(|p| cs.starts_with(p))
    {
        return true;
    }
    let cls = t
        .airframe_or_vessel_class
        .as_deref()
        .unwrap_or("")
        .to_uppercase();
    let info = t.info.to_uppercase();
    let combined = format!("{} {}", cls, info);
    if [
        "K35R",
        "KC135",
        "KC-135",
        "KC46",
        "KC-46",
        "KC10",
        "KC-10",
        "KC30",
        "KC-30",
        "IL78",
        "IL-78",
        "KC2",
        "KC-2",
        "MRTT",
        "STRATOTANKER",
        "TANKER",
    ]
    .iter()
    .any(|k| combined.contains(k))
    {
        return true;
    }
    t.is_military && combined.contains("A332")
}

fn is_sea_tanker(s: &TargetTrack) -> bool {
    let vt = s.vessel_type.as_deref().unwrap_or("").to_uppercase();
    let cls = s
        .airframe_or_vessel_class
        .as_deref()
        .unwrap_or("")
        .to_uppercase();
    let cs = s.callsign.to_uppercase();
    [vt.as_str(), cls.as_str(), cs.as_str()].iter().any(|txt| {
        txt.contains("TANKER")
            || txt.contains("PETROL")
            || txt.contains("OIL")
            || txt.contains("CHEM")
            || txt.contains("VLCC")
            || txt.contains("LNG")
            || txt.contains("LPG")
    })
}

fn merge_anomaly(map: &mut HashMap<String, String>, id: &str, tag: &str) {
    if let Some(existing) = map.get_mut(id) {
        if !existing.contains(tag) {
            existing.push_str(" + ");
            existing.push_str(tag);
        }
    } else {
        map.insert(id.to_string(), tag.to_string());
    }
}

fn classify_recon_sensor(sat_name: &str) -> Option<&'static str> {
    let u = sat_name.to_uppercase();
    if [
        "SENTINEL-1",
        "SENTINEL 1",
        "TERRASAR",
        "TANDEM",
        "COSMO-SKY",
        "RADARSAT",
        "ICEYE",
        "CAPELLA",
        "PAZ",
        "SAOCOM",
        "ALOS",
    ]
    .iter()
    .any(|k| u.contains(k))
    {
        return Some("SAR");
    }
    if [
        "COSMOS 2558",
        "COSMOS-2558",
        "YAOGAN",
        "NOSS",
        "MENTOR",
        "ORION",
        "INTRUDER",
        "LOTOS",
        "PION",
        "ELINT",
        "SIGINT",
        "INSPECTOR",
    ]
    .iter()
    .any(|k| u.contains(k))
    {
        return Some("SIGINT");
    }
    if [
        "GOKTURK",
        "LANDSAT",
        "SENTINEL-2",
        "SENTINEL 2",
        "TERRA",
        "AQUA",
        "SUOMI",
        "NOAA 20",
        "NOAA-20",
        "NOAA 21",
        "SPOT",
        "PLEIADES",
        "WORLDVIEW",
        "GEOEYE",
        "JILIN",
        "GAOFEN",
        "MAXAR",
        "SKYSAT",
    ]
    .iter()
    .any(|k| u.contains(k))
    {
        return Some("EO");
    }
    None
}

fn sat_elevation_deg(
    target_lat: f64,
    target_lon: f64,
    sat_lat: f64,
    sat_lon: f64,
    sat_alt_km: f64,
) -> f64 {
    let d_km = haversine_km(target_lat, target_lon, sat_lat, sat_lon);
    let gamma = (d_km / EARTH_RADIUS_KM).clamp(0.0, std::f64::consts::PI);
    let r_sat = EARTH_RADIUS_KM + sat_alt_km.max(100.0);
    let up = r_sat * gamma.cos() - EARTH_RADIUS_KM;
    let horiz = r_sat * gamma.sin();
    up.atan2(horiz).to_degrees()
}

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
    for (id, tag) in &state.track_anomalies {
        if tag.contains("AIS-SPOOF") && state.sea_targets.iter().any(|s| &s.id == id) {
            anomalies.insert(id.clone(), tag.clone());
        }
    }

    for t in state.air_targets.iter() {
        if !t.is_military {
            continue;
        }
        if let Some(trail) = state.target_trails.get(&t.id) {
            if is_air_orbit_pattern(trail) {
                merge_anomaly(&mut anomalies, &t.id, "ORBIT / REFUEL-CAP");
            }
        }
    }

    for s in state.sea_targets.iter() {
        let spd = s.speed_kts.unwrap_or(0.0);
        if (0.2..=2.2).contains(&spd) {
            if let Some(trail) = state.target_trails.get(&s.id) {
                if trail.len() >= 4 {
                    merge_anomaly(&mut anomalies, &s.id, "LOITERING");
                }
            }
        } else if spd > 65.0 {
            merge_anomaly(&mut anomalies, &s.id, "AIS ANOMALI (>65kt)");
        }
    }

    let mut pending_alerts: Vec<(AlertItem, Option<(f64, f64)>)> = Vec::new();
    let mut signals: Vec<(f64, f64, &'static str, String)> = Vec::new();

    // 1) Tanker-Avci AAR (Havadan Yakit Ikmal Eslesmesi)
    let tankers: Vec<&TargetTrack> = state
        .air_targets
        .iter()
        .filter(|t| {
            let alt_ft = t.geo.alt_m.unwrap_or(0.0) * 3.28084;
            alt_ft >= 10000.0 && is_tanker_aircraft(t)
        })
        .collect();
    let receivers: Vec<&TargetTrack> = state
        .air_targets
        .iter()
        .filter(|t| {
            let alt_ft = t.geo.alt_m.unwrap_or(0.0) * 3.28084;
            t.is_military && alt_ft >= 10000.0 && !is_tanker_aircraft(t)
        })
        .collect();

    let mut aar_pairs: Vec<AarPair> = Vec::new();
    for tanker in &tankers {
        let t_alt_ft = tanker.geo.alt_m.unwrap_or(0.0) * 3.28084;
        for receiver in &receivers {
            if tanker.id == receiver.id || (tanker.geo.lat - receiver.geo.lat).abs() > 0.2 {
                continue;
            }
            let r_alt_ft = receiver.geo.alt_m.unwrap_or(0.0) * 3.28084;
            if (t_alt_ft - r_alt_ft).abs() > 1500.0 {
                continue;
            }
            let dist_km = haversine_km(
                tanker.geo.lat,
                tanker.geo.lon,
                receiver.geo.lat,
                receiver.geo.lon,
            );
            if dist_km <= 15.0 {
                let mid_lat = (tanker.geo.lat + receiver.geo.lat) * 0.5;
                let mid_lon = (tanker.geo.lon + receiver.geo.lon) * 0.5;
                let mid_alt_ft = (t_alt_ft + r_alt_ft) * 0.5;
                let is_locked = dist_km <= 5.0 && (t_alt_ft - r_alt_ft).abs() <= 500.0;
                aar_pairs.push(AarPair {
                    tanker_id: tanker.id.clone(),
                    tanker_callsign: tanker.callsign.clone(),
                    receiver_id: receiver.id.clone(),
                    receiver_callsign: receiver.callsign.clone(),
                    lat: mid_lat,
                    lon: mid_lon,
                    alt_ft: mid_alt_ft,
                    dist_km,
                    is_locked,
                });
                merge_anomaly(&mut anomalies, &tanker.id, "AAR-IKMAL");
                merge_anomaly(&mut anomalies, &receiver.id, "AAR-IKMAL");
                pending_alerts.push((
                    AlertItem {
                        timestamp: Utc::now(),
                        domain: "HAVA-AAR".to_string(),
                        severity: ThreatSeverity::Warning,
                        message: format!(
                            "[AAR-IKMAL] {} <-> {} ({:.1} km, FL{:.0})",
                            tanker.callsign,
                            receiver.callsign,
                            dist_km,
                            mid_alt_ft / 100.0
                        ),
                        target_id: Some(format!("AAR:{}:{}", tanker.id, receiver.id)),
                    },
                    Some((mid_lat, mid_lon)),
                ));
            }
        }
    }
    aar_pairs.sort_by(|a, b| {
        a.dist_km
            .partial_cmp(&b.dist_km)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    aar_pairs.truncate(32);
    state.aar_pairs = aar_pairs;

    // 2a) Shadow-Fleet STS Transferi (<0.6 km & <=2.2 kt iki tanker)
    let slow_tankers: Vec<&TargetTrack> = state
        .sea_targets
        .iter()
        .filter(|s| s.speed_kts.is_some_and(|spd| spd <= 2.2) && is_sea_tanker(s))
        .collect();
    for i in 0..slow_tankers.len() {
        for j in (i + 1)..slow_tankers.len() {
            let a = slow_tankers[i];
            let b = slow_tankers[j];
            if (a.geo.lat - b.geo.lat).abs() > 0.02 || (a.geo.lon - b.geo.lon).abs() > 0.03 {
                continue;
            }
            let d_km = haversine_km(a.geo.lat, a.geo.lon, b.geo.lat, b.geo.lon);
            if d_km < 0.6 {
                merge_anomaly(&mut anomalies, &a.id, "STS-TRANSFER");
                merge_anomaly(&mut anomalies, &b.id, "STS-TRANSFER");
                let mid_lat = (a.geo.lat + b.geo.lat) * 0.5;
                let mid_lon = (a.geo.lon + b.geo.lon) * 0.5;
                signals.push((
                    mid_lat,
                    mid_lon,
                    "STS-TRANSFER",
                    format!("{} <-> {}", a.callsign, b.callsign),
                ));
                pending_alerts.push((
                    AlertItem {
                        timestamp: Utc::now(),
                        domain: "DENIZ-STS".to_string(),
                        severity: ThreatSeverity::Warning,
                        message: format!(
                            "[STS-TRANSFER] {} <-> {} ({:.2} km, <=2.2kt)",
                            a.callsign, b.callsign, d_km
                        ),
                        target_id: Some(format!("STS:{}:{}", a.id, b.id)),
                    },
                    Some((mid_lat, mid_lon)),
                ));
            }
        }
    }

    state.track_anomalies = anomalies;

    // 2b) Denizalti Kablo Sabotaj Korelatoru (LOITERING/DARK-SHIP <= 15 km kablo + BGP-OUTAGE)
    let mut direct_clusters: Vec<FusionCluster> = Vec::new();
    let bgp_outage = state
        .cyber_incidents
        .iter()
        .find(|c| c.event_type.to_uppercase().contains("BGP"))
        .map(|c| c.title.clone());
    if let Some(bgp_title) = bgp_outage {
        if !state.subsea_cables.is_empty() {
            let mut suspect_vessels: Vec<(f64, f64, String, &str)> = Vec::new();
            for s in state.sea_targets.iter() {
                if let Some(anom) = state.track_anomalies.get(&s.id) {
                    if anom.contains("LOITERING") || anom.contains("STS-TRANSFER") {
                        let vtag = if anom.contains("STS-TRANSFER") {
                            "STS-TRANSFER"
                        } else {
                            "LOITERING"
                        };
                        suspect_vessels.push((s.geo.lat, s.geo.lon, s.callsign.clone(), vtag));
                    }
                }
            }
            for d in &state.dark_ships {
                suspect_vessels.push((d.last_lat, d.last_lon, d.callsign.clone(), "DARK-SHIP"));
            }

            for (vlat, vlon, vcs, vtag) in suspect_vessels {
                let mut closest_cable: Option<(&str, f64)> = None;
                for cable in &state.subsea_cables {
                    for seg in &cable.lines {
                        for pair in seg.windows(2) {
                            let (a_lon, a_lat) = pair[0];
                            let (b_lon, b_lat) = pair[1];
                            let min_lat = a_lat.min(b_lat) - 0.25;
                            let max_lat = a_lat.max(b_lat) + 0.25;
                            let min_lon = a_lon.min(b_lon) - 0.50;
                            let max_lon = a_lon.max(b_lon) + 0.50;
                            if vlat < min_lat || vlat > max_lat || vlon < min_lon || vlon > max_lon {
                                continue;
                            }
                            let d_km =
                                cross_track_distance_km(vlat, vlon, a_lat, a_lon, b_lat, b_lon);
                            if d_km <= 15.0
                                && closest_cable.is_none_or(|(_, prev_d)| d_km < prev_d)
                            {
                                closest_cable = Some((cable.name.as_str(), d_km));
                            }
                        }
                    }
                }
                if let Some((cable_name, dist_km)) = closest_cable {
                    let summary = format!(
                        "[KABLO-SABOTAJ] {} ({}) -> {} ({:.1}km) + {}",
                        vcs, vtag, cable_name, dist_km, bgp_title
                    );
                    direct_clusters.push(FusionCluster {
                        lat: vlat,
                        lon: vlon,
                        score: 115,
                        domains: vec![
                            "DENIZ".to_string(),
                            "KABLO".to_string(),
                            "SIBER-BGP".to_string(),
                        ],
                        summary: summary.clone(),
                    });
                    pending_alerts.push((
                        AlertItem {
                            timestamp: Utc::now(),
                            domain: "KABLO-SABOTAJ".to_string(),
                            severity: ThreatSeverity::Critical,
                            message: summary,
                            target_id: Some(format!("FUZ:{:.1}:{:.1}", vlat, vlon)),
                        },
                        Some((vlat, vlon)),
                    ));
                }
            }
        }
    }

    // 3) MASINT Patlama Ayirt Edici (Sig deprem <= 3.5 km + FIRMS >=340K <=25km veya ACLED <=45km)
    for q in state.earthquakes.iter() {
        if q.depth_km > 0.0 && q.depth_km <= 3.5 {
            let fire_hit = state
                .firms_fires
                .iter()
                .filter(|f| f.brightness >= 340.0)
                .map(|f| (f.brightness, haversine_km(q.lat, q.lon, f.lat, f.lon)))
                .find(|&(_, d)| d <= 25.0);
            let acled_hit = state
                .acled_events
                .iter()
                .map(|a| (&a.event_type, haversine_km(q.lat, q.lon, a.lat, a.lon)))
                .find(|&(_, d)| d <= 45.0);

            if fire_hit.is_some() || acled_hit.is_some() {
                let reason = if let Some((br, d)) = fire_hit {
                    format!("FIRMS {:.0}K ({:.1}km)", br, d)
                } else if let Some((etype, d)) = acled_hit {
                    format!("CATISMA {} ({:.1}km)", etype, d)
                } else {
                    String::new()
                };
                signals.push((
                    q.lat,
                    q.lon,
                    "MASINT",
                    format!("M{:.1} d={:.1}km {}", q.mag, q.depth_km, reason),
                ));
                pending_alerts.push((
                    AlertItem {
                        timestamp: Utc::now(),
                        domain: "MASINT-PATLAMA".to_string(),
                        severity: ThreatSeverity::Critical,
                        message: format!(
                            "[MASINT-PATLAMA] Sig Sismik M{:.1} (d={:.1}km) {} + {}",
                            q.mag, q.depth_km, q.place, reason
                        ),
                        target_id: Some(format!("MASINT:{}", q.id)),
                    },
                    Some((q.lat, q.lon)),
                ));
            }
        }
    }

    // Çoklu sensör coğrafi füzyon (160 km yarıçap içinde >= 2 bağımsız disiplin çakışması)
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
    let mut clusters = direct_clusters;

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

    // 4) SGP4 Recon Window (EO/SAR/SIGINT kesif uydulari + bulut ortusu filtresi, top 8)
    let (target_lat, target_lon, target_name) = state
        .selected_target_id
        .as_deref()
        .and_then(|sel_id| state.targets.iter().find(|t| t.id == sel_id))
        .map(|t| (t.geo.lat, t.geo.lon, t.callsign.clone()))
        .or_else(|| {
            clusters.first().map(|fc| {
                (
                    fc.lat,
                    fc.lon,
                    format!("FUZYON-KUME (Skor:{})", fc.score),
                )
            })
        })
        .unwrap_or_else(|| {
            (
                state.station_lat,
                state.station_lon,
                state.config.station.callsign.clone(),
            )
        });

    let clouds_pct: Option<u8> = state
        .weather_points
        .iter()
        .map(|w| (w.clouds_pct, haversine_km(target_lat, target_lon, w.lat, w.lon)))
        .filter(|&(_, d)| d <= 300.0)
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(c, _)| c)
        .or_else(|| {
            let d_st = haversine_km(target_lat, target_lon, state.station_lat, state.station_lon);
            if d_st <= 300.0 || state.weather_points.is_empty() {
                state.station_weather.as_ref().map(|w| w.clouds_pct)
            } else {
                None
            }
        });
    let cloud_blocked = clouds_pct.is_some_and(|c| c > 60);

    let mut recon_windows: Vec<ReconWindow> = Vec::new();
    let near_station = haversine_km(target_lat, target_lon, state.station_lat, state.station_lon) <= 50.0;
    for sat in &state.satellites {
        let Some(sensor_type) = classify_recon_sensor(&sat.name) else {
            continue;
        };
        let mut best_el = f64::NEG_INFINITY;
        let mut aos_secs: Option<u64> = None;

        if sat.ground_track.is_empty() {
            let el = sat_elevation_deg(target_lat, target_lon, sat.lat, sat.lon, sat.alt_km);
            best_el = if near_station { el.max(sat.el_deg) } else { el };
            if best_el > 0.0 {
                aos_secs = Some(0);
            } else if near_station {
                aos_secs = Some(sat.next_pass_secs);
            }
        } else {
            for (k, &(pt_lon, pt_lat)) in sat.ground_track.iter().enumerate() {
                let el = sat_elevation_deg(target_lat, target_lon, pt_lat, pt_lon, sat.alt_km);
                if el > best_el {
                    best_el = el;
                }
                if el > 0.0 && aos_secs.is_none() {
                    aos_secs = Some((k as u64) * 300);
                }
            }
        }

        let final_aos = aos_secs.unwrap_or(if near_station && sat.next_pass_secs > 0 {
            sat.next_pass_secs
        } else {
            u64::MAX
        });

        recon_windows.push(ReconWindow {
            sat_name: sat.name.clone(),
            norad_id: sat.norad_id,
            sensor_type: sensor_type.to_string(),
            target_name: target_name.clone(),
            target_lat,
            target_lon,
            aos_secs: final_aos,
            max_el_deg: best_el.max(0.0),
            cloud_blocked,
        });
    }
    recon_windows.sort_by(|a, b| {
        let a_vis = if a.aos_secs < u64::MAX { 0u8 } else { 1u8 };
        let b_vis = if b.aos_secs < u64::MAX { 0u8 } else { 1u8 };
        a_vis
            .cmp(&b_vis)
            .then_with(|| a.aos_secs.cmp(&b.aos_secs))
            .then_with(|| {
                b.max_el_deg
                    .partial_cmp(&a.max_el_deg)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    for w in &mut recon_windows {
        if w.aos_secs == u64::MAX {
            w.aos_secs = 5400;
        }
    }
    recon_windows.truncate(8);
    state.recon_windows = recon_windows;

    // E-3: Yüksek skorlu K-4 füzyon kümeleri ve askeri anomali yörüngeleri için otomatik taktik alarmlar
    let anom_alerts: Vec<(AlertItem, Option<(f64, f64)>)> = state
        .track_anomalies
        .iter()
        .filter(|(_, tag)| tag.contains("ORBIT") || tag.contains("REFUEL") || tag.contains("LOW-FAST"))
        .map(|(tid, tag)| {
            let found = state.air_targets.iter().find(|t| &t.id == tid);
            let cs = found
                .map(|t| t.callsign.clone())
                .unwrap_or_else(|| tid.clone());
            let coords = found.map(|t| (t.geo.lat, t.geo.lon));
            let aid = format!("ANOM:{}", tid);
            (
                AlertItem {
                    timestamp: Utc::now(),
                    domain: "HAVA-ANOMALI".to_string(),
                    severity: ThreatSeverity::Warning,
                    message: format!("[ANOMALI] {} ({}): {}", cs, tid, tag),
                    target_id: Some(aid),
                },
                coords,
            )
        })
        .collect();

    for (item, coords) in anom_alerts {
        state.push_priority_alert(item, coords);
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
                target_id: Some(fid),
            };
            state.push_priority_alert(item, Some((fc.lat, fc.lon)));
        }
    }

    for (item, coords) in pending_alerts {
        state.push_priority_alert(item, coords);
    }

    state.fusion_clusters = clusters;

    // 5) DVR Zaman Çizelgesi Kaydı
    let mut dvr = std::mem::take(&mut state.dvr);
    dvr.record_if_due(state);
    state.dvr = dvr;
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

    fn make_air(id: &str, cs: &str, lat: f64, lon: f64, alt_ft: f64, cls: Option<&str>, is_mil: bool) -> TargetTrack {
        TargetTrack {
            id: id.to_string(),
            callsign: cs.to_string(),
            domain: crate::types::Domain::Air,
            geo: crate::types::GeoPoint::new(lat, lon, Some(alt_ft * 0.3048)),
            speed_kts: Some(380.0),
            heading_deg: Some(90.0),
            vertical_rate_fpm: None,
            squawk: None,
            is_military: is_mil,
            operator_country: None,
            operator_branch: None,
            airframe_or_vessel_class: cls.map(str::to_string),
            origin: None,
            destination: None,
            vessel_type: None,
            info: String::new(),
            last_seen: Utc::now(),
        }
    }

    #[test]
    fn test_aar_tanker_pairing() {
        let mut st = AppState::new(crate::config::AppConfig::default());
        st.dvr.last_recorded_at = Some(Utc::now());
        st.upsert_target(make_air("TNK1", "LAGR11", 37.0, 35.0, 22000.0, Some("K35R"), true));
        st.upsert_target(make_air("FTR1", "VIPER1", 37.02, 35.02, 22200.0, Some("F16"), true));

        recompute_fusion_and_anomalies(&mut st);
        assert_eq!(st.aar_pairs.len(), 1);
        assert_eq!(st.aar_pairs[0].tanker_id, "TNK1");
        assert_eq!(st.aar_pairs[0].receiver_id, "FTR1");
        assert!(st.aar_pairs[0].is_locked);
        assert_eq!(st.track_anomalies.get("FTR1").map(String::as_str), Some("AAR-IKMAL"));
        assert!(st.alerts.iter().any(|a| a.message.contains("[AAR-IKMAL]")));
    }

    #[test]
    fn test_sts_and_subsea_cable_sabotage_correlation() {
        let mut st = AppState::new(crate::config::AppConfig::default());
        st.dvr.last_recorded_at = Some(Utc::now());

        let mut t1 = TargetTrack {
            id: "TANK1".to_string(),
            callsign: "SHADOW OIL 1".to_string(),
            domain: crate::types::Domain::Sea,
            geo: crate::types::GeoPoint::new(36.0, 28.0, Some(0.0)),
            speed_kts: Some(1.0),
            heading_deg: Some(0.0),
            vertical_rate_fpm: None,
            squawk: None,
            is_military: false,
            operator_country: None,
            operator_branch: None,
            airframe_or_vessel_class: None,
            origin: None,
            destination: None,
            vessel_type: Some("Tanker".to_string()),
            info: String::new(),
            last_seen: Utc::now(),
        };
        let mut t2 = t1.clone();
        t2.id = "TANK2".to_string();
        t2.callsign = "SHADOW OIL 2".to_string();
        t2.geo = crate::types::GeoPoint::new(36.002, 28.002, Some(0.0)); // ~0.28 km

        // Give t1 a 4-point trail so LOITERING also registers
        for k in 0..4 {
            t1.geo.lat = 36.0 + (k as f64) * 0.001;
            st.upsert_target(t1.clone());
        }
        st.upsert_target(t2);

        st.subsea_cables.push(crate::types::SubseaCable {
            id: "c1".to_string(),
            name: "Med Nautilus".to_string(),
            color_hex: "#38bdf8".to_string(),
            lines: vec![vec![(27.5, 36.05), (28.5, 36.05)]],
        });
        st.upsert_cyber_incident(crate::types::CyberIncident {
            id: "IODA-1".to_string(),
            event_type: "BGP-OUTAGE".to_string(),
            title: "EastMed ISP Drop".to_string(),
            target: "AS123".to_string(),
            severity: ThreatSeverity::Critical,
            cve: None,
            asn: Some("AS123".to_string()),
            timestamp: Utc::now(),
        });

        recompute_fusion_and_anomalies(&mut st);
        assert!(
            st.track_anomalies
                .get("TANK1")
                .is_some_and(|v| v.contains("STS-TRANSFER"))
        );
        assert!(st.alerts.iter().any(|a| a.message.contains("[KABLO-SABOTAJ]")));
        assert!(st.active_flash_alert.as_ref().is_some_and(|f| !f.acked));
        assert!(st
            .fusion_clusters
            .iter()
            .any(|fc| fc.summary.contains("[KABLO-SABOTAJ]")));
    }

    #[test]
    fn test_masint_explosion_and_recon_windows() {
        let mut st = AppState::new(crate::config::AppConfig::default());
        st.dvr.last_recorded_at = Some(Utc::now());
        st.station_lat = 41.0;
        st.station_lon = 29.0;

        st.upsert_earthquake(crate::types::Earthquake {
            id: "EQ-SHALLOW".to_string(),
            place: "Test Basin".to_string(),
            mag: 3.2,
            depth_km: 1.5,
            time: Utc::now(),
            lat: 35.0,
            lon: 35.0,
            url: String::new(),
        });
        st.upsert_fire(crate::types::FirmsFire {
            lat: 35.05,
            lon: 35.05,
            brightness: 360.0,
            confidence: "high".to_string(),
            acq_date: "2026-11-18".to_string(),
            acq_time: "1200".to_string(),
            satellite: "NOAA-20".to_string(),
        });

        st.satellites.push(crate::types::SatellitePass {
            norad_id: 39634,
            name: "SENTINEL-1A".to_string(),
            lat: 40.9,
            lon: 29.1,
            alt_km: 693.0,
            velocity_kms: 7.5,
            az_deg: 180.0,
            el_deg: 75.0,
            next_pass_secs: 0,
            pass_duration_secs: 580,
            ground_track: vec![(29.1, 40.9), (29.2, 41.2)],
        });
        st.satellites.push(crate::types::SatellitePass {
            norad_id: 56123,
            name: "STARLINK-6115".to_string(),
            lat: 41.0,
            lon: 29.0,
            alt_km: 550.0,
            velocity_kms: 7.5,
            az_deg: 90.0,
            el_deg: 80.0,
            next_pass_secs: 0,
            pass_duration_secs: 400,
            ground_track: vec![(29.0, 41.0)],
        });
        st.station_weather = Some(crate::types::WeatherPoint {
            name: "Station".to_string(),
            lat: 41.0,
            lon: 29.0,
            temp_c: 15.0,
            wind_kts: 10.0,
            wind_deg: 180.0,
            clouds_pct: 80,
            visibility_km: 10.0,
            condition: "Clouds".to_string(),
        });

        recompute_fusion_and_anomalies(&mut st);
        assert!(st.alerts.iter().any(|a| a.message.contains("[MASINT-PATLAMA]")));
        assert_eq!(st.recon_windows.len(), 1);
        assert_eq!(st.recon_windows[0].sensor_type, "SAR");
        assert!(st.recon_windows[0].cloud_blocked);
    }
}
