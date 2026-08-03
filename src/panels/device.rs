//! 左侧设备检测面板：探针选择、连接方式、自动识别、手动指定目标、高级芯片配置与目标信息。

use std::path::PathBuf;

use eframe::egui;

use crate::app::{ProbeUiApp, TARGET_INFO_MIN_H};
use crate::i18n::Msg;
use crate::t;
use crate::worker::{BootMode, WorkerCommand};

impl ProbeUiApp {
    /// 左侧设备检测面板（探针选择、自动识别、手动指定目标、目标信息）。
    pub(crate) fn device_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("detect_panel")
            .resizable(false)
            .default_width(440.0)
            .show(ctx, |ui| {
                egui::TopBottomPanel::bottom("target_info_pinned")
                    .resizable(false)
                    .min_height(TARGET_INFO_MIN_H)
                    .show_inside(ui, |ui| {
                        let rect = ui.scope(|ui| self.target_info_ui(ui)).response.rect;
                        self.target_info_h = rect.height();
                    });
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    ui.add_space(6.0);
                    ui.heading(self.t(Msg::DeviceDetection));
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .id_salt("device_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.device_panel_ui(ui));
                });
            });
    }

    fn device_panel_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(self.t(Msg::ProbeLabel));
            egui::ComboBox::from_id_salt("probe_sel")
                .width(210.0)
                .selected_text(
                    self.probes
                        .get(self.selected_probe)
                        .map(|p| p.identifier.as_str())
                        .unwrap_or(self.t(Msg::NotSelected)),
                )
                .show_ui(ui, |ui| {
                    for p in &self.probes {
                        let label = format!(
                            "{}  [{}] (SN: {})",
                            p.identifier,
                            p.probe_type,
                            p.serial_number.as_deref().unwrap_or("N/A")
                        );
                        ui.selectable_value(&mut self.selected_probe, p.index, label);
                    }
                });
            let enabled = !self.probing && !self.connecting && !self.busy;
            if ui
                .add_enabled(enabled, egui::Button::new(self.icon("🔄", Msg::Rescan)))
                .clicked()
            {
                self.probing = true;
                self.log_info(self.t(Msg::ScanningProbes));
                self.send(WorkerCommand::Scan);
            }
        });
        if self.probing {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new());
                ui.label(self.t(Msg::Scanning));
            });
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(self.t(Msg::ConnectionMode));
            egui::ComboBox::from_id_salt("boot_mode_sel")
                .width(200.0)
                .selected_text(match self.boot_mode {
                    BootMode::Normal => self.t(Msg::BootNormal),
                    BootMode::UnderReset => self.t(Msg::BootUnderReset),
                })
                .show_ui(ui, |ui| {
                    let l_normal = self.t(Msg::BootNormal);
                    let l_under_reset = self.t(Msg::BootUnderReset);
                    ui.selectable_value(&mut self.boot_mode, BootMode::Normal, l_normal);
                    ui.selectable_value(&mut self.boot_mode, BootMode::UnderReset, l_under_reset);
                });
        });
        ui.label(
            egui::RichText::new(self.t(Msg::BootModeHint))
                .small()
                .weak(),
        );

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let has_probe = !self.probes.is_empty()
                && !self.connecting
                && !self.probing
                && !self.busy
                && !self.rtt_on;
            if ui
                .add_enabled(
                    has_probe,
                    egui::Button::new(self.icon("🔍", Msg::AutoDetectTarget))
                        .fill(egui::Color32::from_rgb(0x1f, 0x6f, 0xc3)),
                )
                .clicked()
            {
                self.connecting = true;
                self.busy = true;
                self.op_bars.clear();
                self.log_info(self.t(Msg::AutoDetecting));
                self.send(WorkerCommand::ConnectAuto {
                    probe: self.selected_probe,
                    boot_mode: self.boot_mode,
                });
            }
            let connected = self.connected.is_some() && !self.busy;
            if ui
                .add_enabled(
                    connected,
                    egui::Button::new(self.icon("🔌", Msg::Disconnect)),
                )
                .clicked()
            {
                self.connected = None;
                self.log_info(self.t(Msg::Disconnected));
                self.send(WorkerCommand::Disconnect);
            }
        });
        if self.connecting {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new());
                ui.label(self.t(Msg::Connecting));
            });
        }

        ui.add_space(4.0);
        // 高级芯片配置：加载本地 CMSIS Pack，自动生成芯片描述并连接目标。
        egui::CollapsingHeader::new(self.t(Msg::AdvancedChipConfig))
            .id_salt("advanced_chip_config")
            .default_open(false)
            .show(ui, |ui| self.advanced_chip_config_ui(ui));
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.heading(self.t(Msg::ManualTargetSel));
            if self.show_manual {
                ui.colored_label(
                    egui::Color32::from_rgb(0xc0, 0x3a, 0x2b),
                    self.t(Msg::AutoDetectFailedHint),
                );
            }
        });
        ui.label(
            egui::RichText::new(self.t(Msg::ManualTargetHint))
                .small()
                .weak(),
        );
        ui.horizontal(|ui| {
            ui.label(self.t(Msg::SearchModel));
            let hint = self.t(Msg::SearchHint);
            ui.add(
                egui::TextEdit::singleline(&mut self.chip_search)
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Small)
                    .hint_text(hint),
            );
            if ui.button(self.icon("📄", Msg::LoadChipFile)).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("YAML", &["yaml", "yml"])
                    .pick_file()
                {
                    self.log_info(t!(self.lang, Msg::LoadingChipFile, path.display()));
                    self.send(WorkerCommand::LoadChipFile { path });
                }
            }
            if ui.button(self.icon("📦", Msg::GenerateFromPack)).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("CMSIS Pack", &["pack", "pdsc", "zip"])
                    .pick_file()
                {
                    self.log_info(t!(self.lang, Msg::GeneratingFromPack, path.display()));
                    self.send(WorkerCommand::GeneratePack { path });
                }
            }
        });

        if !self.manual_target.is_empty() {
            ui.label(t!(self.lang, Msg::SelectedChip, self.manual_target));
        }

        let filter = self.chip_search.trim().to_lowercase();
        let mut brand_fams: Vec<(usize, Vec<usize>)> = Vec::new();
        for (bi, brand) in self.chip_brands.iter().enumerate() {
            let keep: Vec<usize> = brand
                .families
                .iter()
                .copied()
                .filter(|&i| {
                    let f = &self.chip_families[i];
                    filter.is_empty()
                        || f.name.to_lowercase().contains(&filter)
                        || f.chips.iter().any(|c| c.to_lowercase().contains(&filter))
                })
                .collect();
            if !keep.is_empty() {
                brand_fams.push((bi, keep));
            }
        }

        let brand_pos = self
            .selected_brand
            .and_then(|b| brand_fams.iter().position(|(bi, _)| *bi == b))
            .unwrap_or(0);
        let sel_brand = brand_fams.get(brand_pos).map(|(bi, _)| *bi);
        let fam_matches: Vec<usize> = brand_fams
            .get(brand_pos)
            .map(|(_, fams)| fams.clone())
            .unwrap_or_default();

        let mut sel_family = self
            .selected_family
            .filter(|i| fam_matches.contains(i))
            .or_else(|| fam_matches.first().copied());
        if !filter.is_empty() {
            let good = sel_family
                .and_then(|i| self.chip_families.get(i))
                .map(|f| f.chips.iter().any(|c| c.to_lowercase().contains(&filter)))
                .unwrap_or(false);
            if !good {
                sel_family = fam_matches
                    .iter()
                    .copied()
                    .find(|&i| {
                        self.chip_families[i]
                            .chips
                            .iter()
                            .any(|c| c.to_lowercase().contains(&filter))
                    })
                    .or_else(|| fam_matches.first().copied());
            }
        }

        self.selected_brand = sel_brand;
        self.selected_family = sel_family;

        // 三级联动：三列等宽，总宽度（含 2 个 4px 间距）不超过设备检测面板可用宽度。
        let col_w = ((ui.available_width() - 8.0) / 3.0).floor();
        let brand_w = col_w;
        let family_w = col_w;
        let variant_w = col_w;

        let mut picked_brand: Option<(usize, Option<usize>)> = None;
        let mut picked_family: Option<usize> = None;
        let mut picked_chip: Option<String> = None;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;

            ui.allocate_ui_with_layout(
                egui::vec2(brand_w, 240.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.label(egui::RichText::new(self.t(Msg::Brand)).strong().small());
                    egui::ScrollArea::vertical()
                        .id_salt("brand_list")
                        .max_height(215.0)
                        .show(ui, |ui| {
                            if brand_fams.is_empty() {
                                ui.label(egui::RichText::new(self.t(Msg::NoMatchingBrand)).weak());
                            } else {
                                for (bi, fams) in brand_fams.iter() {
                                    let brand = &self.chip_brands[*bi];
                                    let selected = Some(*bi) == sel_brand;
                                    let label = format!(
                                        "{} ({})",
                                        self.brand_label(&brand.name),
                                        fams.len()
                                    );
                                    if ui.selectable_label(selected, label).clicked() {
                                        picked_brand = Some((*bi, fams.first().copied()));
                                    }
                                }
                            }
                        });
                },
            );

            ui.allocate_ui_with_layout(
                egui::vec2(family_w, 240.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.label(egui::RichText::new(self.t(Msg::Family)).strong().small());
                    egui::ScrollArea::vertical()
                        .id_salt("fam_list")
                        .max_height(215.0)
                        .show(ui, |ui| {
                            if fam_matches.is_empty() {
                                ui.label(egui::RichText::new(self.t(Msg::NoMatchingFamily)).weak());
                            } else {
                                for &i in &fam_matches {
                                    let fam = &self.chip_families[i];
                                    let selected = Some(i) == sel_family;
                                    if ui
                                        .selectable_label(
                                            selected,
                                            format!("{} ({})", fam.name, fam.chips.len()),
                                        )
                                        .clicked()
                                    {
                                        picked_family = Some(i);
                                    }
                                }
                            }
                        });
                },
            );

            ui.allocate_ui_with_layout(
                egui::vec2(variant_w, 240.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.label(egui::RichText::new(self.t(Msg::Variant)).strong().small());
                    let fam_index = sel_family;
                    egui::ScrollArea::vertical()
                        .id_salt("chip_list")
                        .max_height(215.0)
                        .show(ui, |ui| {
                            match fam_index.and_then(|i| self.chip_families.get(i)) {
                                Some(fam) => {
                                    let mut shown = 0;
                                    for name in &fam.chips {
                                        if !filter.is_empty()
                                            && !name.to_lowercase().contains(&filter)
                                        {
                                            continue;
                                        }
                                        let selected = self.manual_target == *name;
                                        if ui.selectable_label(selected, name).clicked() {
                                            picked_chip = Some(name.clone());
                                        }
                                        shown += 1;
                                    }
                                    if shown == 0 {
                                        ui.label(
                                            egui::RichText::new(self.t(Msg::NoMatchingVariant))
                                                .weak(),
                                        );
                                    }
                                }
                                None => {
                                    ui.label(
                                        egui::RichText::new(self.t(Msg::SelectFamilyFirst)).weak(),
                                    );
                                }
                            }
                        });
                },
            );
        });

        if let Some((bi, first_fam)) = picked_brand {
            self.selected_brand = Some(bi);
            self.selected_family = first_fam;
            if self.manual_target.is_empty() {
                if let Some(fam) = first_fam.and_then(|i| self.chip_families.get(i)) {
                    if let Some(first) = fam.chips.first() {
                        self.manual_target = first.clone();
                    }
                }
            }
        }
        if let Some(i) = picked_family {
            self.selected_family = Some(i);
            if self.manual_target.is_empty() {
                if let Some(fam) = self.chip_families.get(i) {
                    if let Some(first) = fam.chips.first() {
                        self.manual_target = first.clone();
                    }
                }
            }
        }
        if let Some(name) = picked_chip {
            self.manual_target = name;
        }

        let enabled = !self.probes.is_empty()
            && !self.connecting
            && !self.busy
            && !self.rtt_on
            && !self.manual_target.trim().is_empty();
        if ui
            .add_enabled(
                enabled,
                egui::Button::new(self.icon("🔗", Msg::ConnectByModel))
                    .fill(egui::Color32::from_rgb(0x1f, 0x6f, 0xc3)),
            )
            .clicked()
        {
            let target = self.manual_target.trim().to_owned();
            self.connecting = true;
            self.busy = true;
            self.log_info(t!(self.lang, Msg::ConnectingTo, target));
            self.send(WorkerCommand::ConnectManual {
                probe: self.selected_probe,
                target,
                boot_mode: self.boot_mode,
            });
        }
    }

    /// 高级芯片配置：选择 CMSIS Pack（.pack/.pdsc/.zip 或目录），
    /// 自动生成芯片描述并注册，可选一键连接目标。
    fn advanced_chip_config_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new(self.t(Msg::AdvancedChipConfigHint))
                .small()
                .weak(),
        );

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(self.t(Msg::TgInput));
            let hint = self.t(Msg::TgInputHint);
            ui.add(
                egui::TextEdit::singleline(&mut self.tg_input)
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Small)
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

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(self.t(Msg::TgOutputDir));
            let hint = self.t(Msg::TgOutputDirHint);
            ui.add(
                egui::TextEdit::singleline(&mut self.tg_output_dir)
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Small)
                    .hint_text(hint),
            );
            if ui.button(self.icon("📁", Msg::TgBrowseDir)).clicked() {
                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    self.tg_output_dir = dir.display().to_string();
                }
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
                self.tg_auto_connect = false;
                self.start_target_gen();
            }
            let can_auto = can_generate && !self.probes.is_empty() && !self.connecting && !self.busy;
            if ui
                .add_enabled(
                    can_auto,
                    egui::Button::new(self.icon("⚡", Msg::TgGenerateConnect)),
                )
                .clicked()
            {
                self.tg_auto_connect = true;
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
        if let Some(result) = &self.tg_result {
            if !result.families.is_empty() {
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
    }

    /// 校验输入/输出并发送生成命令。
    fn start_target_gen(&mut self) {
        let input = PathBuf::from(self.tg_input.trim());
        if !input.exists() {
            self.log_err(t!(self.lang, Msg::TgInputMissing, self.tg_input));
            self.tg_auto_connect = false;
            return;
        }
        let output_dir = PathBuf::from(self.tg_output_dir.trim());
        let auto_load = self.tg_auto_connect;
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

    /// 左栏底部固定显示的目标信息框（连接成功后展示芯片与内存映射）。
    fn target_info_ui(&mut self, ui: &mut egui::Ui) {
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
