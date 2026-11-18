use argus_core::state::AppState;
use argus_core::types::Domain;
use parking_lot::RwLock;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{timeout, Duration};

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
    selectTarget(best.id);
    L.popup().setLatLng([best.lat, best.lon])
      .setContent(`<b>${best.cs}</b> (${best.id})<br>Model: ${best.cls || '-'} | Bolge: ${best.aor} | Rota: ${best.rt || '-'}<br>${best.info}`)
      .openOn(map);
  }
});

function selectTarget(id) {
  if (!id) return;
  fetch('/api/select', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ id: String(id) })
  }).catch(() => {});
}

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
      .on('click', () => selectTarget(a.id))
      .addTo(layerGroup);
    }
    redrawShapesOverlay();

    if (showSea) {
      for (const s of d.sea) {
        const col = s.mil ? '#f97316' : '#22c55e';
        L.circleMarker([s.lat, s.lon], { renderer: canvasRenderer, radius: s.mil ? 4.5 : 2.5, color: col, fillColor: col, fillOpacity: 0.85, weight: 1 })
          .bindPopup(`<b>${s.cs}</b> (MMSI:${s.id})<br>${s.info}`)
          .on('click', () => selectTarget(s.id))
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
      if (d.conflicts) {
        for (const c of d.conflicts) {
          L.circleMarker([c.lat, c.lon], { renderer: canvasRenderer, radius: 4.5, color: '#ef4444', fillColor: '#dc2626', fillOpacity: 0.85, weight: 1.5 })
            .bindPopup(`<b>[CATISMA] ${c.type}</b> (${c.country})<br>${c.location} | Aktor: ${c.actor1}<br>${c.notes}`)
            .addTo(layerGroup);
        }
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

    if (d.dark_ships) {
      for (const ds of d.dark_ships) {
        L.polyline([[ds.last_lat, ds.last_lon], [ds.dr_lat, ds.dr_lon]], {
          renderer: canvasRenderer, color: '#ef4444', weight: 1.5, dashArray: '4,4', opacity: 0.85
        }).addTo(layerGroup);
        L.circle([ds.dr_lat, ds.dr_lon], {
          radius: 25000, color: '#c026d3', fillColor: '#ef4444', fillOpacity: 0.15, weight: 1.5, dashArray: '5,4'
        }).addTo(layerGroup);
        L.circleMarker([ds.dr_lat, ds.dr_lon], {
          renderer: canvasRenderer, radius: 5, color: '#ef4444', fillColor: '#c026d3', fillOpacity: 0.9, weight: 1.5
        })
        .bindPopup(`<b>[?DARK] ${ds.cs}</b> (${ds.id})<br>Sessiz: ${ds.silent_mins || 0} dk | Hiz: ${(ds.sog || 0).toFixed(1)} kt | Rota: ${(ds.cog || 0).toFixed(0)}°`)
        .on('click', () => selectTarget(ds.id))
        .addTo(layerGroup);
      }
    }
    if (d.aar_pairs) {
      for (const p of d.aar_pairs) {
        L.polyline([[p.t_lat, p.t_lon], [p.r_lat, p.r_lon]], {
          renderer: canvasRenderer, color: '#facc15', weight: 2, dashArray: '6,4', opacity: 0.9
        }).addTo(layerGroup);
        L.circleMarker([p.lat, p.lon], {
          renderer: canvasRenderer, radius: 4, color: '#facc15', fillColor: '#ccaa60', fillOpacity: 0.9, weight: 1.5
        })
        .bindPopup(`<b>[AAR${p.locked ? ' KILITLI' : ''}]</b> ${p.tanker} &lt;-&gt; ${p.receiver}<br>Mesafe: ${(p.dist_km || 0).toFixed(1)} km | Irtifa: ${(p.alt_ft || 0).toFixed(0)} ft`)
        .on('click', () => selectTarget(p.tanker_id))
        .addTo(layerGroup);
      }
    }
    if (d.strait_gates) {
      for (const g of d.strait_gates) {
        if (g.gate_a && g.gate_b) {
          L.polyline([g.gate_a, g.gate_b], {
            renderer: canvasRenderer, color: '#ccaa60', weight: 2.5, dashArray: '3,3', opacity: 0.85
          }).addTo(layerGroup);
        }
        L.circleMarker([g.lat, g.lon], {
          renderer: canvasRenderer, radius: 4, color: '#ccaa60', fillColor: '#1e232b', fillOpacity: 0.9, weight: 1.5
        })
        .bindPopup(`<b>[BOGAZ] ${g.name}</b><br>${g.fwd_label}: ${g.fwd} | ${g.rev_label}: ${g.rev}<br>Askeri: ${g.mil} | Tanker: ${g.tanker}`)
        .addTo(layerGroup);
      }
    }
    if (d.tactical_ruler && d.tactical_ruler.pin_a) {
      const pa = d.tactical_ruler.pin_a;
      L.circleMarker([pa.lat, pa.lon], {
        renderer: canvasRenderer, radius: 5, color: '#facc15', fillColor: '#facc15', fillOpacity: 0.95, weight: 2
      }).bindPopup(`<b>[PIN A] ${pa.label}</b><br>${pa.lat.toFixed(4)}N, ${pa.lon.toFixed(4)}E`).addTo(layerGroup);
      if (d.tactical_ruler.threat_ring_km) {
        L.circle([pa.lat, pa.lon], {
          radius: d.tactical_ruler.threat_ring_km * 1000.0,
          color: '#ef4444', fillColor: '#ef4444', fillOpacity: 0.08, weight: 1.5, dashArray: '6,4'
        }).bindPopup(`<b>TEHDIT HALKASI: ${d.tactical_ruler.threat_ring_km.toFixed(0)} km</b>`).addTo(layerGroup);
      }
      if (d.tactical_ruler.pin_b) {
        const pb = d.tactical_ruler.pin_b;
        L.circleMarker([pb.lat, pb.lon], {
          renderer: canvasRenderer, radius: 5, color: '#38bdf8', fillColor: '#38bdf8', fillOpacity: 0.95, weight: 2
        }).bindPopup(`<b>[PIN B] ${pb.label}</b><br>${pb.lat.toFixed(4)}N, ${pb.lon.toFixed(4)}E`).addTo(layerGroup);
        L.polyline([[pa.lat, pa.lon], [pb.lat, pb.lon]], {
          renderer: canvasRenderer, color: '#facc15', weight: 2, dashArray: '6,5', opacity: 0.9
        }).addTo(layerGroup);
      }
    }

    const defTxt = d.defcon ? `DEFCON ${d.defcon}` : 'DEFCON:--';
    document.getElementById('stats').innerText =
      `SENTINEL | ${defTxt} | HAVA: ${airShown}/${d.air.length} | DENIZ: ${d.sea.length} | DARK: ${(d.dark_ships || []).length} | AAR: ${(d.aar_pairs || []).length} | UYDU: ${d.sats.length} | FUZ: ${(d.fusions || []).length}`;
  } catch (e) {}
}
setInterval(refresh, 3000);
refresh();
</script>
</body>
</html>
"#;

async fn read_http_request(stream: &mut TcpStream) -> Option<(String, Vec<u8>)> {
    let mut buf: Vec<u8> = Vec::with_capacity(2048);
    let mut tmp = [0u8; 2048];
    let mut header_end: Option<usize> = None;
    let mut content_len: usize = 0;

    loop {
        let n = timeout(Duration::from_millis(1500), stream.read(&mut tmp))
            .await
            .ok()?
            .ok()?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);

        if header_end.is_none() {
            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                header_end = Some(pos);
                let hdr_text = String::from_utf8_lossy(&buf[..pos]);
                for line in hdr_text.lines() {
                    if let Some((k, v)) = line.split_once(':') {
                        if k.trim().eq_ignore_ascii_case("content-length") {
                            if let Ok(len) = v.trim().parse::<usize>() {
                                content_len = len.min(65536);
                            }
                        }
                    }
                }
            }
        }

        if let Some(hend) = header_end {
            if buf.len() >= hend + 4 + content_len {
                break;
            }
        }
        if buf.len() >= 65536 {
            break;
        }
    }

    let hend = header_end?;
    let head = String::from_utf8_lossy(&buf[..hend]).into_owned();
    let body_start = hend + 4;
    let body_end = (body_start + content_len).min(buf.len());
    Some((head, buf[body_start..body_end].to_vec()))
}

