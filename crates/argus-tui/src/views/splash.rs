use argus_core::mouse::HitTarget;
use argus_core::state::AppState;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, Paragraph},
    Frame,
};

#[allow(dead_code)]
pub fn render_splash_view(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if area.width < 10 || area.height < 6 {
        return;
    }

    // Ekranı tamamen temizle ve siyah BIOS zeminine boya
    frame.render_widget(Clear, area);
    let bg = Block::default().style(Style::default().bg(Color::Black));
    frame.render_widget(bg, area);

    // Tüm ekranı tıklama hedefi yap (Herhangi bir tıkla açılış geçilsin)
    state.hit_tester.register(area, HitTarget::GenericButton(999));

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // BIOS Başlık ve Telif
            Constraint::Length(5), // Donanım ve Bellek Testi
            Constraint::Length(12), // Alt Sistem Kontrolleri (OK Tablosu)
            Constraint::Length(3), // İlerleme Çubuğu (Gauge)
            Constraint::Min(1),    // Alt Bilgi Notu
        ])
        .split(area);

    // 1. BIOS Başlık
    let header_lines = vec![
        Line::from(vec![
            Span::styled("PROJECT SENTINEL BIOS v4.10.1998", Style::default().fg(Color::Rgb(255, 255, 255)).add_modifier(Modifier::BOLD)),
            Span::styled(", An Energy Star Ally", Style::default().fg(Color::Rgb(0, 220, 220))),
        ]),
        Line::from(Span::styled("Copyright (C) 1998-2026, Sentinel Strategic Intelligence Workstation", Style::default().fg(Color::Rgb(180, 180, 180)))),
        Line::from(Span::styled("Sistem Kimliği: NODE-IST-01 // Güvenli Çekirdek Başlatıcı", Style::default().fg(Color::Rgb(120, 120, 120)))),
        Line::from(""),
    ];
    frame.render_widget(Paragraph::new(header_lines), layout[0]);

    // 2. Donanım ve Bellek Testi
    let mem_kb = (state.splash_progress as u32 * 65536 / 100).min(65536);
    let hw_lines = vec![
        Line::from(vec![
            Span::styled("İşlemci Mimarisi : ", Style::default().fg(Color::Rgb(180, 180, 180))),
            Span::styled("Multi-Core Real-Time Asynchronous Tokio Runtime", Style::default().fg(Color::Rgb(255, 255, 255)).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Matematik Birimi : ", Style::default().fg(Color::Rgb(180, 180, 180))),
            Span::styled("IEEE-754 Kayan Noktalı Vektör Motoru Aktif", Style::default().fg(Color::Rgb(255, 255, 255))),
        ]),
        Line::from(vec![
            Span::styled("Bellek Testi     : ", Style::default().fg(Color::Rgb(180, 180, 180))),
            Span::styled(format!("{}K OK", mem_kb), Style::default().fg(Color::Rgb(0, 255, 120)).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Birincil Sürücü  : ", Style::default().fg(Color::Rgb(180, 180, 180))),
            Span::styled("SENTINEL-NVME 2.0TB HIGH-SPEED BUS (DMA AKTİF)", Style::default().fg(Color::Rgb(200, 200, 200))),
        ]),
        Line::from(""),
    ];
    frame.render_widget(Paragraph::new(hw_lines), layout[1]);

    // 3. Alt Sistem Başlatma Tablosu
    let p = state.splash_progress;
    let subsystems = [
        ("GEOINT", "EPSG:4326 Vektör Harita & Boğazlar/Chokepoint Geometrisi", 10),
        ("AIRINT", "1090 MHz Mode-S OpenSky ADS-B Taktik Askeri Radar", 20),
        ("SEAINT", "AISStream Boğazlar & Akdeniz Donanma Transponderi", 35),
        ("SIGINT", "USAF HFGCS 11175 kHz EAM Dinleyici & FFT Waterfall", 50),
        ("MASINT", "USGS Sismik Ağ ve NASA FIRMS Termal Uydu Sensörü", 65),
        ("CONFLICT", "ACLED Gerçek Zamanlı Silahlı Çatışma Veri Tabanı", 75),
        ("SATINT", "CelesTrak SGP4 Yörünge Propagasyon ve Polar Radar", 85),
        ("CYBERINT", "Shodan InternetDB Zafiyet ve Altyapı Tarayıcısı", 92),
        ("TOOLS", "Win98 Taktik Not Defteri & OSINT Birim Dönüştürücü", 98),
    ];

    let mut subs_lines = Vec::new();
    for (code, desc, threshold) in subsystems.iter() {
        if p >= *threshold {
            subs_lines.push(Line::from(vec![
                Span::styled(" [ OK ] ", Style::default().fg(Color::Rgb(0, 255, 0)).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:<9}: ", code), Style::default().fg(Color::Rgb(255, 255, 0)).add_modifier(Modifier::BOLD)),
                Span::styled(*desc, Style::default().fg(Color::Rgb(220, 220, 220))),
            ]));
        } else {
            subs_lines.push(Line::from(vec![
                Span::styled(" [WAIT] ", Style::default().fg(Color::Rgb(120, 120, 120))),
                Span::styled(format!("{:<9}: ", code), Style::default().fg(Color::Rgb(120, 120, 120))),
                Span::styled("Başlatılıyor...", Style::default().fg(Color::Rgb(100, 100, 100))),
            ]));
        }
    }
    frame.render_widget(Paragraph::new(subs_lines), layout[2]);

    // 4. İlerleme Göstergesi
    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(0, 180, 220)))
                .title(" SİSTEM BAŞLATILIYOR "),
        )
        .gauge_style(Style::default().fg(Color::Rgb(0, 220, 255)).bg(Color::Rgb(20, 30, 40)))
        .percent(state.splash_progress as u16)
        .label(format!("%{} TAMAMLANDI", state.splash_progress));
    frame.render_widget(gauge, layout[3]);

    // 5. Alt Bilgilendirme
    let foot_p = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::Rgb(160, 160, 160))),
            Span::styled("SPACE / ENTER / ESC", Style::default().fg(Color::Rgb(255, 255, 255)).add_modifier(Modifier::BOLD)),
            Span::styled(" or ", Style::default().fg(Color::Rgb(160, 160, 160))),
            Span::styled("CLICK", Style::default().fg(Color::Rgb(255, 255, 0)).add_modifier(Modifier::BOLD)),
            Span::styled(" to enter SENTINEL WORKSTATION immediately...", Style::default().fg(Color::Rgb(160, 160, 160))),
        ]),
    ]);
    frame.render_widget(foot_p, layout[4]);
}
