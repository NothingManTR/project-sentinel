use crate::widgets::{inspector::render_inspector_panel, p100_map::render_p100_map};
use argus_core::state::AppState;
use argus_core::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Block, BorderType, Borders},
    Frame,
};

pub fn render_geoint_view(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
        .split(area);

    let map_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER_ACTIVE))
        .title(" [ GEOINT // TAM EKRAN TAKTIK OPERASYON HARITASI (V: Mod | W: Web Harita | +/-: Zoom) ] ")
        .style(Style::default().bg(BG_PRIMARY));
    let map_inner = map_block.inner(main_chunks[0]);
    frame.render_widget(map_block, main_chunks[0]);
    render_p100_map(frame, map_inner, state);

    render_inspector_panel(frame, main_chunks[1], state);
}
