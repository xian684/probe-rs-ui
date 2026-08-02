use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use eframe::egui;

use crate::chips::{ChipBrandInfo, ChipFamilyInfo};
use crate::firmware::FirmwareCandidate;
use crate::i18n::Lang;
use crate::worker::{
    self, BootMode, OpState, ProbeInfo, TargetSummary, WorkerCommand, WorkerEvent,
};

mod panels;

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

    lang: Lang,

    probes: Vec<ProbeInfo>,
    selected_probe: usize,
    probing: bool,
    connecting: bool,
    boot_mode: BootMode,

    connected: Option<TargetSummary>,
    manual_target: String,
    chip_families: Vec<ChipFamilyInfo>,
    chip_brands: Vec<ChipBrandInfo>,
    selected_brand: Option<usize>,
    selected_family: Option<usize>,
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

    rtt_on: bool,
    rtt_enabled: bool,
    rtt_buf: String,
    rtt_autoscroll: bool,
    rtt_down_input: String,
}

impl ProbeUiApp {
    pub fn new() -> Self {
        let worker = worker::spawn(Lang::Zh);
        let chip_families = crate::chips::builtin_chip_families();
        let chip_brands = crate::chips::group_brands(&chip_families);
        let mut app = ProbeUiApp {
            to_worker: worker.sender,
            from_worker: worker.receiver,
            lang: Lang::Zh,
            probes: Vec::new(),
            selected_probe: 0,
            probing: true,
            connecting: false,
            boot_mode: BootMode::Normal,
            connected: None,
            manual_target: String::new(),
            chip_families,
            chip_brands,
            selected_brand: None,
            selected_family: None,
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
            rtt_on: false,
            rtt_enabled: false,
            rtt_buf: String::new(),
            rtt_autoscroll: true,
            rtt_down_input: String::new(),
        };
        app.log(
            app.lang.pick(
                format!(
                    "已加载 {} 个内置芯片系列（{} 个品牌），可手动指定目标",
                    app.chip_families.len(),
                    app.chip_brands.len()
                ),
                format!(
                    "Loaded {} built-in chip families ({} brands); manual target selection is available",
                    app.chip_families.len(),
                    app.chip_brands.len()
                ),
            ),
            LogLevel::Info,
        );
        app.log(
            app.lang
                .pick("正在扫描调试探针...", "Scanning debug probes..."),
            LogLevel::Info,
        );
        app.send(WorkerCommand::Scan);
        app
    }

    fn send(&self, cmd: WorkerCommand) {
        let _ = self.to_worker.send(cmd);
    }

