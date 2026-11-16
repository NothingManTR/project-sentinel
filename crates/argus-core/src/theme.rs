use ratatui::style::Color;

/// Modern Karanlık Mühendislik / C4ISR Paleti (Sıfır Göz Yorgunluğu, Yüksek Kontrast)
pub const BG_PRIMARY: Color = Color::Rgb(8, 12, 20);        // Ana arka plan (Obsidyen)
pub const BG_PANEL: Color = Color::Rgb(13, 19, 31);         // Panel zemini
pub const BG_SURFACE: Color = Color::Rgb(20, 29, 46);       // Başlık / Kart yüzeyi
pub const BG_HIGHLIGHT: Color = Color::Rgb(22, 62, 98);     // Seçili satır / aktif öğe vurgusu
pub const BORDER_DIM: Color = Color::Rgb(42, 58, 84);       // Pasif çerçeve çizgisi
pub const BORDER_ACTIVE: Color = Color::Rgb(56, 189, 248);  // Aktif odak çerçevesi (Cyan)

pub const FG_PRIMARY: Color = Color::Rgb(226, 232, 240);    // Birincil metin
pub const FG_SECONDARY: Color = Color::Rgb(148, 163, 184);  // İkincil etiket
pub const FG_MUTED: Color = Color::Rgb(100, 116, 139);      // Pasif metin

/// Geriye dönük modül uyumluluğu için karanlık temaya eşlenmiş sabitler
/// (Eski parlak gri #C0C0C0 ve camgöbeği masaüstü renkleri tamamen koyu C4ISR yüzeylerine dönüştürüldü)
pub const WIN98_NAVY: Color = Color::Rgb(22, 62, 98);
pub const WIN98_GRAY: Color = Color::Rgb(26, 36, 54);
pub const WIN98_SILVER: Color = Color::Rgb(13, 19, 31);
pub const WIN98_DARK_GRAY: Color = Color::Rgb(52, 70, 98);
pub const WIN98_WHITE: Color = Color::Rgb(226, 232, 240);
pub const WIN98_TEAL: Color = Color::Rgb(8, 12, 20);
pub const WIN98_BLACK: Color = Color::Rgb(210, 220, 235);

/// Taktik Semantik Vurgu Renkleri
pub const TELETEXT_YELLOW: Color = Color::Rgb(245, 185, 45);   // Başlıklar, uyarılar, seçili vurgu
pub const TELETEXT_CYAN: Color = Color::Rgb(56, 189, 248);     // Hava hedefleri, koordinatlar, telemetri
pub const TELETEXT_GREEN: Color = Color::Rgb(34, 197, 94);     // Deniz hedefleri, aktif bağlantı, güvenli durum
pub const TELETEXT_RED: Color = Color::Rgb(239, 68, 68);       // Askeri/Acil hedefler, Squawk 7700, M5.5+ deprem
pub const TELETEXT_MAGENTA: Color = Color::Rgb(168, 85, 247);  // Uydu yörüngeleri (SGP4)
pub const TELETEXT_BLUE: Color = Color::Rgb(37, 99, 235);      // İkincil aksan
pub const TELETEXT_WHITE: Color = Color::Rgb(226, 232, 240);   // Gövde metni
pub const TELETEXT_BLACK: Color = Color::Rgb(8, 12, 20);       // Tablo koyu zemini

/// Saf ASCII Pencere Kontrol Sembolleri
pub const WIN98_BTN_MIN: &str = "[_]";
pub const WIN98_BTN_MAX: &str = "[+]";
pub const WIN98_BTN_RESTORE: &str = "[-]";
pub const WIN98_BTN_CLOSE: &str = "[X]";
pub const WIN98_START_LABEL: &str = " [MENÜ] ";
