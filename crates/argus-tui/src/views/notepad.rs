use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub fn render_notepad_view(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if area.width < 20 || area.height < 8 {
        return;
    }
    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Komut Arac Cubugu
            Constraint::Min(6),    // Sol Arsiv + Sag Duzenleyici/Okuyucu
            Constraint::Length(1), // Alt Durum Cubugu
        ])
        .split(area);

    // 1. Ust Arac Cubugu (Tiklanabilir butonlar)
    let mut bar_spans = vec![Span::styled(
        " NOTLAR & AI ARSIVI ",
        Style::default()
            .fg(TELETEXT_BLACK)
            .bg(TELETEXT_CYAN)
            .add_modifier(Modifier::BOLD),
    )];
    let actions = [
        (" [Ctrl+S:Kaydet]", 0usize, TELETEXT_GREEN),
        (" [F5:Zaman]", 1, TELETEXT_YELLOW),
        (" [Ctrl+T:Hedef Ekle]", 2, TELETEXT_CYAN),
        (" [Ctrl+N:Yeni Not]", 3, FG_PRIMARY),
        (" [Ctrl+H:HTML Brifing]", 4, TELETEXT_CYAN),
        (" [Ctrl+D:Sil]", 5, TELETEXT_RED),
    ];
    let mut cx = chunks[0].x + 21;
    bar_spans.push(Span::styled("│", Style::default().fg(BORDER_DIM).bg(BG_SURFACE)));
    cx += 1;
    for (lbl, act_id, col) in actions {
        let w = lbl.chars().count() as u16;
        if cx + w < chunks[0].x + chunks[0].width.saturating_sub(14) {
            bar_spans.push(Span::styled(
                lbl,
                Style::default().fg(col).bg(BG_SURFACE).add_modifier(Modifier::BOLD),
            ));
            state.hit_tester.register(
                Rect {
                    x: cx,
                    y: chunks[0].y,
                    width: w,
                    height: 1,
                },
                HitTarget::NotepadAction(act_id),
            );
            cx += w;
        }
    }
    let ai_btn = " [Shift+A:AI SITREP] ";
    let ai_w = ai_btn.chars().count() as u16;
    if chunks[0].x + chunks[0].width > cx + ai_w {
        let pad = (chunks[0].x + chunks[0].width - cx - ai_w) as usize;
        bar_spans.push(Span::styled(" ".repeat(pad), Style::default().bg(BG_SURFACE)));
        bar_spans.push(Span::styled(
            ai_btn,
            Style::default()
                .fg(TELETEXT_BLACK)
                .bg(TELETEXT_YELLOW)
                .add_modifier(Modifier::BOLD),
        ));
        state.hit_tester.register(
            Rect {
                x: chunks[0].x + chunks[0].width - ai_w,
                y: chunks[0].y,
                width: ai_w,
                height: 1,
            },
            HitTarget::TriggerAiAnalysis,
        );
    }
    frame.render_widget(
        Paragraph::new(Line::from(bar_spans)).style(Style::default().bg(BG_SURFACE)),
        chunks[0],
    );

    // 2. Iki Sutunlu Ana Calisma Alani
    let sidebar_w = if chunks[1].width >= 90 { 34 } else { 24 };
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(sidebar_w), Constraint::Min(20)])
        .split(chunks[1]);

    // 2A. Sol: Dosya & AI Rapor Arsivi
    let sb_border = if state.notepad.focus_sidebar {
        BORDER_ACTIVE
    } else {
        BORDER_DIM
    };
    let sb_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(sb_border))
        .title(format!(" [ ARSIV ({}) | Tab ] ", state.notepad.files.len()))
        .style(Style::default().bg(BG_PANEL).fg(FG_PRIMARY));
    let sb_inner = sb_block.inner(panes[0]);
    frame.render_widget(sb_block, panes[0]);

    let max_rows = sb_inner.height as usize;
    let total_files = state.notepad.files.len();
    let sel_idx = state.notepad.selected_file_idx.min(total_files.saturating_sub(1));
    let start_idx = if sel_idx >= max_rows {
        sel_idx - max_rows + 1
    } else {
        0
    };

    let mut file_lines = Vec::new();
    for (vis_row, fidx) in (start_idx..total_files).take(max_rows).enumerate() {
        let fname = &state.notepad.files[fidx];
        let short_name = fname.strip_prefix("reports/").unwrap_or(fname);
        let is_active = fname == &state.notepad.active_file;
        let is_selected = fidx == sel_idx;
        let tag = if fname == "notes.txt" {
            "[NOT]"
        } else if fname.contains("ai_sitrep") {
            "[AI] "
        } else if fname.ends_with(".html") {
            "[HTM]"
        } else {
            "[MD] "
        };
        let label = format!("{} {}", tag, short_name);
        let clipped: String = label.chars().take(sb_inner.width as usize).collect();

        let style = if is_selected && state.notepad.focus_sidebar {
            Style::default()
                .fg(TELETEXT_BLACK)
                .bg(TELETEXT_CYAN)
                .add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default()
                .fg(TELETEXT_YELLOW)
                .bg(BG_SURFACE)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(if fname.contains("ai_sitrep") {
                TELETEXT_GREEN
            } else {
                FG_PRIMARY
            })
        };
        file_lines.push(Line::from(Span::styled(clipped, style)));
        state.hit_tester.register(
            Rect {
                x: sb_inner.x,
                y: sb_inner.y + vis_row as u16,
                width: sb_inner.width,
                height: 1,
            },
            HitTarget::NotepadFile(fidx),
        );
    }
    frame.render_widget(Paragraph::new(file_lines), sb_inner);

    // 2B. Sag: Markdown Destekli & Kaydirilabilir Metin Duzenleyici / Okuyucu
    let ed_border = if !state.notepad.focus_sidebar {
        BORDER_ACTIVE
    } else {
        BORDER_DIM
    };
    let read_only = state.notepad.active_file.ends_with(".html");
    let mode_tag = if read_only { "SALT OKUNUR HTML" } else { "DUZENLE / OKU" };
    let mut text_lines: Vec<Line> = state
        .notepad
        .content
        .lines()
        .map(|l| {
            let trim = l.trim_start();
            let style = if trim.starts_with('#') || l.starts_with("---") {
                Style::default().fg(TELETEXT_CYAN).add_modifier(Modifier::BOLD)
            } else if trim.contains("[NOKTA") || l.contains("Risk Skoru:") {
                Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)
            } else if l.contains("KRITIK") || l.contains("KRİTİK") || l.contains("ACIL") {
                Style::default().fg(TELETEXT_RED).add_modifier(Modifier::BOLD)
            } else if trim.starts_with("- ") || trim.starts_with("* ") {
                Style::default().fg(TELETEXT_GREEN)
            } else {
                Style::default().fg(FG_PRIMARY)
            };
            Line::from(Span::styled(l, style))
        })
        .collect();

    if !read_only && !state.notepad.focus_sidebar {
        if state.notepad.content.ends_with('\n') || text_lines.is_empty() {
            text_lines.push(Line::from(Span::styled("_", Style::default().fg(TELETEXT_YELLOW))));
        } else if let Some(last) = text_lines.last_mut() {
            last.spans.push(Span::styled("_", Style::default().fg(TELETEXT_YELLOW)));
        }
    }

    let text_p = Paragraph::new(text_lines)
        .wrap(Wrap { trim: false })
        .scroll((state.notepad.scroll, 0))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(ed_border))
                .title(format!(" [ {}: {} ] ", mode_tag, state.notepad.active_file))
                .style(Style::default().bg(BG_PRIMARY).fg(FG_PRIMARY)),
        );
    frame.render_widget(text_p, panes[1]);

    // 3. Alt Durum Cubugu
    let total_lines = state.notepad.content.lines().count().max(1);
    let total_chars = state.notepad.content.len();
    let status_line = Line::from(vec![
        Span::styled(
            format!(" DURUM: {} ", state.notepad.status_message),
            Style::default().fg(TELETEXT_GREEN).bg(BG_SURFACE),
        ),
        Span::styled("│", Style::default().fg(BORDER_DIM).bg(BG_SURFACE)),
        Span::styled(
            format!(" Satir:{} Kaydir:{} Karakter:{} ", total_lines, state.notepad.scroll, total_chars),
            Style::default().fg(TELETEXT_YELLOW).bg(BG_SURFACE),
        ),
        Span::styled("│", Style::default().fg(BORDER_DIM).bg(BG_SURFACE)),
        Span::styled(
            " [Tab] Arsiv<->Editor │ [Ok/PgUpDn/Tekerlek] Kaydir ",
            Style::default().fg(FG_SECONDARY).bg(BG_SURFACE),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(status_line).style(Style::default().bg(BG_SURFACE)),
        chunks[2],
    );
}
