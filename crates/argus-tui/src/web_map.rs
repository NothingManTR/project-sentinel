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
  html, body, #map { margin: 0; padding: 0; width: 100%; height: 100%; background: #0f1115; font-family: monospace; color: #dce0e6; }
  #hud { position: absolute; top: 10px; left: 50px; z-index: 1000; background: rgba(21,24,30,0.95); border: 1px solid #6e86a0; padding: 8px 12px; font-size: 12px; border-radius: 4px; display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
  select, button { background: #1e232b; color: #76a2c2; border: 1px solid #343b46; padding: 3px 8px; font-family: monospace; cursor: pointer; font-size: 12px; }
  button.active { background: #ccaa60; color: #0f1115; font-weight: bold; }
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
  <label><input type="checkbox" id="chkCables"/> Denizalti Kablo</label>
  <label><input type="checkbox" id="chkNotam"/> NOTAM/Hava Sahasi</label>
  <label><input type="checkbox" id="chkShapes"/> Platform Sekilleri</label>
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
const cableGroup = L.layerGroup().addTo(map);
let cablesLoaded = false;
const shapeCanvas = document.createElement('canvas');
shapeCanvas.style.cssText = 'position:absolute;top:0;left:0;pointer-events:none;z-index:450;';
map.getContainer().appendChild(shapeCanvas);
let visibleAirForShapes = [];

function classifyKind(a) {
  const txt = ((a.cls || '') + ' ' + (a.info || '')).toUpperCase();
  if (/\b(RQ4|RQ-4|MQ9|MQ-9|MQ1|E3|E-3|E4|E-4|E6|E8|RC135|AWACS|P8|P-8|U2|B52)\b|GLOBAL HAWK|REAPER|BAYRAKTAR|ANKA/.test(txt)) return 'isr';
  if (/\b(H60|UH60|S70|S76|S92|EC35|EC45|H135|H145|A139|CH47|AH64|LYNX|MI8|T129|V22)|AS3|AW1|BELL|R44|R66|HELI/.test(txt)) return 'heli';
  if (/\b(KC135|KC46|KC10|MRTT|TANKER|C17|C5|C130|C30J|A400|IL76|C295|CN35)\b/.test(txt)) return 'tanker';
  if (/\b(F16|F-16|F15|F18|F22|F35|EF20|EUFI|RAFAL|SU27|SU30|SU35|MIG|T38|M346|TEX2|FA-50|GRIP)\b/.test(txt)) return 'fighter';
  if (/B74|A38|B77|B78|A35|A33|A34|B76|MD11/.test(txt)) return 'wide';
  if (/ATR|AT7|DH8|PC12|PC21|C172|C182|C208|C525|C56|SR22|DA40|GLF|G550|G650|CL60|LJ|E55P/.test(txt)) return 'light';
  return a.mil ? 'fighter' : 'narrow';
}

function drawShape(ctx, x, y, kind, hdg, col) {
  ctx.save();
  ctx.translate(x, y);
  ctx.rotate(((hdg || 0) * Math.PI) / 180);
  ctx.fillStyle = col;
  ctx.strokeStyle = col;
  ctx.lineWidth = 1.3;
  ctx.beginPath();
  if (kind === 'fighter') {
    ctx.moveTo(0, -8); ctx.lineTo(5, 4); ctx.lineTo(0, 2); ctx.lineTo(-5, 4); ctx.closePath(); ctx.fill();
  } else if (kind === 'isr') {
    ctx.moveTo(0, -7); ctx.lineTo(0, 7); ctx.moveTo(-10, 0); ctx.lineTo(10, 0); ctx.moveTo(-3, 5); ctx.lineTo(3, 5); ctx.stroke();
  } else if (kind === 'tanker') {
    ctx.moveTo(0, -8); ctx.lineTo(0, 7); ctx.moveTo(-9, -1); ctx.lineTo(9, 2); ctx.moveTo(-4, 6); ctx.lineTo(4, 6); ctx.stroke();
  } else if (kind === 'heli') {
    ctx.moveTo(-6, -6); ctx.lineTo(6, 6); ctx.moveTo(-6, 6); ctx.lineTo(6, -6); ctx.stroke();
    ctx.beginPath(); ctx.arc(0, 0, 2.2, 0, Math.PI * 2); ctx.fill();
  } else if (kind === 'wide') {
    ctx.moveTo(0, -8); ctx.lineTo(2, -2); ctx.lineTo(9, 2); ctx.lineTo(2, 2); ctx.lineTo(1, 7); ctx.lineTo(4, 8); ctx.lineTo(-4, 8); ctx.lineTo(-1, 7); ctx.lineTo(-2, 2); ctx.lineTo(-9, 2); ctx.lineTo(-2, -2); ctx.closePath(); ctx.fill();
  } else if (kind === 'light') {
    ctx.moveTo(0, -5); ctx.lineTo(5, 1); ctx.lineTo(-5, 1); ctx.closePath(); ctx.fill();
  } else {
    ctx.moveTo(0, -6); ctx.lineTo(5, 3); ctx.lineTo(0, 1); ctx.lineTo(-5, 3); ctx.closePath(); ctx.fill();
  }
  ctx.restore();
}

function redrawShapesOverlay() {
  const size = map.getSize();
  shapeCanvas.width = size.x;
  shapeCanvas.height = size.y;
  const ctx = shapeCanvas.getContext('2d');
  ctx.clearRect(0, 0, size.x, size.y);
  if (!document.getElementById('chkShapes').checked) return;
  for (const a of visibleAirForShapes) {
    const pt = map.latLngToContainerPoint([a.lat, a.lon]);
    if (pt.x < -15 || pt.y < -15 || pt.x > size.x + 15 || pt.y > size.y + 15) continue;
    const kind = classifyKind(a);
    const col = a.mil ? '#e06c58' : (kind === 'wide' ? '#76a2c2' : (kind === 'heli' ? '#6aaa80' : '#9cc2dd'));
    drawShape(ctx, pt.x, pt.y, kind, a.hdg, col);
  }
}

map.on('move zoom resize', redrawShapesOverlay);
map.on('click', (ev) => {
  if (!document.getElementById('chkShapes').checked) return;
  const cp = map.latLngToContainerPoint(ev.latlng);
  let best = null, bestD = 12;
  for (const a of visibleAirForShapes) {
    const pt = map.latLngToContainerPoint([a.lat, a.lon]);
    const d = Math.hypot(pt.x - cp.x, pt.y - cp.y);
    if (d < bestD) { bestD = d; best = a; }
  }
  if (best) {
    L.popup().setLatLng([best.lat, best.lon])
      .setContent(`<b>${best.cs}</b> (${best.id})<br>Model: ${best.cls || '-'} | Bolge: ${best.aor} | Rota: ${best.rt || '-'}<br>${best.info}`)
      .openOn(map);
  }
});

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
['chkSea', 'chkSat', 'chkGeo', 'chkNotam', 'chkShapes'].forEach(id => document.getElementById(id).onchange = refresh);
document.getElementById('chkCables').onchange = async function() {
  if (!this.checked) { cableGroup.clearLayers(); cablesLoaded = false; return; }
  if (!cablesLoaded) {
    try {
      const r = await fetch('/api/cables');
      const cables = await r.json();
      cableGroup.clearLayers();
      for (const c of cables) {
        for (const seg of c.lines) {
          L.polyline(seg.map(p => [p[1], p[0]]), { renderer: canvasRenderer, color: '#3a807e', weight: 1.2, opacity: 0.75 })
            .bindPopup(`<b>KABLO:</b> ${c.name}`).addTo(cableGroup);
        }
      }
      cablesLoaded = true;
    } catch (e) {}
  }
};

async function refresh() {
  try {
    const r = await fetch('/api/state');
    const d = await r.json();
    const aor = document.getElementById('aor').value;
    const showSea = document.getElementById('chkSea').checked;
    const showSat = document.getElementById('chkSat').checked;
    const showGeo = document.getElementById('chkGeo').checked;
    const useShapes = document.getElementById('chkShapes').checked;

    layerGroup.clearLayers();
    visibleAirForShapes = [];
    L.circleMarker([d.station_lat, d.station_lon], { renderer: canvasRenderer, radius: 6, color: '#ccaa60', fillOpacity: 0.9 })
      .bindPopup('ISTASYON MERKEZI').addTo(layerGroup);

    let airShown = 0;
    for (const a of d.air) {
      if (milOnly && !a.mil) continue;
      if (aor !== 'ALL' && a.aor !== aor) continue;
      airShown++;
      if (useShapes) {
        visibleAirForShapes.push(a);
        continue;
      }
      const col = a.mil ? '#d28452' : '#76a2c2';
      L.circleMarker([a.lat, a.lon], {
        renderer: canvasRenderer,
        radius: a.mil ? 4.5 : 2.3,
        color: col,
        fillColor: col,
        fillOpacity: a.mil ? 0.9 : 0.65,
        weight: a.mil ? 1.5 : 0.8
      })
      .bindPopup(`<b>${a.cs}</b> (${a.id})<br>Model: ${a.cls || '-'} | Bolge: ${a.aor} | Rota: ${a.rt || '-'}<br>${a.info}`)
      .addTo(layerGroup);
    }
    redrawShapesOverlay();

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
        L.circle([j.lat, j.lon], { radius: 85000, color: '#b86088', fillOpacity: 0.12, weight: 1 })
          .bindPopup(`<b>EW GPS-JAM</b>: ${j.lbl}`).addTo(layerGroup);
      }
    }

    if (d.fusions) {
      for (const fc of d.fusions) {
        L.circle([fc.lat, fc.lon], { radius: 130000, color: '#c45e5a', fillOpacity: 0.18, weight: 1.5 })
          .bindPopup(`<b>FUZYON SKOR ${fc.score}</b> (${fc.domains.join('+')})<br>${fc.summary}`).addTo(layerGroup);
      }
    }
    if (d.weather) {
      for (const w of d.weather) {
        L.circleMarker([w.lat, w.lon], { renderer: canvasRenderer, radius: 5, color: '#76a2c2', fillOpacity: 0.5 })
          .bindPopup(`<b>${w.name}</b>: ${w.temp.toFixed(1)}C | ${w.wind.toFixed(0)}kt | ${w.cond}`).addTo(layerGroup);
      }
    }
    if (document.getElementById('chkNotam').checked && d.airspace_warnings) {
      for (const aw of d.airspace_warnings) {
        if (!aw.polygon || aw.polygon.length < 3) continue;
        const col = aw.kind === 'NOTAM-CONFLICT' ? '#e06c58' : (aw.kind === 'GPS-JAM' ? '#b86088' : '#ccaa60');
        L.polygon(aw.polygon.map(p => [p[1], p[0]]), { renderer: canvasRenderer, color: col, fillOpacity: 0.14, weight: 1.5, dashArray: '5,4' })
          .bindPopup(`<b>[${aw.kind}] ${aw.fir}</b><br>${aw.hazard}<br>${aw.summary}`).addTo(layerGroup);
      }
    }

    const defTxt = d.defcon ? `DEFCON ${d.defcon}` : 'DEFCON:--';
    document.getElementById('stats').innerText =
      `SENTINEL | ${defTxt} | HAVA: ${airShown}/${d.air.length} | DENIZ: ${d.sea.length} | UYDU: ${d.sats.length} | FUZ: ${(d.fusions || []).length}`;
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
                if req.starts_with("GET /api/cables") {
                    let body = build_cables_json(&st_clone);
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(resp.as_bytes()).await;
                } else if req.starts_with("GET /api/state") {
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
                "cls": t.airframe_or_vessel_class.as_deref().unwrap_or(""),
                "hdg": t.heading_deg.unwrap_or(0.0),
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

    let fusions: Vec<serde_json::Value> = st
        .fusion_clusters
        .iter()
        .map(|f| serde_json::json!({ "lat": f.lat, "lon": f.lon, "score": f.score, "domains": f.domains, "summary": f.summary }))
        .collect();

    let weather: Vec<serde_json::Value> = st
        .weather_points
        .iter()
        .chain(st.station_weather.iter())
        .map(|w| serde_json::json!({ "name": w.name, "lat": w.lat, "lon": w.lon, "temp": w.temp_c, "wind": w.wind_kts, "cond": w.condition }))
        .collect();

    let airspace_warnings: Vec<serde_json::Value> = st
        .airspace_warnings
        .iter()
        .map(|aw| {
            serde_json::json!({
                "id": aw.id,
                "fir": aw.fir_or_zone,
                "kind": aw.kind,
                "hazard": aw.hazard,
                "polygon": aw.polygon,
                "summary": aw.summary,
            })
        })
        .collect();

    serde_json::json!({
        "station_lat": st.station_lat,
        "station_lon": st.station_lon,
        "defcon": st.defcon_status.as_ref().map(|d| d.overall_level),
        "air": air,
        "sea": sea,
        "sats": sats,
        "quakes": quakes,
        "fires": fires,
        "rads": rads,
        "jams": jams,
        "fusions": fusions,
        "weather": weather,
        "airspace_warnings": airspace_warnings,
    })
    .to_string()
}

fn build_cables_json(state: &Arc<RwLock<AppState>>) -> String {
    let st = state.read();
    let list: Vec<serde_json::Value> = st
        .subsea_cables
        .iter()
        .map(|c| serde_json::json!({ "name": c.name, "lines": c.lines }))
        .collect();
    serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string())
}
