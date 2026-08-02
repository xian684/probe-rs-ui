//! 底部 RTT 日志面板：启用/停用、上行输出显示与下行通道 0 发送。

use eframe::egui;

use crate::app::ProbeUiApp;
use crate::worker::WorkerCommand;

impl ProbeUiApp {
    /// 底部 RTT 日志面板。
    pub(crate) fn rtt_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("rtt_panel")
            .resizable(true)
            .default_height(220.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.heading(self.t("RTT 日志", "RTT Log"));
                    ui.separator();
                    let was_enabled = self.rtt_enabled;
                    let enabled_label = self.t("启用 RTT", "Enable RTT");
                    ui.checkbox(&mut self.rtt_enabled, enabled_label);
                    if was_enabled && !self.rtt_enabled && self.rtt_on {
                        self.rtt_on = false;
                        self.send(WorkerCommand::RttStop);
                        self.log_info(self.t("正在停止 RTT...", "Stopping RTT..."));
                    }
                });
                if !self.rtt_enabled {
                    ui.label(
                        egui::RichText::new(self.t("RTT 功能已关闭", "RTT is disabled")).weak(),
                    );
                    return;
                }
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
                        let can_start = self.connected.is_some()
                            && !self.busy
                            && !self.connecting
                            && !self.probing;
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
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.rtt_on {
                            ui.label(
                                egui::RichText::new(
                                    self.t("RTT 通道 0 输出", "RTT channel 0 output"),
                                )
                                .small()
                                .weak(),
                            );
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
                    ui.label(self.t("发送 (CH0):", "Send (CH0):"));
                    let hint = self.t(
                        "输入内容后回车或点击发送，写入目标下行通道 0",
                        "Type and press Enter or click Send to write to down channel 0",
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
                    let enter = edit.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        && send_enabled;
                    if send_clicked || enter {
                        let mut line = self.rtt_down_input.trim().to_owned();
                        line.push('\n');
                        self.send(WorkerCommand::RttWrite {
                            data: line.into_bytes(),
                        });
                        self.rtt_down_input.clear();
                        edit.request_focus();
                    }
                });
            });
    }
}
