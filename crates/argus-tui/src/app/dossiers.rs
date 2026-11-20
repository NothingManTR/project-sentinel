use super::App;
use argus_core::state::AppState;
use argus_core::types::{Domain, FeedStatus, TargetTrack};

impl App {
    pub(super) fn open_feed_health_dialog(st: &mut AppState) {
        let (ok_cnt, active_cnt) = st.feed_health_counts();
        let mut lines = vec![
            format!(
                "=== CANLI SENSOR & API SAGLIK MATRISI ({}/{} AKTIF KAYNAK SAGLIKLI) ===",
                ok_cnt, active_cnt
            ),
            format!(
                "Bildirimler: Masaustu={} │ Webhook={} │ DVR Kare: {} │ Oturum: {}",
                if st.config.notifications.desktop_enabled { "ACIK" } else { "KAPALI" },
                if st.config.notifications.webhook_url.is_some() { "TANIMLI" } else { "YOK" },
                st.dvr.snapshots.len(),
                AppState::default_session_path().display()
            ),
            "--- KAYNAK BAZLI TELEMETRI & HATA IZLEME ---".to_string(),
        ];

        if st.feed_health.is_empty() {
            lines.push("Henuz veri kaynagi yoklamasi tamamlanmadi...".to_string());
        } else {
            for fh in st.feed_health.values() {
                let prefix = match fh.status {
                    FeedStatus::Ok => "[*]",
                    FeedStatus::Degraded => "[!]",
                    FeedStatus::Error => "[!] KRITIK",
                    FeedStatus::NoKey => ">",
                };
                let last_ok = fh
                    .last_ok
                    .map(|t| t.format("%H:%M:%S UTC").to_string())
                    .unwrap_or_else(|| "-".to_string());
                let err_suffix = fh
                    .last_error
                    .as_deref()
                    .map(|e| format!(" │ Not: {}", e))
                    .unwrap_or_default();
                lines.push(format!(
                    "{} {:<15} │ {:<11} │ {:>4}ms │ Kayit:{:<5} │ Son:{:<12} │ Hata:{}{}",
                    prefix,
                    fh.name,
                    fh.status.as_str(),
                    fh.latency_ms,
                    fh.record_count,
                    last_ok,
                    fh.fail_streak,
                    err_suffix
                ));
            }
        }
        st.open_dialog("SENSOR & API SAGLIK MATRISI", lines);
    }

    pub(super) fn open_defcon_dialog(st: &mut AppState) {
        if let Some(d) = &st.defcon_status {
            let mut lines = vec![
                format!("=== KURESEL OSINT DEFCON SEVIYESI: DEFCON {} ===", d.overall_level),
                format!("Veri Kaynagi          : {}", d.source),
                format!("Son Guncelleme (UTC)  : {}", d.updated_at.format("%Y-%m-%d %H:%M:%S")),
            ];
            if let Some(idx) = d.doughcon_index {
                lines.push(format!("PizzINT Doughcon Indx : {}", idx));
            }
            if !d.commands.is_empty() {
                lines.push("--- BOLGESEL MUHARIP KOMUTANLIK SEVIYELERI ---".to_string());
                for (cmd, lvl) in &d.commands {
                    let tag = if *lvl <= 2 { "[!] YUKSEK HAZIRLIK" } else { "Hazirlik" };
                    lines.push(format!("{:<22}: DEFCON {} ({})", cmd, lvl, tag));
                }
            }
            st.open_dialog(format!("OSINT DEFCON {} DURUMU", d.overall_level), lines);
        } else {
            st.open_dialog(
                "OSINT DEFCON DURUMU",
                vec![
                    "=== OSINT DEFCON & PIZZINT VERISI ===".to_string(),
                    "defconlevel.com ve PizzINT verileri arka planda cekiliyor...".to_string(),
                ],
            );
        }
    }


