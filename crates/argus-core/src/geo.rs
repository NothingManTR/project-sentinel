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
}
