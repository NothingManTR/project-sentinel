use crate::widgets::{inspector::render_inspector_panel, p100_map::render_p100_map};
use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use argus_core::types::ThreatSeverity;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

pub fn render_osint_view(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
        .split(area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Kategori Filtre Şeridi
            Constraint::Min(10),   // Haber Listesi
        ])
        .split(cols[0]);

    let cats = [
        ("[0: TUMU]", 0, FG_PRIMARY),
        ("[1: KURESEL]", 1, TELETEXT_GREEN),
        ("[2: ASKERI/KRIZ]", 2, TELETEXT_YELLOW),
        ("[3: SIBER]", 3, TELETEXT_CYAN),
        ("[4: ENERJI]", 4, TELETEXT_MAGENTA),
    ];
    let mut cat_spans = Vec::new();
    let mut cat_x = chunks[0].x + 1;
    for (label, cat_id, color) in cats.iter() {
        let is_active = state.news_filter == *cat_id;
        let style = if is_active {
            Style::default()
                .fg(TELETEXT_BLACK)
                .bg(*color)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(*color).add_modifier(Modifier::BOLD)
        };
        let width = label.chars().count() as u16;
        let rect = Rect {
            x: cat_x,
            y: chunks[0].y + 1,
            width,
            height: 1,
        };
        state.hit_tester.register(rect, HitTarget::OsintCategory(*cat_id));
        cat_x += width + 2;
        cat_spans.push(Span::styled(*label, style));
        cat_spans.push(Span::raw("  "));
    }

    let header_p = Paragraph::new(Line::from(cat_spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(" [ KURESEL ACIK KAYNAK VE KRIZ HABER FILTRESI ] ")
            .style(Style::default().bg(BG_PANEL).fg(FG_PRIMARY)),
    );
    frame.render_widget(header_p, chunks[0]);

    let mut lines = Vec::new();
    let q_lower = state.search_query.to_lowercase();
    let news_items: Vec<(usize, _)> = state
        .news
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            let cat_ok = match state.news_filter {
                1 => item.category.starts_with("P201"),
                2 => item.category.starts_with("P202"),
                3 => item.category.starts_with("P203"),
                4 => item.category.starts_with("P204"),
                _ => true,
            };
            let search_ok = if q_lower.is_empty() {
                true
            } else {
                item.title.to_lowercase().contains(&q_lower)
                    || item.source.to_lowercase().contains(&q_lower)
            };
            cat_ok && search_ok
        })
        .collect();
    let total_items = news_items.len();
    let visible_rows = chunks[1].height.saturating_sub(2) as usize;
    let start_idx = state.news_scroll.min(total_items.saturating_sub(1));

    for (row_offset, (orig_idx, item)) in news_items
        .iter()
        .skip(start_idx)
        .take(visible_rows)
        .enumerate()
    {
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
            Span::styled(
                format!("{:<8} ", sev_tag),
                Style::default().fg(sev_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}: ", item.source),
                Style::default().fg(TELETEXT_YELLOW),
            ),
            Span::styled(&item.title, Style::default().fg(FG_PRIMARY)),
        ]));

        let row_area = Rect {
            x: chunks[1].x + 1,
            y: chunks[1].y + 1 + row_offset as u16,
            width: chunks[1].width.saturating_sub(2),
            height: 1,
        };
        state
            .hit_tester
            .register(row_area, HitTarget::TableRow(1, *orig_idx));
    }

    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(" [ CANLI AJANS AKISI (BBC / DEFENSE NEWS / AL JAZEERA) ] ")
            .style(Style::default().bg(BG_PRIMARY).fg(FG_PRIMARY)),
    );
    frame.render_widget(p, chunks[1]);

    if total_items > visible_rows {
        let mut scrollbar_state = ScrollbarState::new(total_items).position(state.news_scroll);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(BORDER_ACTIVE).bg(BG_PANEL));
        frame.render_stateful_widget(scrollbar, chunks[1], &mut scrollbar_state);
    }

    // Sag Sutun: Mini-Harita + Denetci
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(cols[1]);
    let mb = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER_DIM))
        .title(" [ BOLGESEL HARITA ] ");
    let mi = mb.inner(right_chunks[0]);
    frame.render_widget(mb, right_chunks[0]);
    render_p100_map(frame, mi, state);
    render_inspector_panel(frame, right_chunks[1], state);
}
