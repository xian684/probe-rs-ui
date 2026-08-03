//! 中央『Target 生成器』面板：从 CMSIS Pack 生成 target YAML 定义文件。
//!
//! 输入可以是 .pack / .pdsc / .zip 文件，也可以是解压后的目录。
//! 生成结果会写入用户选择的输出目录，并可加载进左侧手动选型列表。

use std::path::PathBuf;

use eframe::egui;

use crate::app::ProbeUiApp;
use crate::i18n::Msg;
use crate::t;
use crate::worker::WorkerCommand;

impl ProbeUiApp {
    /// Target 生成器面板：输入选择、输出目录、生成按钮与结果表格。
    pub(crate) fn target_gen_view_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.heading(self.t(Msg::TargetGenerator));
        ui.separator();

        // ---- 输入 ----
        ui.horizontal(|ui| {
            ui.label(self.t(Msg::TgInput));
            let hint = self.t(Msg::TgInputHint);
            ui.add(
                egui::TextEdit::singleline(&mut self.tg_input)
                    .desired_width(340.0)
                    .hint_text(hint),
            );
            if ui.button(self.icon("📂", Msg::TgBrowseFile)).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("CMSIS Pack", &["pack", "pdsc", "zip"])
                    .pick_file()
                {
                    self.tg_input = path.display().to_string();
                }
            }
            if ui.button(self.icon("📁", Msg::TgBrowseDir)).clicked() {
                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    self.tg_input = dir.display().to_string();
                }
            }
        });
        ui.label(egui::RichText::new(self.t(Msg::TgInputHint)).small().weak());

        // ---- 输出目录 ----
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(self.t(Msg::TgOutputDir));
            let hint = self.t(Msg::TgOutputDirHint);
            ui.add(
                egui::TextEdit::singleline(&mut self.tg_output_dir)
                    .desired_width(340.0)
                    .hint_text(hint),
            );
            if ui.button(self.icon("📁", Msg::TgBrowseDir)).clicked() {
                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    self.tg_output_dir = dir.display().to_string();
                }
            }
        });
        ui.label(
            egui::RichText::new(self.t(Msg::TgOutputDirHint))
                .small()
                .weak(),
        );

        // ---- 选项与生成按钮 ----
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let l_only = self.t(Msg::TgOnlySupported);
            ui.checkbox(&mut self.tg_only_supported, l_only);
            let can_generate = !self.tg_busy && !self.tg_input.trim().is_empty();
            if ui
                .add_enabled(
                    can_generate,
                    egui::Button::new(self.icon("⚙️", Msg::TgGenerate))
                        .fill(egui::Color32::from_rgb(0x1f, 0x6f, 0xc3))
                        .min_size(egui::vec2(150.0, 28.0)),
                )
                .clicked()
            {
                self.start_target_gen();
            }
        });
        ui.label(
            egui::RichText::new(self.t(Msg::TgOnlySupportedHint))
                .small()
                .weak(),
        );
        if self.tg_busy {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new());
                ui.label(self.t(Msg::TgGenerating));
            });
        }

        // ---- 结果 ----
        ui.add_space(8.0);
        ui.separator();
        ui.heading(self.t(Msg::TgResult));
        match &self.tg_result {
            Some(result) if !result.families.is_empty() => {
                egui::ScrollArea::vertical()
                    .id_salt("tg_result_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Grid::new("tg_result_grid")
                            .striped(true)
                            .min_col_width(60.0)
                            .show(ui, |ui| {
                                let h = egui::RichText::new;
                                ui.label(h(self.t(Msg::TgFamily)).strong());
                                ui.label(h(self.t(Msg::TgVariants)).strong());
                                ui.label(h(self.t(Msg::TgFlashAlgos)).strong());
                                ui.label(h(self.t(Msg::TgOutputFile)).strong());
                                ui.end_row();
                                for family in &result.families {
                                    ui.label(&family.name);
                                    ui.label(family.variant_count.to_string());
                                    ui.label(family.flash_algo_count.to_string());
                                    ui.label(
                                        egui::RichText::new(&family.output_file)
                                            .monospace()
                                            .small(),
                                    );
                                    ui.end_row();
                                }
                            });
                    });
            }
            _ => {
                ui.label(egui::RichText::new(self.t(Msg::TgNoResult)).weak());
            }
        }
    }

    /// 校验输入/输出并发送生成命令。
    fn start_target_gen(&mut self) {
        let input = PathBuf::from(self.tg_input.trim());
        if !input.exists() {
            self.log_err(t!(self.lang, Msg::TgInputMissing, self.tg_input));
            return;
        }
        let output_dir = PathBuf::from(self.tg_output_dir.trim());
        if output_dir.as_os_str().is_empty() {
            self.log_err(self.t(Msg::TgOutputDirMissing));
            return;
        }
        self.tg_busy = true;
        self.tg_result = None;
        self.log_info(t!(self.lang, Msg::GeneratingFromPack, self.tg_input));
        self.send(WorkerCommand::TargetGenGenerate {
            input,
            output_dir,
            only_supported: self.tg_only_supported,
        });
    }
}
