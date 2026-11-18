use argus_core::types::SatellitePass;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::Deserialize;
use sgp4::{Constants, Elements};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct DynamicTleEntry {
    pub norad_id: u32,
    pub name: String,
    pub line1: String,
    pub line2: String,
}

pub struct CelesTrakClient {
    client: reqwest::Client,
    live_cache: RwLock<Option<(Vec<DynamicTleEntry>, Instant)>>,
    iss_cache: RwLock<Option<(Option<WhereTheIssResp>, Instant)>>,
}

impl Default for CelesTrakClient {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Deserialize)]
struct WhereTheIssResp {
    latitude: f64,
    longitude: f64,
    altitude: f64,
    velocity: f64, // km/h
}

pub struct SatelliteTleEntry {
    pub norad_id: u32,
    pub name: &'static str,
    pub line1: &'static str,
    pub line2: &'static str,
}

/// Otantik NORAD İki Satırlı Yörünge Elemanları (TLE) Kataloğu
pub const TLE_CATALOG: &[SatelliteTleEntry] = &[
    SatelliteTleEntry {
        norad_id: 25544,
        name: "ISS (ZARYA)",
        line1: "1 25544U 98067A   26306.43427755  .00005374  00000+0  10508-3 0  9999",
        line2: "2 25544  51.6332 335.9757 0007139 339.3739  20.6961 15.49150046593183",
    },
    SatelliteTleEntry {
        norad_id: 48274,
        name: "CSS (TIANHE)",
        line1: "1 48274U 21035A   26306.41282506  .00009783  00000+0  11564-3 0  9997",
        line2: "2 48274  41.4677 202.1822 0004189 247.3799 112.6597 15.61298938314759",
    },
    SatelliteTleEntry {
        norad_id: 20580,
        name: "HST (HUBBLE)",
        line1: "1 20580U 90037B   26306.42508999  .00004962  00000+0  14793-3 0  9994",
        line2: "2 20580  28.4727 214.7620 0001670 250.8889 109.1527 15.32151540809787",
    },
    SatelliteTleEntry {
        norad_id: 43013,
        name: "NOAA 20 (JPSS-1)",
        line1: "1 43013U 17073A   26306.43548814  .00000048  00000+0  43572-4 0  9999",
        line2: "2 43013  98.7905 244.7319 0000684 138.5889 221.5338 14.19534340464081",
    },
    SatelliteTleEntry {
        norad_id: 37849,
        name: "SUOMI NPP",
        line1: "1 37849U 11061A   26306.49203606  .00000053  00000+0  46245-4 0  9999",
        line2: "2 37849  98.8095 246.4919 0000887 319.0558  41.0551 14.19542463778064",
    },
    SatelliteTleEntry {
        norad_id: 25994,
        name: "TERRA (EOS AM-1)",
        line1: "1 25994U 99068A   26306.49832818  .00000191  00000+0  47721-4 0  9996",
        line2: "2 25994  97.9285 350.4422 0003163 131.9480   0.7826 14.61187565429800",
    },
    SatelliteTleEntry {
        norad_id: 27424,
        name: "AQUA (EOS PM-1)",
        line1: "1 27424U 02022A   26306.56883022  .00000466  00000+0  10235-3 0  9995",
        line2: "2 27424  98.4476 278.7564 0000886 100.4232  15.5469 14.62259553303493",
    },
    SatelliteTleEntry {
        norad_id: 49260,
        name: "LANDSAT 9",
        line1: "1 49260U 21088A   26306.15411669  .00000190  00000+0  52313-4 0  9997",
        line2: "2 49260  98.2141  14.5523 0001165  84.1342 275.9989 14.57103793271091",
    },
    SatelliteTleEntry {
        norad_id: 39084,
        name: "LANDSAT 8",
        line1: "1 39084U 13008A   26320.19709380  .00000265  00000-0  68979-4 0  9992",
        line2: "2 39084  98.2145  28.3896 0001210  90.5666 269.5671 14.57113055720037",
    },
    SatelliteTleEntry {
        norad_id: 39634,
        name: "SENTINEL-1A",
        line1: "1 39634U 14016A   26321.51350887  .00000880  00000-0  19222-3 0  9993",
        line2: "2 39634  98.1586 326.6197 0001353  87.1871 272.9484 14.60166931672381",
    },
    SatelliteTleEntry {
        norad_id: 31698,
        name: "TERRASAR-X",
        line1: "1 31698U 07026A   26314.41932625  .00000732  00000-0  38024-4 0  9993",
        line2: "2 31698  97.4455 320.0486 0001838  90.3782 269.7663 15.19153228 75429",
    },
    SatelliteTleEntry {
        norad_id: 53323,
        name: "COSMOS 2558 (INSPECTOR)",
        line1: "1 53323U 22089A   26255.07407622  .09214690  24429-5  34229-3 0  9995",
        line2: "2 53323  97.2053   4.5867 0014198 269.3733  90.5942 16.42755340231631",
    },
    SatelliteTleEntry {
        norad_id: 41875,
        name: "GOKTURK 1A",
        line1: "1 41875U 16073A   26306.55882038  .00000248  00000+0  57767-4 0  9997",
        line2: "2 41875  98.1488 199.3366 0001180  78.4423 281.6911 14.62776917529059",
    },
    SatelliteTleEntry {
        norad_id: 39030,
        name: "GOKTURK 2",
        line1: "1 39030U 12073A   26306.54985333 -.00000115  00000+0 -99249-5 0  9991",
        line2: "2 39030  97.6744 137.0324 0000818  90.8498 269.2806 14.75645991744918",
    },
    SatelliteTleEntry {
        norad_id: 47306,
        name: "TURKSAT 5A",
        line1: "1 47306U 21001A   26306.47602542  .00000146  00000+0  00000+0 0  9990",
        line2: "2 47306   0.0014  52.1468 0000147 263.6521 288.2635  1.00270421 21352",
    },
    SatelliteTleEntry {
        norad_id: 56123,
        name: "STARLINK-6115",
        line1: "1 56123U 23046AH  26306.41668980 -.00000982  00000+0 -26140-4 0  9998",
        line2: "2 56123  43.0019 244.9294 0001695 267.0877  19.5678 15.27583762  5678",
    },
];

