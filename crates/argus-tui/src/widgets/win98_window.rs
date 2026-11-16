use argus_core::mouse::{HitTarget, HitTester};
use argus_core::theme::*;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub struct Win98Window<'a> {
    pub title: &'a str,
    pub tile_idx: usize,
    pub is_active: bool,
    pub is_zoomed: bool,
}

impl<'a> Win98Window<'a> {
    pub fn new(title: &'a str, tile_idx: usize, is_active: bool, is_zoomed: bool) -> Self {
        Self {
            title,
            tile_idx,
            is_active,
            is_zoomed,
        }
    }

    /// Pencereyi ve başlık çubuğunu çizer, iç içerik alanını (inner Rect) döndürür
    pub fn render(&self, frame: &mut Frame, area: Rect, hit_tester: &mut HitTester) -> Rect {
        if area.width < 4 || area.height < 3 {
            return area;
        }

        // 1. Dış 3D Çerçeve
        let frame_border_style = if self.is_active {
            Style::default().fg(WIN98_NAVY).bg(WIN98_SILVER)
        } else {
            Style::default().fg(WIN98_DARK_GRAY).bg(WIN98_SILVER)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(frame_border_style)
            .style(Style::default().bg(WIN98_SILVER));
        frame.render_widget(block, area);

        // Pencere gövdesi için hit test kaydı
        hit_tester.register(area, HitTarget::WindowBody(self.tile_idx));

        // 2. Başlık Çubuğu (Title Bar)
        let title_area = Rect {
            x: area.x + 1,
            y: area.y,
            width: area.width.saturating_sub(2),
            height: 1,
        };

        let title_bg = if self.is_active { WIN98_NAVY } else { WIN98_GRAY };
        let title_fg = WIN98_WHITE;

        let max_sym = if self.is_zoomed { WIN98_BTN_RESTORE } else { WIN98_BTN_MAX };
        let buttons = format!("{} {} {}", WIN98_BTN_MIN, max_sym, WIN98_BTN_CLOSE);
        let btn_len = buttons.chars().count() as u16;

        let title_text_max_width = title_area.width.saturating_sub(btn_len + 2) as usize;
        let title_chars: usize = self.title.chars().count();
        let truncated_title: String = if title_chars > title_text_max_width {
            self.title.chars().take(title_text_max_width).collect()
        } else {
            self.title.to_string()
        };
        let truncated_len = truncated_title.chars().count() as u16;
        let padding_spaces = " ".repeat(title_area.width.saturating_sub(truncated_len + btn_len + 2) as usize);

        let title_line = Line::from(vec![
            Span::styled("> ", Style::default().fg(WIN98_WHITE).add_modifier(Modifier::BOLD)),
            Span::styled(truncated_title, Style::default().fg(title_fg).add_modifier(Modifier::BOLD)),
            Span::raw(padding_spaces),
            Span::styled(WIN98_BTN_MIN, Style::default().fg(WIN98_WHITE).bg(WIN98_DARK_GRAY)),
            Span::raw(" "),
            Span::styled(max_sym, Style::default().fg(WIN98_WHITE).bg(WIN98_DARK_GRAY)),
            Span::raw(" "),
            Span::styled(WIN98_BTN_CLOSE, Style::default().fg(WIN98_WHITE).bg(WIN98_DARK_GRAY)),
        ]);

        let title_p = Paragraph::new(title_line).style(Style::default().bg(title_bg));
        frame.render_widget(title_p, title_area);

        // Başlık çubuğu ve kontrol butonları için HitTest kaydı
        hit_tester.register(title_area, HitTarget::WindowHeader(self.tile_idx));

        // [_] Butonu
        if title_area.width >= 11 {
            let min_btn_area = Rect {
                x: title_area.x + title_area.width.saturating_sub(11),
                y: title_area.y,
                width: 3,
                height: 1,
            };
            hit_tester.register(min_btn_area, HitTarget::WindowMinimize(self.tile_idx));
        }

        // [□] Butonu (Zoom / Maximize)
        if title_area.width >= 7 {
            let max_btn_area = Rect {
                x: title_area.x + title_area.width.saturating_sub(7),
                y: title_area.y,
                width: 3,
                height: 1,
            };
            hit_tester.register(max_btn_area, HitTarget::WindowMaximize(self.tile_idx));
        }

        // [X] Butonu (Close)
        if title_area.width >= 3 {
            let close_btn_area = Rect {
                x: title_area.x + title_area.width.saturating_sub(3),
                y: title_area.y,
                width: 3,
                height: 1,
            };
            hit_tester.register(close_btn_area, HitTarget::WindowClose(self.tile_idx));
        }

        // 3. İç Çalışma Alanı (Inner Canvas / Body)
        Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        }
    }
}
