use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_dialog(frame: &mut Frame, screen_area: Rect, state: &mut AppState) {
    if !state.is_dialog_open {
        return;
    }

    // Geniş ve derin istihbarat dosyası diyalog penceresi
    let dialog_w = 82.min(screen_area.width.saturating_sub(4));
    let dialog_h = 24.min(screen_area.height.saturating_sub(4));

    let dialog_x = (screen_area.width.saturating_sub(dialog_w)) / 2;
    let dialog_y = (screen_area.height.saturating_sub(dialog_h)) / 2;

    let dialog_area = Rect {
        x: dialog_x,
        y: dialog_y,
        width: dialog_w,
        height: dialog_h,
    };

    // 1. Clear arka plan
    frame.render_widget(Clear, dialog_area);

    // 2. Dış 3D Çerçeve
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(WIN98_NAVY).bg(WIN98_SILVER))
        .style(Style::default().bg(WIN98_SILVER));
    frame.render_widget(block, dialog_area);

    // 3. Başlık Çubuğu
    let title_area = Rect {
        x: dialog_area.x + 1,
        y: dialog_area.y,
        width: dialog_area.width.saturating_sub(2),
        height: 1,
    };

    let max_title_chars = title_area.width.saturating_sub(3) as usize;
    let raw_title = format!(" // {} ", state.dialog_title);
    let clipped_title: String = raw_title.chars().take(max_title_chars).collect();
    let clipped_len = clipped_title.chars().count() as u16;
    let title_spaces = " ".repeat(title_area.width.saturating_sub(clipped_len + 3) as usize);

    let title_line = Line::from(vec![
        Span::styled(clipped_title, Style::default().fg(WIN98_WHITE).bg(WIN98_NAVY).add_modifier(Modifier::BOLD)),
        Span::styled(title_spaces, Style::default().bg(WIN98_NAVY)),
        Span::styled("[X]", Style::default().fg(WIN98_WHITE).bg(WIN98_DARK_GRAY)),
    ]);
    frame.render_widget(Paragraph::new(title_line), title_area);

    let close_btn_area = Rect {
        x: title_area.x + title_area.width.saturating_sub(3),
        y: title_area.y,
        width: 3,
        height: 1,
    };
    state.hit_tester.register(close_btn_area, HitTarget::DialogClose);

    // 4. İçerik Alanı (Sunken)
    let content_area = Rect {
        x: dialog_area.x + 2,
        y: dialog_area.y + 2,
        width: dialog_area.width.saturating_sub(4),
        height: dialog_area.height.saturating_sub(5),
    };

    let content_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(WIN98_DARK_GRAY).bg(WIN98_WHITE));
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
                Line::from(Span::styled(s.clone(), Style::default().fg(Color::Rgb(0, 70, 140)).add_modifier(Modifier::BOLD)))
            } else if s.starts_with("[!]") || s.contains("KRITIK") || s.contains("ACIL") {
                Line::from(Span::styled(s.clone(), Style::default().fg(Color::Rgb(180, 0, 0)).add_modifier(Modifier::BOLD)))
            } else if s.starts_with("[*]") || s.starts_with(">") {
                Line::from(Span::styled(s.clone(), Style::default().fg(Color::Rgb(0, 120, 0)).add_modifier(Modifier::BOLD)))
            } else {
                Line::from(Span::styled(s.clone(), Style::default().fg(WIN98_BLACK)))
            }
        })
        .collect();

    let text_p = Paragraph::new(lines).scroll((state.dialog_scroll, 0));
    frame.render_widget(text_p, text_area);

    // 5. [ Tamam ] Butonu ve Kaydırma İpucu
    let btn_width = 39.min(dialog_area.width.saturating_sub(4));
    let btn_area = Rect {
        x: dialog_area.x + (dialog_area.width.saturating_sub(btn_width)) / 2,
        y: dialog_area.y + dialog_area.height.saturating_sub(2),
        width: btn_width,
        height: 1,
    };

    let btn = Paragraph::new(Span::styled("[ E: Not Defterine Aktar | Esc: Kapat ]", Style::default().fg(WIN98_BLACK).bg(WIN98_SILVER).add_modifier(Modifier::BOLD)));
    frame.render_widget(btn, btn_area);
    state.hit_tester.register(btn_area, HitTarget::DialogClose);
}
