//! 底部日志面板：全局显示操作日志（各中央标签共用）。

use eframe::egui;

use crate::app::{LogLevel, ProbeUiApp};
use crate::i18n::Msg;
use crate::t;

impl ProbeUiApp {
    /// 底部日志面板：显示操作日志，可清空。
    pub(crate) fn log_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("log_panel")
            .resizable(false)
            .exact_height(self.target_info_h.max(crate::app::TARGET_INFO_MIN_H))
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.heading(self.t(Msg::Log));
                    if ui.button(self.icon("🗑", Msg::Clear)).clicked() {
                        self.log.clear();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(t!(self.lang, Msg::LogEntries, self.log.len()))
                                .small()
                                .weak(),
                        );
                    });
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("log_scroll")
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for e in &self.log {
                            let color = match e.level {
                                LogLevel::Info => egui::Color32::from_gray(180),
                                LogLevel::Ok => egui::Color32::from_rgb(0x2e, 0xa0, 0x43),
                                LogLevel::Warn => egui::Color32::from_rgb(0xcc, 0x88, 0x00),
                                LogLevel::Error => egui::Color32::from_rgb(0xc0, 0x3a, 0x2b),
                            };
                            ui.label(egui::RichText::new(&e.text).color(color));
                        }
                    });
            });
    }
}
