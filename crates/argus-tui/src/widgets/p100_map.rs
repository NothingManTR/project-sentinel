use crate::widgets::world_vectors::{
    is_land_bitmap, NE_10M_REGIONAL_COASTS, NE_50M_BORDERS, NE_50M_COASTLINES, NE_50M_LAKES,
};
use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use argus_core::types::MapRenderStyle;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    symbols::Marker,
    widgets::{
        canvas::{Canvas, Circle, Context, Line as CanvasLine},
        Block, Clear,
    },
    Frame,
};

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
        for w in pts.windows(2) {
            let (x1, y1) = w[0];
            let (x2, y2) = w[1];
            if (x1 < xmin_f && x2 < xmin_f)
                || (x1 > xmax_f && x2 > xmax_f)
                || (y1 < ymin_f && y2 < ymin_f)
                || (y1 > ymax_f && y2 > ymax_f)
            {
                continue;
            }
            ctx.draw(&CanvasLine {
                x1: x1 as f64,
                y1: y1 as f64,
                x2: x2 as f64,
                y2: y2 as f64,
                color,
            });
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
            Color::Rgb(6, 13, 24),
            Color::Rgb(16, 32, 28),
            Color::Rgb(78, 200, 170),
            Color::Rgb(42, 85, 105),
            Color::Rgb(48, 135, 185),
            Color::Rgb(14, 24, 38),
        ),
        MapRenderStyle::HalfBlockDarkTile => (
            Color::Rgb(7, 10, 16),
            Color::Rgb(18, 24, 34),
            Color::Rgb(110, 160, 215),
            Color::Rgb(50, 70, 95),
            Color::Rgb(65, 115, 165),
            Color::Rgb(16, 20, 30),
        ),
        MapRenderStyle::HalfBlockSatellite => (
            Color::Rgb(6, 18, 34),
            Color::Rgb(26, 42, 26),
            Color::Rgb(140, 195, 120),
            Color::Rgb(75, 105, 70),
            Color::Rgb(55, 135, 195),
            Color::Rgb(14, 28, 46),
        ),
        MapRenderStyle::BrailleVector => (
            Color::Rgb(6, 11, 20),
            Color::Rgb(12, 22, 36),
            Color::Rgb(105, 205, 255),
            Color::Rgb(45, 90, 135),
            Color::Rgb(65, 150, 220),
            Color::Rgb(16, 28, 44),
        ),
    };

    let st_lon = state.station_lon;
    let st_lat = state.station_lat;
    let trails_clone = state.target_trails.clone();
    let jamming_clone = state.gps_jamming_zones.clone();

    // 1. Katman: Yüksek Hassasiyetli (2x4 Sub-Pixel Braille) Resmi Natural Earth Vektör Haritası
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

            // B. Uluslararası Ülke Sınırları (Natural Earth 1:50m — 393 kesintisiz sınır segmenti)
            draw_culled_vectors(
                ctx,
                NE_50M_BORDERS,
                x_min,
                x_max,
                y_min,
                y_max,
                border_col,
            );

            // C. Küresel Göller ve İç Denizler (Natural Earth 1:50m — Hazar, Aral, Van, Büyük Göller vb.)
            draw_culled_vectors(
                ctx,
                NE_50M_LAKES,
                x_min,
                x_max,
                y_min,
                y_max,
                lake_col,
            );

            // D. Küresel Kıyı Şeritleri ve Adalar (Natural Earth 1:50m — 1.429 kesintisiz kıyı segmenti)
            draw_culled_vectors(
                ctx,
                NE_50M_COASTLINES,
                x_min,
                x_max,
                y_min,
                y_max,
                coast_col,
            );

            // E. Bölgesel Ultra-Yüksek Çözünürlüklü Kıyılar ve Adalar (Natural Earth 1:10m — 1.078 segment)
            if zoom >= 1.2 {
                draw_culled_vectors(
                    ctx,
                    NE_10M_REGIONAL_COASTS,
                    x_min,
                    x_max,
                    y_min,
                    y_max,
                    coast_col,
                );
            }

            // F. İstanbul Boğazı, Haliç, İzmit Körfezi, Prens Adaları ve Çanakkale Boğazı Mikro-Vektörleri
            if zoom >= 1.8 {
                let fine_col = Color::Rgb(115, 235, 205);
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
                        color: Color::Rgb(28, 52, 76),
                    });
                }
            }

            // H. Canlı Hedef Rota İzleri (Target Trails)
            if zoom >= 1.2 {
                for trail in trails_clone.values() {
                    if trail.len() >= 2 {
                        draw_polyline_f64(ctx, trail, Color::Rgb(45, 115, 165));
                    }
                }
            }

            // I. Canlı ADS-B NIC<5 GPS Karıştırma Halkaları
            for (lat, lon, rad_deg, _) in jamming_clone.iter() {
                ctx.draw(&Circle {
                    x: *lon,
                    y: *lat,
                    radius: *rad_deg,
                    color: Color::Rgb(220, 50, 160),
                });
            }
        });

    frame.render_widget(canvas, area);

    // 2. Katman: Natural Earth 720x360 Kara/Deniz Maskesi ile Hücre Arka Plan Dolgusu
    // (Braille kıyı ve sınır çizgilerini silmeden karaları land_bg, okyanusları ocean_bg ile doldurur!)
    {
        let buf = frame.buffer_mut();
        let w_f = area.width.max(1) as f64;
        let h_f = area.height.max(1) as f64;
        for row in 0..area.height {
            let lat = y_max - ((row as f64 + 0.5) / h_f) * (y_max - y_min);
            for col in 0..area.width {
                let lon = x_min + ((col as f64 + 0.5) / w_f) * (x_max - x_min);
                if is_land_bitmap(lon, lat) {
                    let cell = &mut buf[(area.x + col, area.y + row)];
                    let s = cell.style().bg(land_bg);
                    cell.set_style(s);
                }
            }
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
            Color::Rgb(0, 235, 255),
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
                    Color::Rgb(255, 90, 20),
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

    // Deniz Araçları (^) [Katman 2]
    if state.geoint_layers & 2 != 0 {
        let vessels = state.vessel_list();
        for (idx, track) in vessels.iter().enumerate() {
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
                let text = if is_selected {
                    format!("[^ {}]", track.callsign)
                } else if zoom >= 2.5 || is_warship {
                    format!("^{}", track.callsign)
                } else {
                    "^".to_string()
                };
                put_overlay_str(frame.buffer_mut(), area, sx, sy, &text, col, None, true);
                let w_hit = (text.chars().count() as u16).max(1);
                state.hit_tester.register(
                    Rect {
                        x: sx,
                        y: sy,
                        width: w_hit.min(area.x + area.width - sx),
                        height: 1,
                    },
                    HitTarget::MapTarget(21, idx),
                );
            }
        }
    }

    // Hava Araçları (+) [Katman 1] — Sıfır kopyalama ile sivil -> askeri -> seçili sırası
    if state.geoint_layers & 1 != 0 {
        let air_indices = state.aircraft_sorted_indices();
        let mut reg_hits: Vec<(u16, u16, u16, usize)> = Vec::with_capacity(1024);
        let buf = frame.buffer_mut();
        for &(draw_mil, draw_sel) in &[(false, false), (true, false), (true, true)] {
            for (sorted_pos, &buf_idx) in air_indices.iter().enumerate() {
                let Some(track) = state.air_targets.get(buf_idx) else {
                    continue;
                };
                let is_selected = state.selected_target_id.as_deref() == Some(track.id.as_str());
                let matches_pass = if draw_sel {
                    is_selected
                } else {
                    !is_selected && track.is_military == draw_mil
                };
                if !matches_pass {
                    continue;
                }
                if let Some((sx, sy)) = to_screen(track.geo.lon, track.geo.lat) {
                    let col = if is_selected {
                        TELETEXT_YELLOW
                    } else if track.is_military {
                        Color::Rgb(255, 110, 30)
                    } else {
                        TELETEXT_CYAN
                    };
                    if is_selected {
                        let text = format!("[+ {}]", track.callsign);
                        put_overlay_str(buf, area, sx, sy, &text, col, None, true);
                        let w_hit = (text.chars().count() as u16).max(1);
                        reg_hits.push((sx, sy, w_hit, sorted_pos));
                    } else if zoom >= 2.5 || track.is_military {
                        let text = format!("+{}", track.callsign);
                        put_overlay_str(buf, area, sx, sy, &text, col, None, true);
                        let w_hit = (text.chars().count() as u16).max(1);
                        reg_hits.push((sx, sy, w_hit, sorted_pos));
                    } else {
                        put_overlay_str(buf, area, sx, sy, "+", col, None, true);
                        reg_hits.push((sx, sy, 1, sorted_pos));
                    }
                }
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
                HitTarget::MapTarget(20, sorted_pos),
            );
        }
    }

    // Uydular (*) [Katman 3]
    if state.geoint_layers & 4 != 0 {
        for (idx, sat) in state.satellites.iter().enumerate() {
            if let Some((sx, sy)) = to_screen(sat.lon, sat.lat) {
                let text = format!("*{}", sat.name);
                put_overlay_str(
                    frame.buffer_mut(),
                    area,
                    sx,
                    sy,
                    &text,
                    TELETEXT_MAGENTA,
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

    // 4. Alt Durum Şeridi
    let fov_km = (2.0 * x_half * 111.0 * cos_lat).max(1.0);
    let bottom_hud = format!(
        " FOV:{:.0}km | Z:{:.1}x | {:.2}N,{:.2}E | [+]Hava [^]Deniz [*]Uydu [@]Deprem [#]Termal ",
        fov_km, zoom, state.map_center_lat, state.map_center_lon
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
