use argus_core::state::AppState;
use argus_core::types::{AircraftMediaStatus, AircraftPhotoEntry};
use ratatui::layout::Rect;
use ratatui::Frame;
use ratatui_image::{
    picker::{Picker, ProtocolType},
    protocol::StatefulProtocol,
    StatefulImage,
};
use std::time::Instant;

/// Terminal içi uçak fotoğrafı decode, önbellek ve Sixel/Kitty/Halfblocks render yöneticisi
pub struct PhotoRenderCtx {
    pub picker: Picker,
    pub active_protocol: Option<(String, StatefulProtocol)>,
    pub last_rendered_slot: Option<(String, (u16, u16, u16, u16))>,
    pub last_inspector_icao: Option<String>,
    pub inspector_changed_at: Instant,
}

impl PhotoRenderCtx {
    pub fn new(picker: Picker) -> Self {
        Self {
            picker,
            active_protocol: None,
            last_rendered_slot: None,
            last_inspector_icao: None,
            inspector_changed_at: Instant::now(),
        }
    }

    pub fn is_graphics_protocol(&self) -> bool {
        self.picker.protocol_type() != ProtocolType::Halfblocks
    }

    /// Gelen ham görsel byte'larını decode edip terminal protokolüne (Sixel/Halfblocks) hazırlar
    fn ensure_protocol(&mut self, entry: &AircraftPhotoEntry) -> bool {
        let key = entry.icao.to_lowercase();
        if let Some((cur_key, _)) = &self.active_protocol {
            if cur_key == &key {
                return true;
            }
        }
        let Some(bytes) = entry.image_bytes.as_ref() else {
            return false;
        };
        let Ok(dyn_img) = image::load_from_memory(bytes) else {
            return false;
        };
        let proto = self.picker.new_resize_protocol(dyn_img);
        self.active_protocol = Some((key, proto));
        true
    }

    /// UI çiziminin en sonunda aktif uçak fotoğrafı alanını (Inspector veya T-Dossier) çizer
    pub fn render_active_slot(&mut self, frame: &mut Frame, state: &mut AppState) {
        let new_slot = state.pending_photo_slot.clone();

        // Sixel / Kitty terminallerinde resim kutusu kapandığında veya konumu değiştiğinde
        // piksel artığı kalmaması için bir sonraki karede ekran temizliği tetiklenir.
        if self.is_graphics_protocol() {
            if let Some(prev) = &self.last_rendered_slot {
                if new_slot.as_ref() != Some(prev) {
                    state.force_terminal_clear = true;
                }
            }
        }

        let Some((icao, (x, y, w, h))) = new_slot else {
            self.last_rendered_slot = None;
            return;
        };

        if w < 6 || h < 3 {
            self.last_rendered_slot = None;
            return;
        }

        let key = icao.to_lowercase();
        let entry_opt = state
            .aircraft_photo_cache
            .get(&key)
            .cloned()
            .or_else(|| match &state.aircraft_media_status {
                AircraftMediaStatus::Ready(e) if e.icao.eq_ignore_ascii_case(&key) => Some(e.clone()),
                _ => None,
            });

        let Some(entry) = entry_opt else {
            self.last_rendered_slot = None;
            return;
        };

        if !self.ensure_protocol(&entry) {
            self.last_rendered_slot = None;
            return;
        }

        if let Some((cur_k, proto)) = &mut self.active_protocol {
            if cur_k == &key {
                let area = Rect::new(x, y, w, h);
                frame.render_stateful_widget(StatefulImage::default(), area, proto);
                self.last_rendered_slot = Some((key, (x, y, w, h)));
            }
        }
    }
}
