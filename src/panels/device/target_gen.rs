//! 高级芯片配置：从 CMSIS Pack 生成 target 定义并注册进选型列表。

use std::path::PathBuf;

use eframe::egui;

use crate::app::ProbeUiApp;
use crate::i18n::Msg;
use crate::t;
use crate::worker::WorkerCommand;

impl ProbeUiApp {
    /// 高级芯片配置：选择 CMSIS Pack（.pack/.pdsc/.zip），
    /// 生成芯片描述（可选自动导入选型列表）。
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
        let l_only = self.t(Msg::TgOnlySupported);
        ui.checkbox(&mut self.tg_only_supported, l_only);
        let can_generate = !self.tg_busy && !self.tg_input.trim().is_empty();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    can_generate,
                    egui::Button::new(self.icon("🔧", Msg::TgGenerate))
                        .fill(egui::Color32::from_rgb(0x1f, 0x6f, 0xc3)),
                )
                .clicked()
            {
                self.start_target_gen(false);
            }
            if ui
                .add_enabled(
                    can_generate,
                    egui::Button::new(self.icon("📥", Msg::TgGenerateImport)),
                )
                .clicked()
            {
                self.start_target_gen(true);
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

        // ARM 在线索引：搜索 Keil.pidx 并下载生成。
        ui.add_space(8.0);
        ui.separator();
        egui::CollapsingHeader::new(self.icon("🌐", Msg::ArmSearchTitle))
            .id_salt("arm_index_panel")
            .default_open(false)
            .show(ui, |ui| self.arm_index_ui(ui));
    }

    /// ARM 在线索引面板：关键字搜索 → 结果列表 → 下载并生成。
    fn arm_index_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(self.t(Msg::ArmSearchKeyword));
            let hint = self.t(Msg::ArmSearchHint);
            ui.add(
                egui::TextEdit::singleline(&mut self.arm_keyword)
                    .desired_width(150.0)
                    .font(egui::TextStyle::Small)
                    .hint_text(hint),
            );
            let can_search = !self.arm_busy;
            if ui
                .add_enabled(
                    can_search,
                    egui::Button::new(self.icon("🔍", Msg::ArmSearchBtn)),
                )
                .clicked()
            {
                self.arm_busy = true;
                self.arm_packs.clear();
                self.log_info(t!(self.lang, Msg::ArmSearching, self.arm_keyword));
                self.send(WorkerCommand::ArmSearch {
                    keyword: self.arm_keyword.clone(),
                });
            }
        });
        ui.label(
            egui::RichText::new(self.t(Msg::ArmSearchHint)).small().weak(),
        );
        if self.arm_busy {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new());
                ui.label(self.t(Msg::ArmSearchingTitle));
            });
        }

        // 结果列表：Pack 名 + 版本，可点选。
        if !self.arm_packs.is_empty() {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(t!(
                    self.lang,
                    Msg::ArmSearchResult,
                    self.arm_packs.len()
                ))
                .strong()
                .small(),
            );
            let mut picked: Option<usize> = None;
            egui::ScrollArea::vertical()
                .id_salt("arm_pack_list")
                .max_height(120.0)
                .show(ui, |ui| {
                    for (i, p) in self.arm_packs.iter().enumerate() {
                        let label = if p.deprecated {
                            format!("{} · {} {} (deprecated)", p.vendor, p.name, p.version)
                        } else {
                            format!("{} · {} {}", p.vendor, p.name, p.version)
                        };
                        if ui
                            .selectable_label(Some(i) == self.arm_selected, label)
                            .clicked()
                        {
                            picked = Some(i);
                        }
                    }
                });
            if let Some(i) = picked {
                self.arm_selected = Some(i);
            }
            let sel_name = self
                .arm_selected
                .and_then(|i| self.arm_packs.get(i))
                .map(|p| p.name.as_str())
                .unwrap_or("");
            let can_gen = !self.arm_busy && !sel_name.is_empty();
            if ui
                .add_enabled(
                    can_gen,
                    egui::Button::new(self.icon("⬇", Msg::ArmGenerateBtn))
                        .fill(egui::Color32::from_rgb(0x2e, 0xa0, 0x43)),
                )
                .clicked()
            {
                let filter = sel_name.to_owned();
                let output_dir = PathBuf::from(self.tg_output_dir.trim());
                let auto_load = true;
                self.arm_busy = true;
                self.log_info(t!(self.lang, Msg::ArmDownloading, filter));
                self.send(WorkerCommand::ArmGenerate {
                    filter,
                    output_dir,
                    only_supported: self.tg_only_supported,
                    auto_load,
                });
            }
        } else if !self.arm_busy {
            ui.label(egui::RichText::new(self.t(Msg::ArmNoResult)).weak());
        }
    }

    /// 校验输入/输出并发送生成命令。
    /// 校验输入/输出并发送生成命令。
    ///
    /// `auto_load` 为 true 时生成后自动导入选型列表（worker 侧 auto_load=true）。
    fn start_target_gen(&mut self, auto_load: bool) {
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
            auto_load,
        });
    }
}
