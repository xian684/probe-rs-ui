//! 中央面板：在『固件烧录』、『内存查看器』与『RTT 日志』三个视图之间切换。

use eframe::egui;

use crate::app::{CentralTab, ProbeUiApp};

impl ProbeUiApp {
    /// 中央面板：顶部标签在固件烧录、内存查看器与 RTT 日志之间切换。
    pub(crate) fn central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(6.0);
            let l_flash = self.icon("⚡", "固件烧录", "Firmware Flashing");
            let l_mem = self.icon("🔬", "内存查看器", "Memory Viewer");
            let l_rtt = self.icon("📡", "RTT 日志", "RTT Log");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.central_tab, CentralTab::Flash, l_flash);
                ui.selectable_value(&mut self.central_tab, CentralTab::Memory, l_mem);
                ui.selectable_value(&mut self.central_tab, CentralTab::Rtt, l_rtt);
            });
            ui.separator();
            match self.central_tab {
                CentralTab::Flash => self.flash_view_ui(ui),
                CentralTab::Memory => self.mem_view_ui(ui),
                CentralTab::Rtt => self.rtt_view_ui(ui),
            }
        });
    }
}
