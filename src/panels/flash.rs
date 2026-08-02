//! 中央固件烧录面板：文件选择、烧录选项、操作按钮、进度与日志。

use eframe::egui;

use crate::app::{LogLevel, ProbeUiApp};
use crate::worker::{OpState, WorkerCommand};

impl ProbeUiApp {
    /// 中央固件烧录面板：文件选择、烧录选项、操作按钮、进度与日志。
    pub(crate) fn flashing_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(6.0);
            ui.heading(self.t("固件烧录", "Firmware Flashing"));
            ui.separator();

            ui.horizontal(|ui| {
                ui.label(self.t("固件文件:", "Firmware file:"));
                let hint = self.t(
                    "选择 .elf / .hex / .bin / .uf2 文件",
                    "Select .elf / .hex / .bin / .uf2 file",
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.file_path)
                        .desired_width(320.0)
                        .hint_text(hint),
                );
                if ui.button(self.icon("📂", "浏览...", "Browse...")).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter(
                            self.t("固件镜像", "Firmware image"),
                            &["elf", "hex", "bin", "uf2"],
                        )
                        .pick_file()
                    {
                        self.file_path = path.display().to_string();
                        self.log_info(self.lang.pick(
                            format!("已选择固件: {}", self.file_path),
                            format!("Selected firmware: {}", self.file_path),
                        ));
                    }
                }
                if ui
                    .button(self.icon("📁", "选择项目文件夹...", "Select Project Folder..."))
                    .clicked()
                {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        self.firmware_root = dir.display().to_string();
                        self.firmware_scanning = true;
                        self.firmware_candidates.clear();
                        self.log_info(self.lang.pick(
                            format!("正在扫描项目文件夹并自动识别固件: {}", self.firmware_root),
                            format!(
                                "Scanning project folder and auto-detecting firmware: {}",
                                self.firmware_root
                            ),
                        ));
                        self.send(WorkerCommand::ScanFirmware { root: dir });
                    }
                }
            });
            if let Some(fmt) = self.detected_format() {
                ui.label(self.lang.pick(
                    format!("文件格式: {}", fmt),
                    format!("File format: {}", fmt),
                ));
            }

            if self.firmware_scanning {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    ui.label(self.lang.pick(
                        format!("扫描中: {}", self.firmware_root),
                        format!("Scanning: {}", self.firmware_root),
                    ));
                });
            }
            if self.firmware_candidates.len() > 1 {
                let current = self
                    .firmware_candidates
                    .iter()
                    .position(|c| c.path.display().to_string() == self.file_path);
                let sel_text = match current {
                    Some(i) => {
                        let c = &self.firmware_candidates[i];
                        let name = c
                            .path
                            .file_name()
                            .map(|f| f.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        format!("[{}] {}", c.kind, name)
                    }
                    None => {
                        let name = std::path::Path::new(&self.file_path)
                            .file_name()
                            .map(|f| f.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        if name.is_empty() {
                            self.t("未选择项目固件", "No project firmware").to_owned()
                        } else {
                            name
                        }
                    }
                };
                let mut chosen: Option<usize> = None;
                ui.horizontal(|ui| {
                    ui.label(self.t("项目固件:", "Project firmware:"));
                    egui::ComboBox::from_id_salt("fw_sel")
                        .width(400.0)
                        .selected_text(sel_text)
                        .show_ui(ui, |ui| {
                            for (i, c) in self.firmware_candidates.iter().enumerate() {
                                let label =
                                    format!("[{}] {} ({} KB)", c.kind, c.path.display(), c.size_kb);
                                if ui.selectable_label(Some(i) == current, label).clicked() {
                                    chosen = Some(i);
                                }
                            }
                        });
                });
                if let Some(i) = chosen {
                    if let Some(c) = self.firmware_candidates.get(i) {
                        self.file_path = c.path.display().to_string();
                        self.log_info(self.lang.pick(
                            format!("已选择固件: {}", self.file_path),
                            format!("Selected firmware: {}", self.file_path),
                        ));
                    }
                }
            }

            ui.add_space(8.0);
            let l_erase = self.t("全片擦除后烧录", "Chip erase before flash");
            let l_verify = self.t("烧录后校验", "Verify after flash");
            let l_keep = self.t("保留未写入字节", "Keep unwritten bytes");
            let l_reset = self.t("烧录后复位运行", "Reset and run after flash");
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.chip_erase, l_erase);
                ui.checkbox(&mut self.verify, l_verify);
                ui.checkbox(&mut self.keep_unwritten, l_keep);
                ui.checkbox(&mut self.reset_after, l_reset);
            });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let can_flash = self.connected.is_some()
                    && !self.file_path.trim().is_empty()
                    && !self.busy
                    && !self.connecting
                    && !self.probing;
                if ui
                    .add_enabled(
                        can_flash,
                        egui::Button::new(self.icon("⚡", "开始烧录", "Flash"))
                            .fill(egui::Color32::from_rgb(0x2e, 0xa0, 0x43))
                            .min_size(egui::vec2(130.0, 32.0)),
                    )
                    .clicked()
                {
                    self.start_flash();
                }
                let can_erase =
                    self.connected.is_some() && !self.busy && !self.connecting && !self.probing;
                if ui
                    .add_enabled(
                        can_erase,
                        egui::Button::new(self.icon("🗑", "全片擦除", "Erase All")),
                    )
                    .clicked()
                {
                    self.busy = true;
                    self.op_bars.clear();
                    self.log_info(self.t("开始全片擦除...", "Erasing all flash..."));
                    self.send(WorkerCommand::EraseAll);
                }
                if ui
                    .add_enabled(
                        self.connected.is_some() && !self.busy,
                        egui::Button::new(self.icon("🔁", "复位目标", "Reset Target")),
                    )
                    .clicked()
                {
                    self.busy = true;
                    self.log_info(self.t("正在复位目标...", "Resetting target..."));
                    self.send(WorkerCommand::Reset);
                }
            });

            ui.add_space(10.0);
            if !self.op_bars.is_empty() {
                for bar in &self.op_bars {
                    let color = match bar.state {
                        OpState::Done => egui::Color32::from_rgb(0x2e, 0xa0, 0x43),
                        OpState::Failed => egui::Color32::from_rgb(0xc0, 0x3a, 0x2b),
                        OpState::Active => egui::Color32::from_rgb(0x1f, 0x6f, 0xc3),
                    };
                    match bar.total {
                        Some(t) if t > 0 => {
                            let frac = (bar.done as f32 / t as f32).clamp(0.0, 1.0);
                            let text =
                                format!("{}  ({}/{} KB)", bar.label, bar.done / 1024, t / 1024);
                            ui.add(egui::ProgressBar::new(frac).fill(color).text(text));
                        }
                        _ => {
                            // 总大小未知（如全片擦除）：进行中显示旋转指示，完成后显示整条结果。
                            if bar.state == OpState::Active {
                                ui.horizontal(|ui| {
                                    ui.add(egui::Spinner::new());
                                    ui.label(format!("{}  ...", bar.label));
                                });
                            } else {
                                let frac = if bar.state == OpState::Done { 1.0 } else { 0.0 };
                                ui.add(egui::ProgressBar::new(frac).fill(color).text(&bar.label));
                            }
                        }
                    }
                }
            }

            ui.add_space(10.0);
            ui.label(egui::RichText::new(self.t("日志", "Log")).strong());
            egui::ScrollArea::vertical()
                .id_salt("log_scroll")
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for e in &self.log {
                        let color = match e.level {
                            LogLevel::Info => egui::Color32::from_gray(180),
                            LogLevel::Ok => egui::Color32::from_rgb(0x2e, 0xa0, 0x43),
                            LogLevel::Warn => egui::Color32::from_rgb(0xcc, 0x88, 0x00),
                            LogLevel::Error => egui::Color32::from_rgb(0xc0, 0x3a, 0x2b),
                        };
                        ui.label(egui::RichText::new(&e.text).color(color));
                    }
                });
        });
    }
}
