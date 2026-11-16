use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_dialog(frame: &mut Frame, screen_area: Rect, state: &mut AppState) {
    if !state.is_dialog_open {
        return;
    }

    let dialog_w = 86.min(screen_area.width.saturating_sub(4));
    let dialog_h = 26.min(screen_area.height.saturating_sub(4));
    let dialog_x = (screen_area.width.saturating_sub(dialog_w)) / 2;
    let dialog_y = (screen_area.height.saturating_sub(dialog_h)) / 2;

    let dialog_area = Rect {
        x: dialog_x,
        y: dialog_y,
        width: dialog_w,
        height: dialog_h,
    };

    frame.render_widget(Clear, dialog_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER_ACTIVE).bg(BG_PANEL))
        .style(Style::default().bg(BG_PANEL));
    frame.render_widget(block, dialog_area);

    let title_area = Rect {
        x: dialog_area.x + 1,
        y: dialog_area.y,
        width: dialog_area.width.saturating_sub(2),
        height: 1,
    };

    let max_title_chars = title_area.width.saturating_sub(4) as usize;
    let raw_title = format!(" // {} ", state.dialog_title);
    let clipped_title: String = raw_title.chars().take(max_title_chars).collect();
    let clipped_len = clipped_title.chars().count() as u16;
    let title_spaces = " ".repeat(title_area.width.saturating_sub(clipped_len + 3) as usize);

    let title_line = Line::from(vec![
        Span::styled(
            clipped_title,
            Style::default().fg(TELETEXT_BLACK).bg(TELETEXT_CYAN).add_modifier(Modifier::BOLD),
        ),
        Span::styled(title_spaces, Style::default().bg(BG_SURFACE)),
        Span::styled("[X]", Style::default().fg(TELETEXT_RED).bg(BG_SURFACE).add_modifier(Modifier::BOLD)),
    ]);
    frame.render_widget(Paragraph::new(title_line), title_area);

    state.hit_tester.register(
        Rect {
            x: title_area.x + title_area.width.saturating_sub(3),
            y: title_area.y,
            width: 3,
            height: 1,
        },
        HitTarget::DialogClose,
    );

    let content_area = Rect {
        x: dialog_area.x + 2,
        y: dialog_area.y + 2,
        width: dialog_area.width.saturating_sub(4),
        height: dialog_area.height.saturating_sub(5),
    };

    let content_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_DIM).bg(BG_PRIMARY))
        .style(Style::default().bg(BG_PRIMARY));
    frame.render_widget(content_block, content_area);

    let text_area = Rect {
        x: content_area.x + 1,
        y: content_area.y + 1,
        width: content_area.width.saturating_sub(2),
        height: content_area.height.saturating_sub(2),
    };

    let lines: Vec<Line> = state
        .dialog_content
        .iter()
        .map(|s| {
            if s.starts_with("---") || s.starts_with("===") {
                Line::from(Span::styled(
                    s.clone(),
                    Style::default().fg(TELETEXT_CYAN).add_modifier(Modifier::BOLD),
                ))
            } else if s.starts_with("[!]") || s.contains("KRITIK") || s.contains("ACIL") {
                Line::from(Span::styled(
                    s.clone(),
                    Style::default().fg(TELETEXT_RED).add_modifier(Modifier::BOLD),
                ))
            } else if s.starts_with("[*]") || s.starts_with(">") {
                Line::from(Span::styled(
                    s.clone(),
                    Style::default().fg(TELETEXT_GREEN).add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(Span::styled(s.clone(), Style::default().fg(FG_PRIMARY)))
            }
        })
        .collect();

    let text_p = Paragraph::new(lines).scroll((state.dialog_scroll, 0));
    frame.render_widget(text_p, text_area);

    let btn_width = 41.min(dialog_area.width.saturating_sub(4));
    let btn_area = Rect {
        x: dialog_area.x + (dialog_area.width.saturating_sub(btn_width)) / 2,
        y: dialog_area.y + dialog_area.height.saturating_sub(2),
        width: btn_width,
        height: 1,
    };

    let btn = Paragraph::new(Span::styled(
        " [ E: Not Defterine Aktar | Esc: Kapat ] ",
        Style::default().fg(TELETEXT_BLACK).bg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(btn, btn_area);
    state.hit_tester.register(btn_area, HitTarget::DialogClose);
}
