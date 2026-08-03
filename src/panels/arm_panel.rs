//! 中央『ARM 在线索引』面板：搜索 Keil.pidx 公共索引，下载并生成芯片描述。

use std::path::PathBuf;

use eframe::egui;

use crate::app::ProbeUiApp;
use crate::i18n::Msg;
use crate::t;
use crate::worker::WorkerCommand;

impl ProbeUiApp {
    /// ARM 在线索引视图：关键字搜索 → 结果列表 → 下载并生成。
    pub(crate) fn arm_view_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.heading(self.t(Msg::ArmSearchTitle));
        ui.separator();
        ui.label(
            egui::RichText::new(self.t(Msg::ArmViewHint))
                .small()
                .weak(),
        );

        // 搜索行：关键字 + 搜索按钮（输出目录复用左侧高级芯片配置的 tg_output_dir）。
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(self.t(Msg::ArmSearchKeyword));
            let hint = self.t(Msg::ArmSearchHint);
            ui.add(
                egui::TextEdit::singleline(&mut self.arm_keyword)
                    .desired_width(260.0)
                    .font(egui::TextStyle::Small)
                    .hint_text(hint),
            );
            let can_search = !self.arm_busy;
            if ui
                .add_enabled(
                    can_search,
                    egui::Button::new(self.icon("🔍", Msg::ArmSearchBtn))
                        .fill(egui::Color32::from_rgb(0x1f, 0x6f, 0xc3)),
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

        // 输出目录提示（可选落盘位置，留空则不落盘仅导入）。
        ui.horizontal(|ui| {
            ui.label(self.t(Msg::TgOutputDir));
            let hint = self.t(Msg::TgOutputDirHint);
            ui.add(
                egui::TextEdit::singleline(&mut self.tg_output_dir)
                    .desired_width(260.0)
                    .font(egui::TextStyle::Small)
                    .hint_text(hint),
            );
            if ui.button(self.icon("📁", Msg::TgBrowseDir)).clicked()
                && let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    self.tg_output_dir = dir.display().to_string();
                }
        });

        if self.arm_busy {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new());
                ui.label(self.t(Msg::ArmSearchingTitle));
            });
        }

        // 结果列表：Pack 名 + 版本，可点选。
        ui.add_space(6.0);
        if !self.arm_packs.is_empty() {
            ui.label(
                egui::RichText::new(t!(
                    self.lang,
                    Msg::ArmSearchResult,
                    self.arm_packs.len()
                ))
                .strong(),
            );
            let mut picked: Option<usize> = None;
            egui::ScrollArea::vertical()
                .id_salt("arm_pack_list")
                .auto_shrink([false, false])
                .max_height(280.0)
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

            // 选中 Pack 后的三个操作：下载 / 下载并添加 / 下载并生成。
            let sel_name: String = self
                .arm_selected
                .and_then(|i| self.arm_packs.get(i))
                .map(|p| p.name.clone())
                .unwrap_or_default();
            let sel_url: String = self
                .arm_selected
                .and_then(|i| self.arm_packs.get(i))
                .map(|p| p.url.clone())
                .unwrap_or_default();
            let can_act = !self.arm_busy && !sel_name.is_empty();
            ui.add_space(4.0);
            // horizontal_wrapped：按钮组超宽时整体折行，三个按钮保持并排。
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        can_act,
                        egui::Button::new(self.icon("⬇", Msg::ArmDownloadBtn)),
                    )
                    .clicked()
                {
                    let output_dir = PathBuf::from(self.tg_output_dir.trim());
                    self.arm_busy = true;
                    self.log_info(t!(self.lang, Msg::ArmDownloading, sel_name));
                    self.send(WorkerCommand::ArmDownload {
                        url: sel_url.clone(),
                        output_dir,
                    });
                }
                if ui
                    .add_enabled(
                        can_act,
                        egui::Button::new(self.icon("➕", Msg::ArmGenerateImportBtn)),
                    )
                    .clicked()
                {
                    // 下载并添加：生成后注册进外部芯片包（不落盘 YAML）。
                    let output_dir = PathBuf::new();
                    self.arm_busy = true;
                    self.log_info(t!(self.lang, Msg::ArmDownloading, sel_name));
                    self.send(WorkerCommand::ArmGenerate {
                        filter: sel_name.clone(),
                        output_dir,
                        only_supported: self.tg_only_supported,
                        auto_load: true,
                    });
                }
                if ui
                    .add_enabled(
                        can_act,
                        egui::Button::new(self.icon("⚙️", Msg::ArmGenerateBtn))
                            .fill(egui::Color32::from_rgb(0x2e, 0xa0, 0x43)),
                    )
                    .clicked()
                {
                    // 下载并生成：生成 YAML 落盘到输出目录（不自动导入）。
                    let output_dir = PathBuf::from(self.tg_output_dir.trim());
                    self.arm_busy = true;
                    self.log_info(t!(self.lang, Msg::ArmDownloading, sel_name));
                    self.send(WorkerCommand::ArmGenerate {
                        filter: sel_name.clone(),
                        output_dir,
                        only_supported: self.tg_only_supported,
                        auto_load: false,
                    });
                }
                if !sel_name.is_empty() {
                    ui.label(
                        egui::RichText::new(t!(self.lang, Msg::ArmSelected, sel_name))
                            .small()
                            .weak(),
                    );
                }
            });
        } else if !self.arm_busy {
            ui.label(egui::RichText::new(self.t(Msg::ArmNoResult)).weak());
        }

        // 生成结果摘要（下载并生成完成后的芯片族列表）。
        if let Some(result) = &self.tg_result
            && !result.families.is_empty() {
                ui.add_space(6.0);
                ui.separator();
                ui.label(egui::RichText::new(self.t(Msg::TgResult)).strong());
                egui::ScrollArea::vertical()
                    .id_salt("arm_gen_result")
                    .max_height(140.0)
                    .show(ui, |ui| {
                        for family in &result.families {
                            ui.label(format!(
                                "{} ({} {})",
                                family.name,
                                family.variant_count,
                                self.t(Msg::TgVariants)
                            ));
                        }
                    });
            }
    }
}