    fn t(&self, zh: &'static str, en: &'static str) -> &'static str {
        self.lang.pick(zh, en)
    }

    /// 图标 + 本地化文本。
    fn icon(&self, emoji: &str, zh: &'static str, en: &'static str) -> String {
        format!("{emoji} {}", self.t(zh, en))
    }

    /// 品牌名本地化（其余品牌名为专有名词，直接显示）。
    fn brand_label(&self, brand: &str) -> String {
        match brand {
            "Other" => self.t("其他", "Other").to_owned(),
            "ARM" => self.t("ARM 通用", "ARM Generic").to_owned(),
            "RISC-V" => self.t("RISC-V 通用", "RISC-V Generic").to_owned(),
            _ => brand.to_owned(),
        }
    }

    fn set_lang(&mut self, lang: Lang) {
        if self.lang != lang {
            self.lang = lang;
            self.send(WorkerCommand::SetLang(lang));
        }
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
                if self.selected_probe >= self.probes.len() {
                    self.selected_probe = 0;
                }
                if self.probes.is_empty() {
                    self.log_warn(self.t(
                        "未检测到任何调试探针，请检查 USB 连接与驱动",
                        "No debug probes detected. Check USB connection and drivers",
                    ));
                } else {
                    self.log_ok(self.lang.pick(
                        format!("检测到 {} 个调试探针", self.probes.len()),
                        format!("Detected {} debug probe(s)", self.probes.len()),
                    ));
                }
            }
            WorkerEvent::Probes(Err(e)) => {
                self.probing = false;
                self.log_err(e);
            }
            WorkerEvent::Connected(Ok(summary)) => {
                self.connecting = false;
                self.busy = false;
                self.log_ok(self.lang.pick(
                    format!("已连接目标: {}", summary.name),
                    format!("Connected to target: {}", summary.name),
                ));
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
                self.log_ok(self.t("操作成功完成", "Operation completed successfully"));
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
                    self.log_warn(self.lang.pick(
                        format!("在 {} 中未找到固件文件 (.elf / .hex / .bin / .uf2)", root),
                        format!(
                            "No firmware file (.elf / .hex / .bin / .uf2) found in {}",
                            root
                        ),
                    ));
                } else if let Some(i) = best {
                    let path = self.firmware_candidates[i].path.display().to_string();
                    self.file_path = path.clone();
                    self.log_ok(self.lang.pick(
                        format!(
                            "自动识别到固件: {}（共 {} 个候选）",
                            path,
                            self.firmware_candidates.len()
                        ),
                        format!(
                            "Auto-detected firmware: {} ({} candidate(s))",
                            path,
                            self.firmware_candidates.len()
                        ),
                    ));
                    if self.firmware_candidates.len() > 1 {
                        self.log_info(self.t(
                            "如需使用其它固件，请在下方下拉列表中选择",
                            "To use another firmware, pick one from the dropdown below",
                        ));
                    }
                }
            }
            WorkerEvent::RttData { channel, data } => {
                let text = String::from_utf8_lossy(&data);
                self.rtt_buf.push_str(&format!("[CH{}] ", channel));
                self.rtt_buf.push_str(&text);
                const RTT_BUF_CAP: usize = 128 * 1024;
                if self.rtt_buf.len() > RTT_BUF_CAP {
                    let overflow = self.rtt_buf.len() - RTT_BUF_CAP;
                    let cut = self.rtt_buf.floor_char_boundary(overflow);
                    self.rtt_buf.drain(..cut);
                }
            }
            WorkerEvent::RttStarted {
                up_channels,
                down_channels,
            } => {
                self.rtt_on = true;
                self.log_ok(self.lang.pick(
                    format!("RTT 已启动（上行 {up_channels}，下行 {down_channels}）"),
                    format!("RTT started ({} up, {} down)", up_channels, down_channels),
                ));
            }
            WorkerEvent::RttStopped => {
                self.rtt_on = false;
            }
        }
    }

    fn detected_format(&self) -> Option<&'static str> {
        let path = std::path::Path::new(&self.file_path);
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                "elf" | "axf" => return Some("ELF"),
                "hex" => return Some("Intel HEX"),
                "bin" => return Some("Binary"),
                "uf2" => return Some("UF2"),
                _ => {}
            }
        }
        // 无扩展名的 Rust 编译产物：按 ELF 魔数识别。
        if crate::firmware::is_elf(path) {
            Some("ELF")
        } else {
            None
        }
    }

    fn start_flash(&mut self) {
        if self.detected_format().is_none() {
            self.log_err(self.t(
                "不支持的文件格式，请选择 .elf / .hex / .bin / .uf2 文件",
                "Unsupported file format. Choose a .elf / .hex / .bin / .uf2 file",
            ));
            return;
        }
        self.busy = true;
        self.op_bars.clear();
        self.log_info(self.lang.pick(
            format!("开始烧录: {}", self.file_path),
            format!("Flashing: {}", self.file_path),
        ));
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

        if self.probing || self.connecting || self.busy || self.rtt_on {
            ctx.request_repaint_after(Duration::from_millis(40));
        }

        self.top_panel(ctx);
        self.device_panel(ctx);
        self.rtt_panel(ctx);
        self.flashing_panel(ctx);
    }
}