async fn handle_web_connection(mut stream: TcpStream, st_clone: Arc<RwLock<AppState>>) {
    let Some((head, body_bytes)) = read_http_request(&mut stream).await else {
        return;
    };
    let req_line = head.lines().next().unwrap_or("");
    if req_line.starts_with("OPTIONS ") {
        let resp = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n\r\n";
        let _ = stream.write_all(resp.as_bytes()).await;
    } else if req_line.starts_with("POST /api/select") {
        let id_opt = serde_json::from_slice::<Value>(&body_bytes)
            .ok()
            .and_then(|v| {
                v.get("id").and_then(|id_val| {
                    id_val
                        .as_str()
                        .map(|s| s.trim().to_string())
                        .or_else(|| id_val.as_u64().map(|n| n.to_string()))
                })
            })
            .filter(|s| !s.is_empty());

        if let Some(target_id) = id_opt {
            {
                let mut st = st_clone.write();
                st.selected_target_id = Some(target_id.clone());
                if st.follow_target_id.as_deref() != Some(target_id.as_str()) {
                    st.follow_target_id = None;
                }
                let air_t = st.air_targets.iter().find(|t| t.id == target_id).cloned();
                let sea_t = if air_t.is_none() {
                    st.sea_targets.iter().find(|t| t.id == target_id).cloned()
                } else {
                    None
                };
                let dark_t = if air_t.is_none() && sea_t.is_none() {
                    st.dark_ships.iter().find(|d| d.id == target_id).cloned()
                } else {
                    None
                };
                let any_t = if air_t.is_none() && sea_t.is_none() && dark_t.is_none() {
                    st.targets.iter().find(|t| t.id == target_id).cloned()
                } else {
                    None
                };

                if let Some(t) = air_t {
                    st.map_center_lat = t.geo.lat;
                    st.map_center_lon = t.geo.lon;
                    st.map_zoom = st.map_zoom.max(3.5);
                    st.selected_domain = 20;
                    st.ensure_air_indices();
                    if let Some(pos) = st.cached_air_indices.iter().position(|&bi| {
                        st.air_targets.get(bi).is_some_and(|x| x.id == target_id)
                    }) {
                        st.air_scroll = pos;
                        st.selected_row = pos;
                    }
                    st.live_status = format!("WEB SECIM: {} ({})", t.callsign, t.id);
                } else if let Some(t) = sea_t {
                    st.map_center_lat = t.geo.lat;
                    st.map_center_lon = t.geo.lon;
                    st.map_zoom = st.map_zoom.max(3.5);
                    st.selected_domain = 21;
                    st.ensure_sea_indices();
                    if let Some(pos) = st.cached_sea_indices.iter().position(|&bi| {
                        st.sea_targets.get(bi).is_some_and(|x| x.id == target_id)
                    }) {
                        st.sea_scroll = pos;
                        st.selected_row = pos;
                    }
                    st.live_status = format!("WEB SECIM: {} ({})", t.callsign, t.id);
                } else if let Some(ds) = dark_t {
                    st.map_center_lat = ds.dr_lat;
                    st.map_center_lon = ds.dr_lon;
                    st.map_zoom = st.map_zoom.max(3.5);
                    st.live_status =
                        format!("WEB DARK-SHIP SECIM: {} ({})", ds.callsign, ds.id);
                } else if let Some(t) = any_t {
                    st.map_center_lat = t.geo.lat;
                    st.map_center_lon = t.geo.lon;
                    st.map_zoom = st.map_zoom.max(3.5);
                    st.selected_domain = match t.domain {
                        Domain::Air => 20,
                        Domain::Sea => 21,
                        _ => 2,
                    };
                    st.live_status = format!("WEB SECIM: {} ({})", t.callsign, t.id);
                }
                st.total_events_received = st.total_events_received.wrapping_add(1);
            }
            let resp_body = json!({ "ok": true, "selected": target_id }).to_string();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{}",
                resp_body.len(),
                resp_body
            );
            let _ = stream.write_all(resp.as_bytes()).await;
        } else {
            let resp_body = r#"{"ok":false,"error":"invalid id"}"#;
            let resp = format!(
                "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{}",
                resp_body.len(),
                resp_body
            );
            let _ = stream.write_all(resp.as_bytes()).await;
        }
    } else if req_line.starts_with("GET /api/cables") {
        let body = build_cables_json(&st_clone);
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(resp.as_bytes()).await;
    } else if req_line.starts_with("GET /api/state") {
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
}

