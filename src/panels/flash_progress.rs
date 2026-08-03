//! 中央烧录面板的进度条渲染（烧录 / 擦除等后台操作进度）。

use eframe::egui;

use crate::app::ProbeUiApp;
use crate::worker::OpState;

impl ProbeUiApp {
    /// 渲染后台操作进度条列表（烧录 / 擦除等）。
    pub(crate) fn op_progress_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        if self.op_bars.is_empty() {
            return;
        }
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
