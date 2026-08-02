//! RTT 日志视图：启动/停用、上行通道显示与下行通道选择发送。

use eframe::egui;

use crate::app::ProbeUiApp;
use crate::worker::WorkerCommand;

impl ProbeUiApp {
    /// RTT 日志视图（在中央面板中由『RTT 日志』标签切换显示）。
    pub(crate) fn rtt_view_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.heading(self.t("RTT 日志", "RTT Log"));
        ui.separator();

        ui.horizontal(|ui| {
            if self.rtt_on {
                if ui.button(self.icon("⏹", "停止", "Stop")).clicked() {
                    self.rtt_on = false;
                    self.send(WorkerCommand::RttStop);
                    self.log_info(self.t("正在停止 RTT...", "Stopping RTT..."));
                }
                ui.colored_label(
                    egui::Color32::from_rgb(0x2e, 0xa0, 0x43),
                    self.t("● 运行中", "● Running"),
                );
            } else {
                let can_start =
                    self.connected.is_some() && !self.busy && !self.connecting && !self.probing;
                if ui
                    .add_enabled(
                        can_start,
                        egui::Button::new(self.icon("▶", "启动", "Start"))
                            .fill(egui::Color32::from_rgb(0x1f, 0x6f, 0xc3)),
                    )
                    .clicked()
                {
                    self.rtt_on = true;
                    self.send(WorkerCommand::RttStart);
                    self.log_info(self.t(
                        "正在启动 RTT（在目标 RAM 中扫描控制块）...",
                        "Starting RTT (scanning target RAM for the control block)...",
                    ));
                }
            }
            if ui.button(self.icon("🗑", "清空", "Clear")).clicked() {
                self.rtt_buf.clear();
            }
            let l_auto = self.t("自动滚动", "Auto-scroll");
            ui.checkbox(&mut self.rtt_autoscroll, l_auto);
            ui.separator();
            ui.label(self.t("显示通道:", "Show channel:"));
            let sel_view = match self.rtt_view_channel {
                Some(c) => format!("CH{c}"),
                None => self.t("全部", "All").to_owned(),
            };
            egui::ComboBox::from_id_salt("rtt_view_ch")
                .width(80.0)
                .selected_text(sel_view)
                .show_ui(ui, |ui| {
                    let l_all = self.t("全部", "All");
                    if ui
                        .selectable_label(self.rtt_view_channel.is_none(), l_all)
                        .clicked()
                    {
                        self.rtt_view_channel = None;
                    }
                    for c in 0..self.rtt_up_channels {
                        if ui
                            .selectable_label(self.rtt_view_channel == Some(c), format!("CH{c}"))
                            .clicked()
                        {
                            self.rtt_view_channel = Some(c);
                        }
                    }
                });
            ui.separator();
            ui.label(self.t("发送通道:", "Send channel:"));
            egui::ComboBox::from_id_salt("rtt_send_ch")
                .width(70.0)
                .selected_text(format!("CH{}", self.rtt_send_channel))
                .show_ui(ui, |ui| {
                    for c in 0..self.rtt_down_channels.max(1) {
                        ui.selectable_value(&mut self.rtt_send_channel, c, format!("CH{c}"));
                    }
                });
        });
        ui.separator();
        let input_h = 28.0;
        egui::ScrollArea::vertical()
            .id_salt("rtt_scroll")
            .auto_shrink([false, false])
            .stick_to_bottom(self.rtt_autoscroll)
            .max_height((ui.available_height() - input_h).max(60.0))
            .show(ui, |ui| {
                ui.monospace(&self.rtt_buf);
            });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let ch = self.rtt_send_channel;
            ui.label(format!("{} (CH{}):", self.t("发送", "Send"), ch));
            let hint = self.t(
                "输入内容后回车或点击发送，写入目标下行通道",
                "Type and press Enter or click Send to write to a down channel",
            );
            let edit = ui.add(
                egui::TextEdit::singleline(&mut self.rtt_down_input)
                    .desired_width(360.0)
                    .hint_text(hint),
            );
            let send_enabled = self.rtt_on && !self.rtt_down_input.trim().is_empty();
            let send_clicked = ui
                .add_enabled(
                    send_enabled,
                    egui::Button::new(self.icon("📤", "发送", "Send")),
                )
                .clicked();
            let enter =
                edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && send_enabled;
            if send_clicked || enter {
                let mut line = self.rtt_down_input.trim().to_owned();
                line.push('\n');
                self.send(WorkerCommand::RttWrite {
                    channel: self.rtt_send_channel,
                    data: line.into_bytes(),
                });
                self.rtt_down_input.clear();
                edit.request_focus();
            }
        });
    }
}
