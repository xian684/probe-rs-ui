//! 应用状态中枢与入口：`ProbeUiApp` 状态结构、生命周期与各面板入口。
//!
//! 拆分子模块：
//! - [`settings`]：配置应用/收集（apply_config / collect_config）
//! - [`events`]：后台事件分发（handle_event）
//! - [`actions`]：烧录 / 内存读写等操作入口与工具函数

use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::egui;

use crate::chips::{ChipBrandInfo, ChipFamilyInfo};
use crate::config;
use crate::firmware::FirmwareCandidate;
use crate::i18n::{Lang, Msg};
use crate::t;
use crate::worker::{
    self, ArmPackInfo, BootMode, ChipFileInfo, OpState, ProbeInfo, TargetGenResult,
    TargetSummary, WorkerCommand, WorkerEvent,
};

mod actions;
mod events;
mod settings;

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
    ArmIndex,
}

/// 左侧设备面板『手动指定目标 / 高级芯片配置』互斥切换。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeviceTab {
    Manual,
    Advanced,
}

/// 手动指定目标下『内置芯片包 / 外部芯片包』互斥切换。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PackTab {
    /// 内置 probe-rs 芯片库（品牌/系列/型号三级联动）。
    Builtin,
    /// 通过 YAML / CMSIS Pack 导入的外部芯片。
    External,
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
    /// 通过加载 YAML / CMSIS Pack 导入的外部芯片族（独立于内置三级菜单）。
    pub(crate) external_families: Vec<ChipFamilyInfo>,
    /// 历史导入过的外部芯片包来源文件路径（持久化，启动时自动恢复）。
    pub(crate) external_sources: Vec<String>,
    /// 外部芯片包下拉选中的家族索引。
    pub(crate) selected_external_family: Option<usize>,
    /// 手动选型视图：内置芯片包 / 外部芯片包。
    pub(crate) pack_tab: PackTab,
    pub(crate) selected_brand: Option<usize>,
    pub(crate) selected_family: Option<usize>,
    pub(crate) chip_search: String,
    pub(crate) show_manual: bool,
    pub(crate) device_tab: DeviceTab,

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

    // ---- Target 生成器（左侧高级芯片配置面板） ----
    pub(crate) tg_input: String,
    pub(crate) tg_output_dir: String,
    pub(crate) tg_only_supported: bool,
    pub(crate) tg_busy: bool,
    pub(crate) tg_result: Option<TargetGenResult>,

    // ---- ARM 在线索引 ----
    pub(crate) arm_keyword: String,
    pub(crate) arm_packs: Vec<ArmPackInfo>,
    pub(crate) arm_busy: bool,
    pub(crate) arm_selected: Option<usize>,

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
            external_families: Vec::new(),
            external_sources: Vec::new(),
            selected_external_family: None,
            pack_tab: PackTab::Builtin,
            selected_brand: None,
            selected_family: None,
            chip_search: String::new(),
            show_manual: false,
            device_tab: DeviceTab::Manual,
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
            tg_input: String::new(),
            tg_output_dir: String::new(),
            tg_only_supported: false,
            tg_busy: false,
            tg_result: None,
            arm_keyword: String::new(),
            arm_packs: Vec::new(),
            arm_busy: false,
            arm_selected: None,
            last_save: Instant::now(),
            win_size: saved.window_size,
            win_pos: saved.window_pos,
            win_clamped: false,
            target_info_h: 180.0,
        };
        app.apply_config(saved);
        app.log(
            t!(
                app.lang,
                Msg::LoadedChips,
                app.chip_families.len(),
                app.chip_brands.len()
            ),
            LogLevel::Info,
        );
        app.log(app.lang.tr(Msg::ScanningDebugProbes), LogLevel::Info);
        app.send(WorkerCommand::Scan);
        app
    }

    pub(crate) fn send(&self, cmd: WorkerCommand) {
        let _ = self.to_worker.send(cmd);
    }

    pub(crate) fn t(&self, msg: Msg) -> &'static str {
        self.lang.tr(msg)
    }

    /// 图标 + 本地化文本。
    pub(crate) fn icon(&self, emoji: &str, msg: Msg) -> String {
        format!("{emoji} {}", self.t(msg))
    }

    /// 品牌名本地化（其余品牌名为专有名词，直接显示）。
    pub(crate) fn brand_label(&self, brand: &str) -> String {
        match brand {
            "Other" => self.t(Msg::BrandOther).to_owned(),
            "ARM" => self.t(Msg::BrandArm).to_owned(),
            "RISC-V" => self.t(Msg::BrandRiscv).to_owned(),
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

    /// 将外部加载的芯片族合并进『外部芯片包』列表（独立于内置三级菜单）。
    ///
    /// 去重规则：
    /// - 已存在同名的外部芯片族：不重复添加，仅并入新型号（并集）；
    /// - 否则新增一条目。
    fn merge_chip_file(&mut self, info: ChipFileInfo) -> ChipMergeResult {
        if let Some(existing) = self
            .external_families
            .iter_mut()
            .find(|f| f.name == info.family_name)
        {
            let before = existing.chips.len();
            for c in &info.chips {
                if !existing.chips.contains(c) {
                    existing.chips.push(c.clone());
                }
            }
            return if existing.chips.len() > before {
                ChipMergeResult::Updated
            } else {
                ChipMergeResult::Skipped
            };
        }
        let family = ChipFamilyInfo {
            name: info.family_name,
            brand: self.t(Msg::BrandExternal).to_owned(),
            chips: info.chips,
        };
        self.external_families.push(family);
        ChipMergeResult::Added
    }

    /// 记录一个外部芯片包来源文件路径（供下次启动自动恢复）。
    ///
    /// 重复路径不重复记录；空路径忽略。
    pub(crate) fn record_external_source(&mut self, path: &std::path::Path) {
        let p = path.display().to_string();
        if p.trim().is_empty() || self.external_sources.contains(&p) {
            return;
        }
        self.external_sources.push(p);
    }
}

/// 芯片族合并结果（供去重日志反馈）。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChipMergeResult {
    /// 新增了一条外部芯片族。
    Added,
    /// 已存在，仅并入了新型号。
    Updated,
    /// 完全重复（或与内置同名），已跳过。
    Skipped,
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

        if self.probing
            || self.connecting
            || self.busy
            || self.rtt_on
            || self.mem_busy
            || self.tg_busy
            || self.arm_busy
        {
            ctx.request_repaint_after(Duration::from_millis(40));
        }

        self.top_panel(ctx);
        self.device_panel(ctx);
        self.log_panel(ctx);
        self.central_panel(ctx);
    }
}
