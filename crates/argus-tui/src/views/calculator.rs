use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_calculator_view(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if area.width < 10 || area.height < 6 {
        return;
    }

    // Şeffaflık sızıntısını önleyen opak Win98 zemin
    frame.render_widget(Clear, area);
    let bg_block = Block::default().style(Style::default().bg(WIN98_SILVER));
    frame.render_widget(bg_block, area);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(44), Constraint::Min(30)])
        .split(area);

    // 1. Sol: Klasik Win98 3D Hesap Makinesi
    let calc_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Ekran (Sunken Display)
            Constraint::Min(12),   // 3D Buton Matrisi
        ])
        .split(main_chunks[0]);

    // Sunken Display
    let display_str = format!(" {:>36} ", state.calculator.display);
    let display_p = Paragraph::new(display_str)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(WIN98_DARK_GRAY))
                .style(Style::default().bg(WIN98_WHITE).fg(WIN98_BLACK).add_modifier(Modifier::BOLD)),
        );
    frame.render_widget(display_p, calc_chunks[0]);

    // 3D Buton Matrisi
    let pad_area = calc_chunks[1];
    let pad_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(WIN98_DARK_GRAY))
        .title(" [3D TUS TAKIMI (TIKLANABILIR)] ")
        .style(Style::default().bg(WIN98_SILVER).fg(WIN98_BLACK));
    let pad_inner = pad_block.inner(pad_area);
    frame.render_widget(pad_block, pad_area);

    // 5 satır buton yerleşimi
    let row_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Min(1),
        ])
        .split(pad_inner);

    let button_rows = [
        // Satır 0
        vec![
            ("C", 'C', TELETEXT_RED, 2),
            ("/", '/', WIN98_BLACK, 1),
            ("*", '*', WIN98_BLACK, 1),
            ("-", '-', WIN98_BLACK, 1),
        ],
        // Satır 1
        vec![
            ("7", '7', WIN98_NAVY, 1),
            ("8", '8', WIN98_NAVY, 1),
            ("9", '9', WIN98_NAVY, 1),
            ("+", '+', WIN98_BLACK, 1),
        ],
        // Satır 2
        vec![
            ("4", '4', WIN98_NAVY, 1),
            ("5", '5', WIN98_NAVY, 1),
            ("6", '6', WIN98_NAVY, 1),
            ("=", '=', TELETEXT_GREEN, 1),
        ],
        // Satır 3
        vec![
            ("1", '1', WIN98_NAVY, 1),
            ("2", '2', WIN98_NAVY, 1),
            ("3", '3', WIN98_NAVY, 1),
            (".", '.', WIN98_BLACK, 1),
        ],
        // Satır 4
        vec![
            ("0", '0', WIN98_NAVY, 2),
            ("MOD (TAB)", '\t', Color::Rgb(140, 70, 0), 2),
        ],
    ];

    for (r_idx, r_btns) in button_rows.iter().enumerate() {
        if r_idx >= row_layout.len() {
            break;
        }
        let row_rect = row_layout[r_idx];
        let total_weight: u32 = r_btns.iter().map(|b| b.3).sum();
        let col_w = row_rect.width / total_weight.max(1) as u16;

        let mut current_x = row_rect.x;
        for (label, ch, col, weight) in r_btns.iter() {
            let btn_w = col_w * (*weight as u16);
            let btn_rect = Rect {
                x: current_x,
                y: row_rect.y,
                width: btn_w.saturating_sub(1),
                height: 1,
            };

            // HitTester kaydı
            state.hit_tester.register(btn_rect, HitTarget::CalcKey(*ch));

            // 3D Beveled Buton Görünümü
            let btn_text = format!(" {:^width$} ", label, width = btn_rect.width.saturating_sub(2) as usize);
            let btn_p = Paragraph::new(btn_text).style(
                Style::default()
                    .fg(*col)
                    .bg(WIN98_GRAY)
                    .add_modifier(Modifier::BOLD),
            );
            frame.render_widget(btn_p, btn_rect);

            current_x += btn_w;
        }
    }

    // 2. Sağ: Taktik OSINT / SIGINT Birim Çevirici
    let conv_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(11), Constraint::Min(8)])
        .split(main_chunks[1]);

    let modes = [
        ("1. Hız: Knots -> km/h / mph", 0),
        ("2. Mesafe: Deniz Mili (NM) -> Kilometre", 1),
        ("3. İrtifa: Feet (FL) -> Metre", 2),
        ("4. SIGINT: Frekans (MHz) -> Dalga Boyu λ (m)", 3),
    ];

    let mut conv_lines = vec![
        Line::from(Span::styled("TAKTIK OSINT / SIGINT BIRIM CEVIRICI:", Style::default().fg(WIN98_NAVY).add_modifier(Modifier::BOLD))),
    ];

    for (i, (m_label, m_idx)) in modes.iter().enumerate() {
        let is_active = state.calculator.conv_mode == *m_idx;
        let prefix = if is_active { " > [*] " } else { "   [ ] " };
        let style = if is_active {
            Style::default().fg(TELETEXT_GREEN).bg(Color::Rgb(20, 30, 45)).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(WIN98_BLACK)
        };
        conv_lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled(format!("(Mod {}) {}", i + 1, m_label), style),
        ]));
    }

    conv_lines.push(Line::from(""));
    conv_lines.push(Line::from(vec![
        Span::styled("Girdi: ", Style::default().fg(WIN98_DARK_GRAY).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" [ {} ] ", state.calculator.conv_input), Style::default().fg(TELETEXT_CYAN).bg(WIN98_NAVY).add_modifier(Modifier::BOLD)),
        Span::styled("   Sonuç: ", Style::default().fg(WIN98_DARK_GRAY).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {} ", state.calculator.conv_result), Style::default().fg(TELETEXT_GREEN).bg(WIN98_NAVY).add_modifier(Modifier::BOLD)),
    ]));

    let conv_p = Paragraph::new(conv_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(WIN98_DARK_GRAY))
            .title(" [TAKTIK CEVIRICI (TAB / TIKLA SEC)] ")
            .style(Style::default().bg(WIN98_SILVER).fg(WIN98_BLACK)),
    );
    frame.render_widget(conv_p, conv_chunks[0]);

    // Mod seçici buton alanlarını HitTester'a kaydet (y + 0 çerçeve, y + 1 başlık, y + 2 ilk mod satırı)
    for i in 0..4 {
        let m_rect = Rect {
            x: conv_chunks[0].x + 1,
            y: conv_chunks[0].y + 2 + i as u16,
            width: conv_chunks[0].width.saturating_sub(2),
            height: 1,
        };
        let ch = match i {
            0 => '!',
            1 => '@',
            2 => '#',
            _ => '$',
        };
        state.hit_tester.register(m_rect, HitTarget::CalcKey(ch));
    }

    // Geçmiş Hesaplamalar
    let history_lines: Vec<Line> = state
        .calculator
        .history
        .iter()
        .rev()
        .take(12)
        .map(|h| Line::from(vec![
            Span::styled(" > ", Style::default().fg(TELETEXT_YELLOW)),
            Span::styled(h, Style::default().fg(WIN98_BLACK)),
        ]))
        .collect();

    let hist_p = Paragraph::new(history_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(WIN98_DARK_GRAY))
            .title(" [HESAPLAMA GECMISI] ")
            .style(Style::default().bg(WIN98_SILVER).fg(WIN98_BLACK)),
    );
    frame.render_widget(hist_p, conv_chunks[1]);
}
