use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use argus_core::types::ThreatSeverity;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

pub fn render_cyberint_view(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Shodan Canlı Sorgu Terminali
            Constraint::Length(7), // Shodan Host Recon Önbellek Sonucu
            Constraint::Min(8),    // CISA KEV & IODA Siber Olaylar
        ])
        .split(area);

    // 1. Shodan Sorgu Terminali
    let query_p = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("SHODAN + INTERNETDB ALTYAPI TARAYICISI ", Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled("[SINIRSIZ PORT & CVE]", Style::default().fg(TELETEXT_GREEN)),
        ]),
        Line::from(vec![
            Span::styled("Hedef Host IP: ", Style::default().fg(WIN98_BLACK).add_modifier(Modifier::BOLD)),
            Span::styled(format!("[ {} ]", state.shodan_input_ip), Style::default().fg(TELETEXT_CYAN).bg(WIN98_DARK_GRAY).add_modifier(Modifier::BOLD)),
            Span::styled("  [Enter: Tara]  [Shodan API: AKTIF]", Style::default().fg(WIN98_DARK_GRAY)),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL).title(" [SHODAN RECON TERMINALI] ").style(Style::default().bg(WIN98_SILVER).fg(WIN98_BLACK)));
    frame.render_widget(query_p, chunks[0]);

    // 2. Taranan Host Detayı
    let host_info_lines = if let Some(info) = state.shodan_recon_cache.get(&state.shodan_input_ip).or_else(|| state.shodan_recon_cache.get(state.shodan_input_ip.trim())) {
        let hostnames_str = if info.hostnames.is_empty() { "-".to_string() } else { info.hostnames.join(", ") };
        let ports_str = if info.ports.is_empty() { "-".to_string() } else { info.ports.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", ") };
        let tags_str = if info.tags.is_empty() { "-".to_string() } else { info.tags.join(", ") };
        let cpes_str = if info.cpes.is_empty() { "-".to_string() } else { info.cpes.join(", ") };
        let vulns_str = if info.vulns.is_empty() { "Yok (Guvenli)".to_string() } else { info.vulns.join(", ") };
        vec![
            Line::from(vec![
                Span::styled("IP: ", Style::default().fg(WIN98_DARK_GRAY)),
                Span::styled(&info.ip, Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)),
                Span::styled(" | Host: ", Style::default().fg(WIN98_DARK_GRAY)),
                Span::styled(hostnames_str, Style::default().fg(WIN98_BLACK).add_modifier(Modifier::BOLD)),
                Span::styled(" | ISP: ", Style::default().fg(WIN98_DARK_GRAY)),
                Span::styled(info.isp.as_deref().unwrap_or("-"), Style::default().fg(TELETEXT_CYAN)),
                Span::styled(" | Konum: ", Style::default().fg(WIN98_DARK_GRAY)),
                Span::styled(format!("{}, {}", info.city.as_deref().unwrap_or("-"), info.country.as_deref().unwrap_or("-")), Style::default().fg(WIN98_BLACK)),
            ]),
            Line::from(vec![
                Span::styled("Acik Portlar: ", Style::default().fg(WIN98_DARK_GRAY)),
                Span::styled(ports_str, Style::default().fg(TELETEXT_GREEN).add_modifier(Modifier::BOLD)),
                Span::styled(" | Etiketler: ", Style::default().fg(WIN98_DARK_GRAY)),
                Span::styled(tags_str, Style::default().fg(TELETEXT_CYAN)),
            ]),
            Line::from(vec![
                Span::styled("CPE: ", Style::default().fg(WIN98_DARK_GRAY)),
                Span::styled(cpes_str, Style::default().fg(WIN98_BLACK)),
                Span::styled(" | Zafiyetler (CVE): ", Style::default().fg(WIN98_DARK_GRAY)),
                Span::styled(vulns_str, Style::default().fg(TELETEXT_RED)),
            ]),
        ]
    } else {
        vec![
            Line::from(Span::styled(
                "Hedef henüz taranmadı. Canlı Shodan InternetDB + GeoIP sorgusu için [Enter] tuşuna basın.",
                Style::default().fg(WIN98_BLACK).add_modifier(Modifier::BOLD),
            )),
        ]
    };

    let host_p = Paragraph::new(host_info_lines)
        .block(Block::default().borders(Borders::ALL).title(" [HOST KESIF DOSYASI] ").style(Style::default().bg(WIN98_SILVER).fg(WIN98_BLACK)));
    frame.render_widget(host_p, chunks[1]);

    // 3. CISA KEV & IODA Tablosu
    let header_cells = ["TUR", "CVE / ASN", "BASLIK / HEDEF SISTEM", "DETAY VE ETKI"]
        .into_iter()
        .map(|h| Cell::from(h).style(Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).style(Style::default().bg(WIN98_DARK_GRAY));

    let mut rows = Vec::new();
    let incidents: Vec<_> = state.cyber_incidents.iter().collect();
    let visible_rows = chunks[2].height.saturating_sub(3) as usize;
    let max_scroll = incidents.len().saturating_sub(1);
    if state.cyber_scroll > max_scroll {
        state.cyber_scroll = max_scroll;
    }
    let start_idx = state.cyber_scroll;

    for (row_offset, inc) in incidents.iter().skip(start_idx).take(visible_rows).enumerate() {
        let actual_idx = start_idx + row_offset;
        let row_style = if row_offset == 0 {
            Style::default()
                .fg(TELETEXT_YELLOW)
                .bg(ratatui::style::Color::Rgb(16, 48, 76))
                .add_modifier(Modifier::BOLD)
        } else {
            match inc.severity {
                ThreatSeverity::Critical | ThreatSeverity::Emergency => {
                    Style::default().fg(TELETEXT_RED).add_modifier(Modifier::BOLD)
                }
                ThreatSeverity::Warning => Style::default().fg(TELETEXT_YELLOW),
                _ => Style::default().fg(TELETEXT_CYAN),
            }
        };

        let tag = inc.cve.clone().or_else(|| inc.asn.clone()).unwrap_or_else(|| "-".to_string());
        let ev_prefix = if row_offset == 0 {
            format!("> {}", inc.event_type)
        } else {
            format!("  {}", inc.event_type)
        };

        let cells = vec![
            Cell::from(ev_prefix),
            Cell::from(tag),
            Cell::from(inc.title.clone()),
            Cell::from(inc.target.clone()),
        ];

        rows.push(Row::new(cells).style(row_style));

        let row_area = Rect {
            x: chunks[2].x + 1,
            y: chunks[2].y + 2 + row_offset as u16,
            width: chunks[2].width.saturating_sub(2),
            height: 1,
        };
        state.hit_tester.register(row_area, HitTarget::TableRow(4, actual_idx));
    }

    let widths = [
        Constraint::Length(14),
        Constraint::Length(16),
        Constraint::Length(28),
        Constraint::Min(25),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(" [CISA KEV VE IODA BGP KESINTILERI] ").style(Style::default().bg(TELETEXT_BLACK).fg(TELETEXT_WHITE)));
    frame.render_widget(table, chunks[2]);
}
