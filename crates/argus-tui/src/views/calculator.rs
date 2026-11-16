use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_calculator_view(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if area.width < 10 || area.height < 6 {
        return;
    }

    frame.render_widget(Clear, area);
    frame.render_widget(Block::default().style(Style::default().bg(BG_PRIMARY)), area);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(44), Constraint::Min(30)])
        .split(area);

    // 1. Sol: Taktik Hesap Makinesi
    let calc_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(12)])
        .split(main_chunks[0]);

    let display_str = format!(" {:>36} ", state.calculator.display);
    let display_p = Paragraph::new(display_str).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_ACTIVE))
            .style(Style::default().bg(BG_SURFACE).fg(TELETEXT_CYAN).add_modifier(Modifier::BOLD)),
    );
    frame.render_widget(display_p, calc_chunks[0]);

    let pad_area = calc_chunks[1];
    let pad_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER_DIM))
        .title(" [ TUS TAKIMI (TIKLANABILIR) ] ")
        .style(Style::default().bg(BG_PANEL).fg(FG_PRIMARY));
    let pad_inner = pad_block.inner(pad_area);
    frame.render_widget(pad_block, pad_area);

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
        vec![
            ("C", 'C', TELETEXT_RED, 2),
            ("/", '/', TELETEXT_YELLOW, 1),
            ("*", '*', TELETEXT_YELLOW, 1),
            ("-", '-', TELETEXT_YELLOW, 1),
        ],
        vec![
            ("7", '7', FG_PRIMARY, 1),
            ("8", '8', FG_PRIMARY, 1),
            ("9", '9', FG_PRIMARY, 1),
            ("+", '+', TELETEXT_YELLOW, 1),
        ],
        vec![
            ("4", '4', FG_PRIMARY, 1),
            ("5", '5', FG_PRIMARY, 1),
            ("6", '6', FG_PRIMARY, 1),
            ("=", '=', TELETEXT_GREEN, 1),
        ],
        vec![
            ("1", '1', FG_PRIMARY, 1),
            ("2", '2', FG_PRIMARY, 1),
            ("3", '3', FG_PRIMARY, 1),
            (".", '.', TELETEXT_CYAN, 1),
        ],
        vec![
            ("0", '0', FG_PRIMARY, 2),
            ("MOD (TAB)", '\t', TELETEXT_CYAN, 2),
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
            state.hit_tester.register(btn_rect, HitTarget::CalcKey(*ch));

            let btn_text = format!(" {:^width$} ", label, width = btn_rect.width.saturating_sub(2) as usize);
            let btn_p = Paragraph::new(btn_text).style(
                Style::default()
                    .fg(*col)
                    .bg(BG_SURFACE)
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
        ("1. Hiz: Knots -> km/h / mph", 0),
        ("2. Mesafe: Deniz Mili (NM) -> Kilometre", 1),
        ("3. Irtifa: Feet (FL) -> Metre", 2),
        ("4. SIGINT: Frekans (MHz) -> Dalga Boyu (m)", 3),
    ];

    let mut conv_lines = vec![Line::from(Span::styled(
        "TAKTIK OSINT / SIGINT BIRIM CEVIRICI:",
        Style::default().fg(TELETEXT_CYAN).add_modifier(Modifier::BOLD),
    ))];

    for (i, (m_label, m_idx)) in modes.iter().enumerate() {
        let is_active = state.calculator.conv_mode == *m_idx;
        let prefix = if is_active { " > [*] " } else { "   [ ] " };
        let style = if is_active {
            Style::default().fg(TELETEXT_GREEN).bg(BG_HIGHLIGHT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(FG_PRIMARY)
        };
        conv_lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(TELETEXT_YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled(format!("(Mod {}) {}", i + 1, m_label), style),
        ]));
    }

    conv_lines.push(Line::from(""));
    conv_lines.push(Line::from(vec![
        Span::styled("Girdi: ", Style::default().fg(FG_MUTED).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!(" [ {} ] ", state.calculator.conv_input),
            Style::default().fg(TELETEXT_BLACK).bg(TELETEXT_CYAN).add_modifier(Modifier::BOLD),
        ),
        Span::styled("   Sonuc: ", Style::default().fg(FG_MUTED).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!(" {} ", state.calculator.conv_result),
            Style::default().fg(TELETEXT_GREEN).bg(Color::Rgb(16, 36, 26)).add_modifier(Modifier::BOLD),
        ),
    ]));

    let conv_p = Paragraph::new(conv_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(" [ TAKTIK CEVIRICI (TAB / TIKLA SEC) ] ")
            .style(Style::default().bg(BG_PANEL).fg(FG_PRIMARY)),
    );
    frame.render_widget(conv_p, conv_chunks[0]);

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

    let history_lines: Vec<Line> = state
        .calculator
        .history
        .iter()
        .rev()
        .take(12)
        .map(|h| {
            Line::from(vec![
                Span::styled(" > ", Style::default().fg(TELETEXT_YELLOW)),
                Span::styled(h, Style::default().fg(FG_PRIMARY)),
            ])
        })
        .collect();

    let hist_p = Paragraph::new(history_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(" [ HESAPLAMA GECMISI ] ")
            .style(Style::default().bg(BG_PRIMARY).fg(FG_PRIMARY)),
    );
    frame.render_widget(hist_p, conv_chunks[1]);
}
