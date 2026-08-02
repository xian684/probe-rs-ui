//! 左侧设备检测面板：探针选择、连接方式、自动识别、手动指定目标与目标信息。

use eframe::egui;

use crate::app::{ProbeUiApp, TARGET_INFO_MIN_H};
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
                    ui.heading(self.t("设备检测", "Device Detection"));
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
            ui.label(self.t("调试探针:", "Probe:"));
            egui::ComboBox::from_id_salt("probe_sel")
                .width(210.0)
                .selected_text(
                    self.probes
                        .get(self.selected_probe)
                        .map(|p| p.identifier.as_str())
                        .unwrap_or(self.t("未选择", "Not selected")),
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
                .add_enabled(
                    enabled,
                    egui::Button::new(self.icon("🔄", "重新扫描", "Rescan")),
                )
                .clicked()
            {
                self.probing = true;
                self.log_info(self.t("正在扫描探针...", "Scanning probes..."));
                self.send(WorkerCommand::Scan);
            }
        });
        if self.probing {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new());
                ui.label(self.t("扫描中...", "Scanning..."));
            });
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(self.t("连接方式:", "Connection mode:"));
            egui::ComboBox::from_id_salt("boot_mode_sel")
                .width(200.0)
                .selected_text(match self.boot_mode {
                    BootMode::Normal => self.t("正常连接", "Normal"),
                    BootMode::UnderReset => self.t("复位期间连接", "Under Reset"),
                })
                .show_ui(ui, |ui| {
                    let l_normal = self.t("正常连接", "Normal");
                    let l_under_reset = self.t("复位期间连接", "Under Reset");
                    ui.selectable_value(&mut self.boot_mode, BootMode::Normal, l_normal);
                    ui.selectable_value(&mut self.boot_mode, BootMode::UnderReset, l_under_reset);
                });
        });
        ui.label(
            egui::RichText::new(self.t(
                "正常连接：从主 Flash 启动（BOOT0=0）；复位期间连接：保持目标复位直至连接完成（常用于 BOOT0 拉高从系统存储器启动等场景）",
                "Normal: boot from main flash (BOOT0=0); Under Reset: keep the target in reset until connected (e.g. booting from system memory with BOOT0 high)",
            ))
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
                    egui::Button::new(self.icon("🔍", "自动识别目标", "Auto-detect Target"))
                        .fill(egui::Color32::from_rgb(0x1f, 0x6f, 0xc3)),
                )
                .clicked()
            {
                self.connecting = true;
                self.busy = true;
                self.op_bars.clear();
                self.log_info(self.t("正在自动识别目标芯片...", "Auto-detecting target chip..."));
                self.send(WorkerCommand::ConnectAuto {
                    probe: self.selected_probe,
                    boot_mode: self.boot_mode,
                });
            }
            let connected = self.connected.is_some() && !self.busy;
            if ui
                .add_enabled(
                    connected,
                    egui::Button::new(self.icon("🔌", "断开", "Disconnect")),
                )
                .clicked()
            {
                self.connected = None;
                self.log_info(self.t("已断开连接", "Disconnected"));
                self.send(WorkerCommand::Disconnect);
            }
        });
        if self.connecting {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new());
                ui.label(self.t("正在连接目标...", "Connecting to target..."));
            });
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.heading(self.t("手动指定目标芯片", "Manual Target Selection"));
            if self.show_manual {
                ui.colored_label(
                    egui::Color32::from_rgb(0xc0, 0x3a, 0x2b),
                    self.t(
                        "（自动识别失败，请手动选择）",
                        "(auto-detection failed, select manually)",
                    ),
                );
            }
        });
        ui.label(
            egui::RichText::new(self.t(
                "DAPLink / CMSIS-DAP 等探针需手动选择芯片型号",
                "DAPLink / CMSIS-DAP probes need manual chip selection",
            ))
            .small()
            .weak(),
        );
        ui.horizontal(|ui| {
            ui.label(self.t("搜索型号:", "Search:"));
            let hint = self.t("如 stm32f103 / nrf52840", "e.g. stm32f103 / nrf52840");
            ui.add(
                egui::TextEdit::singleline(&mut self.chip_search)
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Small)
                    .hint_text(hint),
            );
        });

        if !self.manual_target.is_empty() {
            ui.label(self.lang.pick(
                format!("已选型号: {}", self.manual_target),
                format!("Selected: {}", self.manual_target),
            ));
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

        ui.columns(3, |cols| {
            cols[0].label(
                egui::RichText::new(self.t("品牌", "Brand"))
                    .strong()
                    .small(),
            );
            cols[1].label(
                egui::RichText::new(self.t("系列", "Family"))
                    .strong()
                    .small(),
            );
            cols[2].label(
                egui::RichText::new(self.t("具体型号", "Variant"))
                    .strong()
                    .small(),
            );

            let mut picked_brand: Option<(usize, Option<usize>)> = None;
            egui::ScrollArea::vertical()
                .id_salt("brand_list")
                .max_height(220.0)
                .show(&mut cols[0], |ui| {
                    if brand_fams.is_empty() {
                        ui.label(
                            egui::RichText::new(self.t("未找到匹配的品牌", "No matching brand"))
                                .weak(),
                        );
                    } else {
                        for (bi, fams) in brand_fams.iter() {
                            let brand = &self.chip_brands[*bi];
                            let selected = Some(*bi) == sel_brand;
                            let label =
                                format!("{} ({})", self.brand_label(&brand.name), fams.len());
                            if ui.selectable_label(selected, label).clicked() {
                                picked_brand = Some((*bi, fams.first().copied()));
                            }
                        }
                    }
                });

            let mut picked_family: Option<usize> = None;
            egui::ScrollArea::vertical()
                .id_salt("fam_list")
                .max_height(220.0)
                .show(&mut cols[1], |ui| {
                    if fam_matches.is_empty() {
                        ui.label(
                            egui::RichText::new(self.t("无匹配系列", "No matching family")).weak(),
                        );
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

            let fam_index = sel_family;
            let mut picked_chip: Option<String> = None;
            egui::ScrollArea::vertical()
                .id_salt("chip_list")
                .max_height(220.0)
                .show(&mut cols[2], |ui| {
                    match fam_index.and_then(|i| self.chip_families.get(i)) {
                        Some(fam) => {
                            let mut shown = 0;
                            for name in &fam.chips {
                                if !filter.is_empty() && !name.to_lowercase().contains(&filter) {
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
                                    egui::RichText::new(self.t(
                                        "该系列下无匹配型号",
                                        "No matching variant in this family",
                                    ))
                                    .weak(),
                                );
                            }
                        }
                        None => {
                            ui.label(
                                egui::RichText::new(self.t(
                                    "请先在左侧选择芯片系列",
                                    "Select a chip family on the left first",
                                ))
                                .weak(),
                            );
                        }
                    }
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
        });

        let enabled = !self.probes.is_empty()
            && !self.connecting
            && !self.busy
            && !self.rtt_on
            && !self.manual_target.trim().is_empty();
        if ui
            .add_enabled(
                enabled,
                egui::Button::new(self.icon("🔗", "按型号连接", "Connect by Model"))
                    .fill(egui::Color32::from_rgb(0x1f, 0x6f, 0xc3)),
            )
            .clicked()
        {
            let target = self.manual_target.trim().to_owned();
            self.connecting = true;
            self.busy = true;
            self.log_info(self.lang.pick(
                format!("正在连接 {} ...", target),
                format!("Connecting to {} ...", target),
            ));
            self.send(WorkerCommand::ConnectManual {
                probe: self.selected_probe,
                target,
                boot_mode: self.boot_mode,
            });
        }
    }

    /// 左栏底部固定显示的目标信息框（连接成功后展示芯片与内存映射）。
    fn target_info_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.separator();
        match &self.connected {
            Some(summary) => {
                ui.heading(self.t("目标信息", "Target Info"));
                ui.label(self.lang.pick(
                    format!("芯片型号: {}", summary.name),
                    format!("Chip: {}", summary.name),
                ));
                ui.label(self.lang.pick(
                    format!("架构: {}", summary.architecture),
                    format!("Architecture: {}", summary.architecture),
                ));
                ui.label(self.lang.pick(
                    format!("核心数量: {}", summary.cores.len()),
                    format!("Cores: {}", summary.cores.len()),
                ));
                for (i, c) in &summary.cores {
                    ui.label(self.lang.pick(
                        format!("  核心 {}: {}", i, c),
                        format!("  Core {}: {}", i, c),
                    ));
                }
                ui.add_space(4.0);
                ui.label(egui::RichText::new(self.t("内存映射:", "Memory Map:")).strong());
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
                ui.label(
                    egui::RichText::new(self.t("尚未连接目标", "Not connected")).weak(),
                );
            }
        }
        let pad = (TARGET_INFO_MIN_H - ui.min_rect().height()).max(0.0);
        ui.add_space(pad);
    }
}
