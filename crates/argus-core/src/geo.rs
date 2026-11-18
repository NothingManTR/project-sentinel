//! WGS-84 Jeodezik Mesafe (Haversine) ve Başlangıç Pusula Kerterizi Hesaplamaları

const EARTH_RADIUS_KM: f64 = 6371.0088;

/// İki WGS-84 koordinatı arasındaki büyük daire mesafesini kilometre (km) cinsinden hesaplar
#[inline]
pub fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let rlat1 = lat1.to_radians();
    let rlat2 = lat2.to_radians();

    let a = (dlat * 0.5).sin().powi(2) + rlat1.cos() * rlat2.cos() * (dlon * 0.5).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).max(0.0).sqrt());
    EARTH_RADIUS_KM * c
}

/// İki WGS-84 koordinatı arasındaki mesafeyi deniz mili (NM) cinsinden hesaplar
#[inline]
pub fn haversine_nm(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    haversine_km(lat1, lon1, lat2, lon2) / 1.852
}

/// (lat1, lon1) noktasından (lat2, lon2) noktasına gerçek başlangıç pusula kerterizini (0.0..360.0 derece) hesaplar
#[inline]
pub fn initial_bearing_deg(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let phi1 = lat1.to_radians();
    let phi2 = lat2.to_radians();
    let d_lambda = (lon2 - lon1).to_radians();

    let y = d_lambda.sin() * phi2.cos();
    let x = phi1.cos() * phi2.sin() - phi1.sin() * phi2.cos() * d_lambda.cos();
    y.atan2(x).to_degrees().rem_euclid(360.0)
}

/// Derece cinsinden kerterizi 8 yönlü Türkçe pusula sektör kısaltmasına çevirir
#[inline]
pub fn bearing_sector_tr(deg: f64) -> &'static str {
    let d = deg.rem_euclid(360.0);
    if !(22.5..337.5).contains(&d) {
        "K"
    } else if d < 67.5 {
        "KD"
    } else if d < 112.5 {
        "D"
    } else if d < 157.5 {
        "GD"
    } else if d < 202.5 {
        "G"
    } else if d < 247.5 {
        "GB"
    } else if d < 292.5 {
        "B"
    } else {
        "KB"
    }
}

/// Verilen noktadan belirtilen kerteriz ve mesafede (km) WGS-84 küresel hedef noktayı hesaplar (lon -180..180 normalize)
#[inline]
pub fn dest_point(lat: f64, lon: f64, brg_deg: f64, dist_km: f64) -> (f64, f64) {
    let phi1 = lat.to_radians();
    let lambda1 = lon.to_radians();
    let theta = brg_deg.to_radians();
    let delta = dist_km / EARTH_RADIUS_KM;

    let sin_phi1 = phi1.sin();
    let cos_phi1 = phi1.cos();
    let sin_delta = delta.sin();
    let cos_delta = delta.cos();

    let sin_phi2 = (sin_phi1 * cos_delta + cos_phi1 * sin_delta * theta.cos()).clamp(-1.0, 1.0);
    let phi2 = sin_phi2.asin();
    let y = theta.sin() * sin_delta * cos_phi1;
    let x = cos_delta - sin_phi1 * sin_phi2;
    let lambda2 = lambda1 + y.atan2(x);

    let out_lat = phi2.to_degrees().clamp(-90.0, 90.0);
    let mut out_lon = (lambda2.to_degrees() + 180.0).rem_euclid(360.0) - 180.0;
    if (out_lon + 180.0).abs() < 1e-12 && lambda2.to_degrees() > 0.0 {
        out_lon = 180.0;
    }
    (out_lat, out_lon)
}

#[inline]
fn to_unit_vec(lat_deg: f64, lon_deg: f64) -> [f64; 3] {
    let lat = lat_deg.to_radians();
    let lon = lon_deg.to_radians();
    let cos_lat = lat.cos();
    [cos_lat * lon.cos(), cos_lat * lon.sin(), lat.sin()]
}

#[inline]
fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[inline]
fn norm3(a: [f64; 3]) -> f64 {
    dot3(a, a).sqrt()
}

