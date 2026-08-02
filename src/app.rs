use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::egui;

use crate::chips::{ChipBrandInfo, ChipFamilyInfo};
use crate::config::{self, AppConfig};
use crate::firmware::FirmwareCandidate;
use crate::i18n::Lang;
use crate::worker::{
    self, BootMode, OpState, ProbeInfo, TargetSummary, WorkerCommand, WorkerEvent,
};

/// 左栏『目标信息』框与中央底部日志框对齐时的最小高度。
pub(crate) const TARGET_INFO_MIN_H: f32 = 220.0;

#[derive(Clone, Copy)]
pub(crate) enum LogLevel {
    Info,
    Ok,
    Warn,
    Error,
}

/// 界面主题模式。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThemeMode {
    /// 跟随系统深色/浅色主题。
    System,
    Light,
    Dark,
}

/// 中央面板显示的视图。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CentralTab {
    Flash,
    Memory,
    Rtt,
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
    pub(crate) theme_mode: ThemeMode,
    pub(crate) theme_applied: Option<egui::ThemePreference>,

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
    pub(crate) rtt_up_channels: usize,
    pub(crate) rtt_down_channels: usize,
    pub(crate) rtt_view_channel: Option<usize>,
    pub(crate) rtt_send_channel: usize,
    pub(crate) rtt_buf: String,
    pub(crate) rtt_autoscroll: bool,
    pub(crate) rtt_down_input: String,

    pub(crate) central_tab: CentralTab,
    pub(crate) mem_start: u64,
    pub(crate) mem_len: usize,
    pub(crate) mem_data: Vec<u8>,
    pub(crate) mem_read_addr: u64,
    pub(crate) mem_busy: bool,
    pub(crate) mem_write_start: u64,
    pub(crate) mem_write_input: String,

    last_save: Instant,
    win_size: Option<[f32; 2]>,
    win_pos: Option<[f32; 2]>,
    win_clamped: bool,
    pub(crate) target_info_h: f32,
}

