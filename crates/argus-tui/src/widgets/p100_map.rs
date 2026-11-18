use crate::widgets::world_vectors::{
    is_land_bitmap, NE_10M_REGIONAL_COASTS, NE_50M_BORDERS, NE_50M_COASTLINES, NE_50M_LAKES,
};
use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use argus_core::types::{MapRenderStyle, TargetTrack};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    symbols::Marker,
    widgets::{
        canvas::{Canvas, Circle, Context, Line as CanvasLine},
        Block, Clear, Widget,
    },
    Frame,
};
use std::sync::{Mutex, OnceLock};

type MapCacheKey = (u16, u16, u64, u64, u64, u64, u8, u64, u64);

fn map_base_cache() -> &'static Mutex<Vec<(MapCacheKey, Buffer)>> {
    static CACHE: OnceLock<Mutex<Vec<(MapCacheKey, Buffer)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Vec::with_capacity(4)))
}

// =========================================================================
// BOĞAZLAR VE MARMARA ULTRA-YÜKSEK ÇÖZÜNÜRLÜKLÜ DETAY SEGMENTLERİ
// (Her biri tek ve kesintisiz kıyı hattıdır; asla kara üzerinden atlama yapmaz)
// =========================================================================

pub const BOGAZ_AVRUPA_VE_HALIC: &[(f64, f64)] = &[
    (26.65, 40.40), (27.10, 40.62), (27.51, 40.98), (28.00, 41.04), (28.25, 41.06),
    (28.55, 41.01), (28.68, 40.97), (28.80, 40.97), (28.91, 40.98), (28.98, 41.01),
    (28.985, 41.017), (28.950, 41.030), (28.935, 41.045), (28.975, 41.024),
    (28.985, 41.028), (29.006, 41.041), (29.027, 41.047), (29.043, 41.067),
    (29.055, 41.085), (29.054, 41.104), (29.068, 41.121), (29.049, 41.143),
    (29.055, 41.166), (29.088, 41.178), (29.105, 41.205), (29.112, 41.235),
    (29.080, 41.250), (28.800, 41.300),
];

pub const BOGAZ_ANADOLU_VE_IZMIT: &[(f64, f64)] = &[
    (29.350, 41.180), (29.152, 41.218), (29.130, 41.202), (29.091, 41.173),
    (29.095, 41.132), (29.098, 41.115), (29.068, 41.099), (29.059, 41.074),
    (29.045, 41.043), (29.015, 41.025), (29.006, 41.021), (29.018, 41.001),
    (29.037, 40.969), (29.135, 40.925), (29.235, 40.875), (29.345, 40.785),
    (29.520, 40.765), (29.920, 40.760), (29.830, 40.718), (29.620, 40.690),
    (29.515, 40.725), (29.270, 40.655), (28.825, 40.535), (29.150, 40.430),
    (28.880, 40.375), (27.970, 40.355), (27.850, 40.520), (27.790, 40.400),
    (26.685, 40.345),
];

pub const CANAKKALE_AVRUPA: &[(f64, f64)] = &[
    (26.70, 40.43), (26.63, 40.34), (26.50, 40.28), (26.38, 40.18),
    (26.38, 40.14), (26.25, 40.08), (26.17, 40.04),
];

pub const CANAKKALE_ANADOLU: &[(f64, f64)] = &[
    (26.75, 40.39), (26.68, 40.34), (26.55, 40.26), (26.42, 40.20),
    (26.40, 40.15), (26.32, 40.10), (26.20, 40.01),
];

pub const ADALAR_BUYUKADA: &[(f64, f64)] = &[
    (29.115, 40.870), (29.132, 40.875), (29.126, 40.848), (29.112, 40.858), (29.115, 40.870),
];
pub const ADALAR_HEYBELI: &[(f64, f64)] = &[
    (29.088, 40.875), (29.102, 40.882), (29.096, 40.865), (29.088, 40.875),
];

pub const TACTICAL_CITIES: &[(&str, f64, f64, f64)] = &[
    ("ISTANBUL", 28.9784, 41.0082, 0.7),
    ("ANKARA", 32.8597, 39.9334, 0.7),
    ("IZMIR", 27.1428, 38.4237, 1.2),
    ("ANTALYA", 30.7133, 36.8969, 1.5),
    ("ADANA", 35.3213, 37.0000, 1.6),
    ("TRABZON", 39.7168, 41.0027, 1.6),
    ("DIYARBAKIR", 40.2306, 37.9144, 1.6),
    ("CANAKKALE", 26.4086, 40.1553, 2.0),
    ("GOLCUK", 29.8300, 40.7180, 2.5),
    ("ATHENS", 23.7275, 37.9838, 1.0),
    ("SOFIA", 23.3219, 42.6977, 1.2),
    ("BUCHAREST", 26.1025, 44.4268, 1.2),
    ("ODESSA", 30.7233, 46.4825, 1.2),
    ("SEVASTOPOL", 33.5225, 44.6166, 1.4),
    ("KYIV", 30.5234, 50.4501, 1.0),
    ("TBILISI", 44.8271, 41.7151, 1.2),
    ("BAKU", 49.8671, 40.4093, 1.2),
    ("TEHRAN", 51.3890, 35.6892, 0.9),
    ("BAGHDAD", 44.3661, 33.3152, 1.0),
    ("DAMASCUS", 36.2765, 33.5138, 1.2),
    ("BEIRUT", 35.5018, 33.8938, 1.4),
    ("NICOSIA", 33.3823, 35.1856, 1.4),
    ("TEL AVIV", 34.7818, 32.0853, 1.2),
    ("CAIRO", 31.2357, 30.0444, 0.8),
    ("RIYADH", 46.6753, 24.7136, 0.9),
    ("DUBAI", 55.2708, 25.2048, 1.0),
    ("MOSCOW", 37.6173, 55.7558, 0.7),
    ("WARSAW", 21.0122, 52.2297, 1.0),
    ("BERLIN", 13.4050, 52.5200, 0.8),
    ("ROME", 12.4964, 41.9028, 0.8),
    ("PARIS", 2.3522, 48.8566, 0.8),
    ("MADRID", -3.7038, 40.4168, 0.9),
    ("LONDON", -0.1278, 51.5074, 0.7),
    ("STOCKHOLM", 18.0686, 59.3293, 1.0),
    ("NEW YORK", -74.0060, 40.7128, 0.7),
    ("WASHINGTON", -77.0369, 38.9072, 0.7),
    ("LOS ANGELES", -118.2437, 34.0522, 0.8),
    ("SAO PAULO", -46.6333, -23.5505, 0.8),
    ("BEIJING", 116.4074, 39.9042, 0.7),
    ("TAIPEI", 121.5654, 25.0330, 1.0),
    ("SEOUL", 126.9780, 37.5665, 0.9),
    ("TOKYO", 139.6917, 35.6895, 0.7),
    ("SINGAPORE", 103.8198, 1.3521, 0.9),
    ("SYDNEY", 151.2093, -33.8688, 0.8),
];

