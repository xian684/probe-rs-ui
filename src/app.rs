use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use eframe::egui;

use crate::worker::{
    self, FirmwareCandidate, OpState, ProbeInfo, TargetSummary, WorkerCommand, WorkerEvent,
};

#[derive(Clone, Copy)]
enum LogLevel {
    Info,
    Ok,
    Warn,
    Error,
}

struct LogEntry {
    text: String,
    level: LogLevel,
}

struct OpBar {
    label: String,
    done: u64,
    total: Option<u64>,
    state: OpState,
}

pub struct ProbeUiApp {
    to_worker: Sender<WorkerCommand>,
    from_worker: Receiver<WorkerEvent>,

    probes: Vec<ProbeInfo>,
    selected_probe: usize,
    probing: bool,
    connecting: bool,

    connected: Option<TargetSummary>,
    manual_target: String,
    chips: Vec<String>,
    chip_search: String,
    show_manual: bool,

    file_path: String,
    chip_erase: bool,
    verify: bool,
    keep_unwritten: bool,
    reset_after: bool,

    firmware_root: String,
    firmware_candidates: Vec<FirmwareCandidate>,
    firmware_scanning: bool,

    busy: bool,
    op_bars: Vec<OpBar>,
    log: Vec<LogEntry>,
}

impl ProbeUiApp {
    pub fn setup(ctx: &egui::Context) {
        let mut fonts = egui::FontDefinitions::default();
        let candidates = [
            r"C:\Windows\Fonts\msyh.ttc",
            r"C:\Windows\Fonts\msyhbd.ttc",
            r"C:\Windows\Fonts\simhei.ttf",
            r"C:\Windows\Fonts\simsun.ttc",
            "/System/Library/Fonts/PingFang.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        ];
        for path in candidates {
            if let Ok(bytes) = std::fs::read(path) {
                fonts.font_data.insert("cjk".to_owned(), egui::FontData::from_owned(bytes).into());
                if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                    fam.push("cjk".to_owned());
                }
                if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                    fam.push("cjk".to_owned());
                }
                break;
            }
        }
        ctx.set_fonts(fonts);
    }

    pub fn new() -> Self {
        let worker = worker::spawn();
        let chips = worker::builtin_chip_names();
        let mut app = ProbeUiApp {
            to_worker: worker.sender,
            from_worker: worker.receiver,
            probes: Vec::new(),
            selected_probe: 0,
            probing: true,
            connecting: false,
            connected: None,
            manual_target: String::new(),
            chips,
            chip_search: String::new(),
            show_manual: false,
            file_path: String::new(),
            chip_erase: false,
            verify: true,
            keep_unwritten: true,
            reset_after: true,
            firmware_root: String::new(),
            firmware_candidates: Vec::new(),
            firmware_scanning: false,
            busy: false,
            op_bars: Vec::new(),
            log: Vec::new(),
        };
        app.log(
            format!("已加载 {} 个内置芯片型号，可手动指定目标", app.chips.len()),
            LogLevel::Info,
        );
        app.log("正在扫描调试探针...", LogLevel::Info);
        app.send(WorkerCommand::Scan);
        app
    }

    fn send(&self, cmd: WorkerCommand) {
        let _ = self.to_worker.send(cmd);
    }

    fn log(&mut self, text: impl Into<String>, level: LogLevel) {
        self.log.push(LogEntry {
            text: text.into(),
            level,
        });
        while self.log.len() > 800 {
            self.log.remove(0);
        }
    }

    fn log_info(&mut self, text: impl Into<String>) {
        self.log(text, LogLevel::Info);
    }

    fn log_ok(&mut self, text: impl Into<String>) {
        self.log(text, LogLevel::Ok);
    }

    fn log_warn(&mut self, text: impl Into<String>) {
        self.log(text, LogLevel::Warn);
    }

    fn log_err(&mut self, text: impl Into<String>) {
        self.log(text, LogLevel::Error);
    }

    fn handle_event(&mut self, ev: WorkerEvent) {
        match ev {
            WorkerEvent::Probes(Ok(list)) => {
                self.probing = false;
                self.probes = list;
                if self.probes.is_empty() {
                    self.log_warn("未检测到任何调试探针，请检查 USB 连接与驱动");
                } else {
                    self.log_ok(format!("检测到 {} 个调试探针", self.probes.len()));
                }
            }
            WorkerEvent::Probes(Err(e)) => {
                self.probing = false;
                self.log_err(e);
            }
            WorkerEvent::Connected(Ok(summary)) => {
                self.connecting = false;
                self.busy = false;
                self.log_ok(format!("已连接目标: {}", summary.name));
                self.connected = Some(summary);
            }
            WorkerEvent::Connected(Err(e)) => {
                self.connecting = false;
                self.busy = false;
                self.show_manual = true;
                self.log_err(e);
            }
            WorkerEvent::Status(s) => self.log_info(s),
            WorkerEvent::Diagnostic(s) => self.log_info(s),
            WorkerEvent::Progress {
                operation,
                done,
                total,
                state,
            } => {
                if let Some(bar) = self.op_bars.iter_mut().find(|b| b.label == operation) {
                    if let Some(t) = total {
                        bar.total = Some(t);
                    }
                    match state {
                        OpState::Active => bar.done += done,
                        OpState::Done => {
                            bar.state = OpState::Done;
                            bar.done = bar.total.unwrap_or(bar.done);
                        }
                        OpState::Failed => bar.state = OpState::Failed,
                    }
                } else {
                    self.op_bars.push(OpBar {
                        label: operation.to_owned(),
                        done,
                        total,
                        state,
                    });
                }
            }
            WorkerEvent::OperationDone(Ok(())) => {
                self.busy = false;
                self.log_ok("操作成功完成");
            }
            WorkerEvent::OperationDone(Err(e)) => {
                self.busy = false;
                self.log_err(e);
            }
            WorkerEvent::FirmwareScanned {
                root,
                candidates,
                best,
            } => {
                self.firmware_scanning = false;
                self.firmware_root = root.clone();
                self.firmware_candidates = candidates;
                if self.firmware_candidates.is_empty() {
                    self.log_warn(format!(
                        "在 {} 中未找到固件文件 (.elf / .hex / .bin / .uf2)",
                        root
                    ));
                } else if let Some(i) = best {
                    let path = self.firmware_candidates[i]
                        .path
                        .display()
                        .to_string();
                    self.file_path = path.clone();
                    self.log_ok(format!(
                        "自动识别到固件: {}（共 {} 个候选）",
                        path,
                        self.firmware_candidates.len()
                    ));
                    if self.firmware_candidates.len() > 1 {
                        self.log_info("如需使用其它固件，请在下方下拉列表中选择");
                    }
                }
            }
        }
    }

    fn detected_format(&self) -> Option<&'static str> {
        let ext = std::path::Path::new(&self.file_path)
            .extension()?
            .to_str()?
            .to_lowercase();
        match ext.as_str() {
            "elf" | "axf" => Some("ELF"),
            "hex" => Some("Intel HEX"),
            "bin" => Some("Binary"),
            "uf2" => Some("UF2"),
            _ => None,
        }
    }

    fn start_flash(&mut self) {
        if self.detected_format().is_none() {
            self.log_err("不支持的文件格式，请选择 .elf / .hex / .bin / .uf2 文件");
            return;
        }
        self.busy = true;
        self.op_bars.clear();
        self.log_info(format!("开始烧录: {}", self.file_path));
        self.send(WorkerCommand::Flash {
            path: PathBuf::from(self.file_path.clone()),
            do_chip_erase: self.chip_erase,
            verify: self.verify,
            keep_unwritten_bytes: self.keep_unwritten,
            reset_after: self.reset_after,
        });
    }
}

