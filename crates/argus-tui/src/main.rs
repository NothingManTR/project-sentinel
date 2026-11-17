mod app;
mod audio;
mod photo_ui;
mod ui;
mod views;
mod web_map;
mod widgets;

use app::App;
use audio::AudioManager;
use argus_core::config::AppConfig;
use argus_core::state::AppState;
use clap::Parser;
use crossterm::{
    cursor::{Hide, Show},
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use parking_lot::RwLock;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, stdout};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(author, version, about = "ARGUS-OSINT Situational Awareness & Global Monitoring Terminal", long_about = None)]
struct Args {
    /// Path to config.toml file
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // 1. Yapılandırma dosyasını yükle
    let config = AppConfig::load_from_file(&args.config).unwrap_or_else(|_| {
        AppConfig::load_or_default()
    });

    let initial_muted = !config.audio.enabled;

    // 2. Global State oluştur
    let state = Arc::new(RwLock::new(AppState::new(config)));
    let sdr_queue = Arc::clone(&state.read().sdr_audio_queue);

    // 3. Audio Manager başlat (Gerçek KiwiSDR PCM kuyruğuna bağlı)
    let audio = AudioManager::init(initial_muted, sdr_queue);

    // 4. Veri toplama arka plan işçilerini ve yerel Web Harita sunucusunu (127.0.0.1:8090) başlat
    argus_ingest::start_ingestion_workers(Arc::clone(&state));
    web_map::start_web_map_server(Arc::clone(&state));

    // 5. Terminal Ayarları ve Panic Hook
    setup_panic_hook();

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, Hide)?;
    let picker = if std::env::var_os("TMUX").is_some() {
        ratatui_image::picker::Picker::from_fontsize((8, 16))
    } else {
        ratatui_image::picker::Picker::from_query_stdio()
            .unwrap_or_else(|_| ratatui_image::picker::Picker::from_fontsize((8, 16)))
    };
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 6. Ana TUI Uygulamasını Çalıştır
    let mut app = App::new(state, audio, picker);
    let app_res = app.run(&mut terminal).await;

    // 7. Terminali Güvenle Geri Yükle
    restore_terminal(&mut terminal)?;

    if let Err(e) = app_res {
        eprintln!("[ARGUS HATA]: {}", e);
    }

    Ok(())
}

fn setup_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture, Show);
        original_hook(panic_info);
    }));
}

fn restore_terminal<B: ratatui::backend::Backend + io::Write>(
    terminal: &mut Terminal<B>,
) -> anyhow::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        Show
    )?;
    terminal.show_cursor()?;
    Ok(())
}