#[inline]
fn draw_polyline_f64(ctx: &mut Context, pts: &[(f64, f64)], color: Color) {
    for window in pts.windows(2) {
        ctx.draw(&CanvasLine {
            x1: window[0].0,
            y1: window[0].1,
            x2: window[1].0,
            y2: window[1].1,
            color,
        });
    }
}

/// Görüş alanı (Bounding Box) içinde kalan Natural Earth vektör segmentlerini çizer
#[inline]
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw_culled_vectors(
    ctx: &mut Context,
    dataset: &[(f32, f32, f32, f32, &[(f32, f32)])],
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
    min_seg_deg: f32,
    color: Color,
) {
    let xmin_f = x_min as f32;
    let xmax_f = x_max as f32;
    let ymin_f = y_min as f32;
    let ymax_f = y_max as f32;

    for &(bx1, by1, bx2, by2, pts) in dataset.iter() {
        if bx2 < xmin_f || bx1 > xmax_f || by2 < ymin_f || by1 > ymax_f {
            continue;
        }
        if pts.len() < 2 {
            continue;
        }
        let mut prev = pts[0];
        let last_idx = pts.len() - 1;
        for (i, &cur) in pts.iter().enumerate().skip(1) {
            if i != last_idx
                && (cur.0 - prev.0).abs() < min_seg_deg
                && (cur.1 - prev.1).abs() < min_seg_deg
            {
                continue;
            }
            if !((prev.0 < xmin_f && cur.0 < xmin_f)
                || (prev.0 > xmax_f && cur.0 > xmax_f)
                || (prev.1 < ymin_f && cur.1 < ymin_f)
                || (prev.1 > ymax_f && cur.1 > ymax_f))
            {
                ctx.draw(&CanvasLine {
                    x1: prev.0 as f64,
                    y1: prev.1 as f64,
                    x2: cur.0 as f64,
                    y2: cur.1 as f64,
                    color,
                });
            }
            prev = cur;
        }
    }
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn put_overlay_str(
    buf: &mut Buffer,
    area: Rect,
    x: u16,
    y: u16,
    text: &str,
    fg: Color,
    bg_opt: Option<Color>,
    bold: bool,
) {
    if y < area.y || y >= area.y + area.height {
        return;
    }
    for (i, ch) in text.chars().enumerate() {
        let cx = x + i as u16;
        if cx >= area.x && cx < area.x + area.width {
            let cell = &mut buf[(cx, y)];
            let mut s = cell.style().fg(fg);
            if let Some(bg) = bg_opt {
                s = s.bg(bg);
            }
            if bold {
                s = s.add_modifier(Modifier::BOLD);
            }
            cell.set_char(ch).set_style(s);
        }
    }
}

fn contains_any(haystack: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| haystack.contains(p))
}

/// Kisa kodlarda (orn. C17 vs C172, C5 vs C525, E3 vs BE30) rakam/harf siniri denetler
fn contains_exact(haystack: &str, patterns: &[&str]) -> bool {
    let bytes = haystack.as_bytes();
    patterns.iter().any(|pat| {
        let mut start = 0;
        while let Some(pos) = haystack[start..].find(pat) {
            let abs = start + pos;
            let left_ok = abs == 0 || !bytes[abs - 1].is_ascii_alphanumeric();
            let right_idx = abs + pat.len();
            let right_ok = right_idx >= bytes.len() || !bytes[right_idx].is_ascii_digit();
            if left_ok && right_ok {
                return true;
            }
            start = abs + 1;
        }
        false
    })
}

fn heading_narrow_glyph(heading_deg: Option<f64>) -> &'static str {
    let h = heading_deg.unwrap_or(0.0).rem_euclid(360.0);
    if !(45.0..315.0).contains(&h) {
        "▴"
    } else if h < 135.0 {
        "▸"
    } else if h < 225.0 {
        "▾"
    } else {
        "◂"
    }
}