impl Drop for ProbeUiApp {
    fn drop(&mut self) {
        let _ = self.to_worker.send(WorkerCommand::Shutdown);
    }
}

impl eframe::App for ProbeUiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(ev) = self.from_worker.try_recv() {
            self.handle_event(ev);
        }

        if self.probing || self.connecting || self.busy {
            ctx.request_repaint_after(Duration::from_millis(40));
        }

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("Probe-rs 烧录工具");
                ui.separator();
                if self.connected.is_some() {
                    ui.colored_label(
                        egui::Color32::from_rgb(0x2e, 0xa0, 0x43),
                        "● 已连接",
                    );
                } else {
                    ui.colored_label(
                        egui::Color32::from_rgb(0xcc, 0x88, 0x00),
                        "○ 未连接",
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("基于 probe-rs v0.32").weak());
                });
            });
            ui.add_space(4.0);
        });

        egui::SidePanel::left("detect_panel")
            .resizable(true)
            .default_width(400.0)
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.heading("设备检测");
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("调试探针:");
                    egui::ComboBox::from_id_salt("probe_sel")
                        .width(210.0)
                        .selected_text(
                            self.probes
                                .get(self.selected_probe)
                                .map(|p| p.identifier.as_str())
                                .unwrap_or("未选择"),
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
                        .add_enabled(enabled, egui::Button::new("重新扫描"))
                        .clicked()
                    {
                        self.probing = true;
                        self.log_info("正在扫描探针...");
                        self.send(WorkerCommand::Scan);
                    }
                });
                if self.probing {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new());
                        ui.label("扫描中...");
                    });
                }

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let has_probe = !self.probes.is_empty()
                        && !self.connecting
                        && !self.probing
                        && !self.busy;
                    if ui
                        .add_enabled(
                            has_probe,
                            egui::Button::new("自动识别目标")
                                .fill(egui::Color32::from_rgb(0x1f, 0x6f, 0xc3)),
                        )
                        .clicked()
                    {
                        self.connecting = true;
                        self.busy = true;
                        self.op_bars.clear();
                        self.log_info("正在自动识别目标芯片...");
                        self.send(WorkerCommand::ConnectAuto {
                            probe: self.selected_probe,
                        });
                    }
                    let connected = self.connected.is_some() && !self.busy;
                    if ui
                        .add_enabled(connected, egui::Button::new("断开"))
                        .clicked()
                    {
                        self.connected = None;
                        self.log_info("已断开连接");
                        self.send(WorkerCommand::Disconnect);
                    }
                });
                if self.connecting {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new());
                        ui.label("正在连接目标...");
                    });
                }

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.heading("手动指定目标芯片");
                    if self.show_manual {
                        ui.colored_label(
                            egui::Color32::from_rgb(0xc0, 0x3a, 0x2b),
                            "（自动识别失败，请手动选择）",
                        );
                    }
                });
                ui.label(
                    egui::RichText::new("DAPLink / CMSIS-DAP 等探针需手动选择芯片型号")
                        .small()
                        .weak(),
                );
                ui.horizontal(|ui| {
                    ui.label("搜索型号:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.chip_search)
                            .desired_width(300.0)
                            .hint_text("如 stm32f103 / nrf52840"),
                    );
                });

                let filter = self.chip_search.trim().to_lowercase();
                let matches: Vec<usize> = self
                    .chips
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| filter.is_empty() || c.to_lowercase().contains(&filter))
                    .take(50)
                    .map(|(i, _)| i)
                    .collect();

                if !self.manual_target.is_empty() {
                    ui.label(format!("已选型号: {}", self.manual_target));
                }

                egui::ScrollArea::vertical()
                    .id_salt("chip_list")
                    .max_height(320.0)
                    .show(ui, |ui| {
                        if matches.is_empty() {
                            ui.label(egui::RichText::new("未找到匹配的芯片型号").weak());
                        } else {
                            for i in matches {
                                let name = &self.chips[i];
                                let selected = self.manual_target == *name;
                                if ui.selectable_label(selected, name).clicked() {
                                    self.manual_target = name.clone();
                                }
                            }
                        }
                    });

                let enabled = !self.probes.is_empty()
                    && !self.connecting
                    && !self.busy
                    && !self.manual_target.trim().is_empty();
                if ui
                    .add_enabled(
                        enabled,
                        egui::Button::new("按型号连接")
                            .fill(egui::Color32::from_rgb(0x1f, 0x6f, 0xc3)),
                    )
                    .clicked()
                {
                    let target = self.manual_target.trim().to_owned();
                    self.connecting = true;
                    self.busy = true;
                    self.log_info(format!("正在连接 {} ...", target));
                    self.send(WorkerCommand::ConnectManual {
                        probe: self.selected_probe,
                        target,
                    });
                }
                ui.add_space(4.0);

                ui.add_space(6.0);
                ui.separator();
                match &self.connected {
                    Some(summary) => {
                        ui.heading("目标信息");
                        ui.label(format!("芯片型号: {}", summary.name));
                        ui.label(format!("架构: {}", summary.architecture));
                        ui.label(format!("核心数量: {}", summary.cores.len()));
                        for (i, c) in &summary.cores {
                            ui.label(format!("  核心 {}: {}", i, c));
                        }
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("内存映射:").strong());
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
                        ui.label(egui::RichText::new("尚未连接目标").weak());
                    }
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(6.0);
            ui.heading("固件烧录");
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("固件文件:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.file_path)
                        .desired_width(320.0)
                        .hint_text("选择 .elf / .hex / .bin / .uf2 文件"),
                );
                if ui.button("浏览...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("固件镜像", &["elf", "hex", "bin", "uf2"])
                        .pick_file()
                    {
                        self.file_path = path.display().to_string();
                        self.log_info(format!("已选择固件: {}", self.file_path));
                    }
                }
                if ui.button("选择项目文件夹...").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        self.firmware_root = dir.display().to_string();
                        self.firmware_scanning = true;
                        self.firmware_candidates.clear();
                        self.log_info(format!(
                            "正在扫描项目文件夹并自动识别固件: {}",
                            self.firmware_root
                        ));
                        self.send(WorkerCommand::ScanFirmware { root: dir });
                    }
                }
            });
            if let Some(fmt) = self.detected_format() {
                ui.label(format!("文件格式: {}", fmt));
            }

            if self.firmware_scanning {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    ui.label(format!("扫描中: {}", self.firmware_root));
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
                            "未选择项目固件".to_owned()
                        } else {
                            name
                        }
                    }
                };
                let mut chosen: Option<usize> = None;
                ui.horizontal(|ui| {
                    ui.label("项目固件:");
                    egui::ComboBox::from_id_salt("fw_sel")
                        .width(400.0)
                        .selected_text(sel_text)
                        .show_ui(ui, |ui| {
                            for (i, c) in self.firmware_candidates.iter().enumerate() {
                                let label = format!(
                                    "[{}] {} ({} KB)",
                                    c.kind,
                                    c.path.display(),
                                    c.size_kb
                                );
                                if ui
                                    .selectable_label(Some(i) == current, label)
                                    .clicked()
                                {
                                    chosen = Some(i);
                                }
                            }
                        });
                });
                if let Some(i) = chosen {
                    if let Some(c) = self.firmware_candidates.get(i) {
                        self.file_path = c.path.display().to_string();
                        self.log_info(format!("已选择固件: {}", self.file_path));
                    }
                }
            }

            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.chip_erase, "全片擦除后烧录");
                ui.checkbox(&mut self.verify, "烧录后校验");
                ui.checkbox(&mut self.keep_unwritten, "保留未写入字节");
                ui.checkbox(&mut self.reset_after, "烧录后复位运行");
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
                        egui::Button::new("开始烧录")
                            .fill(egui::Color32::from_rgb(0x2e, 0xa0, 0x43))
                            .min_size(egui::vec2(110.0, 32.0)),
                    )
                    .clicked()
                {
                    self.start_flash();
                }
                let can_erase = self.connected.is_some()
                    && !self.busy
                    && !self.connecting
                    && !self.probing;
                if ui
                    .add_enabled(can_erase, egui::Button::new("全片擦除"))
                    .clicked()
                {
                    self.busy = true;
                    self.op_bars.clear();
                    self.log_info("开始全片擦除...");
                    self.send(WorkerCommand::EraseAll);
                }
                if ui
                    .add_enabled(
                        self.connected.is_some() && !self.busy,
                        egui::Button::new("复位目标"),
                    )
                    .clicked()
                {
                    self.log_info("正在复位目标...");
                    self.send(WorkerCommand::Reset);
                }
            });

            ui.add_space(10.0);
            if !self.op_bars.is_empty() {
                for bar in &self.op_bars {
                    let frac = match bar.total {
                        Some(t) if t > 0 => (bar.done as f32 / t as f32).clamp(0.0, 1.0),
                        _ => 0.0,
                    };
                    let color = match bar.state {
                        OpState::Done => egui::Color32::from_rgb(0x2e, 0xa0, 0x43),
                        OpState::Failed => egui::Color32::from_rgb(0xc0, 0x3a, 0x2b),
                        OpState::Active => egui::Color32::from_rgb(0x1f, 0x6f, 0xc3),
                    };
                    let text = match bar.total {
                        Some(t) => format!(
                            "{}  ({}/{} KB)",
                            bar.label,
                            bar.done / 1024,
                            t / 1024
                        ),
                        None => format!("{}  ({} KB)", bar.label, bar.done / 1024),
                    };
                    ui.add(egui::ProgressBar::new(frac).fill(color).text(text));
                }
            }

            ui.add_space(10.0);
            ui.label(egui::RichText::new("日志").strong());
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
