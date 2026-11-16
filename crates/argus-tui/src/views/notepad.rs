use argus_core::state::AppState;
use argus_core::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_notepad_view(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if area.width < 12 || area.height < 6 {
        return;
    }
    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Komut Seridi
            Constraint::Min(6),    // Metin Duzenleme Alani
            Constraint::Length(1), // Durum Cubugu
        ])
        .split(area);

    let menu_line = Line::from(vec![
        Span::styled(
            " TAKTIK NOT DEFTERI (notes.txt) ",
            Style::default().fg(TELETEXT_BLACK).bg(TELETEXT_CYAN).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " │ [F5: Zaman Damgasi Ekle] [Ctrl+S: Kaydet] [Esc: Komuta Merkezi] ",
            Style::default().fg(FG_SECONDARY).bg(BG_SURFACE),
        ),
    ]);
    frame.render_widget(Paragraph::new(menu_line).style(Style::default().bg(BG_SURFACE)), chunks[0]);

    let mut text_lines: Vec<Line> = state
        .notepad
        .content
        .lines()
        .map(|l| {
            let col = if l.starts_with('#') { TELETEXT_CYAN } else { FG_PRIMARY };
            Line::from(Span::styled(l, Style::default().fg(col)))
        })
        .collect();
    if state.notepad.content.ends_with('\n') || text_lines.is_empty() {
        text_lines.push(Line::from(Span::styled("_", Style::default().fg(TELETEXT_YELLOW))));
    } else if let Some(last) = text_lines.last_mut() {
        last.spans.push(Span::styled("_", Style::default().fg(TELETEXT_YELLOW)));
    }

    let text_p = Paragraph::new(text_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_ACTIVE))
            .style(Style::default().bg(BG_PRIMARY).fg(FG_PRIMARY)),
    );
    frame.render_widget(text_p, chunks[1]);

    let total_lines = state.notepad.content.lines().count().max(1);
    let total_chars = state.notepad.content.len();
    let status_line = Line::from(vec![
        Span::styled(
            format!(" DURUM: {} ", state.notepad.status_message),
            Style::default().fg(TELETEXT_GREEN).bg(BG_SURFACE),
        ),
        Span::styled("│", Style::default().fg(BORDER_DIM).bg(BG_SURFACE)),
        Span::styled(
            format!(" Satir: {} | Karakter: {} ", total_lines, total_chars),
            Style::default().fg(TELETEXT_YELLOW).bg(BG_SURFACE),
        ),
        Span::styled("│", Style::default().fg(BORDER_DIM).bg(BG_SURFACE)),
        Span::styled(" Dosya: notes.txt (UTF-8) ", Style::default().fg(FG_SECONDARY).bg(BG_SURFACE)),
    ]);
    frame.render_widget(Paragraph::new(status_line).style(Style::default().bg(BG_SURFACE)), chunks[2]);
}
