use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use argus_core::types::ThreatSeverity;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table},
    Frame,
};

pub fn render_cyberint_view(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if area.width < 20 || area.height < 8 {
        return;
    }
    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Shodan Canlı Sorgu Terminali & Hızlı Hedefler
            Constraint::Length(6), // Shodan Host Recon Önbellek Sonucu
            Constraint::Min(8),    // CISA KEV & IODA Siber Olaylar
        ])
        .split(area);

    // 1. Shodan Sorgu Terminali
    let presets = ["1.1.1.1", "8.8.8.8", "9.9.9.9"];
    let mut line2 = vec![
        Span::styled("Hedef IP / Host: ", Style::default().fg(FG_SECONDARY).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!(" [ {}_ ] ", state.shodan_input_ip),
            Style::default().fg(TELETEXT_BLACK).bg(TELETEXT_CYAN).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  [Enter: Tara]  Hizli: ", Style::default().fg(FG_MUTED)),
    ];

    let mut px = chunks[0].x
        + 1
        + line2.iter().map(|s| s.content.chars().count() as u16).sum::<u16>();
    for (idx, ip) in presets.iter().enumerate() {
        let lbl = format!("[{}]", ip);
        let w = lbl.chars().count() as u16;
        if px + w < chunks[0].x + chunks[0].width {
            state.hit_tester.register(
                Rect {
                    x: px,
                    y: chunks[0].y + 2,
                    width: w,
                    height: 1,
                },
                HitTarget::ShodanPreset(idx),
            );
            line2.push(Span::styled(
                lbl,
                Style::default().fg(TELETEXT_YELLOW).bg(BG_SURFACE).add_modifier(Modifier::BOLD),
            ));
            line2.push(Span::raw(" "));
            px += w + 1;
        }
    }

    let query_p = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("SHODAN + RIPE BGP + CANLI PORT KESIF TERMINALI ", Style::default().fg(TELETEXT_CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("│ [HIBRIT ALTYAPI, ASN & TCP/BANNER TARAMA]", Style::default().fg(TELETEXT_GREEN)),
        ]),
        Line::from(line2),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_ACTIVE))
            .title(" [ CYBERINT // HIBRIT ALTYAPI, BGP & CANLI PORT KESIF TERMINALI ] ")
            .style(Style::default().bg(BG_PANEL).fg(FG_PRIMARY)),
    );
    frame.render_widget(query_p, chunks[0]);

    // 2. Taranan Host Detayı
    let host_info_lines = if let Some(info) = state
        .shodan_recon_cache
        .get(&state.shodan_input_ip)
        .or_else(|| state.shodan_recon_cache.get(state.shodan_input_ip.trim()))
    {
        let hostnames_str = if info.hostnames.is_empty() { "-".to_string() } else { info.hostnames.join(", ") };
        let ports_str = if info.ports.is_empty() { "-".to_string() } else { info.ports.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", ") };
        let tags_str = if info.tags.is_empty() { "-".to_string() } else { info.tags.join(", ") };
        let cpes_str = if info.cpes.is_empty() { "-".to_string() } else { info.cpes.join(", ") };
        let vulns_str = if info.vulns.is_empty() { "Bilinen CVE yok".to_string() } else { info.vulns.join(", ") };
        vec![
            Line::from(vec![
                Span::styled("IP: ", Style::default().fg(FG_MUTED)),
                Span::styled(&info.ip, Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)),
                Span::styled(" │ Host: ", Style::default().fg(FG_MUTED)),
                Span::styled(hostnames_str, Style::default().fg(FG_PRIMARY).add_modifier(Modifier::BOLD)),
                Span::styled(" │ ISP: ", Style::default().fg(FG_MUTED)),
                Span::styled(info.isp.as_deref().unwrap_or("-"), Style::default().fg(TELETEXT_CYAN)),
                Span::styled(" │ Konum: ", Style::default().fg(FG_MUTED)),
                Span::styled(format!("{}, {}", info.city.as_deref().unwrap_or("-"), info.country.as_deref().unwrap_or("-")), Style::default().fg(FG_PRIMARY)),
            ]),
            Line::from(vec![
                Span::styled("Acik Portlar: ", Style::default().fg(FG_MUTED)),
                Span::styled(ports_str, Style::default().fg(TELETEXT_GREEN).add_modifier(Modifier::BOLD)),
                Span::styled(" │ Etiketler: ", Style::default().fg(FG_MUTED)),
                Span::styled(tags_str, Style::default().fg(TELETEXT_CYAN)),
            ]),
            Line::from(vec![
                Span::styled("CPE: ", Style::default().fg(FG_MUTED)),
                Span::styled(cpes_str, Style::default().fg(FG_SECONDARY)),
                Span::styled(" │ Zafiyetler (CVE): ", Style::default().fg(FG_MUTED)),
                Span::styled(vulns_str, Style::default().fg(TELETEXT_RED).add_modifier(Modifier::BOLD)),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            "Hedef IP yazin ve canli Shodan InternetDB + GeoIP sorgusu icin [Enter] tusuna basin veya hizli IP secin.",
            Style::default().fg(FG_SECONDARY),
        ))]
    };

    let host_p = Paragraph::new(host_info_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(" [ HOST KESIF DOSYASI ] ")
            .style(Style::default().bg(BG_PRIMARY).fg(FG_PRIMARY)),
    );
    frame.render_widget(host_p, chunks[1]);

    // 3. CISA KEV & IODA Tablosu
    let header_cells = ["TUR", "CVE / ASN", "BASLIK / HEDEF SISTEM", "DETAY VE ETKI"]
        .into_iter()
        .map(|h| Cell::from(h).style(Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).style(Style::default().bg(BG_SURFACE));

    let mut rows = Vec::new();
    let total = state.cyber_incidents.len();
    let visible_rows = chunks[2].height.saturating_sub(3) as usize;
    let max_scroll = total.saturating_sub(1);
    if state.cyber_scroll > max_scroll {
        state.cyber_scroll = max_scroll;
    }
    let start_idx = state.cyber_scroll;

    for (row_offset, inc) in state.cyber_incidents.iter().skip(start_idx).take(visible_rows).enumerate() {
        let actual_idx = start_idx + row_offset;
        let is_sel = (state.selected_domain == 4 && state.selected_row == actual_idx)
            || (state.selected_domain != 4 && row_offset == 0);
        let zebra_bg = if row_offset % 2 == 0 {
            BG_PANEL
        } else {
            BG_PRIMARY
        };
        let row_style = if is_sel {
            Style::default()
                .fg(TELETEXT_YELLOW)
                .bg(BG_HIGHLIGHT)
                .add_modifier(Modifier::BOLD)
        } else {
            match inc.severity {
                ThreatSeverity::Critical | ThreatSeverity::Emergency => Style::default()
                    .fg(TELETEXT_RED)
                    .bg(zebra_bg)
                    .add_modifier(Modifier::BOLD),
                ThreatSeverity::Warning => Style::default().fg(TELETEXT_YELLOW).bg(zebra_bg),
                _ => Style::default().fg(TELETEXT_CYAN).bg(zebra_bg),
            }
        };

        let tag = inc.cve.clone().or_else(|| inc.asn.clone()).unwrap_or_else(|| "-".to_string());
        let ev_prefix = if is_sel {
            format!("> {}", inc.event_type)
        } else {
            format!("  {}", inc.event_type)
        };

        rows.push(
            Row::new(vec![
                Cell::from(ev_prefix),
                Cell::from(tag),
                Cell::from(inc.title.as_str()),
                Cell::from(inc.target.as_str()),
            ])
            .style(row_style),
        );

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
        Constraint::Length(18),
        Constraint::Length(34),
        Constraint::Min(25),
    ];

    let table = Table::new(rows, widths).header(header).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(format!(" [ CISA KEV & IODA BGP KESINTILERI ({}/{}) ] ", (start_idx + 1).min(total), total))
            .style(Style::default().bg(BG_PRIMARY).fg(FG_PRIMARY)),
    );
    frame.render_widget(table, chunks[2]);

    if total > visible_rows {
        let mut sb_state = ScrollbarState::new(total).position(start_idx);
        let sb = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(TELETEXT_CYAN).bg(BG_SURFACE));
        frame.render_stateful_widget(sb, chunks[2], &mut sb_state);
    }
}
