//! 内存查看器视图：按地址范围读取/写入内存，十六进制转储显示。

use eframe::egui;

use crate::app::ProbeUiApp;

impl ProbeUiApp {
    /// 内存查看器视图（在中央面板中由『内存查看器』标签切换显示）。
    pub(crate) fn mem_view_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.heading(self.t("内存查看器", "Memory Viewer"));
            if self.mem_busy {
                ui.add(egui::Spinner::new());
            }
        });
        ui.separator();

        let regions: Vec<(String, u64, u64)> = self
            .connected
            .as_ref()
            .map(|s| {
                s.memory
                    .iter()
                    .map(|m| {
                        (
                            format!("[{}] 0x{:08X} - 0x{:08X}", m.kind, m.start, m.end),
                            m.start,
                            m.end,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();

        let sel_text = regions
            .iter()
            .find(|(_, s, e)| self.mem_start >= *s && self.mem_start < *e)
            .map(|(l, _, _)| l.clone())
            .unwrap_or_else(|| self.t("自定义地址", "Custom address").to_owned());
        let mut picked: Option<(u64, u64)> = None;
        ui.horizontal(|ui| {
            ui.label(self.t("区域:", "Region:"));
            egui::ComboBox::from_id_salt("mem_region_sel")
                .width(320.0)
                .selected_text(sel_text)
                .show_ui(ui, |ui| {
                    for (label, start, end) in &regions {
                        if ui.selectable_label(false, label).clicked() {
                            picked = Some((*start, *end));
                        }
                    }
                });
        });
        if let Some((start, end)) = picked {
            let size = end - start;
            if size > 0 {
                self.mem_start = start;
                self.mem_len = size.min(1024) as usize;
            }
        }

        ui.horizontal(|ui| {
            ui.label(self.t("地址:", "Address:"));
            ui.add(
                egui::DragValue::new(&mut self.mem_start)
                    .hexadecimal(8, false, true)
                    .prefix("0x"),
            );
            ui.label(self.t("字节数:", "Bytes:"));
            ui.add(egui::DragValue::new(&mut self.mem_len).range(1..=262_144));
            let can_read = self.connected.is_some() && !self.mem_busy && !self.busy;
            if ui
                .add_enabled(
                    can_read,
                    egui::Button::new(self.icon("📖", "读取", "Read"))
                        .fill(egui::Color32::from_rgb(0x1f, 0x6f, 0xc3)),
                )
                .clicked()
            {
                self.read_memory();
            }
        });

        ui.add_space(6.0);
        let dump_h = (ui.available_height() * 0.45).max(120.0);
        egui::ScrollArea::both()
            .id_salt("mem_dump_scroll")
            .auto_shrink([false, false])
            .min_scrolled_width(440.0)
            .max_height(dump_h)
            .show(ui, |ui| {
                if self.mem_data.is_empty() {
                    ui.label(
                        egui::RichText::new(self.t(
                            "尚未读取，点击上方『读取』",
                            "Not read yet; click 'Read' above",
                        ))
                        .weak(),
                    );
                } else {
                    let base = self.mem_read_addr;
                    for (li, row) in self.mem_data.chunks(16).enumerate() {
                        let addr = base + (li as u64) * 16;
                        let mut hex = String::with_capacity(row.len() * 3);
                        let mut asc = String::with_capacity(row.len());
                        for b in row {
                            hex.push_str(&format!("{b:02X} "));
                            asc.push(if *b >= 0x20 && *b <= 0x7e {
                                *b as char
                            } else {
                                '.'
                            });
                        }
                        ui.monospace(format!("{:08X}  {:<48}  {}", addr, hex, asc));
                    }
                }
            });

        ui.add_space(8.0);
        ui.separator();
        ui.label(egui::RichText::new(self.t("写入内存", "Write Memory")).strong());
        ui.horizontal(|ui| {
            ui.label(self.t("地址:", "Address:"));
            ui.add(
                egui::DragValue::new(&mut self.mem_write_start)
                    .hexadecimal(8, false, true)
                    .prefix("0x"),
            );
        });
        ui.horizontal(|ui| {
            ui.label(self.t("数据:", "Data:"));
            let hint = self.t(
                "十六进制字节，如 DE AD BE EF",
                "Hex bytes, e.g. DE AD BE EF",
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.mem_write_input)
                    .desired_width(240.0)
                    .hint_text(hint),
            );
            let can_write = self.connected.is_some() && !self.mem_busy && !self.busy;
            if ui
                .add_enabled(
                    can_write,
                    egui::Button::new(self.icon("✍", "写入", "Write"))
                        .fill(egui::Color32::from_rgb(0x8a, 0x6d, 0x3b)),
                )
                .clicked()
            {
                self.write_memory();
            }
        });
    }
}
