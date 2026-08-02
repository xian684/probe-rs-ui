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

#[derive(Clone, Copy)]
pub(crate) enum LogLevel {
    Info,
    Ok,
    Warn,
    Error,
}

pub(crate) struct LogEntry {
    pub(crate) text: String,
    pub(crate) level: LogLevel,
}

pub(crate) struct OpBar {
    pub(crate) label: String,
    pub(crate) done: u64,
    pub(crate) total: Option<u64>,
    pub(crate) state: OpState,
}

pub struct ProbeUiApp {
    pub(crate) to_worker: Sender<WorkerCommand>,
    pub(crate) from_worker: Receiver<WorkerEvent>,

    pub(crate) lang: Lang,

    pub(crate) probes: Vec<ProbeInfo>,
    pub(crate) selected_probe: usize,
    pub(crate) probing: bool,
    pub(crate) connecting: bool,
    pub(crate) boot_mode: BootMode,

    pub(crate) connected: Option<TargetSummary>,
    pub(crate) manual_target: String,
    pub(crate) chip_families: Vec<ChipFamilyInfo>,
    pub(crate) chip_brands: Vec<ChipBrandInfo>,
    pub(crate) selected_brand: Option<usize>,
    pub(crate) selected_family: Option<usize>,
    pub(crate) chip_search: String,
    pub(crate) show_manual: bool,

    pub(crate) file_path: String,
    pub(crate) chip_erase: bool,
    pub(crate) verify: bool,
    pub(crate) keep_unwritten: bool,
    pub(crate) reset_after: bool,

    pub(crate) firmware_root: String,
    pub(crate) firmware_candidates: Vec<FirmwareCandidate>,
    pub(crate) firmware_scanning: bool,

    pub(crate) read_start: u64,
    pub(crate) read_end: u64,
    pub(crate) bin_base: u64,

    pub(crate) busy: bool,
    pub(crate) op_bars: Vec<OpBar>,
    pub(crate) log: Vec<LogEntry>,

    pub(crate) rtt_on: bool,
    pub(crate) rtt_enabled: bool,
    pub(crate) rtt_buf: String,
    pub(crate) rtt_autoscroll: bool,
    pub(crate) rtt_down_input: String,

    pub(crate) mem_mode: bool,
    pub(crate) mem_start: u64,
    pub(crate) mem_len: usize,
    pub(crate) mem_data: Vec<u8>,
    pub(crate) mem_read_addr: u64,
    pub(crate) mem_busy: bool,
    pub(crate) mem_write_start: u64,
    pub(crate) mem_write_input: String,
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
            read_start: 0,
            read_end: 0,
            bin_base: 0,
            busy: false,
            op_bars: Vec::new(),
            log: Vec::new(),
            rtt_on: false,
            rtt_enabled: false,
            rtt_buf: String::new(),
            rtt_autoscroll: true,
            rtt_down_input: String::new(),
            mem_mode: false,
            mem_start: 0,
            mem_len: 256,
            mem_data: Vec::new(),
            mem_read_addr: 0,
            mem_busy: false,
            mem_write_start: 0,
            mem_write_input: String::new(),
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

    pub(crate) fn send(&self, cmd: WorkerCommand) {
        let _ = self.to_worker.send(cmd);
    }