/// Uçağı platform tipine ve gövde modeline göre tek hücreli şekle sınıflandırır (emoji kullanılmaz)
pub fn classify_aircraft_shape(track: &TargetTrack) -> (&'static str, Color) {
    let cls = track
        .airframe_or_vessel_class
        .as_deref()
        .unwrap_or("")
        .to_uppercase();
    let branch = track
        .operator_branch
        .as_deref()
        .unwrap_or("")
        .to_uppercase();
    let info = track.info.to_uppercase();
    let combined = format!("{} {} {}", cls, branch, info);

    // 1. Keşif / ISR / AWACS / İHA / Stratejik Bombardıman
    if contains_exact(
        &combined,
        &[
            "RQ4", "RQ-4", "MQ9", "MQ-9", "MQ1", "E3", "E-3", "E4", "E-4", "E6", "E8",
            "RC135", "RC-135", "P8", "P-8", "U2", "B52", "B-52", "B1B",
        ],
    ) || contains_any(
        &combined,
        &[
            "RIVET", "AWACS", "GLOBAL HAWK", "REAPER", "BAYRAKTAR", "ANKA", "AKINCI",
        ],
    ) {
        return ("✦", TELETEXT_YELLOW);
    }

    // 2. Askeri Helikopter / Sivil Helikopter / Tiltrotor
    if contains_any(
        &combined,
        &[
            "H60", "UH60", "S70", "S76", "S92", "EC35", "EC45", "EC55", "EC75",
            "H135", "H145", "H160", "H175", "H225", "AS3", "AS5", "AW1", "A109",
            "A139", "A169", "A189", "BELL", "B06", "B407", "B412", "B429", "R22",
            "R44", "R66", "CH47", "AH64", "NH90", "LYNX", "EH10", "MI8", "MI17",
            "T129", "V22", "HELI",
        ],
    ) {
        return ("❖", TELETEXT_GREEN);
    }

    // 3. Havadan Yakıt İkmal Tankeri & Ağır Askeri Kargo
    if contains_exact(
        &combined,
        &[
            "KC135", "KC-135", "KC46", "KC-46", "KC10", "KC30", "C17", "C-17", "C5",
            "C-5", "C130", "C-130", "C30J", "A400", "IL76", "AN12", "AN26", "C295",
            "CN35", "C27J",
        ],
    ) || contains_any(&combined, &["MRTT", "TANKER"])
    {
        return ("◈", Color::Rgb(210, 132, 82));
    }

    // 4. Muharip Jet & Eğitim Jeti
    if contains_exact(
        &combined,
        &[
            "F16", "F-16", "F15", "F-15", "F18", "F-18", "F22", "F35", "F-35",
            "EF20", "EUFI", "SU27", "SU30", "SU35", "T38", "T-38", "M346", "L39",
            "TEX2", "FA-50", "T45",
        ],
    ) || contains_any(&combined, &["RAFAL", "MIG", "GRIP", "HAWK"])
    {
        return ("▲", TELETEXT_RED);
    }

    // 5. Geniş Gövdeli Ağır Yolcu / Kargo (Wide-body)
    if contains_any(
        &combined,
        &[
            "B74", "A38", "B77", "B78", "A35", "A33", "A34", "B76", "MD11", "DC10",
        ],
    ) {
        return ("◆", TELETEXT_CYAN);
    }

    // 6. İş Jeti / Hafif Pervaneli / Turboprop (GA & Bizjet)
    if contains_any(
        &combined,
        &[
            "ATR", "AT4", "AT7", "DH8", "PC12", "PC21", "PC24", "BE2", "BE3", "BE9",
            "B350", "C15", "C172", "C182", "C206", "C208", "C25", "C510", "C525",
            "C56", "C68", "C75", "SR20", "SR22", "PA28", "PA34", "PA46", "DA40",
            "DA42", "DA62", "GLF", "G280", "G550", "G650", "CL30", "CL35", "CL60",
            "LJ", "FA50", "FA7X", "FA8X", "E50P", "E55P", "HDJT", "SF50",
        ],
    ) {
        return ("▫", FG_PRIMARY);
    }

    if track.is_military {
        return ("▲", Color::Rgb(210, 132, 82));
    }

    // 7. Standart Dar Gövde & Bölgesel Jetler (A320/B737/E190 - Rota yönüne göre ok)
    (heading_narrow_glyph(track.heading_deg), TELETEXT_CYAN)
}