/// P noktasının A->B büyük daire doğru parçasına en yakın jeodezik mesafesini (km) hesaplar (clamped [0..1])
pub fn cross_track_distance_km(
    p_lat: f64,
    p_lon: f64,
    a_lat: f64,
    a_lon: f64,
    b_lat: f64,
    b_lon: f64,
) -> f64 {
    let ua = to_unit_vec(a_lat, a_lon);
    let ub = to_unit_vec(b_lat, b_lon);
    let up = to_unit_vec(p_lat, p_lon);

    let n = cross3(ua, ub);
    let n_len = norm3(n);
    if n_len < 1e-12 {
        return haversine_km(p_lat, p_lon, a_lat, a_lon)
            .min(haversine_km(p_lat, p_lon, b_lat, b_lon));
    }

    let n_hat = [n[0] / n_len, n[1] / n_len, n[2] / n_len];
    let p_dot_n = dot3(up, n_hat);
    let proj = [
        up[0] - p_dot_n * n_hat[0],
        up[1] - p_dot_n * n_hat[1],
        up[2] - p_dot_n * n_hat[2],
    ];
    let proj_len = norm3(proj);
    if proj_len < 1e-12 {
        return EARTH_RADIUS_KM * std::f64::consts::FRAC_PI_2;
    }

    let uc = [proj[0] / proj_len, proj[1] / proj_len, proj[2] / proj_len];
    let t_hat = cross3(n_hat, ua);
    let theta_ab = n_len.atan2(dot3(ua, ub));
    let ang_ac = dot3(uc, t_hat).atan2(dot3(uc, ua));

    if (0.0..=theta_ab).contains(&ang_ac) {
        let cross_len = norm3(cross3(up, uc));
        EARTH_RADIUS_KM * cross_len.atan2(dot3(up, uc))
    } else {
        haversine_km(p_lat, p_lon, a_lat, a_lon)
            .min(haversine_km(p_lat, p_lon, b_lat, b_lon))
    }
}

#[inline]
fn orient_2d(ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64) -> f64 {
    (bx - ax) * (cy - ay) - (by - ay) * (cx - ax)
}

#[inline]
fn on_segment_2d(ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64) -> bool {
    cx >= ax.min(bx) - 1e-12
        && cx <= ax.max(bx) + 1e-12
        && cy >= ay.min(by) - 1e-12
        && cy <= ay.max(by) + 1e-12
}

/// 2D CCW segment kesişimi ve yönlü geçiş kontrolü (kesiyorsa ileri vektör iç çarpımına göre Some(true/false), yoksa None)
#[allow(clippy::too_many_arguments)]
pub fn segments_intersect_directed(
    p1_lat: f64,
    p1_lon: f64,
    p2_lat: f64,
    p2_lon: f64,
    g1_lat: f64,
    g1_lon: f64,
    g2_lat: f64,
    g2_lon: f64,
    fwd_dlat: f64,
    fwd_dlon: f64,
) -> Option<bool> {
    if (p2_lat - p1_lat).abs() < 1e-12 && (p2_lon - p1_lon).abs() < 1e-12 {
        return None;
    }

    let o1 = orient_2d(p1_lon, p1_lat, p2_lon, p2_lat, g1_lon, g1_lat);
    let o2 = orient_2d(p1_lon, p1_lat, p2_lon, p2_lat, g2_lon, g2_lat);
    let o3 = orient_2d(g1_lon, g1_lat, g2_lon, g2_lat, p1_lon, p1_lat);
    let o4 = orient_2d(g1_lon, g1_lat, g2_lon, g2_lat, p2_lon, p2_lat);

    let intersects = (((o1 > 0.0 && o2 < 0.0) || (o1 < 0.0 && o2 > 0.0))
        && ((o3 > 0.0 && o4 < 0.0) || (o3 < 0.0 && o4 > 0.0)))
        || (o1.abs() <= 1e-12 && on_segment_2d(p1_lon, p1_lat, p2_lon, p2_lat, g1_lon, g1_lat))
        || (o2.abs() <= 1e-12 && on_segment_2d(p1_lon, p1_lat, p2_lon, p2_lat, g2_lon, g2_lat))
        || (o3.abs() <= 1e-12 && on_segment_2d(g1_lon, g1_lat, g2_lon, g2_lat, p1_lon, p1_lat))
        || (o4.abs() <= 1e-12 && on_segment_2d(g1_lon, g1_lat, g2_lon, g2_lat, p2_lon, p2_lat));

    if intersects {
        let dot = (p2_lat - p1_lat) * fwd_dlat + (p2_lon - p1_lon) * fwd_dlon;
        Some(dot >= 0.0)
    } else {
        None
    }
}

