use argus_core::state::AppState;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const WEB_MAP_HTML: &str = r#"<!DOCTYPE html>
<html lang="tr">
<head>
<meta charset="utf-8"/>
<title>Project Sentinel // C4ISR Canli Kuresel Harita</title>
<link rel="stylesheet" href="https://unpkg.com/leaflet@1.9.4/dist/leaflet.css"/>
<script src="https://unpkg.com/leaflet@1.9.4/dist/leaflet.js"></script>
<style>
  html, body, #map { margin: 0; padding: 0; width: 100%; height: 100%; background: #070b14; font-family: monospace; color: #e2e8f0; }
  #hud { position: absolute; top: 10px; left: 50px; z-index: 1000; background: rgba(11,18,32,0.94); border: 1px solid #38bdf8; padding: 8px 12px; font-size: 12px; border-radius: 4px; display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
  select, button { background: #162238; color: #38bdf8; border: 1px solid #334155; padding: 3px 8px; font-family: monospace; cursor: pointer; font-size: 12px; }
  button.active { background: #38bdf8; color: #070b14; font-weight: bold; }
  label { cursor: pointer; user-select: none; }
</style>
</head>
<body>
<div id="hud">
  <span id="stats">PROJECT SENTINEL // Baglaniyor...</span>
  <select id="aor">
    <option value="ALL">Bolge: Kuresel Tumu</option>
    <option value="CENTCOM">CENTCOM AOR</option>
    <option value="TR/EUCOM">TR / EUCOM</option>
    <option value="NORTHCOM">NORTHCOM</option>
    <option value="INDOPACOM">INDOPACOM</option>
    <option value="AFRICOM">AFRICOM</option>
    <option value="SOUTHCOM">SOUTHCOM</option>
  </select>
  <button id="milOnly">Sadece Askeri</button>
  <label><input type="checkbox" id="chkSea" checked/> Deniz</label>
  <label><input type="checkbox" id="chkSat" checked/> Uydu</label>
  <label><input type="checkbox" id="chkGeo" checked/> Deprem/Termal/CBRN</label>
</div>
<div id="map"></div>
<script>
const map = L.map('map', { preferCanvas: true }).setView([36.0, 32.0], 3);
const canvasRenderer = L.canvas({ padding: 0.5 });
const darkLayer = L.tileLayer('https://{s}.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}{r}.png', { maxZoom: 18, attribution: 'CartoDB / OSM' }).addTo(map);
const satLayer = L.tileLayer('https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}', { maxZoom: 18 });
const osmLayer = L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', { maxZoom: 18 });
L.control.layers({ "Karanlik Taktik (CartoDB)": darkLayer, "Uydu (Esri)": satLayer, "OpenStreetMap": osmLayer }).addTo(map);

const layerGroup = L.layerGroup().addTo(map);
let milOnly = false;
document.getElementById('milOnly').onclick = function() {
  milOnly = !milOnly;
  this.classList.toggle('active', milOnly);
  refresh();
};
document.getElementById('aor').onchange = function() {
  const v = this.value;
  if (v === 'CENTCOM') map.setView([26, 46], 5);
  else if (v === 'TR/EUCOM') map.setView([42, 26], 5);
  else if (v === 'ALL') map.setView([20, 15], 2);
  refresh();
};
['chkSea', 'chkSat', 'chkGeo'].forEach(id => document.getElementById(id).onchange = refresh);

async function refresh() {
  try {
    const r = await fetch('/api/state');
    const d = await r.json();
    const aor = document.getElementById('aor').value;
    const showSea = document.getElementById('chkSea').checked;
    const showSat = document.getElementById('chkSat').checked;
    const showGeo = document.getElementById('chkGeo').checked;

    layerGroup.clearLayers();
    L.circleMarker([d.station_lat, d.station_lon], { renderer: canvasRenderer, radius: 6, color: '#38bdf8', fillOpacity: 0.9 })
      .bindPopup('ISTASYON MERKEZI').addTo(layerGroup);

    let airShown = 0;
    for (const a of d.air) {
      if (milOnly && !a.mil) continue;
      if (aor !== 'ALL' && a.aor !== aor) continue;
      airShown++;
      const col = a.mil ? '#ef4444' : '#38bdf8';
      L.circleMarker([a.lat, a.lon], {
        renderer: canvasRenderer,
        radius: a.mil ? 4.5 : 2.3,
        color: col,
        fillColor: col,
        fillOpacity: a.mil ? 0.9 : 0.65,
        weight: a.mil ? 1.5 : 0.8
      })
      .bindPopup(`<b>${a.cs}</b> (${a.id})<br>Bolge: ${a.aor} | Rota: ${a.rt || '-'}<br>${a.info}`)
      .addTo(layerGroup);
    }

    if (showSea) {
      for (const s of d.sea) {
        const col = s.mil ? '#f97316' : '#22c55e';
        L.circleMarker([s.lat, s.lon], { renderer: canvasRenderer, radius: s.mil ? 4.5 : 2.5, color: col, fillColor: col, fillOpacity: 0.85, weight: 1 })
          .bindPopup(`<b>${s.cs}</b> (MMSI:${s.id})<br>${s.info}`)
          .addTo(layerGroup);
      }
    }

    if (showSat) {
      for (const sat of d.sats) {
        L.circleMarker([sat.lat, sat.lon], { renderer: canvasRenderer, radius: 5, color: '#c084fc', fillColor: '#a855f7', fillOpacity: 0.9 })
          .bindPopup(`<b>${sat.name}</b> (#${sat.norad})<br>Alt: ${sat.alt.toFixed(0)} km`)
          .addTo(layerGroup);
      }
    }

    if (showGeo) {
      for (const q of d.quakes) {
        L.circleMarker([q.lat, q.lon], { renderer: canvasRenderer, radius: Math.max(4, q.mag * 1.8), color: '#facc15', fillOpacity: 0.5 })
          .bindPopup(`<b>M${q.mag.toFixed(1)}</b> - ${q.place}<br>Derinlik: ${q.depth} km`)
          .addTo(layerGroup);
      }
      for (const f of d.fires) {
        L.circleMarker([f.lat, f.lon], { renderer: canvasRenderer, radius: 3.5, color: '#fb923c', fillOpacity: 0.7 })
          .bindPopup(`<b>TERMAL</b> ${f.b.toFixed(0)} K`)
          .addTo(layerGroup);
      }
      for (const rad of d.rads) {
        L.circleMarker([rad.lat, rad.lon], { renderer: canvasRenderer, radius: 3, color: '#4ade80', fillOpacity: 0.7 })
          .bindPopup(`<b>SAFECAST</b> #${rad.id}: ${rad.val.toFixed(1)} ${rad.unit}`)
          .addTo(layerGroup);
      }
      for (const j of d.jams) {
        L.circle([j.lat, j.lon], { radius: 85000, color: '#ec4899', fillOpacity: 0.12, weight: 1 })
          .bindPopup(`<b>EW GPS-JAM</b>: ${j.lbl}`).addTo(layerGroup);
      }
    }

    document.getElementById('stats').innerText =
      `SENTINEL CANLI | HAVA: ${airShown}/${d.air.length} | DENIZ: ${d.sea.length} | UYDU: ${d.sats.length} | DEPREM: ${d.quakes.length} | TERMAL: ${d.fires.length}`;
  } catch (e) {}
}
setInterval(refresh, 3000);
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
            serde_json::json!({
                "id": t.id,
                "cs": t.callsign,
                "lat": t.geo.lat,
                "lon": t.geo.lon,
                "mil": t.is_military,
                "aor": t.origin.as_deref().unwrap_or("KURESEL"),
                "rt": t.destination.as_deref().unwrap_or(""),
                "info": t.info,
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

    let fires: Vec<serde_json::Value> = st
        .firms_fires
        .iter()
        .map(|f| serde_json::json!({ "lat": f.lat, "lon": f.lon, "b": f.brightness }))
        .collect();

    let rads: Vec<serde_json::Value> = st
        .radiation_measurements
        .iter()
        .map(|r| serde_json::json!({ "id": r.id, "lat": r.lat, "lon": r.lon, "val": r.value, "unit": r.unit }))
        .collect();

    let jams: Vec<serde_json::Value> = st
        .gps_jamming_zones
        .iter()
        .map(|(lat, lon, _, lbl)| serde_json::json!({ "lat": lat, "lon": lon, "lbl": lbl }))
        .collect();

    serde_json::json!({
        "station_lat": st.station_lat,
        "station_lon": st.station_lon,
        "air": air,
        "sea": sea,
        "sats": sats,
        "quakes": quakes,
        "fires": fires,
        "rads": rads,
        "jams": jams,
    })
    .to_string()
}
