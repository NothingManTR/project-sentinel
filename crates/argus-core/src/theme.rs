use ratatui::style::Color;

/// Oturakli, Dusuk Pariltili C4ISR Muharebe ve Muhendislik Paleti (Mat Grafit & Celik Mavi)
pub const BG_PRIMARY: Color = Color::Rgb(15, 17, 21);        // Ana arka plan (Mat grafit)
pub const BG_PANEL: Color = Color::Rgb(21, 24, 30);          // Panel zemini (Koyu arduvaz)
pub const BG_SURFACE: Color = Color::Rgb(30, 35, 43);        // Baslik / Kart yuzeyi
pub const BG_HIGHLIGHT: Color = Color::Rgb(44, 54, 68);      // Secili satir / aktif oge vurgusu
pub const BORDER_DIM: Color = Color::Rgb(52, 59, 70);        // Pasif cerceve cizgisi
pub const BORDER_ACTIVE: Color = Color::Rgb(110, 134, 160);  // Aktif odak cercevesi (Fircalanmis celik)

pub const FG_PRIMARY: Color = Color::Rgb(220, 224, 230);     // Birincil metin (Yumusak kirik beyaz)
pub const FG_SECONDARY: Color = Color::Rgb(148, 156, 168);   // Ikincil etiket (Mat gumus)
pub const FG_MUTED: Color = Color::Rgb(96, 105, 118);        // Pasif metin

/// Geriye donuk modul uyumlulugu icin mat palete eslenmis sabitler
pub const WIN98_NAVY: Color = Color::Rgb(44, 54, 68);
pub const WIN98_GRAY: Color = Color::Rgb(30, 35, 43);
pub const WIN98_SILVER: Color = Color::Rgb(21, 24, 30);
pub const WIN98_DARK_GRAY: Color = Color::Rgb(58, 67, 80);
pub const WIN98_WHITE: Color = Color::Rgb(220, 224, 230);
pub const WIN98_TEAL: Color = Color::Rgb(15, 17, 21);
pub const WIN98_BLACK: Color = Color::Rgb(212, 218, 226);

/// Desature Edilmis, Goz Yormayan Taktik Semantik Vurgu Renkleri
pub const TELETEXT_YELLOW: Color = Color::Rgb(204, 170, 96);   // Basliklar, secili vurgu (Mat pirinc)
pub const TELETEXT_CYAN: Color = Color::Rgb(118, 162, 194);    // Sivil hava hedefleri, koordinat, telemetri (Celik mavi)
pub const TELETEXT_GREEN: Color = Color::Rgb(106, 170, 128);   // Deniz hedefleri, aktif baglanti (Ada cayi yesili)
pub const TELETEXT_RED: Color = Color::Rgb(196, 94, 90);       // Askeri / Acil hedefler (Mat kiremit kirmizisi)
pub const TELETEXT_MAGENTA: Color = Color::Rgb(152, 132, 178); // Uydu yorungeleri (Mat lavanta-arduvaz)
pub const TELETEXT_BLUE: Color = Color::Rgb(82, 118, 164);     // Ikincil celik aksan
pub const TELETEXT_WHITE: Color = Color::Rgb(220, 224, 230);   // Govde metni
pub const TELETEXT_BLACK: Color = Color::Rgb(15, 17, 21);      // Koyu kontrast zemin

/// Saf ASCII Pencere Kontrol Sembolleri
pub const WIN98_BTN_MIN: &str = "[_]";
pub const WIN98_BTN_MAX: &str = "[+]";
pub const WIN98_BTN_RESTORE: &str = "[-]";
pub const WIN98_BTN_CLOSE: &str = "[X]";
pub const WIN98_START_LABEL: &str = " [MENU] ";
