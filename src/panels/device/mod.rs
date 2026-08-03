//! 左侧设备检测面板：探针选择、连接方式、自动识别、目标信息。
//!
//! 拆分子模块：
//! - [`manual`]：手动指定目标（搜索 + 三级联动）
//! - [`external`]：外部芯片包视图
//! - [`target_gen`]：高级芯片配置（CMSIS Pack 生成）
//! - [`info`]：目标信息框

use eframe::egui;

use crate::app::{DeviceTab, ProbeUiApp, TARGET_INFO_MIN_H};
use crate::i18n::Msg;
use crate::worker::{BootMode, WorkerCommand};

mod external;
mod info;
mod manual;
mod target_gen;

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
        // 手动指定目标 / 高级芯片配置：互斥切换的两个子面板。
        ui.horizontal(|ui| {
            let l_manual = self.icon("🏢", Msg::ManualTargetSel);
            let l_adv = self.icon("📦", Msg::AdvancedChipConfig);
            ui.selectable_value(&mut self.device_tab, DeviceTab::Manual, l_manual);
            ui.selectable_value(&mut self.device_tab, DeviceTab::Advanced, l_adv);
        });
        ui.separator();
        match self.device_tab {
            DeviceTab::Manual => self.manual_target_ui(ui),
            DeviceTab::Advanced => self.advanced_chip_config_ui(ui),
        }
    }
}
