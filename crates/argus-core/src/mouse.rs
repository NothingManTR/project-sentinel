use crate::types::ActiveView;
use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitTarget {
    WindowHeader(usize),
    WindowMinimize(usize),
    WindowMaximize(usize),
    WindowClose(usize),
    WindowBody(usize),
    StartButton,
    StartMenuItem(usize),
    TaskbarWindowTab(usize),
    TaskbarViewTab(ActiveView),
    TrayAudio,
    TrayClock,
    MapCanvas,
    TableRow(usize, usize),
    DialogClose,
    GenericButton(usize),
    CalcKey(char),
    GeointLayer(usize),
    SigintPreset(usize),
    ToggleRadioAudio,
    AirFilter(usize),
    SeaFilter(usize),
    OsintCategory(usize),
    MapStyleToggle,
    ToggleMapAircraftShapes,
    MapTarget(usize, usize),
    SortColumn(usize, usize),
    SearchToggle,
    SearchClear,
    HelpButton,
    SelectSdrNode(usize),
    SdrNodeList,
    ToggleSdrAutoFailover,
    ToggleSdrFreqInput,
    CycleSdrMode,
    ToggleSdrRecord,
    ToggleSdrScanner,
    ToggleSdrVox,
    ToggleSdrCw,
    MapSdrNode(usize),
    ShodanPreset(usize),
    MasintTab(usize),
    InspectorAction(usize),
    RegionPreset(usize),
    OpenWebMap,
    ExportReport,
    TriggerAiAnalysis,
    DefconBadge,
    DialogSaveFile,
    DialogExportNote,
    DialogExportHtml,
    AiMode(usize),
    AiRefresh,
    NotepadFile(usize),
    NotepadAction(usize),
    AlertItem(usize),
    FusionAlert(usize),
    StraitGate(usize),
    FlashAlertFocus,
    FlashAlertAck,
    DvrStep(i32),
    RulerSetPinA,
    RulerSetPinB,
    RulerCycleRing,
    RulerClear,
}

#[derive(Debug, Clone, Default)]
pub struct HitTester {
    regions: Vec<(Rect, HitTarget)>,
    pub is_dragging: bool,
    pub drag_start: Option<(u16, u16)>,
    pub last_drag: Option<(u16, u16)>,
}

impl HitTester {
    pub fn new() -> Self {
        Self {
            regions: Vec::with_capacity(64),
            is_dragging: false,
            drag_start: None,
            last_drag: None,
        }
    }

    pub fn clear(&mut self) {
        self.regions.clear();
    }

    pub fn register(&mut self, area: Rect, target: HitTarget) {
        if area.width > 0 && area.height > 0 {
            self.regions.push((area, target));
        }
    }

    pub fn hit_test(&self, col: u16, row: u16) -> Option<HitTarget> {
        // En son eklenen (üst katman / popup) önce kontrol edilir
        for (area, target) in self.regions.iter().rev() {
            if col >= area.x && col < area.x + area.width && row >= area.y && row < area.y + area.height {
                return Some(*target);
            }
        }
        None
    }

    pub fn on_drag_start(&mut self, col: u16, row: u16) {
        self.is_dragging = true;
        self.drag_start = Some((col, row));
        self.last_drag = Some((col, row));
    }

    pub fn on_drag_update(&mut self, col: u16, row: u16) -> (i16, i16) {
        if let Some((last_c, last_r)) = self.last_drag {
            let dx = col as i16 - last_c as i16;
            let dy = row as i16 - last_r as i16;
            self.last_drag = Some((col, row));
            (dx, dy)
        } else {
            self.last_drag = Some((col, row));
            (0, 0)
        }
    }

    pub fn on_drag_end(&mut self) {
        self.is_dragging = false;
        self.drag_start = None;
        self.last_drag = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bordered_table_row_hit_alignment() {
        let mut ht = HitTester::new();
        let table_area = Rect {
            x: 10,
            y: 6,
            width: 100,
            height: 20,
        };
        // y=6: top border, y=7: header, y=8..: data rows (y + 2 + offset)
        ht.register(
            Rect {
                x: table_area.x + 1,
                y: table_area.y + 1,
                width: 15,
                height: 1,
            },
            HitTarget::SortColumn(20, 1),
        );
        for offset in 0..5 {
            ht.register(
                Rect {
                    x: table_area.x + 1,
                    y: table_area.y + 2 + offset as u16,
                    width: table_area.width - 2,
                    height: 1,
                },
                HitTarget::TableRow(20, offset),
            );
        }

        // Clicking header at y=7 hits SortColumn, NOT TableRow(20, 0)
        assert_eq!(ht.hit_test(12, 7), Some(HitTarget::SortColumn(20, 1)));
        // Clicking first data row at y=8 hits TableRow(20, 0)
        assert_eq!(ht.hit_test(25, 8), Some(HitTarget::TableRow(20, 0)));
        // Clicking second data row at y=9 hits TableRow(20, 1)
        assert_eq!(ht.hit_test(25, 9), Some(HitTarget::TableRow(20, 1)));
    }
}