impl ProbeUiApp {
    pub fn new() -> Self {
        let worker = worker::spawn(Lang::Zh);
        let chip_families = crate::chips::builtin_chip_families();
        let chip_brands = crate::chips::group_brands(&chip_families);
        let saved = config::load();
        let mut app = ProbeUiApp {
            to_worker: worker.sender,
            from_worker: worker.receiver,
            lang: Lang::Zh,
            theme_mode: ThemeMode::System,
            theme_applied: None,
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
            rtt_up_channels: 0,
            rtt_down_channels: 0,
            rtt_view_channel: None,
            rtt_send_channel: 0,
            rtt_buf: String::new(),
            rtt_autoscroll: true,
            rtt_down_input: String::new(),
            central_tab: CentralTab::Flash,
            mem_start: 0,
            mem_len: 256,
            mem_data: Vec::new(),
            mem_read_addr: 0,
            mem_busy: false,
            mem_write_start: 0,
            mem_write_input: String::new(),
            last_save: Instant::now(),
            win_size: saved.window_size,
            win_pos: saved.window_pos,
            win_clamped: false,
            target_info_h: 180.0,
        };
        app.apply_config(saved);
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

    pub(crate) fn set_theme(&mut self, mode: ThemeMode) {
        if self.theme_mode != mode {
            self.theme_mode = mode;
            self.theme_applied = None;
        }
    }

    /// 将已保存的配置应用到界面状态。
    fn apply_config(&mut self, cfg: AppConfig) {
        self.lang = if cfg.lang == "en" { Lang::En } else { Lang::Zh };
        self.theme_mode = match cfg.theme.as_str() {
            "light" => ThemeMode::Light,
            "dark" => ThemeMode::Dark,
            _ => ThemeMode::System,
        };
        self.theme_applied = None;
        self.boot_mode = if cfg.boot_mode == "under_reset" {
            BootMode::UnderReset
        } else {
            BootMode::Normal
        };
        self.manual_target = cfg.manual_target;
        if !self.manual_target.trim().is_empty() {
            if let Some(family_idx) = self
                .chip_families
                .iter()
                .position(|f| f.chips.iter().any(|c| c == &self.manual_target))
            {
                self.selected_family = Some(family_idx);
                self.selected_brand = self
                    .chip_brands
                    .iter()
                    .position(|b| b.families.contains(&family_idx));
            }
        }
        self.file_path = cfg.file_path;
        self.firmware_root = cfg.firmware_root;
        self.chip_erase = cfg.chip_erase;
        self.verify = cfg.verify;
        self.keep_unwritten = cfg.keep_unwritten;
        self.reset_after = cfg.reset_after;
        self.bin_base = cfg.bin_base;
        self.rtt_view_channel = cfg.rtt_view_channel;
        self.rtt_send_channel = cfg.rtt_send_channel;
        self.rtt_autoscroll = cfg.rtt_autoscroll;
        self.central_tab = match cfg.central_tab.as_str() {
            "memory" => CentralTab::Memory,
            "rtt" => CentralTab::Rtt,
            _ => CentralTab::Flash,
        };
        self.mem_start = cfg.mem_start;
        self.mem_len = cfg.mem_len;
        self.mem_write_start = cfg.mem_write_start;
        self.send(WorkerCommand::SetLang(self.lang));
        if !self.firmware_root.trim().is_empty() {
            self.firmware_scanning = true;
            self.send(WorkerCommand::ScanFirmware {
                root: PathBuf::from(self.firmware_root.clone()),
            });
        }
    }

    /// 收集当前界面状态（含窗口尺寸/位置）用于保存。
    fn collect_config(&mut self, ctx: &egui::Context) -> AppConfig {
        let (size, pos) = ctx.input(|i| {
            let rect = i.viewport().outer_rect;
            let size = rect.map(|r| [r.width(), r.height()]);
            let pos = rect.map(|r| [r.min.x, r.min.y]);
            (size, pos)
        });
        if let Some(s) = size {
            self.win_size = Some(s);
        }
        if let Some(p) = pos {
            self.win_pos = Some(p);
        }
        AppConfig {
            lang: if self.lang.is_en() { "en" } else { "zh" }.into(),
            theme: match self.theme_mode {
                ThemeMode::System => "system",
                ThemeMode::Light => "light",
                ThemeMode::Dark => "dark",
            }
            .into(),
            boot_mode: match self.boot_mode {
                BootMode::Normal => "normal",
                BootMode::UnderReset => "under_reset",
            }
            .into(),
            manual_target: self.manual_target.clone(),
            file_path: self.file_path.clone(),
            firmware_root: self.firmware_root.clone(),
            chip_erase: self.chip_erase,
            verify: self.verify,
            keep_unwritten: self.keep_unwritten,
            reset_after: self.reset_after,
            bin_base: self.bin_base,
            rtt_view_channel: self.rtt_view_channel,
            rtt_send_channel: self.rtt_send_channel,
            rtt_autoscroll: self.rtt_autoscroll,
            central_tab: match self.central_tab {
                CentralTab::Flash => "flash",
                CentralTab::Memory => "memory",
                CentralTab::Rtt => "rtt",
            }
            .into(),
            mem_start: self.mem_start,
            mem_len: self.mem_len,
            mem_write_start: self.mem_write_start,
            window_size: self.win_size,
            window_pos: self.win_pos,
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
                match self.rtt_view_channel {
                    Some(view) if view != channel => {}
                    Some(_) => self.rtt_buf.push_str(&text),
                    None => {
                        self.rtt_buf.push_str(&format!("[CH{}] ", channel));
                        self.rtt_buf.push_str(&text);
                    }
                }
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
                self.rtt_up_channels = up_channels;
                self.rtt_down_channels = down_channels;
                if let Some(v) = self.rtt_view_channel {
                    if v >= up_channels {
                        self.rtt_view_channel = None;
                    }
                }
                if self.rtt_send_channel >= down_channels.max(1) {
                    self.rtt_send_channel = 0;
                }
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
        self.detected_format_of(std::path::Path::new(&self.file_path))
    }

    pub(crate) fn detected_format_of(&self, path: &std::path::Path) -> Option<&'static str> {
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
        self.flash_file(PathBuf::from(self.file_path.clone()), self.bin_base);
    }

    pub(crate) fn flash_file(&mut self, path: PathBuf, bin_base: u64) {
        if self.detected_format_of(&path).is_none() {
            self.log_err(self.t(
                "不支持的文件格式，请选择 .elf / .hex / .bin / .uf2 文件",
                "Unsupported file format. Choose a .elf / .hex / .bin / .uf2 file",
            ));
            return;
        }
        self.busy = true;
        self.op_bars.clear();
        self.log_info(self.lang.pick(
            format!("开始烧录: {}", path.display()),
            format!("Flashing: {}", path.display()),
        ));
        self.send(WorkerCommand::Flash {
            path,
            do_chip_erase: self.chip_erase,
            verify: self.verify,
            keep_unwritten_bytes: self.keep_unwritten,
            reset_after: self.reset_after,
            bin_base,
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
        let pref = match self.theme_mode {
            ThemeMode::System => egui::ThemePreference::System,
            ThemeMode::Light => egui::ThemePreference::Light,
            ThemeMode::Dark => egui::ThemePreference::Dark,
        };
        if self.theme_applied != Some(pref) {
            ctx.set_theme(pref);
            self.theme_applied = Some(pref);
        }

        // 窗口尺寸/位置钳制：超出屏幕则自动缩放并居中。
        // 注意：pixels_per_point() 必须在 ctx.input 闭包之外调用，
        // 否则会在闭包内再次获取 Context 写锁导致死锁。
        if !self.win_clamped {
            let info = ctx.input(|i| {
                let m = i.viewport().monitor_size?;
                Some((m, i.viewport().outer_rect))
            });
            if let Some((m, Some(r))) = info {
                self.win_clamped = true;
                let cur = r.size();
                let w = cur.x.min(m.x * 0.98);
                let h = cur.y.min(m.y * 0.98);
                if w < cur.x || h < cur.y {
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w, h)));
                }
                let off_screen = r.min.x < 0.0 || r.min.y < 0.0 || r.max.x > m.x || r.max.y > m.y;
                if off_screen {
                    let pos = egui::pos2((m.x - w) / 2.0, (m.y - h) / 2.0);
                    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
                }
            }
        }

        while let Ok(ev) = self.from_worker.try_recv() {
            self.handle_event(ev);
        }

        if self.last_save.elapsed() >= Duration::from_secs(2) {
            self.last_save = Instant::now();
            config::save(&self.collect_config(ctx));
        }

        if self.probing || self.connecting || self.busy || self.rtt_on || self.mem_busy {
            ctx.request_repaint_after(Duration::from_millis(40));
        }

        self.top_panel(ctx);
        self.device_panel(ctx);
        self.log_panel(ctx);
        self.central_panel(ctx);
    }
}
