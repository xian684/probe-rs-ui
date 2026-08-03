//! 左栏底部目标信息框：连接成功后展示芯片与内存映射。

use eframe::egui;

use crate::app::{ProbeUiApp, TARGET_INFO_MIN_H};
use crate::i18n::Msg;
use crate::t;

impl ProbeUiApp {
    /// 左栏底部固定显示的目标信息框（连接成功后展示芯片与内存映射）。
    pub(crate) fn target_info_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.separator();
        match &self.connected {
            Some(summary) => {
                ui.heading(self.t(Msg::TargetInfo));
                ui.label(t!(self.lang, Msg::ChipModel, summary.name));
                ui.label(t!(self.lang, Msg::Arch, summary.architecture));
                ui.label(t!(self.lang, Msg::CoreCount, summary.cores.len()));
                for (i, c) in &summary.cores {
                    ui.label(t!(self.lang, Msg::Core, i, c));
                }
                ui.add_space(4.0);
                ui.label(egui::RichText::new(self.t(Msg::MemoryMap)).strong());
                egui::ScrollArea::vertical()
                    .id_salt("mem_scroll")
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for m in &summary.memory {
                            ui.monospace(format!(
                                "  [{}] 0x{:08X} - 0x{:08X}  ({} KB)",
                                m.kind,
                                m.start,
                                m.end,
                                (m.end - m.start) / 1024
                            ));
                        }
                    });
            }
            None => {
                ui.label(egui::RichText::new(self.t(Msg::TargetNotConnected)).weak());
            }
        }
        let pad = (TARGET_INFO_MIN_H - ui.min_rect().height()).max(0.0);
        ui.add_space(pad);
    }
}
