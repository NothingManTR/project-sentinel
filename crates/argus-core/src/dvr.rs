use crate::types::{Domain, TargetTrack};
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt::Write as FmtWrite;
use std::fs::{self, OpenOptions};
use std::io::Write as IoWrite;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactTrackPoint {
    pub id: String,
    pub callsign: String,
    pub domain: Domain,
    pub lat: f32,
    pub lon: f32,
    pub alt_m: f32,
    pub spd_kts: f32,
    pub hdg_deg: f32,
    pub is_military: bool,
}

impl Default for CompactTrackPoint {
    fn default() -> Self {
        Self {
            id: String::new(),
            callsign: String::new(),
            domain: Domain::Air,
            lat: 0.0,
            lon: 0.0,
            alt_m: 0.0,
            spd_kts: 0.0,
            hdg_deg: 0.0,
            is_military: false,
        }
    }
}

impl CompactTrackPoint {
    pub fn from_track(t: &TargetTrack) -> Self {
        Self {
            id: t.id.clone(),
            callsign: t.callsign.clone(),
            domain: t.domain,
            lat: t.geo.lat as f32,
            lon: t.geo.lon as f32,
            alt_m: t.geo.alt_m.unwrap_or(0.0) as f32,
            spd_kts: t.speed_kts.unwrap_or(0.0) as f32,
            hdg_deg: t.heading_deg.unwrap_or(0.0) as f32,
            is_military: t.is_military,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DvrSnapshot {
    pub timestamp: DateTime<Utc>,
    pub tracks: Vec<CompactTrackPoint>,
    pub conflict_count: usize,
    pub alert_summary: String,
}

impl Default for DvrSnapshot {
    fn default() -> Self {
        Self {
            timestamp: Utc
                .timestamp_opt(0, 0)
                .single()
                .unwrap_or_else(Utc::now),
            tracks: Vec::new(),
            conflict_count: 0,
            alert_summary: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DvrTimeline {
    pub snapshots: VecDeque<DvrSnapshot>,
    pub replay_offset_mins: u32,
    pub last_recorded_at: Option<DateTime<Utc>>,
}

impl DvrTimeline {
    pub fn record_if_due(&mut self, state: &crate::state::AppState) {
        self.record_if_due_in(state, Path::new("recordings"));
    }

    pub fn record_if_due_in(&mut self, state: &crate::state::AppState, dir: &Path) {
        let now = Utc::now();
        if let Some(last) = self.last_recorded_at {
            if (now - last).num_seconds() < 60 {
                return;
            }
        }

        let mut tracks = Vec::with_capacity(state.targets.len().min(2500));
        for t in state.targets.iter().filter(|t| t.is_military) {
            if tracks.len() >= 2500 {
                break;
            }
            tracks.push(CompactTrackPoint::from_track(t));
        }
        if tracks.len() < 2500 {
            for t in state.targets.iter().filter(|t| !t.is_military) {
                if tracks.len() >= 2500 {
                    break;
                }
                tracks.push(CompactTrackPoint::from_track(t));
            }
        }

        let alert_summary = state
            .alerts
            .iter()
            .rev()
            .take(3)
            .map(|a| a.message.as_str())
            .collect::<Vec<_>>()
            .join(" | ");

        let snap = DvrSnapshot {
            timestamp: now,
            tracks,
            conflict_count: state.acled_events.len(),
            alert_summary,
        };

        if self.snapshots.len() >= 60 {
            self.snapshots.pop_front();
        }
        self.snapshots.push_back(snap.clone());
        self.last_recorded_at = Some(now);

        let _ = append_snapshot_jsonl(dir, &snap);
    }

    pub fn active_snapshot(&self) -> Option<&DvrSnapshot> {
        if self.replay_offset_mins == 0 || self.snapshots.is_empty() {
            return None;
        }
        let newest = self.snapshots.back()?;
        let oldest = self.snapshots.front()?;
        let span_secs = (newest.timestamp - oldest.timestamp).num_seconds().abs();

        if span_secs >= 60 {
            let target_ts = newest.timestamp - chrono::Duration::minutes(self.replay_offset_mins as i64);
            let mut best: Option<&DvrSnapshot> = None;
            let mut best_diff = i64::MAX;
            for snap in &self.snapshots {
                let diff = (snap.timestamp - target_ts).num_seconds().abs();
                if diff < best_diff {
                    best_diff = diff;
                    best = Some(snap);
                }
            }
            best
        } else {
            let idx = self
                .snapshots
                .len()
                .saturating_sub(1 + self.replay_offset_mins as usize);
            self.snapshots.get(idx)
        }
    }

    pub fn step_offset(&mut self, delta_mins: i32) {
        if delta_mins == 0 {
            self.replay_offset_mins = 0;
        } else {
            self.replay_offset_mins = (self.replay_offset_mins as i32 - delta_mins).clamp(0, 60) as u32;
        }
    }
}

fn push_json_str(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn push_json_f32(out: &mut String, v: f32) {
    if v.is_finite() {
        let _ = write!(out, "{:.4}", v);
    } else {
        out.push_str("0.0");
    }
}

fn domain_variant_name(d: Domain) -> &'static str {
    match d {
        Domain::Air => "Air",
        Domain::Sea => "Sea",
        Domain::Space => "Space",
        Domain::News => "News",
        Domain::Cyber => "Cyber",
        Domain::Geo => "Geo",
        Domain::Conflict => "Conflict",
    }
}

fn snapshot_to_json_line(snap: &DvrSnapshot) -> String {
    let mut out = String::with_capacity(256 + snap.tracks.len() * 96);
    out.push_str("{\"timestamp\":");
    push_json_str(&mut out, &snap.timestamp.to_rfc3339());
    let _ = write!(out, ",\"conflict_count\":{}", snap.conflict_count);
    out.push_str(",\"alert_summary\":");
    push_json_str(&mut out, &snap.alert_summary);
    out.push_str(",\"tracks\":[");
    for (i, t) in snap.tracks.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"id\":");
        push_json_str(&mut out, &t.id);
        out.push_str(",\"callsign\":");
        push_json_str(&mut out, &t.callsign);
        out.push_str(",\"domain\":");
        push_json_str(&mut out, domain_variant_name(t.domain));
        out.push_str(",\"lat\":");
        push_json_f32(&mut out, t.lat);
        out.push_str(",\"lon\":");
        push_json_f32(&mut out, t.lon);
        out.push_str(",\"alt_m\":");
        push_json_f32(&mut out, t.alt_m);
        out.push_str(",\"spd_kts\":");
        push_json_f32(&mut out, t.spd_kts);
        out.push_str(",\"hdg_deg\":");
        push_json_f32(&mut out, t.hdg_deg);
        let _ = write!(out, ",\"is_military\":{}", t.is_military);
        out.push('}');
    }
    out.push_str("]}");
    out
}

fn append_snapshot_jsonl(dir: &Path, snap: &DvrSnapshot) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let filename = format!("timeline_{}.jsonl", snap.timestamp.format("%Y%m%d"));
    let path = dir.join(filename);
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let line = snapshot_to_json_line(snap);
    writeln!(file, "{}", line)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::state::AppState;
    use crate::types::GeoPoint;

    #[test]
    fn test_dvr_step_and_active_snapshot() {
        let mut dvr = DvrTimeline::default();
        let base = Utc::now();
        for mins_ago in [30i64, 15, 5, 0] {
            dvr.snapshots.push_back(DvrSnapshot {
                timestamp: base - chrono::Duration::minutes(mins_ago),
                tracks: vec![CompactTrackPoint {
                    id: format!("T{}", mins_ago),
                    callsign: format!("CS{}", mins_ago),
                    domain: Domain::Air,
                    lat: 41.0,
                    lon: 29.0,
                    alt_m: 1000.0,
                    spd_kts: 250.0,
                    hdg_deg: 90.0,
                    is_military: true,
                }],
                conflict_count: mins_ago as usize,
                alert_summary: String::new(),
            });
        }

        assert!(dvr.active_snapshot().is_none());

        dvr.step_offset(-15);
        assert_eq!(dvr.replay_offset_mins, 15);
        let snap15 = dvr.active_snapshot().expect("snapshot at -15m");
        assert_eq!(snap15.conflict_count, 15);

        dvr.step_offset(-5);
        assert_eq!(dvr.replay_offset_mins, 20);

        dvr.step_offset(-50);
        assert_eq!(dvr.replay_offset_mins, 60);

        dvr.step_offset(0);
        assert_eq!(dvr.replay_offset_mins, 0);
        assert!(dvr.active_snapshot().is_none());
    }

    #[test]
    fn test_dvr_record_if_due_prioritizes_military_and_writes_jsonl() {
        let tmp_dir = std::env::temp_dir().join("argus_osint_test_dvr");
        let _ = fs::remove_dir_all(&tmp_dir);

        let mut st = AppState::new(AppConfig::default());
        for i in 0..5 {
            st.upsert_target(TargetTrack {
                id: format!("CIV{}", i),
                callsign: format!("THY{}", i),
                domain: Domain::Air,
                geo: GeoPoint::new(41.0, 29.0, Some(9000.0)),
                speed_kts: Some(450.0),
                heading_deg: Some(90.0),
                vertical_rate_fpm: None,
                squawk: None,
                is_military: false,
                operator_country: None,
                operator_branch: None,
                airframe_or_vessel_class: None,
                origin: None,
                destination: None,
                vessel_type: None,
                info: String::new(),
                last_seen: Utc::now(),
            });
        }
        st.upsert_target(TargetTrack {
            id: "MIL1".to_string(),
            callsign: "TUAF01".to_string(),
            domain: Domain::Air,
            geo: GeoPoint::new(40.5, 29.5, Some(10000.0)),
            speed_kts: Some(500.0),
            heading_deg: Some(180.0),
            vertical_rate_fpm: None,
            squawk: None,
            is_military: true,
            operator_country: None,
            operator_branch: None,
            airframe_or_vessel_class: None,
            origin: None,
            destination: None,
            vessel_type: None,
            info: String::new(),
            last_seen: Utc::now(),
        });

        let mut dvr = DvrTimeline::default();
        dvr.record_if_due_in(&st, &tmp_dir);
        assert_eq!(dvr.snapshots.len(), 1);
        assert_eq!(dvr.snapshots[0].tracks.len(), 6);
        assert!(dvr.snapshots[0].tracks[0].is_military);
        assert_eq!(dvr.snapshots[0].tracks[0].id, "MIL1");

        // Immediate second call (< 60s) should skip
        dvr.record_if_due_in(&st, &tmp_dir);
        assert_eq!(dvr.snapshots.len(), 1);

        let filename = format!("timeline_{}.jsonl", dvr.snapshots[0].timestamp.format("%Y%m%d"));
        let jsonl = fs::read_to_string(tmp_dir.join(filename)).expect("jsonl file");
        assert!(jsonl.contains("\"id\":\"MIL1\""));
        assert!(jsonl.contains("\"is_military\":true"));

        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