pub fn render_p100_map(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if area.width < 8 || area.height < 5 {
        return;
    }

    state.hit_tester.register(area, HitTarget::MapCanvas);

    let zoom = state.map_zoom.clamp(0.25, 50.0);
    let w = (area.width as f64).max(10.0);
    let h = (area.height as f64).max(5.0);

    let lat_rad = state.map_center_lat.to_radians();
    let cos_lat = lat_rad.cos().abs().clamp(0.15, 1.0);

    let raw_y_half = 45.0 / zoom;
    let y_half = raw_y_half.clamp(0.005, 85.0);
    let x_half = (raw_y_half * (w / (h * 2.0)) / cos_lat).clamp(0.005, 180.0);

    let x_min = (state.map_center_lon - x_half).max(-180.0);
    let x_max = (state.map_center_lon + x_half).min(180.0);
    let y_min = (state.map_center_lat - y_half).clamp(-88.0, 88.0);
    let y_max = (state.map_center_lat + y_half).clamp(-88.0, 88.0);

    frame.render_widget(Clear, area);

    let (ocean_bg, land_bg, coast_col, border_col, lake_col, grid_col) = match state.map_style {
        MapRenderStyle::HalfBlockTerrain => (
            Color::Rgb(13, 17, 23),
            Color::Rgb(24, 30, 32),
            Color::Rgb(92, 146, 140),
            Color::Rgb(54, 72, 84),
            Color::Rgb(58, 108, 142),
            Color::Rgb(22, 28, 36),
        ),
        MapRenderStyle::HalfBlockDarkTile => (
            Color::Rgb(12, 15, 20),
            Color::Rgb(22, 26, 33),
            Color::Rgb(116, 148, 180),
            Color::Rgb(56, 68, 84),
            Color::Rgb(68, 104, 140),
            Color::Rgb(22, 26, 34),
        ),
        MapRenderStyle::HalfBlockSatellite => (
            Color::Rgb(12, 19, 28),
            Color::Rgb(28, 36, 30),
            Color::Rgb(126, 156, 118),
            Color::Rgb(72, 88, 68),
            Color::Rgb(60, 108, 148),
            Color::Rgb(20, 28, 38),
        ),
        MapRenderStyle::BrailleVector => (
            Color::Rgb(13, 16, 22),
            Color::Rgb(19, 24, 32),
            Color::Rgb(110, 158, 190),
            Color::Rgb(54, 74, 96),
            Color::Rgb(66, 116, 158),
            Color::Rgb(22, 28, 38),
        ),
    };

    let st_lon = state.station_lon;
    let st_lat = state.station_lat;
    let cables_on = (state.geoint_layers & 256 != 0) && !state.subsea_cables.is_empty();
    let style_tag: u8 = match state.map_style {
        MapRenderStyle::HalfBlockTerrain => 0,
        MapRenderStyle::HalfBlockDarkTile => 1,
        MapRenderStyle::HalfBlockSatellite => 2,
        MapRenderStyle::BrailleVector => 3,
    } | if cables_on { 8 } else { 0 };
    let cache_key: MapCacheKey = (
        area.width,
        area.height,
        x_min.to_bits(),
        x_max.to_bits(),
        y_min.to_bits(),
        y_max.to_bits(),
        style_tag,
        st_lon.to_bits(),
        st_lat.to_bits(),
    );

    // 1 + 2. Katman: Onbellekli Vektor + Kara/Deniz Maskesi (Ayni harita cercevesinde 0.03 ms blitleme!)
    let mut hit_cache = false;
    if let Ok(mut guard) = map_base_cache().lock() {
        if let Some(pos) = guard.iter().position(|(k, _)| *k == cache_key) {
            let entry = guard.remove(pos);
            let dst = frame.buffer_mut();
            for row in 0..area.height {
                for col in 0..area.width {
                    dst[(area.x + col, area.y + row)] = entry.1[(col, row)].clone();
                }
            }
            guard.push(entry);
            hit_cache = true;
        }
    }

    if !hit_cache {
        let local_rect = Rect {
            x: 0,
            y: 0,
            width: area.width,
            height: area.height,
        };
        let mut base_buf = Buffer::empty(local_rect);
        let min_seg_deg = (((x_max - x_min) / (w * 2.5)).min((y_max - y_min) / (h * 5.0)) * 0.55) as f32;

        let canvas = Canvas::default()
            .marker(Marker::Braille)
            .x_bounds([x_min, x_max])
            .y_bounds([y_min, y_max])
            .block(Block::default().style(Style::default().bg(ocean_bg)))
            .paint(|ctx| {
                // A. Adaptif Enlem/Boylam Izgarası
                let step = if x_half > 60.0 {
                    30.0
                } else if x_half > 20.0 {
                    10.0
                } else if x_half > 6.0 {
                    2.0
                } else if x_half > 1.5 {
                    0.5
                } else if x_half > 0.4 {
                    0.1
                } else {
                    0.02
                };

                let mut curr_lon = (x_min / step).floor() * step;
                let end_lon = (x_max / step).ceil() * step;
                while curr_lon <= end_lon {
                    ctx.draw(&CanvasLine {
                        x1: curr_lon,
                        y1: y_min,
                        x2: curr_lon,
                        y2: y_max,
                        color: grid_col,
                    });
                    curr_lon += step;
                }

                let mut curr_lat = (y_min / step).floor() * step;
                let end_lat = (y_max / step).ceil() * step;
                while curr_lat <= end_lat {
                    ctx.draw(&CanvasLine {
                        x1: x_min,
                        y1: curr_lat,
                        x2: x_max,
                        y2: curr_lat,
                        color: grid_col,
                    });
                    curr_lat += step;
                }

                // B. Uluslararası Ülke Sınırları (Natural Earth 1:50m)
                draw_culled_vectors(ctx, NE_50M_BORDERS, x_min, x_max, y_min, y_max, min_seg_deg, border_col);
                // C. Küresel Göller ve İç Denizler
                draw_culled_vectors(ctx, NE_50M_LAKES, x_min, x_max, y_min, y_max, min_seg_deg, lake_col);
                // C2. K-1 Küresel Denizaltı Fiber Kabloları (Opsiyonel Bit 8)
                if cables_on {
                    let cable_col = Color::Rgb(58, 128, 126);
                    for cable in &state.subsea_cables {
                        for seg in &cable.lines {
                            for pair in seg.windows(2) {
                                let (x1, y1) = pair[0];
                                let (x2, y2) = pair[1];
                                if (x1 - x2).abs() > 180.0
                                    || x1.max(x2) < x_min
                                    || x1.min(x2) > x_max
                                    || y1.max(y2) < y_min
                                    || y1.min(y2) > y_max
                                {
                                    continue;
                                }
                                ctx.draw(&CanvasLine { x1, y1, x2, y2, color: cable_col });
                            }
                        }
                    }
                }
                // D. Küresel Kıyı Şeritleri ve Adalar
                draw_culled_vectors(ctx, NE_50M_COASTLINES, x_min, x_max, y_min, y_max, min_seg_deg, coast_col);
                // E. Bölgesel Ultra-Yüksek Çözünürlüklü Kıyılar (1:10m)
                if zoom >= 1.2 {
                    draw_culled_vectors(ctx, NE_10M_REGIONAL_COASTS, x_min, x_max, y_min, y_max, min_seg_deg, coast_col);
                }
                // F. İstanbul & Çanakkale Boğazları Mikro-Vektörleri
                if zoom >= 1.8 {
                    let fine_col = Color::Rgb(116, 174, 166);
                    draw_polyline_f64(ctx, BOGAZ_AVRUPA_VE_HALIC, fine_col);
                    draw_polyline_f64(ctx, BOGAZ_ANADOLU_VE_IZMIT, fine_col);
                    draw_polyline_f64(ctx, CANAKKALE_AVRUPA, fine_col);
                    draw_polyline_f64(ctx, CANAKKALE_ANADOLU, fine_col);
                    draw_polyline_f64(ctx, ADALAR_BUYUKADA, fine_col);
                    draw_polyline_f64(ctx, ADALAR_HEYBELI, fine_col);
                }
                // G. Taktik Menzil Halkaları
                let ring_step_km = if zoom >= 20.0 {
                    5.0
                } else if zoom >= 8.0 {
                    25.0
                } else if zoom >= 3.0 {
                    100.0
                } else if zoom >= 1.2 {
                    250.0
                } else {
                    1000.0
                };
                for r_mult in 1..=3 {
                    let deg_radius = (ring_step_km * (r_mult as f64)) / 111.0;
                    if deg_radius < x_half.max(y_half) * 1.4 {
                        ctx.draw(&Circle {
                            x: st_lon,
                            y: st_lat,
                            radius: deg_radius,
                            color: Color::Rgb(38, 52, 68),
                        });
                    }
                }
            });

        canvas.render(local_rect, &mut base_buf);

        let w_f = area.width.max(1) as f64;
        let h_f = area.height.max(1) as f64;
        for row in 0..area.height {
            let lat = y_max - ((row as f64 + 0.5) / h_f) * (y_max - y_min);
            for col in 0..area.width {
                let lon = x_min + ((col as f64 + 0.5) / w_f) * (x_max - x_min);
                if is_land_bitmap(lon, lat) {
                    let cell = &mut base_buf[(col, row)];
                    let s = cell.style().bg(land_bg);
                    cell.set_style(s);
                }
            }
        }

        let dst = frame.buffer_mut();
        for row in 0..area.height {
            for col in 0..area.width {
                dst[(area.x + col, area.y + row)] = base_buf[(col, row)].clone();
            }
        }

        if let Ok(mut guard) = map_base_cache().lock() {
            if guard.len() >= 4 {
                guard.remove(0);
            }
            guard.push((cache_key, base_buf));
        }
    }

    // 3. Katman: Kıyı Noktalarını Silmeyen Saf ASCII Hedef ve Şehir Katmanı
    let to_screen = |lon: f64, lat: f64| -> Option<(u16, u16)> {
        if lon < x_min || lon > x_max || lat < y_min || lat > y_max {
            return None;
        }
        let nx = (lon - x_min) / (x_max - x_min).max(1e-9);
        let ny = (y_max - lat) / (y_max - y_min).max(1e-9);
        let sx = area.x + ((nx * (area.width as f64)) as u16).min(area.width.saturating_sub(1));
        let sy = area.y + ((ny * (area.height as f64)) as u16).min(area.height.saturating_sub(1));
        if sy >= area.y + area.height.saturating_sub(1) {
            return None;
        }
        Some((sx, sy))
    };

    for (name, c_lon, c_lat, min_z) in TACTICAL_CITIES.iter() {
        if zoom >= *min_z {
            if let Some((sx, sy)) = to_screen(*c_lon, *c_lat) {
                put_overlay_str(
                    frame.buffer_mut(),
                    area,
                    sx,
                    sy,
                    ".",
                    Color::Rgb(225, 190, 85),
                    None,
                    true,
                );
                if zoom >= 1.5 {
                    put_overlay_str(
                        frame.buffer_mut(),
                        area,
                        sx + 1,
                        sy,
                        name,
                        Color::Rgb(165, 180, 195),
                        None,
                        false,
                    );
                }
            }
        }
    }
    if let Some((sx, sy)) = to_screen(st_lon, st_lat) {
        put_overlay_str(
            frame.buffer_mut(),
            area,
            sx,
            sy,
            "X",
            TELETEXT_YELLOW,
            None,
            true,
        );
    }

    // Canlı Depremler (@) [Katman 5]
    if state.geoint_layers & 16 != 0 {
        let quakes: Vec<_> = state.earthquakes.iter().cloned().collect();
        for (idx, quake) in quakes.iter().enumerate() {
            if let Some((sx, sy)) = to_screen(quake.lon, quake.lat) {
                let col = if quake.mag >= 5.5 {
                    TELETEXT_RED
                } else {
                    TELETEXT_YELLOW
                };
                let label = if zoom >= 2.0 {
                    format!("@M{:.1}", quake.mag)
                } else {
                    "@".to_string()
                };
                put_overlay_str(frame.buffer_mut(), area, sx, sy, &label, col, None, true);
                state.hit_tester.register(
                    Rect {
                        x: sx,
                        y: sy,
                        width: label.len() as u16,
                        height: 1,
                    },
                    HitTarget::MapTarget(50, idx),
                );
            }
        }
    }

    // NASA FIRMS Termal Noktalar (#) [Katman 4]
    if state.geoint_layers & 8 != 0 {
        let fires: Vec<_> = state.firms_fires.iter().cloned().collect();
        for (idx, fire) in fires.iter().enumerate() {
            if let Some((sx, sy)) = to_screen(fire.lon, fire.lat) {
                put_overlay_str(
                    frame.buffer_mut(),
                    area,
                    sx,
                    sy,
                    "#",
                    Color::Rgb(206, 122, 74),
                    None,
                    true,
                );
                state.hit_tester.register(
                    Rect {
                        x: sx,
                        y: sy,
                        width: 1,
                        height: 1,
                    },
                    HitTarget::MapTarget(51, idx),
                );
            }
        }
    }

    // Safecast Radyasyon (%) [Katman 7]
    if state.geoint_layers & 64 != 0 {
        let rads: Vec<_> = state.radiation_measurements.iter().cloned().collect();
        for (idx, rad) in rads.iter().enumerate() {
            if let Some((sx, sy)) = to_screen(rad.lon, rad.lat) {
                put_overlay_str(
                    frame.buffer_mut(),
                    area,
                    sx,
                    sy,
                    "%",
                    TELETEXT_GREEN,
                    None,
                    true,
                );
                state.hit_tester.register(
                    Rect {
                        x: sx,
                        y: sy,
                        width: 1,
                        height: 1,
                    },
                    HitTarget::MapTarget(53, idx),
                );
            }
        }
    }

    // 3.5 Katman: Canlı Rota İzleri (Sıfır kopyalama, yalnız zoom >= 1.2 görünümünde) & EW Jamming
    if zoom >= 1.2 {
        let buf = frame.buffer_mut();
        for trail in state.target_trails.values() {
            if trail.len() < 2 {
                continue;
            }
            for &(tlon, tlat) in &trail[..trail.len() - 1] {
                if let Some((sx, sy)) = to_screen(tlon, tlat) {
                    if buf[(sx, sy)].symbol() == " " {
                        put_overlay_str(buf, area, sx, sy, ".", Color::Rgb(45, 115, 165), None, false);
                    }
                }
            }
        }
    }
    for &(jlat, jlon, _, _) in &state.gps_jamming_zones {
        if let Some((sx, sy)) = to_screen(jlon, jlat) {
            put_overlay_str(
                frame.buffer_mut(),
                area,
                sx,
                sy,
                "[JAM]",
                Color::Rgb(184, 96, 136),
                None,
                true,
            );
        }
    }

    // Deniz Araçları (^) [Katman 2] — Önbellekli & sıfır kopyalamalı
    if state.geoint_layers & 2 != 0 {
        state.ensure_sea_indices();
        let mut reg_hits: Vec<(u16, u16, u16, usize)> = Vec::with_capacity(256);
        let buf = frame.buffer_mut();
        for (sorted_pos, &buf_idx) in state.cached_sea_indices.iter().enumerate() {
            let Some(track) = state.sea_targets.get(buf_idx) else {
                continue;
            };
            if let Some((sx, sy)) = to_screen(track.geo.lon, track.geo.lat) {
                let is_selected = state.selected_target_id.as_deref() == Some(track.id.as_str());
                let is_warship = track.is_military || track.callsign.starts_with("TCG");
                let col = if is_selected {
                    TELETEXT_YELLOW
                } else if is_warship {
                    TELETEXT_RED
                } else {
                    TELETEXT_GREEN
                };
                let w_hit: u16 = if is_selected {
                    let text = format!("[^ {}]", track.callsign);
                    put_overlay_str(buf, area, sx, sy, &text, col, None, true);
                    (text.chars().count() as u16).max(1)
                } else if zoom >= 2.5 || is_warship {
                    let text = format!("^{}", track.callsign);
                    put_overlay_str(buf, area, sx, sy, &text, col, None, true);
                    (text.chars().count() as u16).max(1)
                } else {
                    put_overlay_str(buf, area, sx, sy, "^", col, None, true);
                    1
                };
                reg_hits.push((sx, sy, w_hit, sorted_pos));
            }
        }
        for (sx, sy, w_hit, sorted_pos) in reg_hits {
            state.hit_tester.register(
                Rect {
                    x: sx,
                    y: sy,
                    width: w_hit.min(area.x + area.width - sx),
                    height: 1,
                },
                HitTarget::MapTarget(21, sorted_pos),
            );
        }
    }

    // Hava Araçları (+) [Katman 1] — Tek tarama + önbellekli indeks + hücre bazlı tekilleştirilmiş hit-test
    if state.geoint_layers & 1 != 0 {
        state.ensure_air_indices();
        let gw = area.width as usize;
        let gh = area.height as usize;
        let mut cell_hit: Vec<Option<HitTarget>> = vec![None; gw * gh];
        let mut vis_civ: Vec<(usize, usize, u16, u16)> = Vec::with_capacity(2048);
        let mut vis_mil: Vec<(usize, usize, u16, u16)> = Vec::with_capacity(256);
        let mut vis_sel: Option<(usize, usize, u16, u16)> = None;

        for (sorted_pos, &buf_idx) in state.cached_air_indices.iter().enumerate() {
            let Some(track) = state.air_targets.get(buf_idx) else {
                continue;
            };
            if let Some((sx, sy)) = to_screen(track.geo.lon, track.geo.lat) {
                if state.selected_target_id.as_deref() == Some(track.id.as_str()) {
                    vis_sel = Some((sorted_pos, buf_idx, sx, sy));
                } else if track.is_military {
                    vis_mil.push((sorted_pos, buf_idx, sx, sy));
                } else {
                    vis_civ.push((sorted_pos, buf_idx, sx, sy));
                }
            }
        }

        let buf = frame.buffer_mut();
        let mut mark_cell = |sx: u16, sy: u16, w: u16, sorted_pos: usize| {
            let rx = (sx - area.x) as usize;
            let ry = (sy - area.y) as usize;
            let max_w = (w as usize).min(gw.saturating_sub(rx));
            for dx in 0..max_w {
                cell_hit[ry * gw + rx + dx] = Some(HitTarget::MapTarget(20, sorted_pos));
            }
        };

        let use_shapes = state.map_aircraft_shapes;
        for (sorted_pos, buf_idx, sx, sy) in vis_civ {
            let Some(track) = state.air_targets.get(buf_idx) else {
                continue;
            };
            let w = if use_shapes {
                let (glyph, gcol) = classify_aircraft_shape(track);
                put_overlay_str(buf, area, sx, sy, glyph, gcol, None, true);
                1
            } else if zoom >= 2.5 {
                let text = format!("+{}", track.callsign);
                put_overlay_str(buf, area, sx, sy, &text, TELETEXT_CYAN, None, true);
                (text.chars().count() as u16).max(1)
            } else {
                put_overlay_str(buf, area, sx, sy, "+", TELETEXT_CYAN, None, true);
                1
            };
            mark_cell(sx, sy, w, sorted_pos);
        }

        let mil_col = Color::Rgb(210, 132, 82);
        for (sorted_pos, buf_idx, sx, sy) in vis_mil {
            let Some(track) = state.air_targets.get(buf_idx) else {
                continue;
            };
            let w = if use_shapes {
                let (glyph, gcol) = classify_aircraft_shape(track);
                put_overlay_str(buf, area, sx, sy, glyph, gcol, None, true);
                1
            } else {
                let text = format!("+{}", track.callsign);
                put_overlay_str(buf, area, sx, sy, &text, mil_col, None, true);
                (text.chars().count() as u16).max(1)
            };
            mark_cell(sx, sy, w, sorted_pos);
        }

        if let Some((sorted_pos, buf_idx, sx, sy)) = vis_sel {
            if let Some(track) = state.air_targets.get(buf_idx) {
                let text = if use_shapes {
                    let (glyph, _) = classify_aircraft_shape(track);
                    format!("[{}]", glyph)
                } else {
                    format!("[+ {}]", track.callsign)
                };
                put_overlay_str(buf, area, sx, sy, &text, TELETEXT_YELLOW, None, true);
                let w = (text.chars().count() as u16).max(1);
                mark_cell(sx, sy, w, sorted_pos);
            }
        }

        for ry in 0..gh {
            let mut rx = 0usize;
            while rx < gw {
                if let Some(target) = cell_hit[ry * gw + rx] {
                    let mut rx2 = rx + 1;
                    while rx2 < gw && cell_hit[ry * gw + rx2] == Some(target) {
                        rx2 += 1;
                    }
                    state.hit_tester.register(
                        Rect {
                            x: area.x + rx as u16,
                            y: area.y + ry as u16,
                            width: (rx2 - rx) as u16,
                            height: 1,
                        },
                        target,
                    );
                    rx = rx2;
                } else {
                    rx += 1;
                }
            }
        }
    }

    // Uydular (*) ve 90 Dakikalik Yörünge İzi (Ground Track) [Katman 3]
    if state.geoint_layers & 4 != 0 {
        let sel_sat_idx = if state.selected_domain == 3 {
            Some(state.selected_row)
        } else {
            None
        };
        if let Some(sidx) = sel_sat_idx {
            if let Some(sat) = state.satellites.get(sidx) {
                let buf = frame.buffer_mut();
                for &(glon, glat) in &sat.ground_track {
                    if let Some((gx, gy)) = to_screen(glon, glat) {
                        if buf[(gx, gy)].symbol() == " " {
                            put_overlay_str(buf, area, gx, gy, ".", TELETEXT_MAGENTA, None, true);
                        }
                    }
                }
            }
        }
        for (idx, sat) in state.satellites.iter().enumerate() {
            if let Some((sx, sy)) = to_screen(sat.lon, sat.lat) {
                let is_sel = sel_sat_idx == Some(idx);
                let text = format!("*{}", sat.name);
                put_overlay_str(
                    frame.buffer_mut(),
                    area,
                    sx,
                    sy,
                    &text,
                    if is_sel { TELETEXT_YELLOW } else { TELETEXT_MAGENTA },
                    None,
                    true,
                );
                let w_hit = (text.chars().count() as u16).max(1);
                state.hit_tester.register(
                    Rect {
                        x: sx,
                        y: sy,
                        width: w_hit.min(area.x + area.width - sx),
                        height: 1,
                    },
                    HitTarget::MapTarget(3, idx),
                );
            }
        }
    }

    // A-3: OpenWeatherMap Anlık Meteoroloji İstasyonları (Opsiyonel Bit 9)
    if state.geoint_layers & 512 != 0 {
        for wp in state.weather_points.iter().chain(state.station_weather.iter()) {
            if let Some((sx, sy)) = to_screen(wp.lon, wp.lat) {
                let lbl = format!("{:.0}C/{:.0}kt", wp.temp_c, wp.wind_kts);
                put_overlay_str(frame.buffer_mut(), area, sx, sy, &lbl, TELETEXT_CYAN, Some(BG_SURFACE), false);
            }
        }
    }

    // N-4: Canlı NOTAM / SIGMET / Çatışma Hava Sahası / GPS-JAM Çokgenleri (Opsiyonel Bit 11)
    if state.geoint_layers & (1 << 11) != 0 {
        for aw in &state.airspace_warnings {
            let col = match aw.kind.as_str() {
                "NOTAM-CONFLICT" => TELETEXT_RED,
                "GPS-JAM" => TELETEXT_MAGENTA,
                _ => TELETEXT_YELLOW,
            };
            for pair in aw.polygon.windows(2) {
                let (lon1, lat1) = pair[0];
                let (lon2, lat2) = pair[1];
                if (lon1 - lon2).abs() > 180.0 {
                    continue;
                }
                let steps = 16;
                for s in 0..=steps {
                    let f = (s as f64) / (steps as f64);
                    let plon = lon1 + (lon2 - lon1) * f;
                    let plat = lat1 + (lat2 - lat1) * f;
                    if let Some((sx, sy)) = to_screen(plon, plat) {
                        let buf = frame.buffer_mut();
                        let sym = if s == 0 { "+" } else { ":" };
                        if buf[(sx, sy)].symbol() == " " || s == 0 {
                            put_overlay_str(buf, area, sx, sy, sym, col, None, true);
                        }
                    }
                }
            }
            if let Some((cx, cy)) = to_screen(aw.center_lon, aw.center_lat) {
                let tag = match aw.kind.as_str() {
                    "NOTAM-CONFLICT" => "[!NTM]",
                    "GPS-JAM" => "[!JAM]",
                    _ => "[!SIG]",
                };
                put_overlay_str(frame.buffer_mut(), area, cx, cy, tag, col, Some(BG_PRIMARY), true);
            }
        }
    }

    // K-4: Çoklu-Sensör Füzyon Kriz Kümeleri (Bit 10)
    if state.geoint_layers & 1024 != 0 {
        for fc in &state.fusion_clusters {
            if let Some((sx, sy)) = to_screen(fc.lon, fc.lat) {
                let lbl = format!("[FUZ:{}]", fc.score);
                put_overlay_str(frame.buffer_mut(), area, sx, sy, &lbl, TELETEXT_RED, Some(BG_PRIMARY), true);
            }
        }
    }

    // A-2: Seçili Uçağın Planlı Kalkış -> Uçak -> Varış Büyük Daire Rota Yayı
    if let Some(sel_id) = &state.selected_target_id {
        if let Some(track) = state.air_targets.iter().find(|t| &t.id == sel_id) {
            let cs_key = track.callsign.trim().to_uppercase();
            if let Some(sched) = state.flight_schedule_cache.get(&cs_key) {
                let draw_arc = |frame: &mut Frame, lon1: f64, lat1: f64, lon2: f64, lat2: f64, col: Color| {
                    let steps = 28;
                    for s in 1..steps {
                        let f = (s as f64) / (steps as f64);
                        let p_lon = lon1 + (lon2 - lon1) * f;
                        let p_lat = lat1 + (lat2 - lat1) * f;
                        if let Some((sx, sy)) = to_screen(p_lon, p_lat) {
                            let buf = frame.buffer_mut();
                            if buf[(sx, sy)].symbol() == " " {
                                put_overlay_str(buf, area, sx, sy, ".", col, None, true);
                            }
                        }
                    }
                };
                if let (Some(olat), Some(olon)) = (sched.origin_lat, sched.origin_lon) {
                    if (olon - track.geo.lon).abs() < 180.0 {
                        draw_arc(frame, olon, olat, track.geo.lon, track.geo.lat, TELETEXT_GREEN);
                    }
                    if let Some((ox, oy)) = to_screen(olon, olat) {
                        let tag = format!("[{}]", sched.origin_code.as_deref().unwrap_or("ORIG"));
                        put_overlay_str(frame.buffer_mut(), area, ox, oy, &tag, TELETEXT_GREEN, Some(BG_PRIMARY), true);
                    }
                }
                if let (Some(dlat), Some(dlon)) = (sched.dest_lat, sched.dest_lon) {
                    if (track.geo.lon - dlon).abs() < 180.0 {
                        draw_arc(frame, track.geo.lon, track.geo.lat, dlon, dlat, TELETEXT_YELLOW);
                    }
                    if let Some((dx, dy)) = to_screen(dlon, dlat) {
                        let tag = format!("[{}]", sched.dest_code.as_deref().unwrap_or("DEST"));
                        put_overlay_str(frame.buffer_mut(), area, dx, dy, &tag, TELETEXT_YELLOW, Some(BG_PRIMARY), true);
                    }
                }
            }
        }
    }

    // KiwiSDR HF Radyo Alıcıları (&) [Katman 8 / Bit 7 — Opsiyonel]
    if state.geoint_layers & 128 != 0 {
        for (idx, node) in state.sdr_nodes.iter().enumerate() {
            if let (Some(nlat), Some(nlon)) = (node.lat, node.lon) {
                if let Some((sx, sy)) = to_screen(nlon, nlat) {
                    let is_active = state.active_sdr_idx == idx;
                    let (label, col) = if is_active {
                        ("[&]", TELETEXT_YELLOW)
                    } else {
                        ("&", TELETEXT_GREEN)
                    };
                    put_overlay_str(frame.buffer_mut(), area, sx, sy, label, col, None, true);
                    state.hit_tester.register(
                        Rect {
                            x: sx,
                            y: sy,
                            width: (label.len() as u16).min(area.x + area.width - sx),
                            height: 1,
                        },
                        HitTarget::MapSdrNode(idx),
                    );
                }
            }
        }
    }

    // 4. Alt Durum Şeridi
    let fov_km = (2.0 * x_half * 111.0 * cos_lat).max(1.0);
    let sdr_legend = if state.geoint_layers & 128 != 0 {
        " [&]SDR"
    } else {
        ""
    };
    let air_legend = if state.map_aircraft_shapes {
        "[▲]Jet [✦]ISR [◈]Tnk [❖]Heli [◆]Gns [▴▸]Dar [▫]Hff"
    } else {
        "[+]Hava"
    };
    let bottom_hud = format!(
        " FOV:{:.0}km | Z:{:.1}x | {:.2}N,{:.2}E | {} [^]Deniz [*]Uydu [@]Deprem [#]Termal{} ",
        fov_km, zoom, state.map_center_lat, state.map_center_lon, air_legend, sdr_legend
    );
    put_overlay_str(
        frame.buffer_mut(),
        area,
        area.x + 1,
        area.y + area.height - 1,
        &bottom_hud,
        FG_SECONDARY,
        Some(BG_PRIMARY),
        false,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use argus_core::types::{Domain, GeoPoint};
    use chrono::Utc;

    fn mock_air(cls: &str, mil: bool, hdg: f64) -> TargetTrack {
        TargetTrack {
            id: "abc123".to_string(),
            callsign: "TEST".to_string(),
            domain: Domain::Air,
            geo: GeoPoint::new(40.0, 29.0, Some(10000.0)),
            speed_kts: Some(450.0),
            heading_deg: Some(hdg),
            vertical_rate_fpm: None,
            squawk: None,
            is_military: mil,
            operator_country: None,
            operator_branch: None,
            airframe_or_vessel_class: Some(cls.to_string()),
            origin: None,
            destination: None,
            vessel_type: None,
            info: String::new(),
            last_seen: Utc::now(),
        }
    }

    #[test]
    fn test_aircraft_platform_shape_classification() {
        assert_eq!(classify_aircraft_shape(&mock_air("F16", true, 90.0)).0, "▲");
        assert_eq!(classify_aircraft_shape(&mock_air("RQ-4B Global Hawk", true, 0.0)).0, "✦");
        assert_eq!(classify_aircraft_shape(&mock_air("C17", true, 120.0)).0, "◈");
        assert_eq!(classify_aircraft_shape(&mock_air("A400", true, 120.0)).0, "◈");
        assert_eq!(classify_aircraft_shape(&mock_air("UH60", true, 45.0)).0, "❖");
        assert_eq!(classify_aircraft_shape(&mock_air("B789", false, 270.0)).0, "◆");
        assert_eq!(classify_aircraft_shape(&mock_air("C172", false, 180.0)).0, "▫");
        assert_eq!(classify_aircraft_shape(&mock_air("C525", false, 180.0)).0, "▫");
        assert_eq!(classify_aircraft_shape(&mock_air("A321", false, 90.0)).0, "▸");
        assert_eq!(classify_aircraft_shape(&mock_air("B738", false, 270.0)).0, "◂");
    }
}
