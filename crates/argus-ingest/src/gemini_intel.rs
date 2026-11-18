use argus_core::geo::haversine_km;
use argus_core::state::AppState;
use std::collections::BTreeMap;
use std::time::Duration;

const DEFAULT_GEMINI_MODEL: &str = "gemini-3.8-flash";
const FALLBACK_GEMINI_MODELS: &[&str] = &[
    "gemini-3.8-flash",
    "gemini-3-flash-preview",
    "gemini-3.5-flash-lite",
    "gemini-3.1-flash-lite",
    "gemini-flash-lite-latest",
];

/// Canlı telemetri durumundan tüm istihbarat disiplinlerini kapsayan kapsamlı taktik SITREP bağlamı hazırlar
pub fn build_tactical_prompt(state: &AppState) -> String {
    let st_lat = state.station_lat;
    let st_lon = state.station_lon;
    let now_utc = chrono::Utc::now().format("%Y-%m-%d %H:%M UTC");

    // 1. OSINT DEFCON
    let defcon_line = if let Some(d) = &state.defcon_status {
        let cmds: Vec<String> = d
            .commands
            .iter()
            .map(|(c, l)| format!("{}:D{}", c, l))
            .collect();
        format!(
            "OSINT DEFCON {} (Kaynak: {}, Guncelleme: {}) | Komutanliklar: {}",
            d.overall_level,
            d.source,
            d.updated_at.format("%H:%M UTC"),
            if cmds.is_empty() {
                "-".to_string()
            } else {
                cmds.join(", ")
            }
        )
    } else {
        "OSINT DEFCON verisi henuz alinmadi".to_string()
    };

    // 2. Seçili / Takip Edilen Hedef + A-2 Rota Bilgisi
    let selected_line = if let Some(sel_id) = &state.selected_target_id {
        if let Some(t) = state
            .air_targets
            .iter()
            .chain(state.sea_targets.iter())
            .find(|x| &x.id == sel_id)
        {
            let d = haversine_km(st_lat, st_lon, t.geo.lat, t.geo.lon);
            let cs_key = t.callsign.trim().to_uppercase();
            let sched_txt = state
                .flight_schedule_cache
                .get(&cs_key)
                .map(|s| {
                    format!(
                        " | Rota: {}->{} ({})",
                        s.origin_code.as_deref().unwrap_or("?"),
                        s.dest_code.as_deref().unwrap_or("?"),
                        s.status.as_deref().unwrap_or("-")
                    )
                })
                .unwrap_or_default();
            format!(
                "{} [{}] ({}, {}) | {:.4},{:.4} ({:.0}km) | Irtifa:{:.0}m Hiz:{:.0}kt Rota:{:.0}deg Squawk:{}{}",
                t.callsign,
                t.id,
                t.airframe_or_vessel_class.as_deref().unwrap_or(t.domain.as_str()),
                t.operator_branch
                    .as_deref()
                    .or(t.operator_country.as_deref())
                    .unwrap_or("-"),
                t.geo.lat,
                t.geo.lon,
                d,
                t.geo.alt_m.unwrap_or(0.0),
                t.speed_kts.unwrap_or(0.0),
                t.heading_deg.unwrap_or(0.0),
                t.squawk.as_deref().unwrap_or("-"),
                sched_txt
            )
        } else {
            format!("Secili ID: {}", sel_id)
        }
    } else {
        "Yok".to_string()
    };

    // 3. Otomatik Alarmlar & İzleme Listesi
    let watchlist_vec: Vec<String> = state.watchlist.iter().take(15).cloned().collect();
    let alert_vec: Vec<_> = state.alerts.iter().collect();
    let alerts_txt: Vec<String> = alert_vec
        .into_iter()
        .rev()
        .take(12)
        .map(|a| {
            format!(
                "[{}|{}|{}] {}",
                a.timestamp.format("%H:%M"),
                a.severity.as_str(),
                a.domain,
                a.message
            )
        })
        .collect();

    // 4. AIRINT (AOR dağılımı, Acil Squawk 7500/7600/7700, Öncelikli 25 Askeri/Anomalili Uçuş + AAR)
    let mil_air = state.air_targets.iter().filter(|t| t.is_military).count();
    let mut aor_counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut emerg_flights = Vec::new();
    let mut air_prioritized: Vec<(u8, f64, String)> = Vec::new();
    for t in state.air_targets.iter() {
        let aor = t.origin.as_deref().unwrap_or("KURESEL");
        *aor_counts.entry(aor).or_insert(0) += 1;
        let sq = t.squawk.as_deref().unwrap_or("");
        let is_emerg = matches!(sq, "7500" | "7600" | "7700");
        let anom = state.track_anomalies.get(&t.id);
        let d = haversine_km(st_lat, st_lon, t.geo.lat, t.geo.lon);
        let cs_key = t.callsign.trim().to_uppercase();
        let sched = state
            .flight_schedule_cache
            .get(&cs_key)
            .map(|s| {
                format!(
                    " [{}>{}]",
                    s.origin_code.as_deref().unwrap_or("?"),
                    s.dest_code.as_deref().unwrap_or("?")
                )
            })
            .unwrap_or_default();
        if is_emerg {
            emerg_flights.push(format!(
                "{} SQ:{} ({:.2},{:.2} {:.0}km){}",
                t.callsign, sq, t.geo.lat, t.geo.lon, d, sched
            ));
        }
        if is_emerg || anom.is_some() || t.is_military {
            let prio = if is_emerg {
                0
            } else if anom.is_some() {
                1
            } else {
                2
            };
            let fl = (t.geo.alt_m.unwrap_or(0.0) * 3.28084 / 100.0).round() as i64;
            air_prioritized.push((
                prio,
                d,
                format!(
                    "{} ({}/{}) {:.2},{:.2} {:.0}km FL{} {:.0}kt {} [{}{}{}]",
                    t.callsign,
                    t.airframe_or_vessel_class.as_deref().unwrap_or("Mil"),
                    t.operator_branch
                        .as_deref()
                        .or(t.operator_country.as_deref())
                        .unwrap_or("-"),
                    t.geo.lat,
                    t.geo.lon,
                    d,
                    fl.max(0),
                    t.speed_kts.unwrap_or(0.0),
                    aor,
                    anom.map(|a| format!("ANOM:{}", a)).unwrap_or_else(|| "MIL".to_string()),
                    if sq.is_empty() {
                        String::new()
                    } else {
                        format!(" SQ:{}", sq)
                    },
                    sched
                ),
            ));
        }
    }
    air_prioritized.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });
    let aor_summary: Vec<String> = aor_counts
        .into_iter()
        .map(|(k, v)| format!("{}:{}", k, v))
        .collect();
    let top_air: Vec<String> = air_prioritized.into_iter().take(25).map(|x| x.2).collect();

    // 4.5 Taktik Füzyon Özetleri (DARK-SHIP, BOGAZ, AAR, RECON, EVI)
    let aar_list: Vec<String> = state
        .aar_pairs
        .iter()
        .take(10)
        .map(|p| {
            format!(
                "{}<->{} @ {:.2},{:.2} FL{:.0} ({:.1}km{})",
                p.tanker_callsign,
                p.receiver_callsign,
                p.lat,
                p.lon,
                p.alt_ft / 100.0,
                p.dist_km,
                if p.is_locked { " KILITLI" } else { "" }
            )
        })
        .collect();
    let dark_list: Vec<String> = state
        .dark_ships
        .iter()
        .take(10)
        .map(|d| {
            format!(
                "{} [{}] Son:{:.2},{:.2}->DR:{:.2},{:.2} {:.1}kt/{:.0}deg ({}dk{})",
                d.callsign,
                d.id,
                d.last_lat,
                d.last_lon,
                d.dr_lat,
                d.dr_lon,
                d.sog_kts,
                d.cog_deg,
                d.silent_mins,
                if d.is_military { " ASKERI" } else { "" }
            )
        })
        .collect();
    let strait_list: Vec<String> = state
        .strait_gates
        .iter()
        .map(|g| {
            format!(
                "{} ({}:{}/{} Mil:{} Tanker:{})",
                g.name, g.fwd_label, g.fwd_count, g.rev_count, g.mil_count, g.tanker_count
            )
        })
        .collect();
    let recon_list: Vec<String> = state
        .recon_windows
        .iter()
        .take(10)
        .map(|r| {
            format!(
                "{} [{}] -> {} ({:.2},{:.2}) AOS:{}dk El:{:.0}deg{}",
                r.sat_name,
                r.sensor_type,
                r.target_name,
                r.target_lat,
                r.target_lon,
                r.aos_secs / 60,
                r.max_el_deg,
                if r.cloud_blocked { " BULUT-ENGELLI" } else { " ACIK" }
            )
        })
        .collect();
    let evi_list: Vec<String> = state
        .escalation_metrics
        .iter()
        .take(10)
        .map(|e| {
            format!(
                "{} ({}) EVI:{} ({:+}%) Olay:{}",
                e.country_name, e.country_code, e.evi_score, e.delta_pct, e.event_count
            )
        })
        .collect();

    // 5. SEAINT (Harp/Sahil Güvenlik/Anomali + En Yakın 15 Gemi)
    let mil_sea = state.sea_targets.iter().filter(|t| t.is_military).count();
    let mut sea_prioritized: Vec<(u8, f64, String)> = Vec::new();
    for t in state.sea_targets.iter() {
        let d = haversine_km(st_lat, st_lon, t.geo.lat, t.geo.lon);
        let anom = state.track_anomalies.get(&t.id);
        let prio = if anom.is_some() {
            0
        } else if t.is_military {
            1
        } else {
            2
        };
        sea_prioritized.push((
            prio,
            d,
            format!(
                "{} ({}/{}) {:.2},{:.2} {:.0}km {:.1}kt Hdg:{:.0}{}",
                t.callsign,
                t.vessel_type
                    .as_deref()
                    .or(t.airframe_or_vessel_class.as_deref())
                    .unwrap_or("Gemi"),
                t.operator_country.as_deref().unwrap_or("-"),
                t.geo.lat,
                t.geo.lon,
                d,
                t.speed_kts.unwrap_or(0.0),
                t.heading_deg.unwrap_or(0.0),
                anom.map(|a| format!(" [{}]", a)).unwrap_or_default()
            ),
        ));
    }
    sea_prioritized.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });
    let top_sea: Vec<String> = sea_prioritized.into_iter().take(15).map(|x| x.2).collect();

    // 6. GEOINT (GPS-JAM, Anomali, K-4 Füzyon)
    let jam_zones: Vec<String> = state
        .gps_jamming_zones
        .iter()
        .take(15)
        .map(|(lat, lon, r, lbl)| format!("{} ({:.2},{:.2} r={:.0}km)", lbl, lat, lon, r))
        .collect();
    let anom_list: Vec<String> = state
        .track_anomalies
        .iter()
        .take(15)
        .map(|(id, desc)| format!("{}: {}", id, desc))
        .collect();
    let fusions: Vec<String> = state
        .fusion_clusters
        .iter()
        .take(10)
        .map(|f| {
            format!(
                "({:.2},{:.2}) Skor:{} [{}]: {}",
                f.lat,
                f.lon,
                f.score,
                f.domains.join("+"),
                f.summary
            )
        })
        .collect();

    // 7. SATINT (SGP4 Uydular)
    let above_horizon = state.satellites.iter().filter(|s| s.el_deg > 0.0).count();
    let mut sats_sorted: Vec<_> = state.satellites.iter().collect();
    sats_sorted.sort_by(|a, b| {
        let a_up = if a.el_deg > 0.0 { 0 } else { 1 };
        let b_up = if b.el_deg > 0.0 { 0 } else { 1 };
        a_up.cmp(&b_up).then_with(|| a.next_pass_secs.cmp(&b.next_pass_secs))
    });
    let top_sats: Vec<String> = sats_sorted
        .into_iter()
        .take(15)
        .map(|s| {
            format!(
                "{} (#{}) {:.2},{:.2} Alt:{:.0}km El:{:.1}deg AOS:{}dk",
                s.name,
                s.norad_id,
                s.lat,
                s.lon,
                s.alt_km,
                s.el_deg,
                s.next_pass_secs / 60
            )
        })
        .collect();

    // 8. SIGINT (KiwiSDR + DSP Tarama/CW + EAM)
    let sdr_line = format!(
        "Aktif:{}, {}kHz {}, RSSI:{:.1}dBm, SNR:{:+.1}dB, Dinleme:{}, Tarama:{}, VOX:{}, CW:{}{}, KayitliNode:{}",
        if state.sdr_connected_host.is_empty() {
            "Bagli Degil"
        } else {
            &state.sdr_connected_host
        },
        state.current_frequency_khz,
        state.sdr_mode,
        state.sdr_rssi_dbm,
        state.sdr_snr_db,
        if state.is_radio_playing { "ACIK" } else { "KAPALI" },
        if state.sdr_scanner_enabled { "ACIK" } else { "KAPALI" },
        if state.sdr_vox_enabled { "ACIK" } else { "KAPALI" },
        if state.sdr_cw_enabled { "ACIK" } else { "KAPALI" },
        if state.sdr_cw_text.trim().is_empty() {
            String::new()
        } else {
            format!(" [{}WPM:'{}']", state.sdr_cw_wpm, state.sdr_cw_text.trim())
        },
        state.sdr_nodes.len()
    );
    let scan_hits: Vec<String> = state
        .sdr_scan_hits
        .iter()
        .rev()
        .take(8)
        .map(|h| {
            format!(
                "{} {:.1}kHz {} (+{:.1}dB @ {})",
                h.timestamp.format("%H:%M"),
                h.frequency_khz,
                h.mode,
                h.snr_db,
                h.host_port
            )
        })
        .collect();
    let eam_vec: Vec<_> = state.eam_messages.iter().collect();
    let eams: Vec<String> = eam_vec
        .into_iter()
        .rev()
        .take(10)
        .map(|e| {
            let preview: String = e.content.chars().take(42).collect();
            format!(
                "{} {:.1}kHz [{}] {} ({}{}) \"{}\"",
                e.timestamp.format("%H:%M"),
                e.frequency_khz,
                e.mode,
                e.preamble,
                e.auth_station,
                if e.is_flash_override { "|FLASH" } else { "" },
                preview
            )
        })
        .collect();

    // 9. MASINT (Deprem + NASA FIRMS VIIRS + Safecast CBRN + NOAA Uzay Havası + OpenWeatherMap)
    let mut quakes_sorted: Vec<_> = state.earthquakes.iter().collect();
    quakes_sorted.sort_by(|a, b| b.mag.partial_cmp(&a.mag).unwrap_or(std::cmp::Ordering::Equal));
    let top_quakes: Vec<String> = quakes_sorted
        .into_iter()
        .take(12)
        .map(|q| {
            format!(
                "M{:.1} {} ({:.2},{:.2} d={:.0}km @ {})",
                q.mag,
                q.place,
                q.lat,
                q.lon,
                q.depth_km,
                q.time.format("%H:%M")
            )
        })
        .collect();

    let firms_total = state.firms_fires.len();
    let firms_max_k = state
        .firms_fires
        .iter()
        .map(|f| f.brightness)
        .fold(0.0_f64, f64::max);
    let firms_hot_350 = state.firms_fires.iter().filter(|f| f.brightness >= 350.0).count();
    let mut firms_sorted: Vec<_> = state.firms_fires.iter().collect();
    firms_sorted.sort_by(|a, b| {
        b.brightness
            .partial_cmp(&a.brightness)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let top_firms: Vec<String> = firms_sorted
        .into_iter()
        .take(15)
        .map(|f| {
            format!(
                "({:.2},{:.2}) {:.1}K [conf:{}] {} {} {}",
                f.lat, f.lon, f.brightness, f.confidence, f.satellite, f.acq_date, f.acq_time
            )
        })
        .collect();

    let rad_total = state.radiation_measurements.len();
    let mut rad_sorted: Vec<_> = state.radiation_measurements.iter().collect();
    rad_sorted.sort_by(|a, b| {
        b.value
            .partial_cmp(&a.value)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let top_rad: Vec<String> = rad_sorted
        .into_iter()
        .take(10)
        .map(|r| {
            format!(
                "{:.3} {} @ {} ({:.2},{:.2})",
                r.value,
                r.unit,
                r.location_name.as_deref().unwrap_or("Istasyon"),
                r.lat,
                r.lon
            )
        })
        .collect();

    let solar_line = if let Some(sw) = &state.solar_weather {
        format!(
            "Kp:{:.1} | Firtina:{} | X-Ray:{} ({})",
            sw.kp_index,
            sw.storm_level,
            sw.flare_class,
            sw.updated.format("%H:%M UTC")
        )
    } else {
        "Veri yok".to_string()
    };

    let mut wx_all: Vec<String> = Vec::new();
    if let Some(wx) = &state.station_weather {
        wx_all.push(format!(
            "[ISTASYON {}]: {:.1}C Ruzgar:{:.0}kt/{:.0}deg Bulut:%{} ({})",
            wx.name, wx.temp_c, wx.wind_kts, wx.wind_deg, wx.clouds_pct, wx.condition
        ));
    }
    for wp in state.weather_points.iter().take(10) {
        wx_all.push(format!(
            "{} ({:.2},{:.2}): {:.1}C Ruzgar:{:.0}kt Bulut:%{} ({})",
            wp.name, wp.lat, wp.lon, wp.temp_c, wp.wind_kts, wp.clouds_pct, wp.condition
        ));
    }

    // 10. CYBERINT (CISA KEV, IODA, Shodan)
    let cyber_vec: Vec<_> = state.cyber_incidents.iter().collect();
    let top_cyber: Vec<String> = cyber_vec
        .iter()
        .rev()
        .take(12)
        .map(|c| {
            format!(
                "[{}|{}] {} -> {}",
                c.event_type,
                c.severity.as_str(),
                c.title,
                c.target
            )
        })
        .collect();
    let shodan_list: Vec<String> = state
        .shodan_recon_cache
        .values()
        .take(6)
        .map(|h| {
            format!(
                "{} ({}/{}) Port:{:?} Vuln:{}",
                h.ip,
                h.country.as_deref().unwrap_or("-"),
                h.isp.as_deref().unwrap_or("-"),
                &h.ports[..h.ports.len().min(6)],
                h.vulns.len()
            )
        })
        .collect();

    // 11. ACLED / GDELT Çatışma & OSINT Haberler
    let acled_top: Vec<String> = state
        .acled_events
        .iter()
        .take(15)
        .map(|e| {
            format!(
                "{} [{}/{}] {} ({} vs {}) ({:.2},{:.2}) Kayip:{}",
                e.event_date,
                e.country,
                e.location,
                e.sub_event_type,
                e.actor1,
                e.actor2.as_deref().unwrap_or("-"),
                e.lat,
                e.lon,
                e.fatalities
            )
        })
        .collect();
    let news_vec: Vec<_> = state.news.iter().collect();
    let top_news: Vec<String> = news_vec
        .iter()
        .rev()
        .take(20)
        .map(|n| {
            format!(
                "{} [{}|{}] {}",
                n.published.format("%H:%M"),
                n.source,
                n.category,
                n.title
            )
        })
        .collect();

    let notam_list: Vec<String> = state
        .airspace_warnings
        .iter()
        .take(10)
        .map(|w| format!("[{}] {}: {}", w.kind, w.fir_or_zone, w.hazard))
        .collect();

    format!(
        "Sen Project Sentinel (ARGUS C4ISR) kidemli jeopolitik, savunma ve stratejik istihbarat bas analistisin (`Senior Strategic Intelligence Analyst`).\n\
        Gorevin YALNIZCA anlik sensor sayilarini listelemek degil; asagida sistemdeki TUM aktif sensor ve istihbarat disiplinlerinden gelen CANLI telemetriyi, GENEL JEOPOLITIK BAGLAM, askeri doktrin, kritik deniz/hava gecitleri (Bogazlar, Kizildeniz, Hurmuz, Tayvan, Baltik, Karadeniz, Levant) ve tarihsel tirmanma kaliplariyla BIRLESTIREREK gelecege yonelik olasi savas/silahli catisma riski tasiyan noktalari (`Flashpoints`) ayrintili, somut ve ongoru odakli sekilde analiz etmektir.\n\n\
        ANALITIK ILKELER:\n\
        - 'Gozlemlenen Sensor Verisi' ile 'Stratejik Ongoru / Senaryo' ifadelerini acikca ayirt et; sensor sayilarinda ve cagri adlarinda kesinlikle veri uydurma.\n\
        - Komutan gibi kisa emir cumleleri yerine bir istihbarat analisti gibi neden-sonuc, kuvvet konuslanmasi, lojistik/tanker/ISR isareti ve erken uyari gostergeleri uzerinden derinlemesine yorum yap.\n\
        - Verisi 'Yok' veya 0 olan alanlari yalnizca tek satirda not et, asil agirligi aktif verilere ve yuksek riskli bolgelere ver.\n\n\
        ISTENEN RAPOR YAPISI (TURKCE):\n\
        1. STRATEJIK DURUM OZETI & DEFCON COZUMLEMESI (Kuresel/bolgesel DEFCON seviyelerinin mevcut askeri, siber ve OSINT gundemiyle stratejik baglantisi)\n\
        2. OLASI SAVAS / CATISMA RISK NOKTALARI (`KEY FLASHPOINTS` — En az 5 somut nokta)\n\
           Her bir nokta icin su sablonu kullan:\n\
           * [NOKTA X: Bolge / Kritik Gecit & Koordinat veya Saha] | Risk Skoru: XX/100 (KRITIK/YUKSEK/ORTA) | Vade: 24-72s / 1-4 hafta\n\
             - Gozlemlenen Canli Iz: (Bu sahadaki gercek ADS-B askeri/tanker/ISR, AIS gemi/anomali, GDELT catisma, GPS-JAM, FIRMS veya OSINT bulgulari)\n\
             - Jeopolitik Dinamik & Tetikleyici Senaryo: (Genel jeopolitik gerilimle birlestirilmis olasi catisma veya savas tirmanma senaryosu)\n\
             - Erken Uyari Gostergeleri (Points / Watchlist): (Durumun savas/catisma esigine gectigini kanitlayacak 2-3 somut on gosterge)\n\
        3. COKLU-ALAN ETKILESIMI (K-1 Denizalti Kablo Kirilganligi, GPS Karistirma, Siber/IODA BGP, NASA FIRMS Termal & MASINT Etkileri)\n\
        4. GELECEK SENARYO MATRISI & IZLEME NOKTALARI\n\
           * En Olasi Senaryo (Baz) vs En Tehlikeli Tirmanma Senaryosu (Worst-Case)\n\
           * Analist Izleme Listesi: Takip edilecek somut cagri adi / bogaz / frekans / bolge noktalari (4-6 madde)\n\n\
        === 1. KOMUTA & KONUM ===\n\
        Zaman: {} | Istasyon: {} ({}) @ {:.4}N, {:.4}E | Harita Odak: {:.2}N, {:.2}E (Zoom:{:.1}x) | KatmanMask: {:#06x}\n\
        Secili Hedef: {}\n\
        {}\n\n\
        === 2. ALARMLAR & WATCHLIST ===\n\
        Watchlist ({}): {}\n\
        Son Alarmlar ({}): {}\n\n\
        === 3. AIRINT (ADS-B) ===\n\
        Toplam Ucak: {} | Askeri/ISR: {} | AOR Dagilimi: {}\n\
        Acil Squawk (7500/7600/7700): {}\n\
        Oncelikli Askeri/Anomalili Ucuslar: {}\n\
        AAR Hava Yakit Ikmal Paketleri ({}): {}\n\n\
        === 4. SEAINT (AIS) ===\n\
        Toplam Gemi: {} | Harp/Sahil Guv: {}\n\
        Oncelikli/Yakin Gemiler: {}\n\
        DARK-SHIP (AIS Karartma/DR) ({}): {}\n\
        BOGAZ Kapilari (Gatekeeper) ({}): {}\n\n\
        === 5. GEOINT / KABLO / GPS-JAM / NOTAM / FUZYON ===\n\
        K-1 Denizalti Kablo Sayisi: {}\n\
        N-4 Hava Sahasi / NOTAM / SIGMET ({}): {}\n\
        GPS Jamming Bolgeleri ({}): {}\n\
        Parkur Anomalileri ({}): {}\n\
        K-4 Coklu-Sensor Fuzyon Kumeleri ({}): {}\n\n\
        === 6. SATINT (CelesTrak SGP4) ===\n\
        Toplam Uydu: {} (Ufuk Ustu: {}) | Gorunum/Yakin Gecisler: {}\n\
        RECON Uydu Kesif Pencereleri ({}): {}\n\n\
        === 7. SIGINT (KiwiSDR + HFGCS EAM) ===\n\
        Tuner/DSP: {}\n\
        Bant Tarama Yakalamalari ({}): {}\n\
        HFGCS EAM/SKYKING ({}): {}\n\n\
        === 8. MASINT (JEOSISMIK / TERMAL / CBRN / UZAY / METEO) ===\n\
        Depremler ({}): {}\n\
        NASA FIRMS VIIRS Termal (Toplam:{}, Max:{:.1}K, >=350K:{}): {}\n\
        Safecast CBRN Radyasyon ({}): {}\n\
        NOAA Uzay Havasi: {}\n\
        Meteoroloji (OWM): {}\n\n\
        === 9. CYBERINT & CATISMA & OSINT ===\n\
        CISA KEV / IODA Siber ({}): {}\n\
        Shodan Kesif Onbellegi ({}): {}\n\
        ACLED Catisma Olaylari ({}): {}\n\
        EVI Tirmanma Indeksi (GDELT) ({}): {}\n\
        OSINT Haber Akisi ({}): {}",
        now_utc,
        state.config.station.callsign,
        state.config.station.name,
        st_lat,
        st_lon,
        state.map_center_lat,
        state.map_center_lon,
        state.map_zoom,
        state.geoint_layers,
        selected_line,
        defcon_line,
        state.watchlist.len(),
        if watchlist_vec.is_empty() { "Yok".to_string() } else { watchlist_vec.join(", ") },
        state.alerts.len(),
        if alerts_txt.is_empty() { "Yok".to_string() } else { alerts_txt.join(" | ") },
        state.air_targets.len(),
        mil_air,
        if aor_summary.is_empty() { "Yok".to_string() } else { aor_summary.join(", ") },
        if emerg_flights.is_empty() { "Yok".to_string() } else { emerg_flights.join(" | ") },
        if top_air.is_empty() { "Yok".to_string() } else { top_air.join(" | ") },
        state.aar_pairs.len(),
        if aar_list.is_empty() { "Yok".to_string() } else { aar_list.join(" | ") },
        state.sea_targets.len(),
        mil_sea,
        if top_sea.is_empty() { "Yok".to_string() } else { top_sea.join(" | ") },
        state.dark_ships.len(),
        if dark_list.is_empty() { "Yok".to_string() } else { dark_list.join(" | ") },
        state.strait_gates.len(),
        if strait_list.is_empty() { "Yok".to_string() } else { strait_list.join(" | ") },
        state.subsea_cables.len(),
        state.airspace_warnings.len(),
        if notam_list.is_empty() { "Yok".to_string() } else { notam_list.join(" | ") },
        state.gps_jamming_zones.len(),
        if jam_zones.is_empty() { "Yok".to_string() } else { jam_zones.join(" | ") },
        state.track_anomalies.len(),
        if anom_list.is_empty() { "Yok".to_string() } else { anom_list.join(" | ") },
        state.fusion_clusters.len(),
        if fusions.is_empty() { "Yok".to_string() } else { fusions.join(" | ") },
        state.satellites.len(),
        above_horizon,
        if top_sats.is_empty() { "Yok".to_string() } else { top_sats.join(" | ") },
        state.recon_windows.len(),
        if recon_list.is_empty() { "Yok".to_string() } else { recon_list.join(" | ") },
        sdr_line,
        state.sdr_scan_hits.len(),
        if scan_hits.is_empty() { "Yok".to_string() } else { scan_hits.join(" | ") },
        state.eam_messages.len(),
        if eams.is_empty() { "Yok".to_string() } else { eams.join(" | ") },
        state.earthquakes.len(),
        if top_quakes.is_empty() { "Yok".to_string() } else { top_quakes.join(" | ") },
        firms_total,
        firms_max_k,
        firms_hot_350,
        if top_firms.is_empty() { "Yok".to_string() } else { top_firms.join(" | ") },
        rad_total,
        if top_rad.is_empty() { "Yok".to_string() } else { top_rad.join(" | ") },
        solar_line,
        if wx_all.is_empty() { "Veri yok".to_string() } else { wx_all.join(" | ") },
        state.cyber_incidents.len(),
        if top_cyber.is_empty() { "Yok".to_string() } else { top_cyber.join(" | ") },
        state.shodan_recon_cache.len(),
        if shodan_list.is_empty() { "Yok".to_string() } else { shodan_list.join(" | ") },
        state.acled_events.len(),
        if acled_top.is_empty() { "Yok".to_string() } else { acled_top.join(" | ") },
        state.escalation_metrics.len(),
        if evi_list.is_empty() { "Yok".to_string() } else { evi_list.join(" | ") },
        state.news.len(),
        if top_news.is_empty() { "Yok".to_string() } else { top_news.join(" | ") },
    )
}

/// Seçilen analiz moduna (`0=Küresel Flashpoint`, `1=Seçili Hedef`, `2=Bölgesel Kriz`, `3=Özel Soru`, `4=Kırmızı Hücre OPFOR`)
/// göre özelleştirilmiş kapsamlı Gemini istemi oluşturur.
pub fn build_mode_prompt(state: &AppState, mode: usize, custom_question: &str) -> String {
    let base = build_tactical_prompt(state);
    match mode {
        1 => {
            let focus = if let Some(sel_id) = &state.selected_target_id {
                if let Some(t) = state
                    .air_targets
                    .iter()
                    .chain(state.sea_targets.iter())
                    .find(|x| &x.id == sel_id)
                {
                    let nearby_warns: Vec<String> = state
                        .airspace_warnings
                        .iter()
                        .filter(|w| {
                            haversine_km(t.geo.lat, t.geo.lon, w.center_lat, w.center_lon) <= 900.0
                        })
                        .take(5)
                        .map(|w| format!("[{}] {}: {}", w.kind, w.fir_or_zone, w.hazard))
                        .collect();
                    let nearby_conf: Vec<String> = state
                        .acled_events
                        .iter()
                        .filter(|c| haversine_km(t.geo.lat, t.geo.lon, c.lat, c.lon) <= 900.0)
                        .take(5)
                        .map(|c| format!("{} [{}] {}", c.event_date, c.event_type, c.location))
                        .collect();
                    format!(
                        "HEDEF ODAK: {} ({}) | Tip: {} | Operator: {} | {:.4}N,{:.4}E | Hiz:{:.0}kt | Irtifa:{:.0}m | Rota:{:.0}deg\n\
                        Yakin NOTAM/SIGMET/JAM: {}\n\
                        Yakin Catisma Olaylari: {}",
                        t.callsign,
                        t.id,
                        t.airframe_or_vessel_class.as_deref().or(t.vessel_type.as_deref()).unwrap_or("-"),
                        t.operator_branch.as_deref().or(t.operator_country.as_deref()).unwrap_or("-"),
                        t.geo.lat,
                        t.geo.lon,
                        t.speed_kts.unwrap_or(0.0),
                        t.geo.alt_m.unwrap_or(0.0),
                        t.heading_deg.unwrap_or(0.0),
                        if nearby_warns.is_empty() { "Yok".to_string() } else { nearby_warns.join(" | ") },
                        if nearby_conf.is_empty() { "Yok".to_string() } else { nearby_conf.join(" | ") },
                    )
                } else {
                    format!("Secili Hedef ID: {}", sel_id)
                }
            } else {
                "Harita veya tabloda secili hedef bulunmuyor; istasyona en yakin askeri/kritik hedefleri analiz et.".to_string()
            };
            format!(
                "[MOD 2: SECILI HEDEF DERIN ANALIZI]\n\
                Asagidaki odak hedefe ozel olarak Turkce analiz uret:\n\
                1. PLATFORM & DOKTRIN PROFILI (gorev tipi, kapasitesi, bagli oldugu kuvvet)\n\
                2. ROTA, KONUM & HAREKET NIYETI ANALIZI (olasi gorev, tanker/ISR/devriye senaryosu)\n\
                3. CEVRESEL TEHDIT & HAVA SAHASI RISKLERI (yakin NOTAM/GPS-JAM/catisma korelasyonu)\n\
                4. TAKIP & ERKEN UYARI KRITERLERI\n\n{}\n\n{}",
                focus, base
            )
        }
        2 => {
            let radius_km = 1500.0;
            let clat = state.map_center_lat;
            let clon = state.map_center_lon;
            let reg_mil = state
                .air_targets
                .iter()
                .filter(|t| t.is_military && haversine_km(clat, clon, t.geo.lat, t.geo.lon) <= radius_km)
                .count();
            let reg_sea = state
                .sea_targets
                .iter()
                .filter(|t| haversine_km(clat, clon, t.geo.lat, t.geo.lon) <= radius_km)
                .count();
            let reg_warns: Vec<String> = state
                .airspace_warnings
                .iter()
                .filter(|w| haversine_km(clat, clon, w.center_lat, w.center_lon) <= radius_km)
                .take(8)
                .map(|w| format!("[{}] {} - {}", w.kind, w.fir_or_zone, w.hazard))
                .collect();
            format!(
                "[MOD 3: BOLGESEL KRIZ & HAREKAT ANALIZI]\n\
                Analiz Merkez Sahasi: {:.2}N, {:.2}E (Yaricap: 1500 km) | Askeri Ucus: {} | Deniz Unsuru: {}\n\
                Saha NOTAM/SIGMET/GPS-JAM: {}\n\
                Yalnizca bu bolgesel harekat sahasina (ve komsu gecitlere) derinleserek Turkce olarak: kuvvet yogunlasmasi, olasi catisma/operasyon senaryolari (24-72s & 1-4 hafta), kritik altyapi/kablo/GPS etkileri ve izleme noktalarini raporla.\n\n{}",
                clat,
                clon,
                reg_mil,
                reg_sea,
                if reg_warns.is_empty() { "Yok".to_string() } else { reg_warns.join(" | ") },
                base
            )
        }
        3 => {
            let q = if custom_question.trim().is_empty() {
                "Mevcut verilere gore en acil savas/catisma riski nerededir?"
            } else {
                custom_question.trim()
            };
            format!(
                "[MOD 4: OZEL ISTIHBARAT SORUSU]\n\
                KULLANICI SORUSU: \"{}\"\n\n\
                Asagidaki canli coklu-sensor telemetrisini ve genel jeopolitik/askeri bilgini kullanarak yukaridaki soruyu ayrintili, somut verilere dayali ve madde madde Turkce olarak yanitla. Sensor verisinde olmayan seyi uydurma.\n\n{}",
                q, base
            )
        }
        4 => {
            let recon_summary: Vec<String> = state
                .recon_windows
                .iter()
                .take(10)
                .map(|r| {
                    format!(
                        "{} (#{}, {}) -> {} ({:.2},{:.2}) AOS:{}dk MaxEl:{:.0}deg [{}]",
                        r.sat_name,
                        r.norad_id,
                        r.sensor_type,
                        r.target_name,
                        r.target_lat,
                        r.target_lon,
                        r.aos_secs / 60,
                        r.max_el_deg,
                        if r.cloud_blocked {
                            "BULUT-KAPALI/KOR-PENCERE"
                        } else {
                            "ACIK-GORUS"
                        }
                    )
                })
                .collect();
            let ew_summary: Vec<String> = state
                .gps_jamming_zones
                .iter()
                .take(12)
                .map(|(lat, lon, r, lbl)| format!("{} ({:.2},{:.2} r={:.2})", lbl, lat, lon, r))
                .collect();
            let dark_summary: Vec<String> = state
                .dark_ships
                .iter()
                .take(12)
                .map(|d| {
                    format!(
                        "{} [{}] Son:{:.2},{:.2}->DR:{:.2},{:.2} {:.1}kt/{:.0}deg ({}dk{})",
                        d.callsign,
                        d.id,
                        d.last_lat,
                        d.last_lon,
                        d.dr_lat,
                        d.dr_lon,
                        d.sog_kts,
                        d.cog_deg,
                        d.silent_mins,
                        if d.is_military { " ASKERI" } else { "" }
                    )
                })
                .collect();
            let gate_summary: Vec<String> = state
                .strait_gates
                .iter()
                .map(|g| {
                    format!(
                        "{} ({}:{} / {}:{}, Mil:{}, Tanker:{})",
                        g.name,
                        g.fwd_label,
                        g.fwd_count,
                        g.rev_label,
                        g.rev_count,
                        g.mil_count,
                        g.tanker_count
                    )
                })
                .collect();
            let aar_summary: Vec<String> = state
                .aar_pairs
                .iter()
                .take(10)
                .map(|p| {
                    format!(
                        "{}<->{} @ {:.2},{:.2} FL{:.0} ({:.1}km{})",
                        p.tanker_callsign,
                        p.receiver_callsign,
                        p.lat,
                        p.lon,
                        p.alt_ft / 100.0,
                        p.dist_km,
                        if p.is_locked { " KILITLI" } else { "" }
                    )
                })
                .collect();
            let evi_summary: Vec<String> = state
                .escalation_metrics
                .iter()
                .take(10)
                .map(|e| {
                    format!(
                        "{} ({}) EVI:{} ({:+}%, Olay:{})",
                        e.country_name, e.country_code, e.evi_score, e.delta_pct, e.event_count
                    )
                })
                .collect();
            let shodan_summary: Vec<String> = state
                .shodan_recon_cache
                .values()
                .take(8)
                .map(|h| {
                    let cves: Vec<String> = h.vulns.iter().take(5).cloned().collect();
                    format!(
                        "{} ({}) Port:{:?} CVE({}):[{}] AltAlan:{}",
                        h.ip,
                        h.country.as_deref().unwrap_or("-"),
                        &h.ports[..h.ports.len().min(10)],
                        h.vulns.len(),
                        if cves.is_empty() {
                            "-".to_string()
                        } else {
                            cves.join(",")
                        },
                        h.subdomains.len()
                    )
                })
                .collect();

            format!(
                "[MOD 5: KIRMIZI HUCRE / OPFOR KARSI KUVVET KOMUTANI]\n\
                Sen simdi Mavi Kuvvet (Project Sentinel) karsisindaki Kirmizi Hucre / OPFOR Karsi Kuvvet Komutanisin.\n\
                Asagidaki canli istihbarat bosluklarini, uydu bulut/gecis pencerelerini, EW/GPS karistirma alanlarini, AIS karartan Dark-Ship gemilerini, bogaz kapilarini, Mavi AAR ikmal paketlerini, GDELT EVI gerilim merkezlerini ve Shodan acik port/CVE yuzeyini kullanarak Turkce olarak TAM 4 BASLIKLI analiz uret:\n\
                1. Mavi Kuvvet Kor Noktalari (uydu bulut/gecis bosluklari, EW karartma, radar ufku ve deniz karartma aciklari)\n\
                2. Kirmizi Hucre Sizma Vektorleri (Dark-Ship DR rotalari, bogaz/kablo sabotaji, Shodan acik port/CVE siber-fiziksel sizma ve yuksek EVI eksenleri)\n\
                3. EW/Aldatma (GNSS spoofing/jamming, Mavi AAR tanker/ISR paketlerine yonelik elektronik taarruz ve sahte iz manevralari)\n\
                4. Mavi Karsi Tedbir & Sensor Gorevlendirme (SAR uydu gecisi, KiwiSDR frekans tarama, Watchlist kilitleme, CAP devriyesi ve Siber/CVE kapatma adimlari)\n\n\
                === KIRMIZI HUCRE TAKTIK TELEMETRI OZETI ===\n\
                RECON Kesif Pencereleri ({}): {}\n\
                GPS-JAM / EW Bolgeleri ({}): {}\n\
                DARK-SHIP Kararan Gemiler ({}): {}\n\
                BOGAZ Gecis Kapilari ({}): {}\n\
                AAR Yakit Ikmal Paketleri ({}): {}\n\
                EVI Tirmanma Indeksi ({}): {}\n\
                SHODAN Acik Port / CVE Yuzeyi ({}): {}\n\n{}",
                state.recon_windows.len(),
                if recon_summary.is_empty() { "Yok".to_string() } else { recon_summary.join(" | ") },
                state.gps_jamming_zones.len(),
                if ew_summary.is_empty() { "Yok".to_string() } else { ew_summary.join(" | ") },
                state.dark_ships.len(),
                if dark_summary.is_empty() { "Yok".to_string() } else { dark_summary.join(" | ") },
                state.strait_gates.len(),
                if gate_summary.is_empty() { "Yok".to_string() } else { gate_summary.join(" | ") },
                state.aar_pairs.len(),
                if aar_summary.is_empty() { "Yok".to_string() } else { aar_summary.join(" | ") },
                state.escalation_metrics.len(),
                if evi_summary.is_empty() { "Yok".to_string() } else { evi_summary.join(" | ") },
                state.shodan_recon_cache.len(),
                if shodan_summary.is_empty() { "Yok".to_string() } else { shodan_summary.join(" | ") },
                base
            )
        }
        _ => base,
    }
}

/// Gemini Interactions API yanıtından düz metni ayıklar (`steps` ve `outputs` uyumlu)
pub fn parse_gemini_interactions_response(raw: &str) -> anyhow::Result<String> {
    let v: serde_json::Value = serde_json::from_str(raw)?;
    if let Some(err_msg) = v["error"]["message"].as_str() {
        anyhow::bail!("Gemini API Hatasi: {}", err_msg);
    }

    let mut collected = Vec::new();
    if let Some(steps) = v["steps"].as_array() {
        for step in steps {
            if step["type"].as_str() == Some("model_output") || step["type"].is_null() {
                if let Some(contents) = step["content"].as_array() {
                    for c in contents {
                        if let Some(txt) = c["text"].as_str() {
                            if !txt.trim().is_empty() {
                                collected.push(txt.trim().to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    if collected.is_empty() {
        if let Some(outputs) = v["outputs"].as_array() {
            for o in outputs {
                if let Some(txt) = o["text"].as_str() {
                    if !txt.trim().is_empty() {
                        collected.push(txt.trim().to_string());
                    }
                }
            }
        }
    }
    if collected.is_empty() {
        if let Some(txt) = v["output_text"].as_str() {
            if !txt.trim().is_empty() {
                collected.push(txt.trim().to_string());
            }
        }
    }

    if collected.is_empty() {
        anyhow::bail!("Gemini yanitindan metin cozumlenemedi");
    }
    Ok(collected.join("\n"))
}

pub async fn generate_ai_sitrep(
    api_key: &str,
    model_opt: Option<&str>,
    prompt: &str,
) -> anyhow::Result<String> {
    let key = api_key.trim();
    if key.is_empty() {
        anyhow::bail!("config.toml veya ARGUS_GEMINI_KEY icinde gemini_key tanimlanmamis");
    }
    let primary = model_opt
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_GEMINI_MODEL);

    let mut candidates: Vec<&str> = Vec::with_capacity(1 + FALLBACK_GEMINI_MODELS.len());
    candidates.push(primary);
    for &fb in FALLBACK_GEMINI_MODELS {
        if !candidates.contains(&fb) {
            candidates.push(fb);
        }
    }

    let client = reqwest::Client::builder()
        .local_address(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED))
        .http1_only()
        .connect_timeout(Duration::from_secs(6))
        .user_agent("Project-Sentinel/0.1")
        .build()?;

    let mut last_err = String::new();
    for (idx, model) in candidates.iter().enumerate() {
        let payload = serde_json::json!({
            "model": model,
            "input": prompt,
        });
        let req_timeout = match idx {
            0 if candidates.len() > 1 => Duration::from_secs(8),
            1 => Duration::from_secs(22),
            _ => Duration::from_secs(35),
        };

        let resp = match client
            .post("https://generativelanguage.googleapis.com/v1beta/interactions")
            .header("x-goog-api-key", key)
            .timeout(req_timeout)
            .json(&payload)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                last_err = format!("{} ag hatasi/zaman asimi: {:#}", model, e);
                continue;
            }
        };

        let status = resp.status();
        let body = match resp.text().await {
            Ok(b) => b,
            Err(e) => {
                last_err = format!("{} yanit okuma hatasi: {:#}", model, e);
                continue;
            }
        };
        if status.is_success() {
            let text = parse_gemini_interactions_response(&body)?;
            return if idx > 0 {
                Ok(format!("[Model: {}]\n{}", model, text))
            } else {
                Ok(text)
            };
        }

        let code = status.as_u16();
        last_err = format!("HTTP {} ({}): {}", status, model, &body[..body.len().min(220)]);
        if code == 503 || code == 429 || code == 404 {
            continue;
        }
        anyhow::bail!("{}", last_err);
    }

    anyhow::bail!("{}", last_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gemini_interactions_response() {
        let sample = r#"{
          "id":"v1_123",
          "model":"gemini-3.8-flash",
          "status":"completed",
          "steps":[{
            "type":"model_output",
            "content":[{"type":"text","text":"1. GENEL OZET: DEFCON 3 seviyesi izleniyor."}]
          }]
        }"#;
        let out = parse_gemini_interactions_response(sample).unwrap();
        assert!(out.contains("DEFCON 3"));
    }

    #[test]
    fn test_build_tactical_prompt_includes_all_domains() {
        let mut st = AppState::new(Default::default());
        st.firms_fires.push(argus_core::types::FirmsFire {
            lat: 36.5,
            lon: 35.5,
            brightness: 382.4,
            confidence: "h".to_string(),
            acq_date: "2026-11-17".to_string(),
            acq_time: "1200".to_string(),
            satellite: "N20".to_string(),
        });
        st.radiation_measurements.push(argus_core::types::RadiationMeasurement {
            id: 1,
            value: 0.14,
            unit: "uSv/h".to_string(),
            lat: 41.0,
            lon: 29.0,
            captured_at: None,
            location_name: Some("Istanbul".to_string()),
            device_id: None,
        });
        st.satellites.push(argus_core::types::SatellitePass {
            norad_id: 25544,
            name: "ISS".to_string(),
            lat: 41.0,
            lon: 29.0,
            alt_km: 420.0,
            velocity_kms: 7.66,
            az_deg: 180.0,
            el_deg: 45.0,
            next_pass_secs: 0,
            pass_duration_secs: 400,
            ground_track: vec![],
        });
        let prompt = build_tactical_prompt(&st);
        for required in [
            "=== 1. KOMUTA & KONUM ===",
            "=== 2. ALARMLAR & WATCHLIST ===",
            "=== 3. AIRINT (ADS-B) ===",
            "AAR Hava Yakit Ikmal Paketleri",
            "=== 4. SEAINT (AIS) ===",
            "DARK-SHIP (AIS Karartma/DR)",
            "BOGAZ Kapilari (Gatekeeper)",
            "=== 5. GEOINT / KABLO / GPS-JAM",
            "=== 6. SATINT (CelesTrak SGP4) ===",
            "RECON Uydu Kesif Pencereleri",
            "=== 7. SIGINT (KiwiSDR + HFGCS EAM) ===",
            "=== 8. MASINT (JEOSISMIK / TERMAL / CBRN / UZAY / METEO) ===",
            "=== 9. CYBERINT & CATISMA & OSINT ===",
            "EVI Tirmanma Indeksi (GDELT)",
            "382.4K",
            "0.140 uSv/h",
            "ISS (#25544)",
        ] {
            assert!(prompt.contains(required), "missing section: {}", required);
        }
    }

    #[test]
    fn test_build_mode_prompts() {
        let mut st = AppState::new(Default::default());
        st.airspace_warnings.push(argus_core::types::AirspaceWarning {
            id: "NOTAM-TEST".to_string(),
            fir_or_zone: "LLLL".to_string(),
            kind: "NOTAM-CONFLICT".to_string(),
            hazard: "GNSS BOZUNMA".to_string(),
            top_fl: Some(600),
            polygon: vec![(34.0, 31.0), (36.0, 31.0), (35.0, 33.0), (34.0, 31.0)],
            center_lat: 31.6,
            center_lon: 35.0,
            summary: "Test sahasi".to_string(),
        });
        st.recon_windows.push(argus_core::types::ReconWindow {
            sat_name: "SENTINEL-1A".to_string(),
            norad_id: 39634,
            sensor_type: "SAR".to_string(),
            target_name: "Hurmuz".to_string(),
            target_lat: 26.5,
            target_lon: 56.2,
            aos_secs: 240,
            max_el_deg: 55.0,
            cloud_blocked: false,
        });
        st.dark_ships.push(argus_core::types::DarkShipTrack {
            id: "271000999".to_string(),
            callsign: "GHOST SHIP".to_string(),
            last_lat: 35.1,
            last_lon: 28.2,
            dr_lat: 35.3,
            dr_lon: 28.4,
            sog_kts: 12.0,
            cog_deg: 45.0,
            silent_mins: 22,
            is_military: false,
            vessel_type: Some("Tanker".to_string()),
        });
        st.aar_pairs.push(argus_core::types::AarPair {
            tanker_id: "ae01".to_string(),
            tanker_callsign: "LAGR10".to_string(),
            receiver_id: "ae02".to_string(),
            receiver_callsign: "VIPER01".to_string(),
            lat: 36.2,
            lon: 35.8,
            alt_ft: 24000.0,
            dist_km: 1.8,
            is_locked: true,
        });
        st.escalation_metrics.push(argus_core::types::EscalationMetric {
            country_code: "TR".to_string(),
            country_name: "Turkiye".to_string(),
            evi_score: 68,
            delta_pct: 25,
            event_count: 14,
        });
        st.shodan_recon_cache.insert(
            "1.2.3.4".to_string(),
            argus_core::types::ShodanHostInfo {
                ip: "1.2.3.4".to_string(),
                ports: vec![22, 443],
                vulns: vec!["CVE-2024-1234".to_string()],
                subdomains: vec!["vpn.test.mil".to_string()],
                ..Default::default()
            },
        );

        let m0 = build_mode_prompt(&st, 0, "");
        assert!(m0.contains("N-4 Hava Sahasi / NOTAM / SIGMET (1)"));
        let m1 = build_mode_prompt(&st, 1, "");
        assert!(m1.contains("[MOD 2: SECILI HEDEF DERIN ANALIZI]"));
        let m2 = build_mode_prompt(&st, 2, "");
        assert!(m2.contains("[MOD 3: BOLGESEL KRIZ & HAREKAT ANALIZI]"));
        let m3 = build_mode_prompt(&st, 3, "Hurmuz Bogazi kapatilirsa ne olur?");
        assert!(m3.contains("[MOD 4: OZEL ISTIHBARAT SORUSU]"));
        assert!(m3.contains("Hurmuz Bogazi kapatilirsa ne olur?"));
        let m4 = build_mode_prompt(&st, 4, "");
        assert!(m4.contains("[MOD 5: KIRMIZI HUCRE / OPFOR KARSI KUVVET KOMUTANI]"));
        assert!(m4.contains("Mavi Kuvvet Kor Noktalari"));
        assert!(m4.contains("Kirmizi Hucre Sizma Vektorleri"));
        assert!(m4.contains("EW/Aldatma"));
        assert!(m4.contains("Mavi Karsi Tedbir & Sensor Gorevlendirme"));
        assert!(m4.contains("SENTINEL-1A"));
        assert!(m4.contains("GHOST SHIP"));
        assert!(m4.contains("LAGR10"));
        assert!(m4.contains("CVE-2024-1234"));
    }
}
