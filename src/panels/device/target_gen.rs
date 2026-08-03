//! 高级芯片配置：从 CMSIS Pack 生成 target 定义并注册进选型列表。

use std::path::PathBuf;

use eframe::egui;

use crate::app::ProbeUiApp;
use crate::i18n::Msg;
use crate::t;
use crate::worker::WorkerCommand;

impl ProbeUiApp {
    /// 高级芯片配置：选择 CMSIS Pack（.pack/.pdsc/.zip 或目录），
    /// 自动生成芯片描述并注册，可选一键连接目标。
    pub(crate) fn advanced_chip_config_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new(self.t(Msg::AdvancedChipConfigHint))
                .small()
                .weak(),
        );

        ui.add_space(4.0);
        // horizontal_wrapped：宽度不足时文件按钮自动折行，避免被输入框挤出面板。
        ui.horizontal_wrapped(|ui| {
            ui.label(self.t(Msg::TgInput));
            let hint = self.t(Msg::TgInputHint);
            ui.add(
                egui::TextEdit::singleline(&mut self.tg_input)
                    .desired_width(230.0)
                    .font(egui::TextStyle::Small)
                    .hint_text(hint),
            );
            if ui.button(self.icon("📂", Msg::TgBrowseFile)).clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("CMSIS Pack", &["pack", "pdsc", "zip"])
                    .pick_file()
                {
                    self.tg_input = path.display().to_string();
                }
        });

        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(self.t(Msg::TgOutputDir));
            let hint = self.t(Msg::TgOutputDirHint);
            ui.add(
                egui::TextEdit::singleline(&mut self.tg_output_dir)
                    .desired_width(230.0)
                    .font(egui::TextStyle::Small)
                    .hint_text(hint),
            );
            if ui.button(self.icon("📁", Msg::TgBrowseDir)).clicked()
                && let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    self.tg_output_dir = dir.display().to_string();
                }
        });

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let l_only = self.t(Msg::TgOnlySupported);
            ui.checkbox(&mut self.tg_only_supported, l_only);
            let can_generate = !self.tg_busy && !self.tg_input.trim().is_empty();
            if ui
                .add_enabled(
                    can_generate,
                    egui::Button::new(self.icon("🔧", Msg::TgGenerate))
                        .fill(egui::Color32::from_rgb(0x1f, 0x6f, 0xc3)),
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

        // 生成结果摘要（仅显示芯片族名与型号数，保持面板紧凑）。
        if let Some(result) = &self.tg_result
            && !result.families.is_empty() {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(self.t(Msg::TgResult)).strong().small());
                for family in &result.families {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} ({} {})",
                            family.name,
                            family.variant_count,
                            self.t(Msg::TgVariants)
                        ))
                        .small(),
                    );
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
        self.tg_busy = true;
        self.tg_result = None;
        self.log_info(t!(self.lang, Msg::GeneratingFromPack, self.tg_input));
        self.send(WorkerCommand::TargetGenGenerate {
            input,
            output_dir,
            only_supported: self.tg_only_supported,
            auto_load: true,
        });
    }
}
