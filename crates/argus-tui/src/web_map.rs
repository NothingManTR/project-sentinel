use argus_core::state::AppState;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const WEB_MAP_HTML: &str = r#"<!DOCTYPE html>
<html lang="tr">
<head>
<meta charset="utf-8"/>
<title>ARGUS-OSINT // C4ISR Canli Harita</title>
<link rel="stylesheet" href="https://unpkg.com/leaflet@1.9.4/dist/leaflet.css"/>
<script src="https://unpkg.com/leaflet@1.9.4/dist/leaflet.js"></script>
<style>
  html, body, #map { margin: 0; padding: 0; width: 100%; height: 100%; background: #080c14; font-family: monospace; color: #e2e8f0; }
  #hud { position: absolute; top: 10px; left: 50px; z-index: 1000; background: rgba(13,19,31,0.92); border: 1px solid #38bdf8; padding: 8px 14px; font-size: 13px; }
  .lbl { color: #38bdf8; font-weight: bold; font-size: 11px; text-shadow: 1px 1px 2px #000; white-space: nowrap; }
  .mil { color: #ef4444; font-weight: bold; font-size: 11px; text-shadow: 1px 1px 2px #000; white-space: nowrap; }
  .sea { color: #22c55e; font-weight: bold; font-size: 11px; text-shadow: 1px 1px 2px #000; white-space: nowrap; }
  .sat { color: #a855f7; font-weight: bold; font-size: 11px; text-shadow: 1px 1px 2px #000; white-space: nowrap; }
</style>
</head>
<body>
<div id="hud">ARGUS-OSINT C4ISR CANLI HARITA | Baglaniyor...</div>
<div id="map"></div>
<script>
const map = L.map('map').setView([41.0082, 28.9784], 6);
const darkLayer = L.tileLayer('https://{s}.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}{r}.png', { maxZoom: 18, attribution: 'CartoDB / OSM' }).addTo(map);
const satLayer = L.tileLayer('https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}', { maxZoom: 18 });
const osmLayer = L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', { maxZoom: 18 });
L.control.layers({ "Karanlik Taktik (CartoDB)": darkLayer, "Uydu Goruntusu (Esri)": satLayer, "OpenStreetMap": osmLayer }).addTo(map);

const layerGroup = L.layerGroup().addTo(map);
let centeredOnce = false;

async function refresh() {
  try {
    const r = await fetch('/api/state');
    const d = await r.json();
    if (!centeredOnce && d.station_lat && d.station_lon) {
      map.setView([d.station_lat, d.station_lon], 6);
      centeredOnce = true;
    }
    layerGroup.clearLayers();
    L.circleMarker([d.station_lat, d.station_lon], { radius: 6, color: '#38bdf8', fillOpacity: 0.9 })
      .bindPopup('ISTASYON MERKEZI').addTo(layerGroup);

    for (const a of d.air) {
      const cls = a.mil ? 'mil' : 'lbl';
      const col = a.mil ? '#ef4444' : '#38bdf8';
      const icon = L.divIcon({ className: cls, html: '+ ' + a.cs });
      L.marker([a.lat, a.lon], { icon })
        .bindPopup(`<b>${a.cs}</b> (${a.id})<br>${a.info}`)
        .addTo(layerGroup);
      if (a.trail && a.trail.length > 1) {
        L.polyline(a.trail.map(p => [p[1], p[0]]), { color: col, weight: 1, opacity: 0.6 }).addTo(layerGroup);
      }
    }
    for (const s of d.sea) {
      const col = s.mil ? '#ef4444' : '#22c55e';
      const icon = L.divIcon({ className: s.mil ? 'mil' : 'sea', html: '^ ' + s.cs });
      L.marker([s.lat, s.lon], { icon })
        .bindPopup(`<b>${s.cs}</b> (MMSI:${s.id})<br>${s.info}`)
        .addTo(layerGroup);
    }
    for (const sat of d.sats) {
      const icon = L.divIcon({ className: 'sat', html: '* ' + sat.name });
      L.marker([sat.lat, sat.lon], { icon })
        .bindPopup(`<b>${sat.name}</b> (#${sat.norad})<br>Alt: ${sat.alt.toFixed(0)} km`)
        .addTo(layerGroup);
    }
    for (const q of d.quakes) {
      L.circleMarker([q.lat, q.lon], { radius: Math.max(4, q.mag * 2), color: '#f59e0b', fillOpacity: 0.5 })
        .bindPopup(`<b>M${q.mag.toFixed(1)}</b> - ${q.place}<br>Derinlik: ${q.depth} km`)
        .addTo(layerGroup);
    }
    document.getElementById('hud').innerText =
      `ARGUS-OSINT CANLI HARITA | HAVA: ${d.air.length} | DENIZ: ${d.sea.length} | UYDU: ${d.sats.length} | DEPREM: ${d.quakes.length}`;
  } catch (e) {}
}
setInterval(refresh, 1500);
refresh();
</script>
</body>
</html>
"#;

pub fn start_web_map_server(state: Arc<RwLock<AppState>>) {
    tokio::spawn(async move {
        let Ok(listener) = TcpListener::bind("127.0.0.1:8090").await else {
            return;
        };

        while let Ok((mut stream, _)) = listener.accept().await {
            let st_clone = Arc::clone(&state);
            tokio::spawn(async move {
                let mut buf = [0u8; 2048];
                let Ok(n) = stream.read(&mut buf).await else {
                    return;
                };
                if n == 0 {
                    return;
                }
                let req = String::from_utf8_lossy(&buf[..n]);
                if req.starts_with("GET /api/state") {
                    let body = build_state_json(&st_clone);
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(resp.as_bytes()).await;
                } else {
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        WEB_MAP_HTML.len(),
                        WEB_MAP_HTML
                    );
                    let _ = stream.write_all(resp.as_bytes()).await;
                }
            });
        }
    });
}

fn build_state_json(state: &Arc<RwLock<AppState>>) -> String {
    let st = state.read();
    let air: Vec<serde_json::Value> = st
        .air_targets
        .iter()
        .map(|t| {
            let trail = st.target_trails.get(&t.id).cloned().unwrap_or_default();
            serde_json::json!({
                "id": t.id,
                "cs": t.callsign,
                "lat": t.geo.lat,
                "lon": t.geo.lon,
                "mil": t.is_military,
                "info": t.info,
                "trail": trail,
            })
        })
        .collect();

    let sea: Vec<serde_json::Value> = st
        .sea_targets
        .iter()
        .map(|t| {
            serde_json::json!({
                "id": t.id,
                "cs": t.callsign,
                "lat": t.geo.lat,
                "lon": t.geo.lon,
                "mil": t.is_military,
                "info": t.info,
            })
        })
        .collect();

    let sats: Vec<serde_json::Value> = st
        .satellites
        .iter()
        .map(|s| {
            serde_json::json!({
                "norad": s.norad_id,
                "name": s.name,
                "lat": s.lat,
                "lon": s.lon,
                "alt": s.alt_km,
            })
        })
        .collect();

    let quakes: Vec<serde_json::Value> = st
        .earthquakes
        .iter()
        .map(|q| {
            serde_json::json!({
                "place": q.place,
                "mag": q.mag,
                "depth": q.depth_km,
                "lat": q.lat,
                "lon": q.lon,
            })
        })
        .collect();

    serde_json::json!({
        "station_lat": st.station_lat,
        "station_lon": st.station_lon,
        "air": air,
        "sea": sea,
        "sats": sats,
        "quakes": quakes,
    })
    .to_string()
}
