//! 外部芯片包视图：选择通过 YAML / CMSIS Pack 导入的芯片族并连接。

use eframe::egui;

use crate::app::ProbeUiApp;
use crate::i18n::Msg;

impl ProbeUiApp {
    /// 外部芯片包选择区：显示通过 YAML / CMSIS Pack 导入的芯片族，
    /// 家族下拉 + 型号列表，选中型号写入 `manual_target`。
    pub(crate) fn external_pack_ui(&mut self, ui: &mut egui::Ui) {
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
        // 型号列表：显示选中家族的芯片型号（高度压缩，保持连接按钮可见）。
        if let Some(fam) = self
            .selected_external_family
            .and_then(|i| self.external_families.get(i))
        {
            let mut picked_chip: Option<String> = None;
            egui::ScrollArea::vertical()
                .id_salt("ext_chip_list")
                .max_height(130.0)
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

        // 按型号连接：与内置芯片包视图共用同一个连接按钮。
        ui.add_space(6.0);
        self.connect_manual_btn(ui);
    }
}