    pub(super) fn open_air_target_dossier(st: &mut AppState, t: &TargetTrack) {
        st.selected_target_id = Some(t.id.clone());
        let alt_m = t.geo.alt_m.unwrap_or(0.0);
        let alt_ft = alt_m * 3.28084;
        let alt_fl = alt_ft / 100.0;
        let spd_kts = t.speed_kts.unwrap_or(0.0);
        let sqwk = t.squawk.as_deref().unwrap_or("----");

        st.open_dialog(
            format!("T-DOSSIER: {} [{}]", t.callsign, t.id),
            vec![
                format!("=== HAVA HEDEFI VE TRANSONDER TELEMETRISI: {} ===", t.callsign),
                format!("ICAO 24-Bit Hex Kodu : {}", t.id),
                format!("Telsiz Cagri Kodu     : {}", t.callsign),
                format!("Hedef Siniflandirma   : {}", if t.is_military { "[ASKERI GOREV PLATFORMU]" } else { "[SIVIL TICARI HAVACILIK]" }),
                format!("Kuvvet / Birlik       : {}", t.operator_branch.as_deref().unwrap_or("Sivil Hava Yollari")),
                format!("Platform / Govde Tipi : {}", t.airframe_or_vessel_class.as_deref().unwrap_or("Standart Yolcu/Kargo Govdesi")),
                format!("Tescil / Bayrak Ulke  : {}", t.operator_country.as_deref().unwrap_or("Bilinmeyen Ulke")),
                format!("Anlik Konum (GPS)     : {:.4}° N, {:.4}° E", t.geo.lat, t.geo.lon),
                format!("Barometrik Irtifa     : FL{:.0} ({:.0} metre / {:.0} feet)", alt_fl, alt_m, alt_ft),
                format!("Yer Hizi (Ground Spd) : {:.0} Knot ({:.0} km/h)", spd_kts, spd_kts * 1.852),
                format!("Dikey Tirmanis Hizi   : {:+0.0} ft/min", t.vertical_rate_fpm.unwrap_or(0.0)),
                format!("Ucus Rotasi (Track)   : {:.1}°", t.heading_deg.unwrap_or(0.0)),
                format!("Mode-S Squawk Kodu    : {}", sqwk),
                format!("Squawk Durum Analizi  : {}", match sqwk { "7700" => "[!] ACIL DURUM (EMERGENCY DECLARED)", "7600" => "[!] TELSİZ KAYBI (RADIO FAILURE)", "7500" => "[!] UCAK KACIRILMA (HIJACK ALARM)", _ => "Standart IFR/VFR Radar Kontrol" }),
                "--- STRATEJIK GOREV VE TEHDIT DEGERLENDIRMESI ---".to_string(),
                format!("Taktik Misyon Analizi : {}", if t.is_military { "Elektronik Harp (EW), Taktik Nakliye veya Muharip Hava Devriyesi (CAP)" } else { "Ticari Tarifeli Sefer / Uluslararasi Sivil Ucus" }),
                "Sinyal Bütünlüğü      : ADS-B 1090MHz Mode-S Dogrulandi (CRC OK)".to_string(),
                format!("Son Gorulme (ZULU)    : {}", t.last_seen.format("%Y-%m-%d %H:%M:%S UTC")),
            ],
        );
        st.dialog_aircraft_icao = Some(t.id.clone());
    }

