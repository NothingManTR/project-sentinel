use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use argus_core::types::ThreatSeverity;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

pub fn render_p200_news(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if area.height < 2 || area.width < 10 {
        return;
    }

    let mut lines = Vec::new();

    // Teletext Başlık Şeridi
    lines.push(Line::from(vec![
        Span::styled("P200 ", Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)),
        Span::styled("CEEFAX / CANLI OSINT HABER AKIŞI ", Style::default().fg(TELETEXT_WHITE).add_modifier(Modifier::BOLD)),
        Span::styled("=====================", Style::default().fg(TELETEXT_CYAN)),
    ]));

    let news_items: Vec<_> = state.news.iter().collect();
    let total_items = news_items.len();

    let visible_rows = area.height.saturating_sub(1) as usize;
    let start_idx = state.news_scroll.min(total_items.saturating_sub(1));

    for (row_offset, item) in news_items.iter().skip(start_idx).take(visible_rows).enumerate() {
        let actual_idx = start_idx + row_offset;
        let sev_color = match item.severity {
            ThreatSeverity::Critical | ThreatSeverity::Emergency => TELETEXT_RED,
            ThreatSeverity::Warning => TELETEXT_YELLOW,
            ThreatSeverity::Advisory => TELETEXT_CYAN,
            ThreatSeverity::Low => TELETEXT_GREEN,
        };

        let sev_tag = format!("[{}]", item.severity.as_str());
        let time_str = item.published.format("%H:%M").to_string();

        lines.push(Line::from(vec![
            Span::styled(format!("{} ", time_str), Style::default().fg(TELETEXT_CYAN)),
            Span::styled(format!("{:<8} ", sev_tag), Style::default().fg(sev_color).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{}: ", item.source), Style::default().fg(TELETEXT_YELLOW)),
            Span::styled(&item.title, Style::default().fg(TELETEXT_WHITE)),
        ]));

        // Her satırı fareyle tıklanabilir yap
        let row_area = Rect {
            x: area.x,
            y: area.y + 1 + row_offset as u16,
            width: area.width.saturating_sub(1),
            height: 1,
        };
        state.hit_tester.register(row_area, HitTarget::TableRow(1, actual_idx));
    }

    let p = Paragraph::new(lines).style(Style::default().bg(TELETEXT_BLACK));
    frame.render_widget(p, area);

    // Dikey Kaydırma Çubuğu (Scrollbar)
    if total_items > visible_rows {
        let mut scrollbar_state = ScrollbarState::new(total_items).position(state.news_scroll);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(WIN98_DARK_GRAY).bg(WIN98_SILVER));
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}
