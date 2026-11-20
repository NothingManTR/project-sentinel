use crate::widgets::{inspector::render_inspector_panel, p100_map::render_p100_map};
use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use argus_core::theme::*;
use argus_core::types::ThreatSeverity;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table,
    },
    Frame,
};

pub fn render_osint_view(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if area.width < 24 || area.height < 8 {
        return;
    }
    frame.render_widget(Clear, area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
        .split(area);

    let has_evi = !state.escalation_metrics.is_empty();
    let evi_h = if has_evi && area.height >= 24 { 6 } else { 0 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),     // 3'lu Taktik HUD Ozet Kartlari
            Constraint::Length(evi_h), // Y-6: GDELT EVI Kuresel Gerilim Grafik Paneli
            Constraint::Length(3),     // Tiklanabilir Sekme & Filtre Cubugu
            Constraint::Min(8),        // Ana Tablo (Ajans Haberleri veya GDELT/ACLED Catisma)
        ])
        .split(cols[0]);

    render_osint_hud(frame, chunks[0], state);
    if evi_h > 0 {
        render_osint_evi_chart(frame, chunks[1], state);
    }
    render_osint_filter_bar(frame, chunks[2], state);

    if state.news_filter == 5 {
        render_conflict_table(frame, chunks[3], state);
    } else {
        render_news_table(frame, chunks[3], state);
    }

    // Sag Sutun: Bolgesel & Catisma Mini-Haritasi + Denetci
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(cols[1]);

    let mb = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER_ACTIVE))
        .title(" [ BOLGESEL & CATISMA HARITASI ] ")
        .style(Style::default().bg(BG_PRIMARY));
    let mi = mb.inner(right_chunks[0]);
    frame.render_widget(mb, right_chunks[0]);
    render_p100_map(frame, mi, state);

    render_inspector_panel(frame, right_chunks[1], state);
}

fn render_osint_hud(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let hud_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(36),
            Constraint::Percentage(30),
        ])
        .split(area);

    let (mode_val, mode_col) = if state.news_filter == 5 {
        ("GDELT / ACLED Catisma", TELETEXT_RED)
    } else {
        ("Canli Ajans Haberleri", TELETEXT_YELLOW)
    };

    let (evi_str, evi_col) = if let Some(top) = state.escalation_metrics.iter().max_by_key(|m| m.evi_score) {
        let col = if top.evi_score >= 70 || top.delta_pct >= 30 {
            TELETEXT_RED
        } else if top.evi_score >= 40 {
            TELETEXT_YELLOW
        } else {
            TELETEXT_GREEN
        };
        (
            format!("{} EVI:{} ({:+}%)", top.country_code, top.evi_score, top.delta_pct),
            col,
        )
    } else {
        ("EVI: Hesaplaniyor...".to_string(), FG_MUTED)
    };

    let cards = [
        (
            "KURESEL AJANS AKISI",
            format!("{} Haber Bulteni", state.news.len()),
            TELETEXT_CYAN,
            "BBC / DefNews / AJ".to_string(),
            FG_SECONDARY,
            BORDER_DIM,
        ),
        (
            "SAHA CATISMA / GDELT",
            format!("{} Kinetik Catisma", state.acled_events.len()),
            TELETEXT_RED,
            "[5: Catisma Tablosu]".to_string(),
            FG_SECONDARY,
            if state.news_filter == 5 {
                TELETEXT_RED
            } else {
                BORDER_DIM
            },
        ),
        (
            "AKTIF MOD & EVI",
            mode_val.to_string(),
            mode_col,
            evi_str,
            evi_col,
            BORDER_DIM,
        ),
    ];

    for (idx, (title, val, fg_col, sub_val, sub_col, border_col)) in cards.iter().enumerate() {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(*border_col))
            .title(format!(" {} ", title))
            .style(Style::default().bg(BG_PANEL));
        let inner = block.inner(hud_cols[idx]);
        frame.render_widget(block, hud_cols[idx]);

        let p = Paragraph::new(vec![
            Line::from(Span::styled(
                val.as_str(),
                Style::default().fg(*fg_col).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                sub_val.as_str(),
                Style::default().fg(*sub_col).add_modifier(Modifier::BOLD),
            )),
        ]);
        frame.render_widget(p, inner);
    }

    state
        .hit_tester
        .register(hud_cols[1], HitTarget::OsintCategory(5));
}

