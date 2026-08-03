//! 中央固件烧录面板：文件选择、烧录选项、操作按钮、进度与日志。

use eframe::egui;

use crate::app::ProbeUiApp;
use crate::i18n::Msg;
use crate::t;
use crate::worker::{OpState, WorkerCommand};

impl ProbeUiApp {
    /// 中央固件烧录面板：文件选择、烧录选项、操作按钮、进度与日志。
    pub(crate) fn flash_view_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.heading(self.t(Msg::FirmwareFlashing));
        ui.separator();

        ui.horizontal(|ui| {
            ui.label(self.t(Msg::FirmwareFile));
            let hint = self.t(Msg::FirmwareHint);
            ui.add(
                egui::TextEdit::singleline(&mut self.file_path)
                    .desired_width(320.0)
                    .hint_text(hint),
            );
            if ui.button(self.icon("📂", Msg::Browse)).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(self.t(Msg::FirmwareImage), &["elf", "hex", "bin", "uf2"])
                    .pick_file()
                {
                    self.file_path = path.display().to_string();
                    self.log_info(t!(self.lang, Msg::SelectedFirmware, self.file_path));
                }
            }
            if ui
                .button(self.icon("📁", Msg::SelectProjectFolder))
                .clicked()
            {
                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    self.firmware_root = dir.display().to_string();
                    self.firmware_scanning = true;
                    self.firmware_candidates.clear();
                    self.log_info(t!(self.lang, Msg::ScanningProject, self.firmware_root));
                    self.send(WorkerCommand::ScanFirmware { root: dir });
                }
            }
        });
        if let Some(fmt) = self.detected_format() {
            ui.label(t!(self.lang, Msg::FileFormat, fmt));
            if fmt == "Binary" {
                ui.horizontal(|ui| {
                    ui.label(self.t(Msg::BaseAddress));
                    ui.add(
                        egui::DragValue::new(&mut self.bin_base)
                            .hexadecimal(8, false, true)
                            .prefix("0x"),
                    );
                });
            }
        }

        if self.firmware_scanning {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new());
                ui.label(t!(self.lang, Msg::ScanningRoot, self.firmware_root));
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
                        self.t(Msg::NoProjectFirmware).to_owned()
                    } else {
                        name
                    }
                }
            };
            let mut chosen: Option<usize> = None;
            ui.horizontal(|ui| {
                ui.label(self.t(Msg::ProjectFirmware));
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
                    self.log_info(t!(self.lang, Msg::SelectedFirmware, self.file_path));
                }
            }
        }

        ui.add_space(8.0);
        let l_erase = self.t(Msg::ChipEraseBeforeFlash);
        let l_verify = self.t(Msg::VerifyAfterFlash);
        let l_keep = self.t(Msg::KeepUnwritten);
        let l_reset = self.t(Msg::ResetAndRun);
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
                    egui::Button::new(self.icon("⚡", Msg::FlashBtn))
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
                    egui::Button::new(self.icon("🗑", Msg::EraseAllBtn)),
                )
                .clicked()
            {
                self.busy = true;
                self.op_bars.clear();
                self.log_info(self.t(Msg::ErasingAll));
                self.send(WorkerCommand::EraseAll);
            }
            if ui
                .add_enabled(
                    self.connected.is_some() && !self.busy,
                    egui::Button::new(self.icon("🔁", Msg::ResetTargetBtn)),
                )
                .clicked()
            {
                self.busy = true;
                self.log_info(self.t(Msg::ResettingTarget));
                self.send(WorkerCommand::Reset);
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.heading(self.t(Msg::ReadFirmwareTitle));
        ui.horizontal(|ui| {
            ui.label(self.t(Msg::Range));
            ui.add(
                egui::DragValue::new(&mut self.read_start)
                    .hexadecimal(8, false, true)
                    .prefix("0x"),
            );
            ui.label("-");
            ui.add(
                egui::DragValue::new(&mut self.read_end)
                    .hexadecimal(8, false, true)
                    .prefix("0x"),
            );
            ui.label(self.t(Msg::Bytes));
        });
        if self.read_end > self.read_start {
            let size = self.read_end - self.read_start;
            ui.label(
                egui::RichText::new(t!(self.lang, Msg::SizeKb, size / 1024))
                    .small()
                    .weak(),
            );
        }
        let can_read = self.connected.is_some()
            && self.read_end > self.read_start
            && !self.busy
            && !self.connecting
            && !self.probing;
        if ui
            .add_enabled(
                can_read,
                egui::Button::new(self.icon("💾", Msg::ReadFirmwareBtn))
                    .fill(egui::Color32::from_rgb(0x8a, 0x6d, 0x3b)),
            )
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("BIN", &["bin"])
                .set_file_name("firmware.bin")
                .save_file()
            {
                let start = self.read_start;
                let end = self.read_end;
                self.busy = true;
                self.op_bars.clear();
                self.log_info(t!(self.lang, Msg::StartingRead, start, end));
                self.send(WorkerCommand::ReadFlash { path, start, end });
            }
        }

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
                        let text = format!("{}  ({}/{} KB)", bar.label, bar.done / 1024, t / 1024);
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
    }
}