/// 4/3 Dünya yarıçapı atmosferik kırılma modeline göre radar görüş hattı ufuk mesafesini (km) hesaplar
#[inline]
pub fn radar_horizon_km(radar_alt_m: f64, target_alt_m: f64) -> f64 {
    4.12 * (radar_alt_m.max(0.0).sqrt() + target_alt_m.max(0.0).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_istanbul_to_ankara_distance_and_bearing() {
        let dist_km = haversine_km(41.0082, 28.9784, 39.9334, 32.8597);
        assert!(dist_km > 340.0 && dist_km < 360.0, "dist_km was {}", dist_km);
        let brg = initial_bearing_deg(41.0082, 28.9784, 39.9334, 32.8597);
        assert!(brg > 100.0 && brg < 115.0, "bearing was {}", brg);
        assert_eq!(bearing_sector_tr(brg), "D");
    }

    #[test]
    fn test_dest_point_and_antimeridian_wrap() {
        let (lat2, lon2) = dest_point(41.0, 29.0, 90.0, 100.0);
        let d = haversine_km(41.0, 29.0, lat2, lon2);
        assert!((d - 100.0).abs() < 0.2, "dist mismatch: {}", d);
        assert!(lon2 > 29.0);

        let (_, lon_wrap) = dest_point(0.0, 179.5, 90.0, 200.0);
        assert!((-180.0..=180.0).contains(&lon_wrap));
        assert!(lon_wrap < 0.0, "expected antimeridian wrap < 0, got {}", lon_wrap);
    }

    #[test]
    fn test_cross_track_distance_clamped() {
        // Segment along equator 0E..10E
        let d_mid = cross_track_distance_km(1.0, 5.0, 0.0, 0.0, 0.0, 10.0);
        assert!((d_mid - 111.195).abs() < 1.0, "d_mid={}", d_mid);

        // Point before segment start -> clamped to A=(0,0)
        let d_before = cross_track_distance_km(0.0, -2.0, 0.0, 0.0, 0.0, 10.0);
        let expected_before = haversine_km(0.0, -2.0, 0.0, 0.0);
        assert!((d_before - expected_before).abs() < 0.1);

        // Point past segment end -> clamped to B=(0,10)
        let d_after = cross_track_distance_km(0.0, 12.0, 0.0, 0.0, 0.0, 10.0);
        let expected_after = haversine_km(0.0, 12.0, 0.0, 10.0);
        assert!((d_after - expected_after).abs() < 0.1);
    }

    #[test]
    fn test_segments_intersect_directed_and_radar_horizon() {
        // Gate across Bosphorus (41.13, 29.02) -> (41.11, 29.12), fwd = south (-1, 0)
        assert_eq!(
            segments_intersect_directed(41.15, 29.07, 41.09, 29.07, 41.13, 29.02, 41.11, 29.12, -1.0, 0.0),
            Some(true)
        );
        assert_eq!(
            segments_intersect_directed(41.09, 29.07, 41.15, 29.07, 41.13, 29.02, 41.11, 29.12, -1.0, 0.0),
            Some(false)
        );
        assert_eq!(
            segments_intersect_directed(41.20, 29.07, 41.16, 29.07, 41.13, 29.02, 41.11, 29.12, -1.0, 0.0),
            None
        );

        let rh = radar_horizon_km(25.0, 10000.0);
        assert!((rh - 4.12 * (5.0 + 100.0)).abs() < 1e-6);
        assert_eq!(radar_horizon_km(-10.0, 0.0), 0.0);
    }
}