fn render_osint_evi_chart(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let mut sorted: Vec<(usize, &argus_core::types::EscalationMetric)> = state
        .escalation_metrics
        .iter()
        .enumerate()
        .collect();
    sorted.sort_by(|a, b| {
        b.1.evi_score
            .cmp(&a.1.evi_score)
            .then_with(|| b.1.event_count.cmp(&a.1.event_count))
    });

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER_DIM))
        .title(" [ GDELT EVI KURESEL GERILIM GRAFIGI (Tikla: Ulke Catismalarini Filtrele) ] ")
        .style(Style::default().bg(BG_PANEL));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows_avail = inner.height as usize;
    if rows_avail == 0 || inner.width < 30 {
        return;
    }

    // 2 sutunlu kompakt bar grafigi (en yuksek 8 kriz ulkesi)
    let two_cols = inner.width >= 68;
    let col_areas = if two_cols {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(inner)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)])
            .split(inner)
    };

    let per_col = rows_avail.min(4);
    for (c_idx, col_rect) in col_areas.iter().enumerate() {
        let mut lines = Vec::new();
        let slice_start = c_idx * per_col;
        for (r_off, &(orig_idx, m)) in sorted.iter().skip(slice_start).take(per_col).enumerate() {
            let col = if m.evi_score >= 70 || m.delta_pct >= 30 {
                TELETEXT_RED
            } else if m.evi_score >= 40 {
                TELETEXT_YELLOW
            } else {
                TELETEXT_GREEN
            };
            let name_clip: String = m.country_name.chars().take(10).collect();
            let bar_w = (col_rect.width as usize).saturating_sub(28).clamp(6, 18);
            let filled = ((m.evi_score.min(100) as usize) * bar_w) / 100;
            let bar = format!("{}{}", "█".repeat(filled), "░".repeat(bar_w.saturating_sub(filled)));
            lines.push(Line::from(vec![
                Span::styled(
                    format!(" {:<10} ", name_clip),
                    Style::default().fg(FG_PRIMARY).add_modifier(Modifier::BOLD),
                ),
                Span::styled(bar, Style::default().fg(col)),
                Span::styled(
                    format!(" {:>3} ({:+}% │{})", m.evi_score, m.delta_pct, m.event_count),
                    Style::default().fg(col).add_modifier(Modifier::BOLD),
                ),
            ]));
            state.hit_tester.register(
                Rect {
                    x: col_rect.x,
                    y: col_rect.y + r_off as u16,
                    width: col_rect.width,
                    height: 1,
                },
                HitTarget::OsintEviCountry(orig_idx),
            );
        }
        frame.render_widget(Paragraph::new(lines), *col_rect);
    }
}

