//! 手动指定目标：搜索 + 品牌/系列/型号三级联动 + 按型号连接。

use eframe::egui;

use crate::app::ProbeUiApp;
use crate::i18n::Msg;
use crate::t;
use crate::worker::WorkerCommand;

impl ProbeUiApp {
    /// 手动指定目标：搜索 + 品牌/系列/型号三级联动 + 按型号连接。
    pub(crate) fn manual_target_ui(&mut self, ui: &mut egui::Ui) {
        if self.show_manual {
            ui.colored_label(
                egui::Color32::from_rgb(0xc0, 0x3a, 0x2b),
                self.t(Msg::AutoDetectFailedHint),
            );
        }
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
        });
        // 加载芯片描述文件 / 从 CMSIS 包生成：固定在同一行（宽度不足时自动折行的是整行按钮组）。
        ui.horizontal_wrapped(|ui| {
            if ui.button(self.icon("📄", Msg::LoadChipFile)).clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("YAML", &["yaml", "yml"])
                    .pick_file()
                {
                    self.log_info(t!(self.lang, Msg::LoadingChipFile, path.display()));
                    self.send(WorkerCommand::LoadChipFile { path });
                }
            if ui.button(self.icon("📦", Msg::GenerateFromPack)).clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("CMSIS Pack", &["pack", "pdsc", "zip"])
                    .pick_file()
                {
                    self.log_info(t!(self.lang, Msg::GeneratingFromPack, path.display()));
                    self.send(WorkerCommand::GeneratePack { path });
                }
        });

        // 外部芯片包：导入的芯片独立于内置三级菜单，在此选择家族与型号。
        self.external_pack_ui(ui);

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
            if self.manual_target.is_empty()
                && let Some(fam) = first_fam.and_then(|i| self.chip_families.get(i))
                    && let Some(first) = fam.chips.first() {
                        self.manual_target = first.clone();
                    }
        }
        if let Some(i) = picked_family {
            self.selected_family = Some(i);
            if self.manual_target.is_empty()
                && let Some(fam) = self.chip_families.get(i)
                    && let Some(first) = fam.chips.first() {
                        self.manual_target = first.clone();
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

    /// 外部芯片包选择区：显示通过 YAML / CMSIS Pack 导入的芯片族，
    /// 家族下拉 + 型号列表，选中型号写入 `manual_target`。
    fn external_pack_ui(&mut self, ui: &mut egui::Ui) {
        if self.external_families.is_empty() {
            ui.label(
                egui::RichText::new(self.t(Msg::ExternalPackNone))
                    .small()
                    .weak(),
            );
            return;
        }
        ui.add_space(4.0);
        ui.label(egui::RichText::new(self.t(Msg::ExternalPackHint)).small().weak());
        ui.horizontal(|ui| {
            ui.label(self.t(Msg::ExternalPackSel));
            let sel_text = self
                .selected_external_family
                .and_then(|i| self.external_families.get(i))
                .map(|f| format!("{} ({})", f.name, f.chips.len()))
                .unwrap_or_else(|| self.t(Msg::NotSelected).to_owned());
            let mut picked_family: Option<usize> = None;
            egui::ComboBox::from_id_salt("ext_fam_sel")
                .width(220.0)
                .selected_text(sel_text)
                .show_ui(ui, |ui| {
                    for (i, f) in self.external_families.iter().enumerate() {
                        let label = format!("{} ({})", f.name, f.chips.len());
                        if ui.selectable_label(Some(i) == self.selected_external_family, label).clicked() {
                            picked_family = Some(i);
                        }
                    }
                });
            if let Some(i) = picked_family {
                self.selected_external_family = Some(i);
                // 切换家族时默认选中其第一个型号。
                if let Some(fam) = self.external_families.get(i)
                    && let Some(first) = fam.chips.first() {
                        self.manual_target = first.clone();
                    }
            }
        });
        // 型号列表：显示选中家族的芯片型号。
        if let Some(fam) = self
            .selected_external_family
            .and_then(|i| self.external_families.get(i))
        {
            let mut picked_chip: Option<String> = None;
            egui::ScrollArea::vertical()
                .id_salt("ext_chip_list")
                .max_height(150.0)
                .show(ui, |ui| {
                    for name in &fam.chips {
                        let selected = self.manual_target == *name;
                        if ui.selectable_label(selected, name).clicked() {
                            picked_chip = Some(name.clone());
                        }
                    }
                });
            if let Some(name) = picked_chip {
                self.manual_target = name;
            }
        }
    }
}
