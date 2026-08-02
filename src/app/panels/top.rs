//! 顶栏面板：标题、连接状态与语言切换。

use eframe::egui;

use crate::i18n::Lang;

use super::super::ProbeUiApp;

impl ProbeUiApp {
    /// 顶栏：标题、连接状态与语言切换。
    pub(crate) fn top_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading(self.t("Probe-rs 烧录工具", "Probe-rs Flasher"));
                ui.separator();
                if self.connected.is_some() {
                    ui.colored_label(
                        egui::Color32::from_rgb(0x2e, 0xa0, 0x43),
                        self.t("● 已连接", "● Connected"),
                    );
                } else {
                    ui.colored_label(
                        egui::Color32::from_rgb(0xcc, 0x88, 0x00),
                        self.t("○ 未连接", "○ Not connected"),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("基于 probe-rs v0.32").weak());
                    egui::ComboBox::from_id_salt("lang_sel")
                        .width(90.0)
                        .selected_text(if self.lang.is_en() {
                            "English"
                        } else {
                            "中文"
                        })
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(!self.lang.is_en(), "中文").clicked() {
                                self.set_lang(Lang::Zh);
                            }
                            if ui.selectable_label(self.lang.is_en(), "English").clicked() {
                                self.set_lang(Lang::En);
                            }
                        });
                });
            });
            ui.add_space(4.0);
        });
    }
}
