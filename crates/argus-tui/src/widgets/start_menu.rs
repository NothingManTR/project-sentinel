use argus_core::mouse::{HitTarget, HitTester};
use argus_core::theme::*;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_start_menu(frame: &mut Frame, screen_area: Rect, hit_tester: &mut HitTester) {
    let menu_width = 38;
    let menu_height = 18;

    let menu_x = 0;
    let menu_y = screen_area.height.saturating_sub(menu_height + 1);

    let menu_area = Rect {
        x: menu_x,
        y: menu_y,
        width: menu_width.min(screen_area.width),
        height: menu_height.min(screen_area.height),
    };

    // 1. Clear arka plan
    frame.render_widget(Clear, menu_area);

    // 2. Dış 3D Çerçeve
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(WIN98_DARK_GRAY).bg(WIN98_SILVER))
        .style(Style::default().bg(WIN98_SILVER));
    frame.render_widget(block, menu_area);

    // Sol Dikey Şerit: "SENTINEL 98"
    let items = [
        " [F1] DASHBOARD (Genel Konsol)     ",
        " [F2] GEOINT (Cografi Harita)      ",
        " [F3] HAVA SAHASI (ADS-B Radar)    ",
        " [F4] DENIZ FILOSU (AIS Donanma)   ",
        " [F5] SIGINT (Sinyal & HFGCS EAM)  ",
        " [F6] OSINT (Kriz Haber Bulteni)   ",
        " [F7] CYBERINT (Shodan IP Kesfi)   ",
        " [F8] SATINT (Uydu Radar & Gecis)  ",
        " [F9] MASINT (Jeofizik & Yangin)   ",
        " --------------------------------- ",
        " [F10] Not Defteri (Notepad)       ",
        " [F11] Hesap Makinesi & Cevirici   ",
        " [M] Ses Ac/Kapat                  ",
        " [C] Harita Kontrast               ",
        " [I] Istasyon Bilgisi              ",
        " [Q] Programdan Cikis              ",
    ];

    let mut lines = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        let is_sep = item.contains("---");
        if is_sep {
            lines.push(Line::from(vec![
                Span::styled("▌", Style::default().fg(WIN98_NAVY).bg(WIN98_NAVY)),
                Span::styled(*item, Style::default().fg(WIN98_DARK_GRAY).bg(WIN98_SILVER)),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("▌", Style::default().fg(WIN98_NAVY).bg(WIN98_NAVY)),
                Span::styled(*item, Style::default().fg(WIN98_BLACK).bg(WIN98_SILVER)),
            ]));
        }

        let row_area = Rect {
            x: menu_area.x + 1,
            y: menu_area.y + 1 + idx as u16,
            width: menu_area.width.saturating_sub(2),
            height: 1,
        };
        hit_tester.register(row_area, HitTarget::StartMenuItem(idx));
    }

    let p = Paragraph::new(lines);
    let inner_area = Rect {
        x: menu_area.x + 1,
        y: menu_area.y + 1,
        width: menu_area.width.saturating_sub(2),
        height: menu_area.height.saturating_sub(2),
    };
    frame.render_widget(p, inner_area);
}
