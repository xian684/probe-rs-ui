use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use eframe::egui;

use crate::i18n::Lang;
use crate::worker::{
    self, ChipBrandInfo, ChipFamilyInfo, FirmwareCandidate, OpState, ProbeInfo, TargetSummary,
    WorkerCommand, WorkerEvent,
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

    lang: Lang,

    probes: Vec<ProbeInfo>,
    selected_probe: usize,
    probing: bool,
    connecting: bool,

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
        let worker = worker::spawn(Lang::Zh);
        let chip_families = worker::builtin_chip_families();
        let chip_brands = worker::group_brands(&chip_families);
        let mut app = ProbeUiApp {
            to_worker: worker.sender,
            from_worker: worker.receiver,
            lang: Lang::Zh,
            probes: Vec::new(),
            selected_probe: 0,
            probing: true,
            connecting: false,
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
            app.lang.pick(
                "正在扫描调试探针...",
                "Scanning debug probes...",
            ),
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
                if self.probes.is_empty() {
                    self.log_warn(self.t(
                        "未检测到任何调试探针，请检查 USB 连接与驱动",
                        "No debug probes detected. Check USB connection and drivers",
                    ));
                } else {
                    self.log_ok(
                        self.lang.pick(
                            format!("检测到 {} 个调试探针", self.probes.len()),
                            format!("Detected {} debug probe(s)", self.probes.len()),
                        ),
                    );
                }
            }
            WorkerEvent::Probes(Err(e)) => {
                self.probing = false;
                self.log_err(e);
            }
            WorkerEvent::Connected(Ok(summary)) => {
                self.connecting = false;
                self.busy = false;
                self.log_ok(
                    self.lang.pick(
                        format!("已连接目标: {}", summary.name),
                        format!("Connected to target: {}", summary.name),
                    ),
                );
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
                    self.log_warn(
                        self.lang.pick(
                            format!(
                                "在 {} 中未找到固件文件 (.elf / .hex / .bin / .uf2)",
                                root
                            ),
                            format!(
                                "No firmware file (.elf / .hex / .bin / .uf2) found in {}",
                                root
                            ),
                        ),
                    );
                } else if let Some(i) = best {
                    let path = self.firmware_candidates[i]
                        .path
                        .display()
                        .to_string();
                    self.file_path = path.clone();
                    self.log_ok(
                        self.lang.pick(
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
                        ),
                    );
                    if self.firmware_candidates.len() > 1 {
                        self.log_info(self.t(
                            "如需使用其它固件，请在下方下拉列表中选择",
                            "To use another firmware, pick one from the dropdown below",
                        ));
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
            self.log_err(self.t(
                "不支持的文件格式，请选择 .elf / .hex / .bin / .uf2 文件",
                "Unsupported file format. Choose a .elf / .hex / .bin / .uf2 file",
            ));
            return;
        }
        self.busy = true;
        self.op_bars.clear();
        self.log_info(
            self.lang.pick(
                format!("开始烧录: {}", self.file_path),
                format!("Flashing: {}", self.file_path),
            ),
        );
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
                ui.heading(self.t("Probe-rs 烧录工具", "Probe-rs Flasher"));
                ui.separator();
                if self.connected.is_some() {
                    ui.colored_label(
                        egui::Color32::from_rgb(0x2e, 0xa0, 0x43),
                        self.t("● 已连接", "● Connected"),
                    );
                } else {
                    ui.colored_label(
                        egui::Color32::from_rgb(0xcc, 0x88, 0x00),
                        self.t("○ 未连接", "○ Not connected"),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("基于 probe-rs v0.32").weak());
                    egui::ComboBox::from_id_salt("lang_sel")
                        .width(90.0)
                        .selected_text(if self.lang.is_en() { "English" } else { "中文" })
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(!self.lang.is_en(), "中文")
                                .clicked()
                            {
                                self.set_lang(Lang::Zh);
                            }
                            if ui.selectable_label(self.lang.is_en(), "English").clicked() {
                                self.set_lang(Lang::En);
                            }
                        });
                });
            });
            ui.add_space(4.0);
        });

        egui::SidePanel::left("detect_panel")
            .resizable(false)
            .default_width(520.0)
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.heading(self.t("设备检测", "Device Detection"));
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label(self.t("调试探针:", "Probe:"));
                    egui::ComboBox::from_id_salt("probe_sel")
                        .width(210.0)
                        .selected_text(
                            self.probes
                                .get(self.selected_probe)
                                .map(|p| p.identifier.as_str())
                                .unwrap_or(self.t("未选择", "Not selected")),
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
                        .add_enabled(
                            enabled,
                            egui::Button::new(self.icon(
                                "🔄",
                                "重新扫描",
                                "Rescan",
                            )),
                        )
                        .clicked()
                    {
                        self.probing = true;
                        self.log_info(self.t("正在扫描探针...", "Scanning probes..."));
                        self.send(WorkerCommand::Scan);
                    }
                });
                if self.probing {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new());
                        ui.label(self.t("扫描中...", "Scanning..."));
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
                            egui::Button::new(self.icon(
                                "🔍",
                                "自动识别目标",
                                "Auto-detect Target",
                            ))
                            .fill(egui::Color32::from_rgb(0x1f, 0x6f, 0xc3)),
                        )
                        .clicked()
                    {
                        self.connecting = true;
                        self.busy = true;
                        self.op_bars.clear();
                        self.log_info(self.t(
                            "正在自动识别目标芯片...",
                            "Auto-detecting target chip...",
                        ));
                        self.send(WorkerCommand::ConnectAuto {
                            probe: self.selected_probe,
                        });
                    }
                    let connected = self.connected.is_some() && !self.busy;
                    if ui
                        .add_enabled(
                            connected,
                            egui::Button::new(self.icon(
                                "🔌",
                                "断开",
                                "Disconnect",
                            )),
                        )
                        .clicked()
                    {
                        self.connected = None;
                        self.log_info(self.t("已断开连接", "Disconnected"));
                        self.send(WorkerCommand::Disconnect);
                    }
                });
                if self.connecting {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new());
                        ui.label(self.t("正在连接目标...", "Connecting to target..."));
                    });
                }

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.heading(self.t("手动指定目标芯片", "Manual Target Selection"));
                    if self.show_manual {
                        ui.colored_label(
                            egui::Color32::from_rgb(0xc0, 0x3a, 0x2b),
                            self.t(
                                "（自动识别失败，请手动选择）",
                                "(auto-detection failed, select manually)",
                            ),
                        );
                    }
                });
                ui.label(
                    egui::RichText::new(self.t(
                        "DAPLink / CMSIS-DAP 等探针需手动选择芯片型号",
                        "DAPLink / CMSIS-DAP probes need manual chip selection",
                    ))
                    .small()
                    .weak(),
                );
                ui.horizontal(|ui| {
                    ui.label(self.t("搜索型号:", "Search:"));
                    let hint = self.t("如 stm32f103 / nrf52840", "e.g. stm32f103 / nrf52840");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.chip_search)
                            .desired_width(300.0)
                            .font(egui::TextStyle::Small)
                            .hint_text(hint),
                    );
                });

                if !self.manual_target.is_empty() {
                    ui.label(
                        self.lang.pick(
                            format!("已选型号: {}", self.manual_target),
                            format!("Selected: {}", self.manual_target),
                        ),
                    );
                }

                let filter = self.chip_search.trim().to_lowercase();
                let mut brand_fams: Vec<(usize, Vec<usize>)> = Vec::new();
                for (bi, brand) in self.chip_brands.iter().enumerate() {
                    let keep: Vec<usize> = brand
                        .families
                        .iter()
                        .copied()
                        .filter(|&i| {
                            let f = &self.chip_families[i];
                            filter.is_empty()
                                || f.name.to_lowercase().contains(&filter)
                                || f.chips.iter().any(|c| c.to_lowercase().contains(&filter))
                        })
                        .collect();
                    if !keep.is_empty() {
                        brand_fams.push((bi, keep));
                    }
                }

                let brand_pos = self
                    .selected_brand
                    .and_then(|b| brand_fams.iter().position(|(bi, _)| *bi == b))
                    .unwrap_or(0);
                let sel_brand = brand_fams.get(brand_pos).map(|(bi, _)| *bi);
                let fam_matches: Vec<usize> = brand_fams
                    .get(brand_pos)
                    .map(|(_, fams)| fams.clone())
                    .unwrap_or_default();

                let mut sel_family = self
                    .selected_family
                    .filter(|i| fam_matches.contains(i))
                    .or_else(|| fam_matches.first().copied());
                if !filter.is_empty() {
                    let good = sel_family
                        .and_then(|i| self.chip_families.get(i))
                        .map(|f| f.chips.iter().any(|c| c.to_lowercase().contains(&filter)))
                        .unwrap_or(false);
                    if !good {
                        sel_family = fam_matches
                            .iter()
                            .copied()
                            .find(|&i| {
                                self.chip_families[i]
                                    .chips
                                    .iter()
                                    .any(|c| c.to_lowercase().contains(&filter))
                            })
                            .or_else(|| fam_matches.first().copied());
                    }
                }

                self.selected_brand = sel_brand;
                self.selected_family = sel_family;

                ui.columns(3, |cols| {
                    cols[0].label(
                        egui::RichText::new(self.t("品牌", "Brand"))
                            .strong()
                            .small(),
                    );
                    cols[1].label(
                        egui::RichText::new(self.t("系列", "Family"))
                            .strong()
                            .small(),
                    );
                    cols[2].label(
                        egui::RichText::new(self.t("具体型号", "Variant"))
                            .strong()
                            .small(),
                    );

                    let mut picked_brand: Option<(usize, Option<usize>)> = None;
                    egui::ScrollArea::vertical()
                        .id_salt("brand_list")
                        .max_height(300.0)
                        .show(&mut cols[0], |ui| {
                            if brand_fams.is_empty() {
                                ui.label(
                                    egui::RichText::new(self.t(
                                        "未找到匹配的品牌",
                                        "No matching brand",
                                    ))
                                    .weak(),
                                );
                            } else {
                                for (_, (bi, fams)) in brand_fams.iter().enumerate() {
                                    let brand = &self.chip_brands[*bi];
                                    let selected = Some(*bi) == sel_brand;
                                    let label = format!(
                                        "{} ({})",
                                        self.brand_label(&brand.name),
                                        fams.len()
                                    );
                                    if ui.selectable_label(selected, label).clicked() {
                                        picked_brand = Some((*bi, fams.first().copied()));
                                    }
                                }
                            }
                        });

                    let mut picked_family: Option<usize> = None;
                    egui::ScrollArea::vertical()
                        .id_salt("fam_list")
                        .max_height(300.0)
                        .show(&mut cols[1], |ui| {
                            if fam_matches.is_empty() {
                                ui.label(
                                    egui::RichText::new(self.t(
                                        "无匹配系列",
                                        "No matching family",
                                    ))
                                    .weak(),
                                );
                            } else {
                                for &i in &fam_matches {
                                    let fam = &self.chip_families[i];
                                    let selected = Some(i) == sel_family;
                                    if ui
                                        .selectable_label(
                                            selected,
                                            format!("{} ({})", fam.name, fam.chips.len()),
                                        )
                                        .clicked()
                                    {
                                        picked_family = Some(i);
                                    }
                                }
                            }
                        });

                    let fam_index = sel_family;
                    let mut picked_chip: Option<String> = None;
                    egui::ScrollArea::vertical()
                        .id_salt("chip_list")
                        .max_height(300.0)
                        .show(&mut cols[2], |ui| {
                            match fam_index.and_then(|i| self.chip_families.get(i)) {
                                Some(fam) => {
                                    let mut shown = 0;
                                    for name in &fam.chips {
                                        if !filter.is_empty()
                                            && !name.to_lowercase().contains(&filter)
                                        {
                                            continue;
                                        }
                                        let selected = self.manual_target == *name;
                                        if ui.selectable_label(selected, name).clicked() {
                                            picked_chip = Some(name.clone());
                                        }
                                        shown += 1;
                                    }
                                    if shown == 0 {
                                        ui.label(
                                            egui::RichText::new(self.t(
                                                "该系列下无匹配型号",
                                                "No matching variant in this family",
                                            ))
                                            .weak(),
                                        );
                                    }
                                }
                                None => {
                                    ui.label(
                                        egui::RichText::new(self.t(
                                            "请先在左侧选择芯片系列",
                                            "Select a chip family on the left first",
                                        ))
                                        .weak(),
                                    );
                                }
                            }
                        });

                    if let Some((bi, first_fam)) = picked_brand {
                        self.selected_brand = Some(bi);
                        self.selected_family = first_fam;
                        if self.manual_target.is_empty() {
                            if let Some(fam) =
                                first_fam.and_then(|i| self.chip_families.get(i))
                            {
                                if let Some(first) = fam.chips.first() {
                                    self.manual_target = first.clone();
                                }
                            }
                        }
                    }
                    if let Some(i) = picked_family {
                        self.selected_family = Some(i);
                        if self.manual_target.is_empty() {
                            if let Some(fam) = self.chip_families.get(i) {
                                if let Some(first) = fam.chips.first() {
                                    self.manual_target = first.clone();
                                }
                            }
                        }
                    }
                    if let Some(name) = picked_chip {
                        self.manual_target = name;
                    }
                });

                let enabled = !self.probes.is_empty()
                    && !self.connecting
                    && !self.busy
                    && !self.manual_target.trim().is_empty();
                if ui
                    .add_enabled(
                        enabled,
                        egui::Button::new(self.icon(
                            "🔗",
                            "按型号连接",
                            "Connect by Model",
                        ))
                        .fill(egui::Color32::from_rgb(0x1f, 0x6f, 0xc3)),
                    )
                    .clicked()
                {
                    let target = self.manual_target.trim().to_owned();
                    self.connecting = true;
                    self.busy = true;
                    self.log_info(
                        self.lang.pick(
                            format!("正在连接 {} ...", target),
                            format!("Connecting to {} ...", target),
                        ),
                    );
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
                        ui.heading(self.t("目标信息", "Target Info"));
                        ui.label(
                            self.lang.pick(
                                format!("芯片型号: {}", summary.name),
                                format!("Chip: {}", summary.name),
                            ),
                        );
                        ui.label(
                            self.lang.pick(
                                format!("架构: {}", summary.architecture),
                                format!("Architecture: {}", summary.architecture),
                            ),
                        );
                        ui.label(
                            self.lang.pick(
                                format!("核心数量: {}", summary.cores.len()),
                                format!("Cores: {}", summary.cores.len()),
                            ),
                        );
                        for (i, c) in &summary.cores {
                            ui.label(
                                self.lang.pick(
                                    format!("  核心 {}: {}", i, c),
                                    format!("  Core {}: {}", i, c),
                                ),
                            );
                        }
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(self.t("内存映射:", "Memory Map:")).strong());
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
                        ui.label(egui::RichText::new(self.t("尚未连接目标", "Not connected")).weak());
                    }
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(6.0);
            ui.heading(self.t("固件烧录", "Firmware Flashing"));
            ui.separator();

            ui.horizontal(|ui| {
                ui.label(self.t("固件文件:", "Firmware file:"));
                let hint = self.t(
                    "选择 .elf / .hex / .bin / .uf2 文件",
                    "Select .elf / .hex / .bin / .uf2 file",
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.file_path)
                        .desired_width(320.0)
                        .hint_text(hint),
                );
                if ui
                    .button(self.icon(
                        "📂",
                        "浏览...",
                        "Browse...",
                    ))
                    .clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter(self.t("固件镜像", "Firmware image"), &["elf", "hex", "bin", "uf2"])
                        .pick_file()
                    {
                        self.file_path = path.display().to_string();
                        self.log_info(
                            self.lang.pick(
                                format!("已选择固件: {}", self.file_path),
                                format!("Selected firmware: {}", self.file_path),
                            ),
                        );
                    }
                }
                if ui
                    .button(self.icon(
                        "📁",
                        "选择项目文件夹...",
                        "Select Project Folder...",
                    ))
                    .clicked() {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        self.firmware_root = dir.display().to_string();
                        self.firmware_scanning = true;
                        self.firmware_candidates.clear();
                        self.log_info(
                            self.lang.pick(
                                format!(
                                    "正在扫描项目文件夹并自动识别固件: {}",
                                    self.firmware_root
                                ),
                                format!(
                                    "Scanning project folder and auto-detecting firmware: {}",
                                    self.firmware_root
                                ),
                            ),
                        );
                        self.send(WorkerCommand::ScanFirmware { root: dir });
                    }
                }
            });
            if let Some(fmt) = self.detected_format() {
                ui.label(
                    self.lang.pick(
                        format!("文件格式: {}", fmt),
                        format!("File format: {}", fmt),
                    ),
                );
            }

            if self.firmware_scanning {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    ui.label(
                        self.lang.pick(
                            format!("扫描中: {}", self.firmware_root),
                            format!("Scanning: {}", self.firmware_root),
                        ),
                    );
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
                            self.t("未选择项目固件", "No project firmware").to_owned()
                        } else {
                            name
                        }
                    }
                };
                let mut chosen: Option<usize> = None;
                ui.horizontal(|ui| {
                    ui.label(self.t("项目固件:", "Project firmware:"));
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
                        self.log_info(
                            self.lang.pick(
                                format!("已选择固件: {}", self.file_path),
                                format!("Selected firmware: {}", self.file_path),
                            ),
                        );
                    }
                }
            }

            ui.add_space(8.0);
            let l_erase = self.t("全片擦除后烧录", "Chip erase before flash");
            let l_verify = self.t("烧录后校验", "Verify after flash");
            let l_keep = self.t("保留未写入字节", "Keep unwritten bytes");
            let l_reset = self.t("烧录后复位运行", "Reset and run after flash");
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
                        egui::Button::new(self.icon(
                            "⚡",
                            "开始烧录",
                            "Flash",
                        ))
                        .fill(egui::Color32::from_rgb(0x2e, 0xa0, 0x43))
                        .min_size(egui::vec2(130.0, 32.0)),
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
                    .add_enabled(
                        can_erase,
                        egui::Button::new(self.icon(
                            "🗑",
                            "全片擦除",
                            "Erase All",
                        )),
                    )
                    .clicked()
                {
                    self.busy = true;
                    self.op_bars.clear();
                    self.log_info(self.t("开始全片擦除...", "Erasing all flash..."));
                    self.send(WorkerCommand::EraseAll);
                }
                if ui
                    .add_enabled(
                        self.connected.is_some() && !self.busy,
                        egui::Button::new(self.icon(
                            "🔁",
                            "复位目标",
                            "Reset Target",
                        )),
                    )
                    .clicked()
                {
                    self.log_info(self.t("正在复位目标...", "Resetting target..."));
                    self.send(WorkerCommand::Reset);
                }
            });

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
                            let text = format!(
                                "{}  ({}/{} KB)",
                                bar.label,
                                bar.done / 1024,
                                t / 1024
                            );
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
                                ui.add(
                                    egui::ProgressBar::new(frac).fill(color).text(&bar.label),
                                );
                            }
                        }
                    }
                }
            }

            ui.add_space(10.0);
            ui.label(egui::RichText::new(self.t("日志", "Log")).strong());
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
