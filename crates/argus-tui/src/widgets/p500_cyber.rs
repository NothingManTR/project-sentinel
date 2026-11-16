use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use argus_core::types::ThreatSeverity;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Sparkline, Table},
    Frame,
};

pub fn render_p500_cyber(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if area.height < 4 || area.width < 10 {
        return;
    }

    // Üstte Olay Tablosu, altta Shodan & Sparkline özeti
    let chunks = Layout::default()
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(area);

    // 1. CISA KEV & IODA Siber Olaylar Tablosu
    let header_cells = ["TÜR", "CVE / ASN", "BAŞLIK / SİSTEM", "AÇIKLAMA"]
        .into_iter()
        .map(|h| Cell::from(h).style(Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).style(Style::default().bg(WIN98_DARK_GRAY));

    let incidents: Vec<_> = state.cyber_incidents.iter().collect();
    let total_items = incidents.len();
    let visible_rows = chunks[0].height.saturating_sub(1) as usize;
    let start_idx = state.cyber_scroll.min(total_items.saturating_sub(1));

    let mut rows = Vec::new();

    for (row_offset, inc) in incidents.iter().skip(start_idx).take(visible_rows).enumerate() {
        let actual_idx = start_idx + row_offset;

        let row_style = match inc.severity {
            ThreatSeverity::Critical | ThreatSeverity::Emergency => Style::default().fg(TELETEXT_RED).add_modifier(Modifier::BOLD),
            ThreatSeverity::Warning => Style::default().fg(TELETEXT_YELLOW),
            _ => Style::default().fg(TELETEXT_CYAN),
        };

        let tag = inc.cve.clone().or_else(|| inc.asn.clone()).unwrap_or_else(|| "-".to_string());

        let cells = vec![
            Cell::from(inc.event_type.clone()),
            Cell::from(tag),
            Cell::from(inc.title.clone()),
            Cell::from(inc.target.clone()),
        ];

        rows.push(Row::new(cells).style(row_style));

        let row_area = Rect {
            x: chunks[0].x,
            y: chunks[0].y + 1 + row_offset as u16,
            width: chunks[0].width,
            height: 1,
        };
        state.hit_tester.register(row_area, HitTarget::TableRow(4, actual_idx));
    }

    let widths = [
        Constraint::Length(12),
        Constraint::Length(14),
        Constraint::Length(22),
        Constraint::Min(20),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .style(Style::default().bg(TELETEXT_BLACK));

    frame.render_widget(table, chunks[0]);

    // 2. Alt Shodan Recon Durumu & Sparkline
    let sub_chunks = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(chunks[1]);

    let shodan_status = if state.config.api_keys.shodan_key.is_some() {
        Span::styled(" [SHODAN API: AKTİF] Çift Tıkla IP Tara ", Style::default().fg(TELETEXT_GREEN).add_modifier(Modifier::BOLD))
    } else {
        Span::styled(" [SHODAN API: BEKLEMEDE] ", Style::default().fg(TELETEXT_YELLOW))
    };

    let last_scanned_str = if let Some(info) = state.shodan_recon_cache.get(&state.shodan_input_ip) {
        format!("{} -> Portlar: {:?} | CVE: {}", info.ip, info.ports, info.vulns.len())
    } else {
        format!("Hedef: {} (Sorgulamak için F7 CYBERINT -> Enter)", state.shodan_input_ip)
    };

    let p = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("SİBER DURUM: ", Style::default().fg(TELETEXT_YELLOW)),
            shodan_status,
        ]),
        Line::from(vec![
            Span::styled("Son Taranan: ", Style::default().fg(TELETEXT_CYAN)),
            Span::styled(last_scanned_str, Style::default().fg(TELETEXT_WHITE)),
        ]),
    ]).style(Style::default().bg(WIN98_DARK_GRAY));
    frame.render_widget(p, sub_chunks[0]);

    // Sparkline (Olay Frekansı)
    let mut spark_data: Vec<u64> = incidents
        .iter()
        .map(|inc| match inc.severity {
            ThreatSeverity::Critical | ThreatSeverity::Emergency => 15,
            ThreatSeverity::Warning => 10,
            ThreatSeverity::Advisory => 5,
            ThreatSeverity::Low => 2,
        })
        .collect();
    if spark_data.is_empty() {
        spark_data = vec![5, 5, 10, 5, 15, 10, 5, 5];
    }
    let sparkline = Sparkline::default()
        .data(&spark_data)
        .style(Style::default().fg(TELETEXT_RED).bg(WIN98_DARK_GRAY));
    frame.render_widget(sparkline, sub_chunks[1]);
}
