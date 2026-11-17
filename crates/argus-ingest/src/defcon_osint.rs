use argus_core::types::DefconStatus;
use chrono::Utc;

pub fn parse_defconlevel_html(html: &str) -> (Option<u8>, Vec<(String, u8)>) {
    let mut overall = None;
    let marker = "home-hero-eyebrow-level\">";
    if let Some(pos) = html.find(marker) {
        let rest = &html[pos + marker.len()..];
        let end = rest.find('<').unwrap_or(rest.len().min(20));
        let snippet = rest[..end].trim();
        if let Some(num) = snippet.chars().find(|c| ('1'..='5').contains(c)) {
            overall = num.to_digit(10).map(|d| d as u8);
        }
    }

    let mut commands = Vec::new();
    let mut scan = html;
    let needle = " - DEFCON ";
    while let Some(idx) = scan.find(needle) {
        let before = &scan[..idx];
        let after = &scan[idx + needle.len()..];
        if let Some(quote_start) = before.rfind("title=\"") {
            let cmd_name = before[quote_start + 7..].trim();
            if let Some(lvl_ch) = after.chars().next() {
                if ('1'..='5').contains(&lvl_ch) && !cmd_name.is_empty() && cmd_name.len() < 40 {
                    let lvl = (lvl_ch as u8) - b'0';
                    let short = cmd_name
                        .replace(" Command", "")
                        .replace(" Operations", " Ops")
                        .trim()
                        .to_string();
                    if !commands.iter().any(|(c, _): &(String, u8)| c == &short) {
                        commands.push((short, lvl));
                    }
                }
            }
        }
        scan = &after[after.len().min(1)..];
        if commands.len() >= 14 {
            break;
        }
    }

    (overall, commands)
}

pub fn parse_pizzint_json(raw: &str) -> (Option<u8>, Option<i64>) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) else {
        return (None, None);
    };
    let lvl = v["defcon_level"]
        .as_u64()
        .filter(|&n| (1..=5).contains(&n))
        .map(|n| n as u8);
    let idx = v["overall_index"].as_i64();
    (lvl, idx)
}

/// Açık OSINT DEFCON & PizzINT akışlarını sorgular
pub async fn fetch_osint_defcon(client: &reqwest::Client) -> Option<DefconStatus> {
    let dl_req = client
        .get("https://www.defconlevel.com/")
        .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36");
    let pz_req = client
        .get("https://www.pizzint.watch/api/dashboard-data")
        .header("User-Agent", "Project-Sentinel/0.1");

    let (dl_res, pz_res) = tokio::join!(dl_req.send(), pz_req.send());

    let (dl_overall, commands) = match dl_res {
        Ok(resp) if resp.status().is_success() => {
            let txt = resp.text().await.unwrap_or_default();
            parse_defconlevel_html(&txt)
        }
        _ => (None, Vec::new()),
    };

    let (pz_lvl, doughcon_index) = match pz_res {
        Ok(resp) if resp.status().is_success() => {
            let txt = resp.text().await.unwrap_or_default();
            parse_pizzint_json(&txt)
        }
        _ => (None, None),
    };

    let overall_level = dl_overall.or(pz_lvl)?;
    Some(DefconStatus {
        overall_level,
        source: match (dl_overall.is_some(), pz_lvl.is_some()) {
            (true, true) => "defconlevel.com + PizzINT".to_string(),
            (true, false) => "defconlevel.com".to_string(),
            _ => "PizzINT.watch".to_string(),
        },
        commands,
        doughcon_index,
        updated_at: Utc::now(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_defcon_and_pizzint() {
        let html = r#"
          <span class="home-hero-eyebrow-level">DEFCON 3</span>
          <a title="Central Command - DEFCON 2"></a>
          <a title="European Command - DEFCON 3"></a>
        "#;
        let (ovr, cmds) = parse_defconlevel_html(html);
        assert_eq!(ovr, Some(3));
        assert_eq!(cmds, vec![("Central".to_string(), 2), ("European".to_string(), 3)]);

        let pz = r#"{"success":true,"overall_index":142,"defcon_level":4}"#;
        assert_eq!(parse_pizzint_json(pz), (Some(4), Some(142)));
    }
}
