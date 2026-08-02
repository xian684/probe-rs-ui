//! 中央面板：在『固件烧录』与『内存查看器』两个视图之间切换。

use eframe::egui;

use crate::app::ProbeUiApp;

impl ProbeUiApp {
    /// 中央面板：顶部标签在固件烧录与内存查看器之间切换。
    pub(crate) fn central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(6.0);
            let l_flash = self.icon("⚡", "固件烧录", "Firmware Flashing");
            let l_mem = self.icon("🔬", "内存查看器", "Memory Viewer");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.mem_mode, false, l_flash);
                ui.selectable_value(&mut self.mem_mode, true, l_mem);
            });
            ui.separator();
            if self.mem_mode {
                self.mem_view_ui(ui);
            } else {
                self.flash_view_ui(ui);
            }
        });
    }
}
