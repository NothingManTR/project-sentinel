use argus_core::state::AppState;
use argus_core::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render_notepad_view(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Menü Çubuğu (Dosya, Düzen, Görünüm, Yardım)
            Constraint::Min(8),    // Metin Düzenleme Alanı (Sunken)
            Constraint::Length(1), // Durum Çubuğu
        ])
        .split(area);

    // 1. Klasik Win98 Not Defteri Menü Çubuğu
    let menu_line = Line::from(vec![
        Span::styled(" [Dosya] ", Style::default().fg(WIN98_BLACK).bg(WIN98_SILVER)),
        Span::styled(" [Duzen] ", Style::default().fg(WIN98_BLACK).bg(WIN98_SILVER)),
        Span::styled(" [Bicim] ", Style::default().fg(WIN98_BLACK).bg(WIN98_SILVER)),
        Span::styled(" [Yardim] ", Style::default().fg(WIN98_BLACK).bg(WIN98_SILVER)),
        Span::styled("       [F5: Tarih/Zaman Ekle | Ctrl+S: Kaydet | Esc: Konsola Don]", Style::default().fg(WIN98_DARK_GRAY).bg(WIN98_SILVER)),
    ]);
    frame.render_widget(Paragraph::new(menu_line).style(Style::default().bg(WIN98_SILVER)), chunks[0]);

    // 2. Metin Alanı
    let text_lines: Vec<Line> = state
        .notepad
        .content
        .lines()
        .map(|l| Line::from(Span::styled(l, Style::default().fg(WIN98_BLACK).bg(WIN98_WHITE))))
        .collect();

    let text_p = Paragraph::new(text_lines)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(WIN98_DARK_GRAY)).style(Style::default().bg(WIN98_WHITE)));
    frame.render_widget(text_p, chunks[1]);

    // 3. Durum Çubuğu
    let total_lines = state.notepad.content.lines().count().max(1);
    let total_chars = state.notepad.content.len();
    let status_line = Line::from(vec![
        Span::styled(format!(" Durum: {} ", state.notepad.status_message), Style::default().fg(WIN98_BLACK).bg(WIN98_SILVER)),
        Span::styled("│", Style::default().fg(WIN98_DARK_GRAY).bg(WIN98_SILVER)),
        Span::styled(format!(" Satir: {} | Karakter: {} ", total_lines, total_chars), Style::default().fg(WIN98_BLACK).bg(WIN98_SILVER)),
        Span::styled("│", Style::default().fg(WIN98_DARK_GRAY).bg(WIN98_SILVER)),
        Span::styled(" Dosya: notes.txt (ANSI/UTF-8) ", Style::default().fg(WIN98_BLACK).bg(WIN98_SILVER)),
    ]);
    frame.render_widget(Paragraph::new(status_line).style(Style::default().bg(WIN98_SILVER)), chunks[2]);
}