fn render_osint_filter_bar(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let cats = [
        ("[0:TUMU]", 0usize, FG_PRIMARY),
        ("[1:KURESEL]", 1usize, TELETEXT_GREEN),
        ("[2:ASKERI]", 2usize, TELETEXT_YELLOW),
        ("[3:SIBER]", 3usize, TELETEXT_CYAN),
        ("[4:ENERJI]", 4usize, TELETEXT_MAGENTA),
    ];

    let mut cat_spans = Vec::new();
    let mut cat_x = area.x + 1;
    let cat_y = area.y + 1;
    let max_x = area.x + area.width.saturating_sub(1);

    for (idx, (label, cat_id, color)) in cats.iter().enumerate() {
        if idx > 0 {
            if cat_x >= max_x {
                break;
            }
            cat_spans.push(Span::raw(" "));
            cat_x += 1;
        }

        let width = label.chars().count() as u16;
        if cat_x + width > max_x {
            break;
        }

        let is_active = state.news_filter == *cat_id;
        let style = if is_active {
            Style::default()
                .fg(TELETEXT_BLACK)
                .bg(*color)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(*color)
                .bg(BG_SURFACE)
                .add_modifier(Modifier::BOLD)
        };

        state.hit_tester.register(
            Rect {
                x: cat_x,
                y: cat_y,
                width,
                height: 1,
            },
            HitTarget::OsintCategory(*cat_id),
        );
        cat_x += width;
        cat_spans.push(Span::styled(*label, style));
    }

    let sep = " │ ";
    let sep_w = sep.chars().count() as u16;
    if cat_x + sep_w <= max_x {
        cat_spans.push(Span::styled(sep, Style::default().fg(FG_MUTED)));
        cat_x += sep_w;

        let conflict_lbl = format!("[5:CATISMA/GDELT({})]", state.acled_events.len());
        let conflict_style = if state.news_filter == 5 {
            Style::default()
                .fg(TELETEXT_BLACK)
                .bg(TELETEXT_YELLOW)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(TELETEXT_RED)
                .bg(BG_SURFACE)
                .add_modifier(Modifier::BOLD)
        };

        let avail = max_x.saturating_sub(cat_x);
        let cw = conflict_lbl.chars().count() as u16;
        if cw <= avail {
            state.hit_tester.register(
                Rect {
                    x: cat_x,
                    y: cat_y,
                    width: cw,
                    height: 1,
                },
                HitTarget::OsintCategory(5),
            );
            cat_spans.push(Span::styled(conflict_lbl, conflict_style));
        } else if avail >= 4 {
            let clipped: String = conflict_lbl.chars().take(avail as usize).collect();
            let clipped_w = clipped.chars().count() as u16;
            state.hit_tester.register(
                Rect {
                    x: cat_x,
                    y: cat_y,
                    width: clipped_w,
                    height: 1,
                },
                HitTarget::OsintCategory(5),
            );
            cat_spans.push(Span::styled(clipped, conflict_style));
        }
    }

    let header_p = Paragraph::new(Line::from(cat_spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(" [ KURESEL ACIK KAYNAK & CATISMA FILTRESI ] ")
            .style(Style::default().bg(BG_PANEL).fg(FG_PRIMARY)),
    );
    frame.render_widget(header_p, area);
}

fn render_conflict_table(frame: &mut Frame, table_area: Rect, state: &mut AppState) {
    let q_lower = state.search_query.to_lowercase();
    let filtered: Vec<(usize, _)> = state
        .acled_events
        .iter()
        .enumerate()
        .filter(|(_, ev)| {
            if q_lower.is_empty() {
                true
            } else {
                ev.country.to_lowercase().contains(&q_lower)
                    || ev.location.to_lowercase().contains(&q_lower)
                    || ev.event_type.to_lowercase().contains(&q_lower)
                    || ev.actor1.to_lowercase().contains(&q_lower)
                    || ev.notes.to_lowercase().contains(&q_lower)
            }
        })
        .collect();

    let total_items = filtered.len();
    let visible_rows = table_area.height.saturating_sub(3) as usize;
    let scroll = state.news_scroll.min(total_items.saturating_sub(1));

    let header = Row::new(
        ["TARIH", "ULKE / KONUM", "OLAY TURU", "TARAFLAR", "NOT / DETAY"]
            .into_iter()
            .map(|h| {
                Cell::from(h).style(
                    Style::default()
                        .fg(TELETEXT_YELLOW)
                        .add_modifier(Modifier::BOLD),
                )
            }),
    )
    .height(1)
    .style(Style::default().bg(BG_SURFACE));

    let mut rows = Vec::new();
    for (offset, (orig_idx, ev)) in filtered.iter().skip(scroll).take(visible_rows).enumerate() {
        let is_sel = (state.selected_domain == 52 && state.selected_row == *orig_idx)
            || (state.selected_domain != 52 && offset == 0);
        let zebra_bg = if offset.is_multiple_of(2) {
            BG_PANEL
        } else {
            BG_PRIMARY
        };
        let row_style = if is_sel {
            Style::default()
                .fg(TELETEXT_YELLOW)
                .bg(BG_HIGHLIGHT)
                .add_modifier(Modifier::BOLD)
        } else if ev.fatalities > 0 {
            Style::default()
                .fg(TELETEXT_RED)
                .bg(zebra_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(TELETEXT_CYAN).bg(zebra_bg)
        };

        let date_str = if is_sel {
            format!("> {}", ev.event_date)
        } else {
            format!("  {}", ev.event_date)
        };
        let loc_str = format!("{}, {}", ev.country, ev.location);
        let type_str = format!("{} ({})", ev.event_type, ev.fatalities);
        let actors = match ev.actor2.as_deref() {
            Some(a2) if !a2.is_empty() => format!("{} vs {}", ev.actor1, a2),
            _ => ev.actor1.clone(),
        };

        rows.push(
            Row::new(vec![
                Cell::from(date_str),
                Cell::from(loc_str),
                Cell::from(type_str),
                Cell::from(actors),
                Cell::from(ev.notes.as_str()),
            ])
            .style(row_style),
        );

        state.hit_tester.register(
            Rect {
                x: table_area.x + 1,
                y: table_area.y + 2 + offset as u16,
                width: table_area.width.saturating_sub(2),
                height: 1,
            },
            HitTarget::TableRow(52, *orig_idx),
        );
    }

    let tbl = Table::new(
        rows,
        [
            Constraint::Length(13),
            Constraint::Length(24),
            Constraint::Length(22),
            Constraint::Length(24),
            Constraint::Min(20),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(format!(
                " [ GDELT / ACLED KINETIK CATISMA & KRIZ OLAYLARI ({}/{}) ] ",
                (scroll + 1).min(total_items),
                total_items
            ))
            .style(Style::default().bg(BG_PRIMARY).fg(FG_PRIMARY)),
    );
    frame.render_widget(tbl, table_area);
    render_table_scrollbar(frame, table_area, total_items, visible_rows, scroll);
}

fn render_news_table(frame: &mut Frame, table_area: Rect, state: &mut AppState) {
    let q_lower = state.search_query.to_lowercase();
    let news_items: Vec<(usize, _)> = state
        .news
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            let cat_ok = match state.news_filter {
                1 => item.category.starts_with("P201"),
                2 => item.category.starts_with("P202"),
                3 => item.category.starts_with("P203"),
                4 => item.category.starts_with("P204"),
                _ => true,
            };
            let search_ok = if q_lower.is_empty() {
                true
            } else {
                item.title.to_lowercase().contains(&q_lower)
                    || item.source.to_lowercase().contains(&q_lower)
            };
            cat_ok && search_ok
        })
        .collect();

    let total_items = news_items.len();
    let visible_rows = table_area.height.saturating_sub(3) as usize;
    let scroll = state.news_scroll.min(total_items.saturating_sub(1));

    let header = Row::new(
        ["SAAT", "ONEM", "AJANS", "HABER BASLIGI"]
            .into_iter()
            .map(|h| {
                Cell::from(h).style(
                    Style::default()
                        .fg(TELETEXT_YELLOW)
                        .add_modifier(Modifier::BOLD),
                )
            }),
    )
    .height(1)
    .style(Style::default().bg(BG_SURFACE));

    let mut rows = Vec::new();
    for (row_offset, (orig_idx, item)) in news_items
        .iter()
        .skip(scroll)
        .take(visible_rows)
        .enumerate()
    {
        let is_sel = (state.selected_domain == 1 && state.selected_row == *orig_idx)
            || (state.selected_domain != 1 && row_offset == 0);
        let zebra_bg = if row_offset.is_multiple_of(2) {
            BG_PANEL
        } else {
            BG_PRIMARY
        };
        let sev_color = match item.severity {
            ThreatSeverity::Critical | ThreatSeverity::Emergency => TELETEXT_RED,
            ThreatSeverity::Warning => TELETEXT_YELLOW,
            ThreatSeverity::Advisory => TELETEXT_CYAN,
            ThreatSeverity::Low => TELETEXT_GREEN,
        };

        let time_str = item.published.format("%H:%M").to_string();
        let time_cell = if is_sel {
            format!("> {}", time_str)
        } else {
            format!("  {}", time_str)
        };
        let sev_tag = format!("[{}]", item.severity.as_str());

        let (row_style, time_c, sev_c, src_c) = if is_sel {
            (
                Style::default()
                    .fg(TELETEXT_YELLOW)
                    .bg(BG_HIGHLIGHT)
                    .add_modifier(Modifier::BOLD),
                Cell::from(time_cell),
                Cell::from(sev_tag),
                Cell::from(item.source.as_str()),
            )
        } else {
            (
                Style::default().fg(FG_PRIMARY).bg(zebra_bg),
                Cell::from(time_cell).style(Style::default().fg(TELETEXT_CYAN)),
                Cell::from(sev_tag)
                    .style(Style::default().fg(sev_color).add_modifier(Modifier::BOLD)),
                Cell::from(item.source.as_str()).style(Style::default().fg(TELETEXT_YELLOW)),
            )
        };

        rows.push(
            Row::new(vec![time_c, sev_c, src_c, Cell::from(item.title.as_str())]).style(row_style),
        );

        state.hit_tester.register(
            Rect {
                x: table_area.x + 1,
                y: table_area.y + 2 + row_offset as u16,
                width: table_area.width.saturating_sub(2),
                height: 1,
            },
            HitTarget::TableRow(1, *orig_idx),
        );
    }

    let filter_title = match state.news_filter {
        1 => "P201 KURESEL",
        2 => "P202 ASKERI/KRIZ",
        3 => "P203 SIBER",
        4 => "P204 ENERJI",
        _ => "TUMU",
    };

    let tbl = Table::new(
        rows,
        [
            Constraint::Length(9),
            Constraint::Length(11),
            Constraint::Length(18),
            Constraint::Min(30),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(format!(
                " [ CANLI AJANS AKISI - {} ({}/{}) ] ",
                filter_title,
                (scroll + 1).min(total_items),
                total_items
            ))
            .style(Style::default().bg(BG_PRIMARY).fg(FG_PRIMARY)),
    );
    frame.render_widget(tbl, table_area);
    render_table_scrollbar(frame, table_area, total_items, visible_rows, scroll);
}

fn render_table_scrollbar(
    frame: &mut Frame,
    area: Rect,
    total_items: usize,
    visible_rows: usize,
    scroll: usize,
) {
    if total_items > visible_rows {
        let mut scrollbar_state = ScrollbarState::new(total_items).position(scroll);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(BORDER_ACTIVE).bg(BG_PANEL));
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}
