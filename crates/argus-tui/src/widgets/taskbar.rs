use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use argus_core::types::ActiveView;
use chrono::Utc;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub fn render_taskbar(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if area.height < 1 || area.width < 10 {
        return;
    }

    let mut spans = Vec::new();

    // 1. [MENU] Butonu (Saf ASCII)
    let start_style = if state.is_start_menu_open {
        Style::default().fg(TELETEXT_BLACK).bg(TELETEXT_CYAN).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(FG_PRIMARY).bg(BG_SURFACE).add_modifier(Modifier::BOLD)
    };
    let start_label = " [MENU] ";
    let start_btn_width = start_label.chars().count() as u16;
    spans.push(Span::styled(start_label, start_style));
    spans.push(Span::styled("|", Style::default().fg(BORDER_DIM).bg(BG_PANEL)));

    state.hit_tester.register(
        Rect {
            x: area.x,
            y: area.y,
            width: start_btn_width,
            height: 1,
        },
        HitTarget::StartButton,
    );

    // 2. Calisma Alani Sekmeleri
    let view_tabs = [
        (ActiveView::Dashboard, "F1:GENEL"),
        (ActiveView::Geoint, "F2:HARITA"),
        (ActiveView::AirTracks, "F3:HAVA"),
        (ActiveView::SeaTracks, "F4:DENIZ"),
        (ActiveView::Sigint, "F5:SIGINT"),
        (ActiveView::Osint, "F6:OSINT"),
        (ActiveView::Cyberint, "F7:SIBER"),
        (ActiveView::Satint, "F8:UYDU"),
        (ActiveView::Masint, "F9:AFET"),
        (ActiveView::Notepad, "F10:NOT"),
        (ActiveView::Calculator, "F11:HESAP"),
    ];

    let mut current_x = area.x + start_btn_width + 1;

    for (view, label) in view_tabs.iter() {
        let is_selected = state.active_view == *view;

        let tab_style = if is_selected {
            Style::default()
                .fg(TELETEXT_BLACK)
                .bg(TELETEXT_YELLOW)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(FG_PRIMARY).bg(BG_PANEL)
        };

        let tab_text = format!("[{}]", label);
        let tab_len = tab_text.chars().count() as u16;

        if current_x + tab_len < area.x + area.width {
            spans.push(Span::styled(tab_text, tab_style));
            spans.push(Span::styled(" ", Style::default().bg(BG_PANEL)));

            state.hit_tester.register(
                Rect {
                    x: current_x,
                    y: area.y,
                    width: tab_len,
                    height: 1,
                },
                HitTarget::TaskbarViewTab(*view),
            );

            current_x += tab_len + 1;
        }
    }

    // 3. Sag Sistem Tepsisi (Saf ASCII)
    let web_label = "[WEB:W]";
    let rep_label = "[RAPOR]";
    let help_label = "[?:F12]";
    let audio_label = if state.audio_muted { "[SES:KAPALI]" } else { "[SES:ACIK]" };
    let clock_str = format!(" {}Z ", Utc::now().format("%H:%M:%S"));

    let web_len = web_label.chars().count() as u16;
    let rep_len = rep_label.chars().count() as u16;
    let help_len = help_label.chars().count() as u16;
    let audio_len = audio_label.chars().count() as u16;
    let clock_len = clock_str.chars().count() as u16;

    let tray_items_len = web_len + 1 + rep_len + 1 + help_len + 1 + audio_len + clock_len;
    let tray_total_len = tray_items_len + 1;
    let space_fill = area
        .width
        .saturating_sub(current_x - area.x + tray_total_len) as usize;

    spans.push(Span::raw(" ".repeat(space_fill)));
    spans.push(Span::styled("|", Style::default().fg(BORDER_DIM).bg(BG_PANEL)));

    let mut tx = area.x + area.width.saturating_sub(tray_items_len);

    // [WEB:W]
    spans.push(Span::styled(
        web_label,
        Style::default().fg(TELETEXT_CYAN).bg(BG_SURFACE).add_modifier(Modifier::BOLD),
    ));
    state.hit_tester.register(
        Rect { x: tx, y: area.y, width: web_len, height: 1 },
        HitTarget::OpenWebMap,
    );
    tx += web_len;
    spans.push(Span::raw(" "));
    tx += 1;

    // [RAPOR]
    spans.push(Span::styled(
        rep_label,
        Style::default().fg(TELETEXT_GREEN).bg(BG_SURFACE).add_modifier(Modifier::BOLD),
    ));
    state.hit_tester.register(
        Rect { x: tx, y: area.y, width: rep_len, height: 1 },
        HitTarget::ExportReport,
    );
    tx += rep_len;
    spans.push(Span::raw(" "));
    tx += 1;

    // [?:F12]
    spans.push(Span::styled(
        help_label,
        Style::default().fg(TELETEXT_YELLOW).bg(BG_SURFACE).add_modifier(Modifier::BOLD),
    ));
    state.hit_tester.register(
        Rect { x: tx, y: area.y, width: help_len, height: 1 },
        HitTarget::HelpButton,
    );
    tx += help_len;
    spans.push(Span::raw(" "));
    tx += 1;

    // [SES]
    spans.push(Span::styled(
        audio_label,
        Style::default()
            .fg(if state.audio_muted { TELETEXT_RED } else { TELETEXT_GREEN })
            .bg(BG_SURFACE)
            .add_modifier(Modifier::BOLD),
    ));
    state.hit_tester.register(
        Rect { x: tx, y: area.y, width: audio_len, height: 1 },
        HitTarget::TrayAudio,
    );
    tx += audio_len;

    // Saat
    spans.push(Span::styled(
        clock_str,
        Style::default().fg(FG_PRIMARY).bg(BG_SURFACE).add_modifier(Modifier::BOLD),
    ));
    state.hit_tester.register(
        Rect { x: tx, y: area.y, width: clock_len, height: 1 },
        HitTarget::TrayClock,
    );

    let bar = Paragraph::new(Line::from(spans)).style(Style::default().bg(BG_PANEL));
    frame.render_widget(bar, area);
}