    pub(crate) fn t(&self, zh: &'static str, en: &'static str) -> &'static str {
        self.lang.pick(zh, en)
    }

    /// 图标 + 本地化文本。
    pub(crate) fn icon(&self, emoji: &str, zh: &'static str, en: &'static str) -> String {
        format!("{emoji} {}", self.t(zh, en))
    }

    /// 品牌名本地化（其余品牌名为专有名词，直接显示）。
    pub(crate) fn brand_label(&self, brand: &str) -> String {
        match brand {
            "Other" => self.t("其他", "Other").to_owned(),
            "ARM" => self.t("ARM 通用", "ARM Generic").to_owned(),
            "RISC-V" => self.t("RISC-V 通用", "RISC-V Generic").to_owned(),
            _ => brand.to_owned(),
        }
    }

    pub(crate) fn set_lang(&mut self, lang: Lang) {
        if self.lang != lang {
            self.lang = lang;
            self.send(WorkerCommand::SetLang(lang));
        }
    }

    pub(crate) fn log(&mut self, text: impl Into<String>, level: LogLevel) {
        self.log.push(LogEntry {
            text: text.into(),
            level,
        });
        while self.log.len() > 800 {
            self.log.remove(0);
        }
    }

    pub(crate) fn log_info(&mut self, text: impl Into<String>) {
        self.log(text, LogLevel::Info);
    }

    pub(crate) fn log_ok(&mut self, text: impl Into<String>) {
        self.log(text, LogLevel::Ok);
    }

    pub(crate) fn log_warn(&mut self, text: impl Into<String>) {
        self.log(text, LogLevel::Warn);
    }

    pub(crate) fn log_err(&mut self, text: impl Into<String>) {
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
                if let Some(flash) = self
                    .connected
                    .as_ref()
                    .and_then(|s| s.memory.iter().find(|m| m.kind == "FLASH"))
                {
                    self.read_start = flash.start;
                    self.read_end = flash.end;
                    self.bin_base = flash.start;
                }
                if let Some(ram) = self
                    .connected
                    .as_ref()
                    .and_then(|s| s.memory.iter().find(|m| m.kind == "RAM"))
                {
                    self.mem_start = ram.start;
                    self.mem_write_start = ram.start;
                }
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
            WorkerEvent::MemoryRead(Ok(data)) => {
                self.mem_busy = false;
                self.mem_read_addr = self.mem_start;
                self.mem_data = data;
                self.log_ok(self.lang.pick(
                    format!("读取内存完成: {} 字节", self.mem_data.len()),
                    format!("Memory read: {} bytes", self.mem_data.len()),
                ));
            }
            WorkerEvent::MemoryRead(Err(e)) => {
                self.mem_busy = false;
                self.mem_data.clear();
                self.log_err(e);
            }
            WorkerEvent::MemoryWrite(result) => {
                self.mem_busy = false;
                match result {
                    Ok(()) => self.log_ok(self.t("内存写入完成", "Memory written")),
                    Err(e) => self.log_err(e),
                }
            }
        }
    }

    pub(crate) fn detected_format(&self) -> Option<&'static str> {
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

    pub(crate) fn start_flash(&mut self) {
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
            bin_base: self.bin_base,
        });
    }

    pub(crate) fn read_memory(&mut self) {
        if self.connected.is_none() {
            self.log_warn(self.t("请先连接目标芯片", "Connect to a target first"));
            return;
        }
        let len = self.mem_len.clamp(1, 256 * 1024);
        if len != self.mem_len {
            self.log_warn(self.lang.pick(
                format!("读取长度已限制为 {len} 字节"),
                format!("Read length clamped to {len} bytes"),
            ));
            self.mem_len = len;
        }
        let start = self.mem_start;
        self.mem_busy = true;
        self.log_info(self.lang.pick(
            format!("正在读取内存: 0x{start:X}，{len} 字节"),
            format!("Reading memory: 0x{start:X}, {len} bytes"),
        ));
        self.send(WorkerCommand::MemoryRead { start, len });
    }

    pub(crate) fn write_memory(&mut self) {
        let Some(bytes) = parse_hex_bytes(&self.mem_write_input) else {
            self.log_err(self.t(
                "数据格式错误：请输入十六进制字节（如 DE AD BE EF）",
                "Invalid data: enter hex bytes (e.g. DE AD BE EF)",
            ));
            return;
        };
        if bytes.is_empty() {
            return;
        }
        if self.connected.is_none() {
            self.log_warn(self.t("请先连接目标芯片", "Connect to a target first"));
            return;
        }
        let start = self.mem_write_start;
        self.mem_busy = true;
        self.log_info(self.lang.pick(
            format!("正在写入内存: 0x{start:X}，{} 字节", bytes.len()),
            format!("Writing memory: 0x{start:X}, {} bytes", bytes.len()),
        ));
        self.send(WorkerCommand::MemoryWrite { start, data: bytes });
    }
}

/// 将形如 "DE AD BE EF" 或 "DEADBEEF" 的十六进制字符串解析为字节序列。
fn parse_hex_bytes(s: &str) -> Option<Vec<u8>> {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.is_empty() || compact.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(compact.len() / 2);
    for i in (0..compact.len()).step_by(2) {
        out.push(u8::from_str_radix(&compact[i..i + 2], 16).ok()?);
    }
    Some(out)
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

        if self.probing || self.connecting || self.busy || self.rtt_on || self.mem_busy {
            ctx.request_repaint_after(Duration::from_millis(40));
        }

        self.top_panel(ctx);
        self.device_panel(ctx);
        self.rtt_panel(ctx);
        self.central_panel(ctx);
    }
}
