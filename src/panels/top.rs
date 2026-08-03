//! 顶栏面板：标题、连接状态、主题与语言切换。

use eframe::egui;

use crate::app::{ProbeUiApp, ThemeMode};
use crate::i18n::{Lang, Msg};

impl ProbeUiApp {
    /// 顶栏：标题、连接状态、主题与语言切换。
    pub(crate) fn top_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading(self.t(Msg::AppTitle));
                ui.separator();
                if self.connected.is_some() {
                    ui.colored_label(
                        egui::Color32::from_rgb(0x2e, 0xa0, 0x43),
                        self.t(Msg::ConnectedDot),
                    );
                } else {
                    ui.colored_label(
                        egui::Color32::from_rgb(0xcc, 0x88, 0x00),
                        self.t(Msg::NotConnectedDot),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("基于 probe-rs v0.32").weak());
                    let theme_text = match self.theme_mode {
                        ThemeMode::System => self.t(Msg::ThemeSystem),
                        ThemeMode::Light => self.t(Msg::ThemeLight),
                        ThemeMode::Dark => self.t(Msg::ThemeDark),
                    };
                    egui::ComboBox::from_id_salt("theme_sel")
                        .width(110.0)
                        .selected_text(format!("🌗 {}", theme_text))
                        .show_ui(ui, |ui| {
                            let l_sys = format!("💻 {}", self.t(Msg::ThemeSystem));
                            let l_light = format!("☀ {}", self.t(Msg::ThemeLight));
                            let l_dark = format!("🌙 {}", self.t(Msg::ThemeDark));
                            if ui
                                .selectable_value(&mut self.theme_mode, ThemeMode::System, l_sys)
                                .clicked()
                            {
                                self.set_theme(ThemeMode::System);
                            }
                            if ui
                                .selectable_value(&mut self.theme_mode, ThemeMode::Light, l_light)
                                .clicked()
                            {
                                self.set_theme(ThemeMode::Light);
                            }
                            if ui
                                .selectable_value(&mut self.theme_mode, ThemeMode::Dark, l_dark)
                                .clicked()
                            {
                                self.set_theme(ThemeMode::Dark);
                            }
                        });
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