impl CelesTrakClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .user_agent("SentinelOSINT/2.0")
                .build()
                .unwrap_or_default(),
            live_cache: RwLock::new(None),
            iss_cache: RwLock::new(None),
        }
    }

    /// CelesTrak GP API üzerinden canlı 3 satırlı TLE verilerini çeker ve 2 saat (7200 sn) önbellekte tutar
    async fn get_live_or_catalog_tles(&self) -> Vec<DynamicTleEntry> {
        {
            let guard = self.live_cache.read();
            if let Some((entries, ts)) = guard.as_ref() {
                if ts.elapsed() < Duration::from_secs(7200) && !entries.is_empty() {
                    return entries.clone();
                }
            }
        }

        let mut fetched = Vec::new();
        let urls = [
            "https://celestrak.org/NORAD/elements/gp.php?GROUP=stations&FORMAT=tle",
            "https://live.ariss.org/iss.txt",
            "https://www.amsat.org/tle/current/nasabare.txt",
        ];

        for url in urls {
            if let Ok(resp) = self.client.get(url).send().await {
                if resp.status().is_success() {
                    if let Ok(body) = resp.text().await {
                        let clean_lines: Vec<&str> = body
                            .lines()
                            .map(|l| l.trim())
                            .filter(|l| !l.is_empty())
                            .collect();
                        for chunk in clean_lines.as_chunks::<3>().0 {
                            let name = chunk[0].to_string();
                            let l1 = chunk[1].to_string();
                            let l2 = chunk[2].to_string();
                            if l1.starts_with("1 ") && l2.starts_with("2 ") && l1.len() >= 10 {
                                let norad: u32 = l1[2..7].trim().parse().unwrap_or(0);
                                if norad > 0 && !fetched.iter().any(|e: &DynamicTleEntry| e.norad_id == norad) {
                                    fetched.push(DynamicTleEntry {
                                        norad_id: norad,
                                        name,
                                        line1: l1,
                                        line2: l2,
                                    });
                                }
                            }
                        }
                    }
                }
            }
            if fetched.len() >= 24 {
                break;
            }
        }
        fetched.truncate(28);

        // Eksik kalan temel uyduları yerleşik TLE kataloğundan tamamla
        for item in TLE_CATALOG {
            if !fetched.iter().any(|e| e.norad_id == item.norad_id) {
                fetched.push(DynamicTleEntry {
                    norad_id: item.norad_id,
                    name: item.name.to_string(),
                    line1: item.line1.to_string(),
                    line2: item.line2.to_string(),
                });
            }
        }

        let mut guard = self.live_cache.write();
        *guard = Some((fetched.clone(), Instant::now()));
        fetched
    }

    /// Canlı ISS koordinatlarını wheretheiss.at üzerinden çeker (en az 60 sn önbellekli)
    async fn fetch_live_iss(&self) -> Option<WhereTheIssResp> {
        {
            let guard = self.iss_cache.read();
            if let Some((cached, ts)) = guard.as_ref() {
                if ts.elapsed() < Duration::from_secs(60) {
                    return cached.clone();
                }
            }
        }
        let url = "https://api.wheretheiss.at/v1/satellites/25544";
        let fetched = match self.client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => resp.json::<WhereTheIssResp>().await.ok(),
            _ => None,
        };
        let mut guard = self.iss_cache.write();
        *guard = Some((fetched.clone(), Instant::now()));
        fetched
    }

    /// Verilen UTC zamanı için Greenwich Ortalama Yıldız Zamanı (GMST) açısını (radyan) hesaplar
    fn gmst_radians(now: DateTime<Utc>) -> f64 {
        let jd = (now.timestamp_millis() as f64 / 86_400_000.0) + 2_440_587.5;
        let d = jd - 2_451_545.0;
        let t = d / 36_525.0;
        let gmst_deg = (280.46061837 + 360.98564736629 * d + 0.000387933 * t * t - (t * t * t) / 38_710_000.0)
            .rem_euclid(360.0);
        gmst_deg.to_radians()
    }

    /// TEME eylemsiz koordinatlarını Z ekseni etrafında -GMST açısıyla döndürerek ECEF'e çevirir
    fn teme_to_ecef(pos_teme: [f64; 3], now: DateTime<Utc>) -> [f64; 3] {
        let gmst_rad = Self::gmst_radians(now);
        let (sin_g, cos_g) = gmst_rad.sin_cos();
        let x_ecef = pos_teme[0] * cos_g + pos_teme[1] * sin_g;
        let y_ecef = -pos_teme[0] * sin_g + pos_teme[1] * cos_g;
        let z_ecef = pos_teme[2];
        [x_ecef, y_ecef, z_ecef]
    }

    /// İstasyon ECEF konumundan uydu ECEF konumuna Toposentrik ENU (Doğu-Kuzey-Yukarı) dönüşümüyle gerçek Az/El hesaplar
    fn compute_az_el(station_lat: f64, station_lon: f64, sat_ecef: [f64; 3]) -> (f64, f64) {
        let lat_rad = station_lat.to_radians();
        let lon_rad = station_lon.to_radians();
        let (sin_lat, cos_lat) = lat_rad.sin_cos();
        let (sin_lon, cos_lon) = lon_rad.sin_cos();

        let r_earth = 6378.137;
        let st_x = r_earth * cos_lat * cos_lon;
        let st_y = r_earth * cos_lat * sin_lon;
        let st_z = r_earth * sin_lat;

        let dx = sat_ecef[0] - st_x;
        let dy = sat_ecef[1] - st_y;
        let dz = sat_ecef[2] - st_z;

        let east = -sin_lon * dx + cos_lon * dy;
        let north = -sin_lat * cos_lon * dx - sin_lat * sin_lon * dy + cos_lat * dz;
        let up = cos_lat * cos_lon * dx + cos_lat * sin_lon * dy + sin_lat * dz;

        let range = (east * east + north * north + up * up).sqrt().max(1e-6);
        let az_deg = east.atan2(north).to_degrees().rem_euclid(360.0);
        let el_deg = (up / range).asin().to_degrees().clamp(-90.0, 90.0);
        (az_deg, el_deg)
    }

    /// SGP4 orbital mekanik modeliyle uyduların anlık telemetrisini ve istasyon geçişlerini hesaplar
    pub async fn fetch_satellites(&self, station_lat: f64, station_lon: f64) -> anyhow::Result<Vec<SatellitePass>> {
        let now = Utc::now();
        let live_iss = self.fetch_live_iss().await;
        let tles = self.get_live_or_catalog_tles().await;
        Ok(self.propagate_entries_at(&tles, station_lat, station_lon, now, live_iss.as_ref()))
    }

    #[cfg(test)]
    fn propagate_catalog_at(
        &self,
        station_lat: f64,
        station_lon: f64,
        now: DateTime<Utc>,
        live_iss: Option<&WhereTheIssResp>,
    ) -> Vec<SatellitePass> {
        let entries: Vec<DynamicTleEntry> = TLE_CATALOG
            .iter()
            .map(|e| DynamicTleEntry {
                norad_id: e.norad_id,
                name: e.name.to_string(),
                line1: e.line1.to_string(),
                line2: e.line2.to_string(),
            })
            .collect();
        self.propagate_entries_at(&entries, station_lat, station_lon, now, live_iss)
    }

    fn propagate_entries_at(
        &self,
        entries: &[DynamicTleEntry],
        station_lat: f64,
        station_lon: f64,
        now: DateTime<Utc>,
        live_iss: Option<&WhereTheIssResp>,
    ) -> Vec<SatellitePass> {
        let mut passes = Vec::new();

        for entry in entries.iter() {
            if let Ok(elements) = Elements::from_tle(
                Some(entry.name.clone()),
                entry.line1.as_bytes(),
                entry.line2.as_bytes(),
            ) {
                if let Ok(constants) = Constants::from_elements(&elements) {
                    let epoch = elements.datetime;
                    let diff_mins = (now.naive_utc() - epoch).num_milliseconds() as f64 / 60000.0;

                    if let Ok(prediction) = constants.propagate(diff_mins) {
                        let mut ecef = Self::teme_to_ecef(prediction.position, now);
                        let r = (ecef[0].powi(2) + ecef[1].powi(2) + ecef[2].powi(2)).sqrt();
                        let mut alt_km = (r - 6378.137).max(100.0);
                        let mut lat = (ecef[2] / r.max(1e-6)).asin().to_degrees();
                        let mut lon = ecef[1].atan2(ecef[0]).to_degrees();
                        let mut vel_kms = (prediction.velocity[0].powi(2)
                            + prediction.velocity[1].powi(2)
                            + prediction.velocity[2].powi(2))
                        .sqrt();

                        if entry.norad_id == 25544 {
                            if let Some(iss) = live_iss {
                                lat = iss.latitude;
                                lon = iss.longitude;
                                alt_km = iss.altitude;
                                vel_kms = iss.velocity / 3600.0;
                                let r_iss = 6378.137 + alt_km;
                                let (sin_lat, cos_lat) = lat.to_radians().sin_cos();
                                let (sin_lon, cos_lon) = lon.to_radians().sin_cos();
                                ecef = [r_iss * cos_lat * cos_lon, r_iss * cos_lat * sin_lon, r_iss * sin_lat];
                            }
                        }

                        let (az, el) = Self::compute_az_el(station_lat, station_lon, ecef);

                        // Gerçek SGP4 ileri zaman taraması (24 saate kadar 2 dk adımlarla gerçek ufuk üstü geçiş anını bulur)
                        let next_pass_secs = if el > 0.0 {
                            0
                        } else {
                            let mut found_secs = 0u64;
                            for m in (2..=1440).step_by(2) {
                                if let Ok(fut_pred) = constants.propagate(diff_mins + m as f64) {
                                    let fut_dt = now + chrono::Duration::minutes(m);
                                    let fut_ecef = Self::teme_to_ecef(fut_pred.position, fut_dt);
                                    let (_, fut_el) = Self::compute_az_el(station_lat, station_lon, fut_ecef);
                                    if fut_el > 0.0 {
                                        found_secs = (m as u64) * 60;
                                        break;
                                    }
                                }
                            }
                            found_secs
                        };

                        let mut ground_track = Vec::with_capacity(19);
                        ground_track.push((lon, lat));
                        for m in (5..=90).step_by(5) {
                            if let Ok(fut_pred) = constants.propagate(diff_mins + m as f64) {
                                let fut_dt = now + chrono::Duration::minutes(m);
                                let fut_ecef = Self::teme_to_ecef(fut_pred.position, fut_dt);
                                let r_f = (fut_ecef[0].powi(2) + fut_ecef[1].powi(2) + fut_ecef[2].powi(2)).sqrt().max(1e-6);
                                let f_lat = (fut_ecef[2] / r_f).asin().to_degrees();
                                let f_lon = fut_ecef[1].atan2(fut_ecef[0]).to_degrees();
                                ground_track.push((f_lon, f_lat));
                            }
                        }

                        passes.push(SatellitePass {
                            norad_id: entry.norad_id,
                            name: entry.name.clone(),
                            lat,
                            lon,
                            alt_km,
                            velocity_kms: vel_kms,
                            az_deg: az,
                            el_deg: el,
                            next_pass_secs,
                            pass_duration_secs: if entry.norad_id == 47306 { 86400 } else { 580 },
                            ground_track,
                        });
                    }
                }
            }
        }

        passes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turksat_5a_geostationary_and_topocentric_el() {
        let client = CelesTrakClient::default();
        let sats = client.propagate_catalog_at(41.0082, 28.9784, Utc::now(), None);
        assert!(!sats.is_empty(), "Propagated satellites should not be empty");

        let turksat = sats
            .iter()
            .find(|s| s.norad_id == 47306)
            .expect("TURKSAT 5A (NORAD 47306) must be present");
        assert!(
            turksat.el_deg > 20.0,
            "TURKSAT 5A elevation from Istanbul should be > 20.0 deg, got {:.2}",
            turksat.el_deg
        );
        assert_eq!(turksat.next_pass_secs, 0, "TURKSAT 5A is geostationary over Turkey (active pass)");

        let all_above_horizon = sats.iter().all(|s| s.el_deg > 0.0);
        assert!(
            !all_above_horizon,
            "Not all LEO/GEO satellites should simultaneously have el_deg > 0.0 from a single station"
        );
    }

    #[test]
    fn test_recon_and_sar_catalog_entries_propagate() {
        let client = CelesTrakClient::default();
        let sats = client.propagate_catalog_at(41.0082, 28.9784, Utc::now(), None);
        for expected in [
            "SENTINEL-1A",
            "TERRASAR-X",
            "LANDSAT 8",
            "GOKTURK 1A",
            "GOKTURK 2",
            "COSMOS 2558",
            "NOAA 20",
        ] {
            assert!(
                sats.iter().any(|s| s.name.contains(expected)),
                "Missing expected satellite {} in propagated catalog",
                expected
            );
        }
    }
}