    /// Derin, teknik ve GitHub-star seviyesinde çok bölümlü istihbarat dosyası açar
    pub(super) fn open_item_dossier(st: &mut AppState, tile_idx: usize, row_idx: usize) -> bool {
        // 1. AirTracks Ekranından Tıklanan Uçak (tile_idx == 20)
        if tile_idx == 20 {
            if let Some(t) = st.aircraft_at_sorted(row_idx).cloned() {
                Self::open_air_target_dossier(st, &t);
                return true;
            }
        }

        // 2. SeaTracks Ekranından Tıklanan Gemi (tile_idx == 21)
        if tile_idx == 21 {
            if let Some(t) = st.vessel_at_sorted(row_idx).cloned() {
                st.selected_target_id = Some(t.id.clone());
                st.open_dialog(
                    format!("M-DOSSIER: {} [MMSI:{}]", t.callsign, t.id),
                    vec![
                        format!("=== DENIZ HARBI & AIS FILO TAKIP DOSYASI: {} ===", t.callsign),
                        format!("MMSI Tanim Numarasi   : {}", t.id),
                        format!("Gemi / Birlik Adi     : {}", t.callsign),
                        format!("Taktik Sinif          : {}", t.vessel_type.as_deref().unwrap_or("Bilinmeyen")),
                        format!("Komuta / Donanma      : {}", t.operator_branch.as_deref().unwrap_or("Ticari Denizcilik")),
                        format!("Gemi Govde Sinifi     : {}", t.airframe_or_vessel_class.as_deref().unwrap_or("Ticari Yuk/Kargo")),
                        format!("Bayrak Devleti (MID)  : {}", t.operator_country.as_deref().unwrap_or("Bilinmeyen")),
                        format!("Anlik Konum (WGS-84)  : {:.4}° N, {:.4}° E", t.geo.lat, t.geo.lon),
                        format!("Seyir Sürati (SOG)    : {:.1} Knot ({:.1} km/h)", t.speed_kts.unwrap_or(0.0), t.speed_kts.unwrap_or(0.0) * 1.852),
                        format!("Rotalanan Seyir (COG) : {:.1}°", t.heading_deg.unwrap_or(0.0)),
                        "--- BOGAZLAR VE STRATEJIK GECIS KORIDORU ---".to_string(),
                        format!("Bogaz / Koridor Rejimi: {}", if t.is_military { "Montro Bogazlar Sozlesmesi Md. 10-18 Kapsaminda Harp Gemisi Gecis Statüsü" } else { "Uluslararasi Ticari Gecis ve Trafik Ayrim Duzeni (TSS)" }),
                        "AIS Verici Tipi      : Class-A Taktik Deniz Transponderi (VHF 161.975 / 162.025 MHz)".to_string(),
                        format!("Son Rapor Zamani      : {}", t.last_seen.format("%Y-%m-%d %H:%M:%S UTC")),
                    ],
                );
                return true;
            }
        }

        // 3. Genel Paneller (Dashboard P100 - P600)
        match tile_idx {
            1 => {
                if let Some(news) = st.news.get(row_idx) {
                    st.open_dialog(
                        format!("N-DOSSIER: {}", news.source),
                        vec![
                            format!("=== ACIK KAYNAK VE KRIZ HABER BULTENI: {} ===", news.source),
                            format!("Haber Basligi         : {}", news.title),
                            format!("Istihbarat Kaynagi    : {}", news.source),
                            format!("Tehdit Onem Derecesi  : {}", news.severity.as_str()),
                            format!("Yayinlanma Zamani     : {}", news.published.format("%Y-%m-%d %H:%M:%S UTC")),
                            format!("Baglanti URI          : {}", news.link),
                            "--- KRIZ ANALIZI VE ONEMLI BASLIKLAR ---".to_string(),
                            "Bulten Ozeti          : Bolgesel jeopolitik riskler, jeostratejik askeri hareketlilik ve kriz gostergeleri izlenmektedir.".to_string(),
                        ],
                    );
                    return true;
                }
            }
            2 => {
                if let Some(t) = st.targets.get(row_idx).cloned() {
                    if t.domain == Domain::Air {
                        Self::open_air_target_dossier(st, &t);
                        return true;
                    }
                    let domain_name = match t.domain {
                        argus_core::types::Domain::Air => "HAVA (ADS-B)",
                        argus_core::types::Domain::Sea => "DENİZ (AIS)",
                        _ => "DİĞER",
                    };
                    st.open_dialog(
                        format!("TARGET-DOSSIER: {}", t.callsign),
                        vec![
                            format!("=== TAKTİK HEDEF DOSYASI: {} ===", t.callsign),
                            format!("Hedef Tanımlayıcı     : {}", t.id),
                            format!("Çağrı / Birlik Adı    : {}", t.callsign),
                            format!("İstihbarat Alanı      : {}", domain_name),
                            format!("Askeri Durum          : {}", if t.is_military { "[ASKERİ HEDEF]" } else { "[SİVİL / TİCARİ]" }),
                            format!("Kuvvet / Operatör     : {}", t.operator_branch.as_deref().unwrap_or("Belirtilmemiş")),
                            format!("Platform Tipi         : {}", t.airframe_or_vessel_class.as_deref().unwrap_or("Standart Platform")),
                            format!("Konum Koordinatları   : {:.4}° N, {:.4}° E", t.geo.lat, t.geo.lon),
                            format!("Hız (Speed)           : {:.1} Knot", t.speed_kts.unwrap_or(0.0)),
                            format!("Rota (Heading)        : {:.1}°", t.heading_deg.unwrap_or(0.0)),
                            format!("Squawk Kodu           : {}", t.squawk.as_deref().unwrap_or("----")),
                            format!("Detay Bilgisi         : {}", t.info),
                            format!("Son Görülme Zamanı    : {}", t.last_seen.format("%Y-%m-%d %H:%M:%S UTC")),
                        ],
                    );
                    return true;
                }
            }
            3 => {
                if let Some(sat) = st.satellites.get(row_idx) {
                    st.open_dialog(
                        format!("S-DOSSIER: {}", sat.name),
                        vec![
                            format!("=== YORUNGE VE UYDU TELEMETRISI: {} ===", sat.name),
                            format!("NORAD Katalog No      : {}", sat.norad_id),
                            format!("Yorunge Konumu        : {:.4}° N, {:.4}° E", sat.lat, sat.lon),
                            format!("Irtifa (Apogee/Perig) : {:.1} km", sat.alt_km),
                            format!("Yorunge Hizi (Vel)    : {:.2} km/s", sat.velocity_kms),
                            format!("Yer Istasyonu Azimut  : {:.1}°", sat.az_deg),
                            format!("Yukselis Acisi (Elev) : {:.1}°", sat.el_deg),
                            format!("Sonraki Gecis Zamani  : T-{} saniye", sat.next_pass_secs),
                            format!("Gecis Suresi (Pass)   : {} saniye", sat.pass_duration_secs),
                            "Propagasyon Modeli   : SGP4 / SDP4 CelesTrak TLE Matematiksel Modelleyici".to_string(),
                        ],
                    );
                    return true;
                }
            }
            30 => {
                let mut eams: Vec<_> = st.eam_messages.iter().collect();
                eams.reverse();
                if let Some(msg) = eams.get(row_idx) {
                    st.open_dialog(
                        "E-DOSSIER: HFGCS EAM / SKYKING MESAJ ANALIZI".to_string(),
                        vec![
                            format!("=== USAF HFGCS ACIL EYLEM MESAJI (EAM): {} ===", msg.preamble),
                            format!("Yakalama Zamani (UTC) : {}", msg.timestamp.format("%Y-%m-%d %H:%M:%S UTC")),
                            format!("Tasiyici Frekans      : {:.1} kHz ({})", msg.frequency_khz, msg.mode),
                            format!("Komuta Istasyonu      : {}", msg.auth_station),
                            format!("Mesaj Preamble Kodu   : {}", msg.preamble),
                            format!("Mesaj Uzunlugu        : {} Karakter", msg.length_chars),
                            format!("Sinyal / Gurultu (SNR): {:+.1} dB", msg.raw_signal_snr_db),
                            format!(
                                "Aciliyet Onceligi     : {}",
                                if msg.is_flash_override {
                                    "[!] FLASH OVERRIDE / SKYKING (YUKSEK ONCELIKLI KOMUTA)"
                                } else {
                                    "STANDART EAM YAYINI"
                                }
                            ),
                            "--- KRIPTOLU FONETIK KOD BLOKLARI ---".to_string(),
                            msg.content.clone(),
                        ],
                    );
                    return true;
                }
            }
            4 => {
                if let Some(inc) = st.cyber_incidents.get(row_idx) {
                    st.open_dialog(
                        format!("C-DOSSIER: {}", inc.title),
                        vec![
                            format!("=== SIBER TEHDIT VE OLAY RAPORU: {} ===", inc.title),
                            format!("Olay ID               : {}", inc.id),
                            format!("Olay Turu             : {}", inc.event_type),
                            format!("Baslik                : {}", inc.title),
                            format!("Hedef / Sektor        : {}", inc.target),
                            format!("Tehdit Seviyesi       : {}", inc.severity.as_str()),
                            format!("CVE Zafiyet Kodu      : {}", inc.cve.as_deref().unwrap_or("N/A")),
                            format!("ASN / Ag Bilgisi      : {}", inc.asn.as_deref().unwrap_or("N/A")),
                            format!("Tespit Zamani         : {}", inc.timestamp.format("%Y-%m-%d %H:%M:%S UTC")),
                            "--- TAVSIYE VE AKSIYON ---".to_string(),
                            "Cozum Onerisi         : Ilgili yazilim yamalarini ivedilikle uygulayin ve acik portlari guvenlik duvari ile izole edin.".to_string(),
                        ],
                    );
                    return true;
                }
            }
            50 => {
                if let Some(q) = st.earthquakes.get(row_idx) {
                    st.open_dialog(
                        format!("S-DOSSIER: M{:.1} - {}", q.mag, q.place),
                        vec![
                            format!("=== SİSMOLOJİK AFET ANALİZ DOSYASI: M {:.1} ===", q.mag),
                            format!("Deprem Büyüklüğü      : M {:.1} (Moment Magnitüd)", q.mag),
                            format!("Merkez Üssü Konumu    : {}", q.place),
                            format!("Odak Derinliği        : {:.1} km ({})", q.depth_km, if q.depth_km < 30.0 { "Sığ Odaklı - Yüksek Yüzey Sarsıntısı" } else { "Derin Odaklı" }),
                            format!("Koordinatlar          : {:.4}° N, {:.4}° E", q.lat, q.lon),
                            format!("Meydana Geliş Zamanı  : {}", q.time.format("%Y-%m-%d %H:%M:%S UTC")),
                            "--- JEOFİZİK TEHDİT VE LEVHA TEKTONİĞİ ---".to_string(),
                            format!("Fay ve Tektonik Kuşak : {}", if q.place.contains("Turkey") || q.place.contains("Turk") || q.place.contains("Greece") { "Anadolu Levhası / Ege Graben Sistemi / Alp-Himalaya Kuşağı" } else { "Küresel Sismik Kuşak / Pasifik Ateş Çemberi" }),
                            format!("Tahmini Mercalli (MMI): {}", match q.mag { m if m >= 7.0 => "MMI VIII-X (Yıkıcı / Çok Şiddetli)", m if m >= 5.5 => "MMI VI-VII (Kuvvetli)", _ => "MMI III-V (Hafif / Orta)" }),
                            format!("Tsunami Riski         : {}", if q.depth_km < 35.0 && q.mag >= 6.5 { "[!] TSUNAMİ DİKKAT VE ERKEN UYARI DEĞERLENDİRMESİ" } else { "Tsunami Riski Bulunmamaktadır" }),
                            format!("USGS Doğrulama URI    : {}", q.url),
                        ],
                    );
                    return true;
                }
            }
            51 => {
                if let Some(fire) = st.firms_fires.get(row_idx) {
                    st.open_dialog(
                        format!("F-DOSSIER: TERMAL [{:.2}N, {:.2}E]", fire.lat, fire.lon),
                        vec![
                            "=== NASA FIRMS TERMAL RADYASYON VE YANGIN ANALİZİ ===".to_string(),
                            format!("Koordinatlar          : {:.4}° N, {:.4}° E", fire.lat, fire.lon),
                            format!("Parlaklık Sıcaklığı   : {:.1} Kelvin ({:.1} °C)", fire.brightness, fire.brightness - 273.15),
                            format!("Tespit Tarihi / Saat  : {} {} UTC", fire.acq_date, fire.acq_time),
                            format!("Güvenilirlik Oranı    : %{}", fire.confidence),
                            "Algılayan Uydu Sensörü: NASA MODIS / VIIRS (Suomi-NPP / NOAA-20)".to_string(),
                            "Termal Anomali Sınıfı : Endüstriyel Parlama, Geniş Ölçekli Orman Yangını veya Patlama".to_string(),
                        ],
                    );
                    return true;
                }
            }
            52 => {
                if let Some(acled) = st.acled_events.get(row_idx) {
                    st.open_dialog(
                        format!("C-DOSSIER: {} [{}]", acled.event_type, acled.country),
                        vec![
                            format!("=== ACLED SİLAHLI ÇATIŞMA VE RİSK DOSYASI: {} ===", acled.event_type),
                            format!("Olay Yeri ve Ülke     : {}, {}", acled.location, acled.country),
                            format!("Olay Tarihi           : {}", acled.event_date),
                            format!("Taktik Olay Türü      : {} / {}", acled.event_type, acled.sub_event_type),
                            format!("Doğrulanmış Zayiat    : {} Kişi (Fatalities)", acled.fatalities),
                            format!("Birincil Taraf Aktörü : {}", acled.actor1),
                            format!("Karşı Taraf Aktörü    : {}", acled.actor2.as_deref().unwrap_or("Belirtilmedi")),
                            "--- OLAY İSTİHBARAT NOTU ---".to_string(),
                            format!("Olay Detayı           : {}", acled.notes),
                        ],
                    );
                    return true;
                }
            }
            53 => {
                if let Some(rad) = st.radiation_measurements.get(row_idx) {
                    let usvh = if rad.unit == "cpm" {
                        rad.value / 334.0
                    } else {
                        rad.value
                    };
                    let (status, advice) = if usvh >= 1.0 {
                        ("[!] KRİTİK ALARM (CBRN TEHLİKE)", "İvedi radyolojik tahliye ve KBRN müdahale protokolü tavsiye edilir.")
                    } else if usvh >= 0.3 {
                        ("[!] YÜKSELMİŞ FON (ELEVATED)", "Doğal fon üzerinde radyasyon artışı. Anomali izlemeye alındı.")
                    } else {
                        ("NORMAL FON (BACKGROUND SAFE)", "Güvenli doğal arka plan seviyesi. Herhangi bir radyolojik sızıntı tespit edilmedi.")
                    };

                    st.open_dialog(
                        format!("R-DOSSIER: SAFECAST SENSOR #{}", rad.id),
                        vec![
                            format!("=== SAFECAST ÇEVRESEL RADYASYON VE GEIGER VERİSİ #{} ===", rad.id),
                            format!("Sensör / İstasyon ID  : {}", rad.id),
                            format!("Ölçüm Cihaz Numarası  : {}", rad.device_id.map(|d| d.to_string()).unwrap_or_else(|| "Mobil Geiger-Müller".to_string())),
                            format!("Radyasyon Doz Hızı    : {:.4} µSv/h (Mikrosievert/saat)", usvh),
                            format!("Sensör Sayım Hızı     : {:.1} {} (Counts Per Minute)", rad.value, rad.unit),
                            format!("Radyolojik Durum      : {}", status),
                            format!("İstasyon / Bölge Adı  : {}", rad.location_name.as_deref().unwrap_or("Sabit / Mobil Çevresel İstasyon")),
                            format!("Koordinatlar (WGS-84) : {:.5}° N, {:.5}° E", rad.lat, rad.lon),
                            format!("Kayıt Zamanı (ZULU)   : {}", rad.captured_at.map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string()).unwrap_or_else(|| "N/A".to_string())),
                            "--- RADYOLOJİK GÜVENLİK VE CBRN RAPORU ---".to_string(),
                            format!("Taktik Değerlendirme  : {}", advice),
                            "Veri Kaynağı          : Safecast Uluslararası Açık Radyasyon Ölçüm Ağı (API v1)".to_string(),
                        ],
                    );
                    return true;
                }
            }
            54 => {
                if let Some(aw) = st.airspace_warnings.get(row_idx) {
                    let fl_str = aw
                        .top_fl
                        .map(|f| format!("FL{} ({} ft)", f, f * 100))
                        .unwrap_or_else(|| "SFC - UNL".to_string());
                    let poly_pts = if aw.polygon.is_empty() {
                        "Yok".to_string()
                    } else {
                        aw.polygon
                            .iter()
                            .take(6)
                            .map(|(lon, lat)| format!("{:.2}N/{:.2}E", lat, lon))
                            .collect::<Vec<_>>()
                            .join(" -> ")
                    };
                    st.open_dialog(
                        format!("N-DOSSIER: {} [{}]", aw.kind, aw.fir_or_zone),
                        vec![
                            "=== CANLI HAVA SAHASI NOTAM / SIGMET / EW UYARISI ===".to_string(),
                            format!("Kayit Kimligi         : {}", aw.id),
                            format!("Uyari Sinifi          : {}", aw.kind),
                            format!("FIR / Sorumluluk Alani: {}", aw.fir_or_zone),
                            format!("Tehlike Turu          : {}", aw.hazard),
                            format!("Ust Irtifa Tavani     : {}", fl_str),
                            format!("Merkez Koordinat      : {:.3}° N, {:.3}° E", aw.center_lat, aw.center_lon),
                            format!("Cokgen Kose Noktalari : {}", poly_pts),
                            "--- TAM UYARI & KORIDOR METNI ---".to_string(),
                            aw.summary.clone(),
                        ],
                    );
                    return true;
                }
            }
            55 => {
                if let Some(fc) = st.fusion_clusters.get(row_idx) {
                    st.open_dialog(
                        format!("K4-FUZYON: SKOR {} [{}]", fc.score, fc.domains.join("+")),
                        vec![
                            "=== K-4 COKLU-SENSOR KORELASYON & KRIZ KUMESI ===".to_string(),
                            format!("Tehdit Füzyon Skoru   : {}/100+", fc.score),
                            format!("Merkez Koordinatlar   : {:.4}° N, {:.4}° E (160 km yaricap)", fc.lat, fc.lon),
                            format!("Aktif İstihbarat Alani: {}", fc.domains.join(" │ ")),
                            "--- KORELASYON KANIT OZETI ---".to_string(),
                            fc.summary.clone(),
                            "Degerlendirme         : Ayni 160 km yaricapli sahada birden fazla bagimsiz istihbarat alaninda eszamanli hareketlilik tespit edilmistir.".to_string(),
                        ],
                    );
                    return true;
                }
            }
            56 => {
                if let Some(a) = st.alerts.get(row_idx) {
                    st.open_dialog(
                        format!("ALARM-DOSSIER: {} [{}]", a.domain, a.severity.as_str()),
                        vec![
                            "=== OTOMATIK ANOMALI, SENSOR VE KORELASYON ALARM DOSYASI ===".to_string(),
                            format!("Alarm Zamani (ZULU)   : {}", a.timestamp.format("%Y-%m-%d %H:%M:%S UTC")),
                            format!("Disiplin / Alan       : {}", a.domain),
                            format!("Tehdit Onem Seviyesi  : {}", a.severity.as_str()),
                            format!("Ilgili Hedef Kimligi  : {}", a.target_id.as_deref().unwrap_or("Genel / Coklu-Hedef")),
                            "--- ALARM & ANOMALI DETAYI ---".to_string(),
                            a.message.clone(),
                        ],
                    );
                    return true;
                }
            }
            5 => {
                let q_len = st.earthquakes.len();
                let f_len = st.firms_fires.len();

                if row_idx < q_len {
                    if let Some(q) = st.earthquakes.get(row_idx) {
                        st.open_dialog(
                            format!("S-DOSSIER: M{:.1} - {}", q.mag, q.place),
                            vec![
                                format!("=== SISMOLOJIK AFET ANALIZ DOSYASI: M {:.1} ===", q.mag),
                                format!("Deprem Buyuklugu      : M {:.1} (Moment Magnitud)", q.mag),
                                format!("Merkez Ussu Konumu    : {}", q.place),
                                format!("Odak Derinligi        : {:.1} km ({})", q.depth_km, if q.depth_km < 30.0 { "Sig Odakli - Yuksek Yuzey Sarsintisi" } else { "Derin Odakli" }),
                                format!("Koordinatlar          : {:.4}° N, {:.4}° E", q.lat, q.lon),
                                format!("Meydana Gelis Zamani  : {}", q.time.format("%Y-%m-%d %H:%M:%S UTC")),
                                "--- JEOFISIK TEHDIT VE LEVHA TEKTONIGI ---".to_string(),
                                format!("Fay ve Tektonik Kusak : {}", if q.place.contains("Turkey") || q.place.contains("Turk") || q.place.contains("Greece") { "Anadolu Levhasi / Ege Graben Sistemi / Alp-Himalaya Kusagi" } else { "Kuresel Sismik Kusak / Pasifik Ates Cemberi" }),
                                format!("Tahmini Mercalli (MMI): {}", match q.mag { m if m >= 7.0 => "MMI VIII-X (Yikici / Cok Siddetli)", m if m >= 5.5 => "MMI VI-VII (Kuvvetli)", _ => "MMI III-V (Hafif / Orta)" }),
                                format!("Tsunami Riski         : {}", if q.depth_km < 35.0 && q.mag >= 6.5 { "[!] TSUNAMI DIKKAT VE ERKEN UYARI DEGERLENDIRMESI" } else { "Tsunami Riski Bulunmamaktadir" }),
                                format!("USGS Dogrulama URI    : {}", q.url),
                            ],
                        );
                        return true;
                    }
                } else if row_idx < q_len + f_len {
                    if let Some(fire) = st.firms_fires.get(row_idx - q_len) {
                        st.open_dialog(
                            format!("F-DOSSIER: TERMAL [{:.2}N, {:.2}E]", fire.lat, fire.lon),
                            vec![
                                "=== NASA FIRMS TERMAL RADYASYON VE YANGIN ANALIZI ===".to_string(),
                                format!("Koordinatlar          : {:.4}° N, {:.4}° E", fire.lat, fire.lon),
                                format!("Parlaklik Sicakligi   : {:.1} Kelvin ({:.1} °C)", fire.brightness, fire.brightness - 273.15),
                                format!("Tespit Tarihi / Saat  : {} {} UTC", fire.acq_date, fire.acq_time),
                                format!("Guvenilirlik Orani    : %{}", fire.confidence),
                                "Algilayan Uydu Sensoru: NASA MODIS / VIIRS (Suomi-NPP / NOAA-20)".to_string(),
                                "Termal Anomali Sinifi : Endustriyel Parlama, Genis Olcekli Orman Yangini veya Patlama".to_string(),
                            ],
                        );
                        return true;
                    }
                } else if let Some(acled) = st.acled_events.get(row_idx - q_len - f_len) {
                    st.open_dialog(
                        format!("C-DOSSIER: {} [{}]", acled.event_type, acled.country),
                        vec![
                            format!("=== ACLED SILAHLI CATISMA VE RISK DOSYASI: {} ===", acled.event_type),
                            format!("Olay Yeri ve Ulke     : {}, {}", acled.location, acled.country),
                            format!("Olay Tarihi           : {}", acled.event_date),
                            format!("Taktik Olay Turu      : {} / {}", acled.event_type, acled.sub_event_type),
                            format!("Dogrulanmis Zayiat    : {} Kisi (Fatalities)", acled.fatalities),
                            format!("Birincil Taraf Aktoru : {}", acled.actor1),
                            format!("Karsi Taraf Aktoru    : {}", acled.actor2.as_deref().unwrap_or("Belirtilmedi")),
                            "--- OLAY ISTIHBARAT NOTU ---".to_string(),
                            format!("Olay Detayi           : {}", acled.notes),
                        ],
                    );
                    return true;
                }
            }
            _ => {}
        }
        false
    }


    pub(super) fn open_help_dialog(st: &mut AppState) {
        st.open_dialog(
            "PROJECT SENTINEL: KULLANICI KILAVUZU & KISAYOLLAR (F12 / ?)",
            vec![
                "=== 1. CALISMA ALANLARI & KOMUT PALETI (Ctrl+K) ===".to_string(),
                "> [Ctrl+K]       : Komut Paleti (Sekme, Bolge, Katman, AI, DVR ve Canli Hedef Ara)".to_string(),
                "> [F1] GENEL     : 7 Panelli Komuta Konsolu ([1-7/Tab] Sec │ [Z/Bosluk] Buyut)".to_string(),
                "> [F2] HARITA    : Taktik Harita, Sol Kenar Cubugu & [G] Koordinata Git (Lat,Lon)".to_string(),
                "> [F3] HAVA (H)  : Canli ADS-B Hava Sahasi Radari, Mesafe & Kerteriz".to_string(),
                "> [F4] DENIZ (D) : Canli AIS Deniz Filosu, 6-Bogaz Gatekeeper & Dark-Ship DR".to_string(),
                "> [F5] SIGINT    : Canli KiwiSDR WebSocket (/SND) HF Alicisi, DSP & EAM".to_string(),
                "> [F6] OSINT     : Kriz Haberleri [0-4], GDELT Catisma [5] & EVI Gerilim Grafigi".to_string(),
                "> [F7] SIBER     : Hibrit Kesif (Shodan + RIPE BGP + 16-Port), CISA & IODA ([/] Ara)".to_string(),
                "> [F8] UYDU      : CelesTrak GP SGP4 Polar Radar Skobu & Gecisler ([/] Ara)".to_string(),
                "> [F9] MASINT    : Deprem [1], Termal [2], CBRN [3], NOTAM [4], Anomali/Alarm [5]".to_string(),
                "> [F10/F11]      : Notlar & AI Rapor Arsivi ([/] Dosya/Satir Ara) / Hesap Makinesi".to_string(),
                "--- 2. HARITA, DVR OYNATICI & SENSOR SAGLIK KONTROLLERI ---".to_string(),
                "> [API:X/Y]      : Ust cubukta 16 veri kaynaginin canli gecikme ve hata matrisini acar".to_string(),
                "> [DVR Cubugu]   : [>] Oynat/Duraklat │ [1x/5x/10x] Hiz │ [Arsiv] JSONL │ [Iz] Gunluk Rota".to_string(),
                "> [Shift+W / WEB]: http://127.0.0.1:8090 uzerinde canli Leaflet haritasini acar".to_string(),
                "> [G / F / B]    : Odakla / F2'de Koord Git (G) │ Takip (F) │ Izleme Listesi (B)".to_string(),
                "--- 3. GEMINI AI ANALIST (6 MOD + COK-TURLU TAKIP SORUSU) ---".to_string(),
                "> [Shift+A/Ctrl+A]: AI SITREP ac (1:Flashpoint 2:Hedef 3:Bolge 4:Ozel 5:OPFOR 6:Delta)".to_string(),
                "> [F] Tusu (AI)  : Acik AI brifingi uzerinde cok-turlu takip sorusu sor".to_string(),
                "> [K / H / E]    : reports/*.md Kaydet (K) │ HTML Brifing (H) │ Notlara Ekle (E)".to_string(),
                "--- 4. KIWISDR KISA DALGA RADYO & DISA AKTARIM ---".to_string(),
                "> [S / 1-7 / N]  : SIGINT ses akisi (S) │ Frekans (1-7) │ Sonraki alici (N)".to_string(),
                "> [Ctrl+E]       : Anlik istihbarat raporunu Markdown olarak disa aktar".to_string(),
            ],
        );
    }
}
