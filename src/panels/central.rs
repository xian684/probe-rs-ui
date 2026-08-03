//! 中央面板：在『固件烧录』、『内存查看器』、『RTT 日志』与『ARM 在线索引』之间切换。

use eframe::egui;

use crate::app::{CentralTab, ProbeUiApp};
use crate::i18n::Msg;

impl ProbeUiApp {
    /// 中央面板：顶部标签在固件烧录、内存查看器、RTT 日志与 ARM 在线索引之间切换。
    pub(crate) fn central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(6.0);
            let l_flash = self.icon("⚡", Msg::FirmwareFlashing);
            let l_mem = self.icon("🔬", Msg::MemoryViewer);
            let l_rtt = self.icon("📡", Msg::RttLog);
            let l_arm = self.icon("🌐", Msg::ArmSearchTitle);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.central_tab, CentralTab::Flash, l_flash);
                ui.selectable_value(&mut self.central_tab, CentralTab::Memory, l_mem);
                ui.selectable_value(&mut self.central_tab, CentralTab::Rtt, l_rtt);
                ui.selectable_value(&mut self.central_tab, CentralTab::ArmIndex, l_arm);
            });
            ui.separator();
            match self.central_tab {
                CentralTab::Flash => self.flash_view_ui(ui),
                CentralTab::Memory => self.mem_view_ui(ui),
                CentralTab::Rtt => self.rtt_view_ui(ui),
                CentralTab::ArmIndex => self.arm_view_ui(ui),
            }
        });
    }
}