pub fn start_web_map_server(state: Arc<RwLock<AppState>>) {
    tokio::spawn(async move {
        let Ok(listener) = TcpListener::bind("127.0.0.1:8090").await else {
            return;
        };

        while let Ok((stream, _)) = listener.accept().await {
            let st_clone = Arc::clone(&state);
            tokio::spawn(handle_web_connection(stream, st_clone));
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

    let conflicts: Vec<Value> = st
        .acled_events
        .iter()
        .map(|c| {
            json!({
                "id": c.event_id,
                "type": c.event_type,
                "country": c.country,
                "location": c.location,
                "actor1": c.actor1,
                "lat": c.lat,
                "lon": c.lon,
                "notes": c.notes,
            })
        })
        .collect();

    let dark_ships: Vec<Value> = st
        .dark_ships
        .iter()
        .map(|ds| {
            json!({
                "id": ds.id,
                "cs": ds.callsign,
                "last_lat": ds.last_lat,
                "last_lon": ds.last_lon,
                "dr_lat": ds.dr_lat,
                "dr_lon": ds.dr_lon,
                "sog": ds.sog_kts,
                "cog": ds.cog_deg,
                "silent_mins": ds.silent_mins,
                "mil": ds.is_military,
                "vtype": ds.vessel_type.as_deref().unwrap_or(""),
            })
        })
        .collect();

    let aar_pairs: Vec<Value> = st
        .aar_pairs
        .iter()
        .map(|p| {
            let (t_lat, t_lon) = st
                .air_targets
                .iter()
                .find(|t| t.id == p.tanker_id)
                .map(|t| (t.geo.lat, t.geo.lon))
                .unwrap_or((p.lat, p.lon));
            let (r_lat, r_lon) = st
                .air_targets
                .iter()
                .find(|t| t.id == p.receiver_id)
                .map(|t| (t.geo.lat, t.geo.lon))
                .unwrap_or((p.lat, p.lon));
            json!({
                "tanker_id": p.tanker_id,
                "tanker": p.tanker_callsign,
                "receiver_id": p.receiver_id,
                "receiver": p.receiver_callsign,
                "lat": p.lat,
                "lon": p.lon,
                "t_lat": t_lat,
                "t_lon": t_lon,
                "r_lat": r_lat,
                "r_lon": r_lon,
                "alt_ft": p.alt_ft,
                "dist_km": p.dist_km,
                "locked": p.is_locked,
            })
        })
        .collect();

    let strait_gates: Vec<Value> = st
        .strait_gates
        .iter()
        .map(|g| {
            json!({
                "id": g.id,
                "name": g.name,
                "lat": g.center_lat,
                "lon": g.center_lon,
                "gate_a": [g.gate_a.0, g.gate_a.1],
                "gate_b": [g.gate_b.0, g.gate_b.1],
                "fwd_label": g.fwd_label,
                "rev_label": g.rev_label,
                "fwd": g.fwd_count,
                "rev": g.rev_count,
                "mil": g.mil_count,
                "tanker": g.tanker_count,
            })
        })
        .collect();

    let recon_windows: Vec<Value> = st
        .recon_windows
        .iter()
        .map(|rw| {
            json!({
                "sat": rw.sat_name,
                "norad": rw.norad_id,
                "sensor": rw.sensor_type,
                "target": rw.target_name,
                "lat": rw.target_lat,
                "lon": rw.target_lon,
                "aos_secs": rw.aos_secs,
                "max_el": rw.max_el_deg,
                "cloud_blocked": rw.cloud_blocked,
            })
        })
        .collect();

    let tactical_ruler: Value = match &st.tactical_ruler {
        Some(r) => json!({
            "pin_a": { "lat": r.pin_a.0, "lon": r.pin_a.1, "label": r.pin_a.2 },
            "pin_b": r.pin_b.as_ref().map(|b| json!({ "lat": b.0, "lon": b.1, "label": b.2 })),
            "threat_ring_km": r.threat_ring_km,
        }),
        None => Value::Null,
    };

    json!({
        "station_lat": st.station_lat,
        "station_lon": st.station_lon,
        "selected_target_id": st.selected_target_id,
        "defcon": st.defcon_status.as_ref().map(|d| d.overall_level),
        "air": air,
        "sea": sea,
        "sats": sats,
        "quakes": quakes,
        "fires": fires,
        "rads": rads,
        "jams": jams,
        "conflicts": conflicts,
        "fusions": fusions,
        "weather": weather,
        "airspace_warnings": airspace_warnings,
        "dark_ships": dark_ships,
        "aar_pairs": aar_pairs,
        "strait_gates": strait_gates,
        "recon_windows": recon_windows,
        "tactical_ruler": tactical_ruler,
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

#[cfg(test)]
mod tests {
    use super::*;
    use argus_core::types::{
        AarPair, DarkShipTrack, GeoPoint, ReconWindow, TacticalRuler, TargetTrack,
    };
    use argus_core::AppConfig;
    use chrono::Utc;

    #[tokio::test]
    async fn test_web_map_state_json_and_post_select() {
        let state = Arc::new(RwLock::new(AppState::new(AppConfig::default())));
        {
            let mut st = state.write();
            st.upsert_target(TargetTrack {
                id: "4ba123".to_string(),
                callsign: "THY1".to_string(),
                domain: Domain::Air,
                geo: GeoPoint::new(41.05, 29.05, Some(9000.0)),
                speed_kts: Some(420.0),
                heading_deg: Some(90.0),
                vertical_rate_fpm: None,
                squawk: None,
                is_military: true,
                operator_country: None,
                operator_branch: None,
                airframe_or_vessel_class: Some("F16".to_string()),
                origin: None,
                destination: None,
                vessel_type: None,
                info: "TEST".to_string(),
                last_seen: Utc::now(),
            });
            st.dark_ships.push(DarkShipTrack {
                id: "mmsi-999".to_string(),
                callsign: "SHADOW1".to_string(),
                last_lat: 41.2,
                last_lon: 29.1,
                dr_lat: 41.3,
                dr_lon: 29.2,
                sog_kts: 12.0,
                cog_deg: 45.0,
                silent_mins: 18,
                is_military: false,
                vessel_type: Some("Tanker".to_string()),
            });
            st.aar_pairs.push(AarPair {
                tanker_id: "tkr1".to_string(),
                tanker_callsign: "LAGR1".to_string(),
                receiver_id: "4ba123".to_string(),
                receiver_callsign: "THY1".to_string(),
                lat: 41.05,
                lon: 29.05,
                alt_ft: 24000.0,
                dist_km: 4.5,
                is_locked: true,
            });
            st.recon_windows.push(ReconWindow {
                sat_name: "SENTINEL-1A".to_string(),
                norad_id: 39634,
                sensor_type: "SAR".to_string(),
                target_name: "ISTASYON".to_string(),
                target_lat: 41.0,
                target_lon: 29.0,
                aos_secs: 240,
                max_el_deg: 55.0,
                cloud_blocked: false,
            });
            st.tactical_ruler = Some(TacticalRuler {
                pin_a: (41.0, 29.0, "IST".to_string()),
                pin_b: Some((40.0, 32.8, "ANK".to_string())),
                threat_ring_km: Some(160.0),
            });
        }

        let raw_json = build_state_json(&state);
        let parsed: Value = serde_json::from_str(&raw_json).expect("valid state json");
        assert!(parsed.get("selected_target_id").is_some());
        assert_eq!(parsed["dark_ships"].as_array().map(|a| a.len()), Some(1));
        assert_eq!(parsed["aar_pairs"].as_array().map(|a| a.len()), Some(1));
        assert_eq!(parsed["strait_gates"].as_array().map(|a| a.len()), Some(6));
        assert_eq!(parsed["recon_windows"].as_array().map(|a| a.len()), Some(1));
        assert_eq!(parsed["tactical_ruler"]["pin_a"]["label"], "IST");

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local_addr");
        let srv_state = Arc::clone(&state);
        let srv = tokio::spawn(async move {
            for _ in 0..2 {
                if let Ok((stream, _)) = listener.accept().await {
                    handle_web_connection(stream, Arc::clone(&srv_state)).await;
                }
            }
        });

        let body = r#"{"id":"4ba123"}"#;
        let hdr = format!(
            "POST /api/select HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        let mut client = TcpStream::connect(addr).await.expect("connect post");
        client.write_all(hdr.as_bytes()).await.expect("write hdr");
        tokio::time::sleep(Duration::from_millis(15)).await;
        client
            .write_all(body.as_bytes())
            .await
            .expect("write body");
        let mut resp_buf = Vec::new();
        client
            .read_to_end(&mut resp_buf)
            .await
            .expect("read post resp");
        let resp_str = String::from_utf8_lossy(&resp_buf);
        assert!(resp_str.starts_with("HTTP/1.1 200 OK"), "resp={}", resp_str);
        assert!(
            resp_str.contains(r#""selected":"4ba123""#),
            "resp={}",
            resp_str
        );

        {
            let st = state.read();
            assert_eq!(st.selected_target_id.as_deref(), Some("4ba123"));
            assert!((st.map_center_lat - 41.05).abs() < 1e-6);
            assert!((st.map_center_lon - 29.05).abs() < 1e-6);
            assert!(st.total_events_received >= 1);
        }

        let mut client2 = TcpStream::connect(addr).await.expect("connect get");
        client2
            .write_all(b"GET /api/state HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .expect("write get");
        let mut resp2_buf = Vec::new();
        client2
            .read_to_end(&mut resp2_buf)
            .await
            .expect("read get resp");
        let resp2_str = String::from_utf8_lossy(&resp2_buf);
        assert!(resp2_str.starts_with("HTTP/1.1 200 OK"));
        let json_part = resp2_str.split("\r\n\r\n").nth(1).expect("http body");
        let state_v: Value = serde_json::from_str(json_part).expect("json body");
        assert_eq!(state_v["selected_target_id"], "4ba123");
        assert_eq!(state_v["tactical_ruler"]["threat_ring_km"], 160.0);

        let _ = srv.await;
    }
}
