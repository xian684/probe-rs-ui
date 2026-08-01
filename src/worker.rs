use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::UNIX_EPOCH;

use crate::i18n::Lang;

use probe_rs::config::{MemoryRegion, TargetSelector};
use probe_rs::flashing::{
    erase_all, BinLoader, BinOptions, DownloadOptions, ElfLoader, ElfOptions, FlashProgress,
    HexLoader, ProgressEvent, ProgressOperation, Uf2Loader,
};
use probe_rs::probe::{list::Lister, DebugProbeInfo};
use probe_rs::{Permissions, Session};

/// 一个可展示给界面的探针描述。
#[derive(Clone)]
pub struct ProbeInfo {
    pub identifier: String,
    pub serial_number: Option<String>,
    pub probe_type: String,
    pub index: usize,
}

/// 一段内存区域摘要。
#[derive(Clone)]
pub struct MemRegionInfo {
    pub kind: &'static str,
    pub start: u64,
    pub end: u64,
}

/// 连接成功后展示给界面的目标摘要。
#[derive(Clone)]
pub struct TargetSummary {
    pub name: String,
    pub architecture: String,
    pub cores: Vec<(usize, String)>,
    pub memory: Vec<MemRegionInfo>,
}

/// 在项目文件夹中扫描到的固件候选文件。
#[derive(Clone)]
pub struct FirmwareCandidate {
    pub path: PathBuf,
    pub kind: &'static str,
    pub size_kb: u64,
    pub modified: u64,
}

/// 进度条状态。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OpState {
    Active,
    Done,
    Failed,
}

/// 发送给后台工作线程的命令。
pub enum WorkerCommand {
    Scan,
    ConnectAuto { probe: usize },
    ConnectManual { probe: usize, target: String },
    Flash {
        path: PathBuf,
        do_chip_erase: bool,
        verify: bool,
        keep_unwritten_bytes: bool,
        reset_after: bool,
    },
    EraseAll,
    Reset,
    Disconnect,
    Shutdown,
    ScanFirmware { root: PathBuf },
    SetLang(Lang),
}

/// 后台工作线程回传给界面的事件。
pub enum WorkerEvent {
    Probes(Result<Vec<ProbeInfo>, String>),
    Connected(Result<TargetSummary, String>),
    Status(String),
    Diagnostic(String),
    Progress {
        operation: &'static str,
        done: u64,
        total: Option<u64>,
        state: OpState,
    },
    OperationDone(Result<(), String>),
    FirmwareScanned {
        root: String,
        candidates: Vec<FirmwareCandidate>,
        best: Option<usize>,
    },
}

pub struct Worker {
    pub sender: mpsc::Sender<WorkerCommand>,
    pub receiver: mpsc::Receiver<WorkerEvent>,
}

/// 芯片系列及其下的具体型号（用于三列选择器）。
#[derive(Clone)]
pub struct ChipFamilyInfo {
    pub name: String,
    pub brand: String,
    pub chips: Vec<String>,
}

/// 枚举 probe-rs 内置芯片，按系列分组（按名称排序），并附上制造商品牌。
pub fn builtin_chip_families() -> Vec<ChipFamilyInfo> {
    let registry = probe_rs::config::Registry::from_builtin_families();
    let mut families: Vec<ChipFamilyInfo> = registry
        .families()
        .iter()
        .map(|f| {
            let mut chips: Vec<String> = f.variants.iter().map(|c| c.name.clone()).collect();
            chips.sort();
            chips.dedup();
            ChipFamilyInfo {
                name: f.name.trim_end_matches(" Series").to_owned(),
                brand: family_brand(f),
                chips,
            }
        })
        .collect();
    families.sort_by(|a, b| a.name.cmp(&b.name));
    families
}

/// 品牌及其下的系列在 chip_families 列表中的索引。
#[derive(Clone)]
pub struct ChipBrandInfo {
    pub name: String,
    pub families: Vec<usize>,
}

/// 将系列列表按品牌分组（按品牌名排序，"其他" 排最后）。
pub fn group_brands(families: &[ChipFamilyInfo]) -> Vec<ChipBrandInfo> {
    let mut brands: Vec<ChipBrandInfo> = Vec::new();
    for (i, f) in families.iter().enumerate() {
        match brands.iter_mut().find(|b| b.name == f.brand) {
            Some(b) => b.families.push(i),
            None => brands.push(ChipBrandInfo {
                name: f.brand.clone(),
                families: vec![i],
            }),
        }
    }
    brands.sort_by(|a, b| a.name.cmp(&b.name));
    if let Some(pos) = brands.iter().position(|b| b.name == "Other") {
        let b = brands.remove(pos);
        brands.push(b);
    }
    brands
}

/// 将系列归属到品牌。优先按系列名前缀匹配已知品牌表，
/// 其次推断通用 ARM/RISC-V 目标，最后回退到 JEP106 制造商信息。
fn family_brand(f: &probe_rs::config::ChipFamily) -> String {
    let name = f.name.trim_end_matches(" Series");
    for &(prefix, brand) in BRAND_RULES {
        if name.starts_with(prefix) {
            return brand.to_owned();
        }
    }
    let lower = name.to_lowercase();
    if lower.contains("generic") && lower.contains("arm") {
        return "ARM".to_owned();
    }
    if lower.contains("generic") && (lower.contains("risc-v") || lower.contains("riscv")) {
        return "RISC-V".to_owned();
    }
    if let Some(code) = f.manufacturer {
        if let Some(raw) = code.get() {
            return normalize_brand(raw);
        }
    }
    "Other".to_owned()
}

/// 系列名前缀 -> 品牌。必须按前缀长度从长到短排列。
const BRAND_RULES: &[(&str, &str)] = &[
    ("Generic RISC-V", "RISC-V"),
    ("Generic ARMv", "ARM"),
    ("MIMXRT", "NXP"),
    ("Raspberry", "Raspberry Pi"),
    ("Microchip", "Microchip"),
    ("OpenTitan", "lowRISC"),
    ("Trident", "Trident IoT"),
    ("Nuclei", "Nuclei"),
    ("STM32", "ST"),
    ("ADuCM", "Analog Devices"),
    ("MSP432", "TI"),
    ("MSPM0", "TI"),
    ("MAX326", "Maxim"),
    ("MAX780", "Maxim"),
    ("MAX326", "Maxim"),
    ("MAX32", "Maxim"),
    ("MAX78", "Maxim"),
    ("MAX7", "Maxim"),
    ("EFM32", "Silicon Labs"),
    ("EFR32", "Silicon Labs"),
    ("EFM8", "Silicon Labs"),
    ("EFM", "Silicon Labs"),
    ("GD32", "GigaDevice"),
    ("AT32", "Artery"),
    ("PIC32", "Microchip"),
    ("PIC24", "Microchip"),
    ("dsPIC", "Microchip"),
    ("ATSAM", "Microchip"),
    ("ATmega", "Microchip"),
    ("ATtiny", "Microchip"),
    ("AT90", "Microchip"),
    ("SAM", "Microchip"),
    ("MSP", "TI"),
    ("nRF", "Nordic"),
    ("LPC", "NXP"),
    ("MCX", "NXP"),
    ("OL23", "NXP"),
    ("S32K", "NXP"),
    ("iMX", "NXP"),
    ("TMS570", "TI"),
    ("CC13", "TI"),
    ("CC23", "TI"),
    ("LM3S", "TI"),
    ("Tiva", "TI"),
    ("AM2", "TI"),
    ("RA", "Renesas"),
    ("XMC", "Infineon"),
    ("FM3", "Infineon"),
    ("PSC3", "Infineon"),
    ("PSOC", "Infineon"),
    ("psoc", "Infineon"),
    ("TLE", "Infineon"),
    ("CY8", "Infineon"),
    ("HT32", "Holtek"),
    ("HT50", "Holtek"),
    ("HF", "Holtek"),
    ("HK32", "Hangshun"),
    ("HC32", "HDSC"),
    ("CH32", "WCH"),
    ("CH6", "WCH"),
    ("CW32", "Xinyuan"),
    ("AIR", "AirM2M"),
    ("HPM", "HPMicro"),
    ("SF32", "Siflower"),
    ("W75", "WIZnet"),
    ("Zynq", "AMD"),
    ("VA1", "Silicon Space"),
    ("VA4", "Silicon Space"),
    ("synwit", "Synwit"),
    ("SiFive", "SiFive"),
    ("fe3", "SiFive"),
    ("PAC5", "Qorvo"),
    ("PY32", "Puya"),
    ("ESP32", "Espressif"),
    ("ESP", "Espressif"),
    ("RP2", "Raspberry Pi"),
    ("ARM", "ARM"),
    ("RISC-V", "RISC-V"),
];

/// 将 JEP106 官方厂商名缩写为常用品牌名。
fn normalize_brand(raw: &str) -> String {
    match raw.trim() {
        "STMicroelectronics" => "ST",
        "Nordic VLSI ASA" => "Nordic",
        "Espressif Systems" => "Espressif",
        "NXP Semiconductors" => "NXP",
        "Microchip Technology Inc" => "Microchip",
        "Atmel Corporation" => "Microchip",
        "Renesas Technology Corp" => "Renesas",
        "Silicon Laboratories" => "Silicon Labs",
        "Texas Instruments" => "TI",
        "Infineon Technologies" => "Infineon",
        "Cypress Semiconductor" => "Infineon",
        "Dialog Semiconductor" => "Dialog",
        "Analog Devices Inc" => "Analog Devices",
        "Maxim Integrated Products" => "Maxim",
        "Nuvoton Technology Corp" => "Nuvoton",
        "GigaDevice Semiconductor" => "GigaDevice",
        "ARM Ltd" => "ARM",
        other => other,
    }
    .to_owned()
}

pub fn spawn(lang: Lang) -> Worker {
    let (tx, rx) = mpsc::channel::<WorkerCommand>();
    let (etx, erx) = mpsc::channel::<WorkerEvent>();
    std::thread::Builder::new()
        .name("probe-rs-worker".to_owned())
        .spawn(move || run(rx, etx, lang))
        .expect("无法创建后台工作线程");
    Worker {
        sender: tx,
        receiver: erx,
    }
}

fn run(rx: mpsc::Receiver<WorkerCommand>, events: mpsc::Sender<WorkerEvent>, mut lang: Lang) {
    let mut session: Option<Session> = None;
    let mut probes: Vec<DebugProbeInfo> = Vec::new();

    while let Ok(cmd) = rx.recv() {
        match cmd {
            WorkerCommand::Scan => match scan() {
                Ok(list) => {
                    probes = list.clone();
                    let display = list
                        .into_iter()
                        .enumerate()
                        .map(|(i, p)| ProbeInfo {
                            identifier: p.identifier.clone(),
                            serial_number: p.serial_number.clone(),
                            probe_type: format!("{:?}", p.probe_type()),
                            index: i,
                        })
                        .collect();
                    let _ = events.send(WorkerEvent::Probes(Ok(display)));
                }
                Err(e) => {
                    let _ = events.send(WorkerEvent::Probes(Err(e)));
                }
            },
            WorkerCommand::ConnectAuto { probe } => match connect(&probes, probe, None, lang) {
                Ok((s, summary)) => {
                    session = Some(s);
                    let _ = events.send(WorkerEvent::Connected(Ok(summary)));
                }
                Err(e) => {
                    let _ = events.send(WorkerEvent::Connected(Err(e)));
                }
            },
            WorkerCommand::ConnectManual { probe, target } => {
                match connect(&probes, probe, Some(target), lang) {
                    Ok((s, summary)) => {
                        session = Some(s);
                        let _ = events.send(WorkerEvent::Connected(Ok(summary)));
                    }
                    Err(e) => {
                        let _ = events.send(WorkerEvent::Connected(Err(e)));
                    }
                }
            }
            WorkerCommand::Flash {
                path,
                do_chip_erase,
                verify,
                keep_unwritten_bytes,
                reset_after,
            } => {
                let result = match &mut session {
                    Some(sess) => flash(
                        sess,
                        &path,
                        do_chip_erase,
                        verify,
                        keep_unwritten_bytes,
                        &events,
                        lang,
                    ),
                    None => Err(lang.pick(
                        "尚未连接到目标芯片，请先自动识别目标".to_owned(),
                        "Not connected to a target. Auto-detect or select the target first"
                            .to_owned(),
                    )),
                };
                match result {
                    Ok(()) => {
                        if reset_after {
                            let _ = reset(session.as_mut().expect("session 必须存在"), lang);
                        }
                        let _ = events.send(WorkerEvent::OperationDone(Ok(())));
                    }
                    Err(e) => {
                        let _ = events.send(WorkerEvent::OperationDone(Err(e)));
                    }
                }
            }
            WorkerCommand::EraseAll => {
                let result = match &mut session {
                    Some(sess) => erase_flash(sess, &events, lang),
                    None => Err(lang.pick(
                        "尚未连接到目标芯片，请先自动识别目标".to_owned(),
                        "Not connected to a target. Auto-detect or select the target first"
                            .to_owned(),
                    )),
                };
                let _ = events.send(WorkerEvent::OperationDone(result));
            }
            WorkerCommand::Reset => {
                let result = match &mut session {
                    Some(sess) => reset(sess, lang),
                    None => Err(lang.pick(
                        "尚未连接到目标芯片".to_owned(),
                        "Not connected to a target".to_owned(),
                    )),
                };
                let _ = events.send(WorkerEvent::OperationDone(result));
            }
            WorkerCommand::Disconnect => {
                session = None;
                let _ = events.send(WorkerEvent::Status(lang.pick(
                    "已断开连接".to_owned(),
                    "Disconnected".to_owned(),
                )));
            }
            WorkerCommand::ScanFirmware { root } => {
                let (candidates, best) = scan_firmware(&root);
                let _ = events.send(WorkerEvent::FirmwareScanned {
                    root: root.display().to_string(),
                    candidates,
                    best,
                });
            }
            WorkerCommand::SetLang(l) => lang = l,
            WorkerCommand::Shutdown => break,
        }
    }
}

fn scan() -> Result<Vec<DebugProbeInfo>, String> {
    let lister = Lister::new();
    let probes = lister.list_all();
    Ok(probes)
}

/// 在项目文件夹中递归查找编译产物（.elf/.hex/.bin/.uf2），按可烧录性排序返回。
pub fn scan_firmware(root: &Path) -> (Vec<FirmwareCandidate>, Option<usize>) {
    let mut candidates: Vec<FirmwareCandidate> = Vec::new();
    let mut stack: Vec<(PathBuf, usize, bool)> = vec![(root.to_path_buf(), 0, false)];
    while let Some((dir, depth, in_target)) = stack.pop() {
        if depth > 10 {
            continue;
        }
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let ft = match entry.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };
            if ft.is_dir() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if is_ignored_dir(&name, in_target) {
                    continue;
                }
                let child_in_target = in_target || name == "target";
                stack.push((path, depth + 1, child_in_target));
            } else if ft.is_file() {
                if let Some(kind) = firmware_kind(&path) {
                    if let Ok(meta) = entry.metadata() {
                        if meta.len() == 0 {
                            continue;
                        }
                        let modified = meta
                            .modified()
                            .ok()
                            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        candidates.push(FirmwareCandidate {
                            size_kb: meta.len() / 1024,
                            modified,
                            path,
                            kind,
                        });
                    }
                }
            }
        }
    }
    candidates.sort_by(|a, b| {
        fw_score(b)
            .cmp(&fw_score(a))
            .then(b.modified.cmp(&a.modified))
            .then(a.path.cmp(&b.path))
    });
    let best = if candidates.is_empty() { None } else { Some(0) };
    (candidates, best)
}

fn is_ignored_dir(name: &str, in_target: bool) -> bool {
    if name.starts_with('.') {
        return true;
    }
    match name {
        "node_modules" | "tmp" | "doc" | "deps" | "incremental" | "examples" | ".fingerprint"
        | "package" | "crates" => true,
        "build" => in_target,
        _ => false,
    }
}

fn firmware_kind(path: &Path) -> Option<&'static str> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase());
    match ext.as_deref() {
        Some("elf") | Some("axf") => Some("ELF"),
        Some("hex") => Some("HEX"),
        Some("bin") => Some("BIN"),
        Some("uf2") => Some("UF2"),
        _ => {
            // Rust 编译产物通常没有扩展名，但仍是 ELF 文件：按魔数识别。
            if ext.is_none() && is_elf(path) {
                Some("ELF")
            } else {
                None
            }
        }
    }
}

/// 判断文件是否为 ELF 二进制（通过魔数 0x7F 'E' 'L' 'F' 识别）。
pub fn is_elf(path: &Path) -> bool {
    use std::io::Read;
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut magic = [0u8; 4];
    f.read_exact(&mut magic).is_ok() && magic == [0x7F, b'E', b'L', b'F']
}

/// 依据文件类型与所在目录对候选固件打分，得分高者优先。
fn fw_score(c: &FirmwareCandidate) -> i64 {
    let mut s: i64 = match c.kind {
        "ELF" | "HEX" => 4,
        "BIN" => 2,
        "UF2" => 1,
        _ => 0,
    };
    let p = c.path.to_string_lossy().to_lowercase();
    if p.contains("\\release\\") {
        s += 20;
    } else if p.contains("\\debug\\") {
        s += 15;
    }
    if p.contains("\\build\\") || p.contains("\\out\\") || p.contains("\\output\\") {
        s += 8;
    }
    if p.contains("\\objects\\") {
        s += 6;
    }
    if p.contains("\\bin\\") {
        s += 3;
    }
    s
}

fn connect(
    probes: &[DebugProbeInfo],
    index: usize,
    target: Option<String>,
    lang: Lang,
) -> Result<(Session, TargetSummary), String> {
    let info = probes
        .get(index)
        .cloned()
        .ok_or_else(|| {
            lang.pick(
                format!("未找到编号为 {index} 的调试探针"),
                format!("No debug probe with index {index} found"),
            )
        })?;

    let permissions = Permissions::new().allow_erase_all();

    let open_err = |e| {
        lang.pick(
            format!("打开探针失败: {e}"),
            format!("Failed to open probe: {e}"),
        )
    };

    let session = match target {
        Some(name) => {
            let probe = info.open().map_err(open_err)?;
            probe
                .attach(TargetSelector::Unspecified(name.clone()), permissions)
                .map_err(|e| {
                    lang.pick(
                        format!("连接目标 {} 失败: {e}", name),
                        format!("Failed to connect to target {}: {e}", name),
                    )
                })?
        }
        None => {
            let probe = info.open().map_err(open_err)?;
            match probe.attach(TargetSelector::Auto, permissions.clone()) {
                Ok(s) => s,
                Err(first) => {
                    let probe2 = info.open().map_err(open_err)?;
                    match probe2.attach_under_reset(TargetSelector::Auto, permissions) {
                        Ok(s) => s,
                        Err(_) => {
                            return Err(lang.pick(
                                format!(
                                    "自动识别目标失败: {first}。该探针可能不支持自动识别芯片（如 DAPLink/CMSIS-DAP），请在左侧『手动指定目标芯片』中搜索并选择芯片型号后重试"
                                ),
                                format!(
                                    "Auto-detection failed: {first}. The probe may not support auto-identification (e.g. DAPLink/CMSIS-DAP). Please search and select the chip model under 'Manual Target Selection' on the left and retry"
                                ),
                            ))
                        }
                    }
                }
            }
        }
    };

    let summary = {
        let target = session.target();
        let name = target.name.clone();
        let architecture = format!("{:?}", session.architecture());
        let cores = session
            .list_cores()
            .into_iter()
            .map(|(i, t)| (i, format!("{t:?}")))
            .collect();
        let memory = target
            .memory_map
            .iter()
            .map(region_info)
            .collect::<Vec<_>>();
        TargetSummary {
            name,
            architecture,
            cores,
            memory,
        }
    };

    Ok((session, summary))
}

fn region_info(r: &MemoryRegion) -> MemRegionInfo {
    match r {
        MemoryRegion::Ram(ram) => MemRegionInfo {
            kind: "RAM",
            start: ram.range.start,
            end: ram.range.end,
        },
        MemoryRegion::Nvm(nvm) => MemRegionInfo {
            kind: "FLASH",
            start: nvm.range.start,
            end: nvm.range.end,
        },
        MemoryRegion::Generic(g) => MemRegionInfo {
            kind: "Generic",
            start: g.range.start,
            end: g.range.end,
        },
    }
}

fn flash(
    session: &mut Session,
    path: &Path,
    do_chip_erase: bool,
    verify: bool,
    keep_unwritten_bytes: bool,
    events: &mpsc::Sender<WorkerEvent>,
    lang: Lang,
) -> Result<(), String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let events2 = events.clone();
    let progress = FlashProgress::new(move |event: ProgressEvent| {
        if let Some(ev) = map_progress(event, lang) {
            let _ = events2.send(ev);
        }
    });

    let mut options = DownloadOptions::new();
    options.progress = progress;
    options.do_chip_erase = do_chip_erase;
    options.verify = verify;
    options.keep_unwritten_bytes = keep_unwritten_bytes;

    let is_elf_file = matches!(ext.as_str(), "elf" | "axf") || (ext.is_empty() && is_elf(path));

    let result = if is_elf_file {
        probe_rs::flashing::download_file_with_options(
            session,
            path,
            ElfLoader(ElfOptions::default()),
            options,
        )
    } else if ext == "hex" {
        probe_rs::flashing::download_file_with_options(session, path, HexLoader, options)
    } else if ext == "bin" {
        probe_rs::flashing::download_file_with_options(
            session,
            path,
            BinLoader(BinOptions::default()),
            options,
        )
    } else if ext == "uf2" {
        probe_rs::flashing::download_file_with_options(session, path, Uf2Loader, options)
    } else {
        return Err(lang.pick(
            format!(
                "不支持的文件格式: .{ext}，请选择 .elf / .hex / .bin / .uf2 文件"
            ),
            format!(
                "Unsupported file format: .{ext}. Choose a .elf / .hex / .bin / .uf2 file"
            ),
        ));
    };

    result.map_err(|e| {
        lang.pick(
            format!("烧录失败: {e}"),
            format!("Flashing failed: {e}"),
        )
    })
}

fn erase_flash(
    session: &mut Session,
    events: &mpsc::Sender<WorkerEvent>,
    lang: Lang,
) -> Result<(), String> {
    let events2 = events.clone();
    let mut progress = FlashProgress::new(move |event: ProgressEvent| {
        if let Some(ev) = map_progress(event, lang) {
            let _ = events2.send(ev);
        }
    });
    erase_all(session, &mut progress, false).map_err(|e| {
        lang.pick(
            format!("全片擦除失败: {e}"),
            format!("Chip erase failed: {e}"),
        )
    })
}

fn reset(session: &mut Session, lang: Lang) -> Result<(), String> {
    let mut core = session
        .core(0)
        .map_err(|e| lang.pick(format!("获取核心失败: {e}"), format!("Failed to get core: {e}")))?;
    core.reset().map_err(|e| {
        lang.pick(
            format!("复位失败: {e}"),
            format!("Reset failed: {e}"),
        )
    })
}

fn map_progress(event: ProgressEvent, lang: Lang) -> Option<WorkerEvent> {
    match event {
        ProgressEvent::FlashLayoutReady { .. } => Some(WorkerEvent::Status(lang.pick(
            "已解析固件布局，准备烧录...".to_owned(),
            "Firmware layout parsed, ready to flash...".to_owned(),
        ))),
        ProgressEvent::AddProgressBar { operation, total } => Some(WorkerEvent::Progress {
            operation: op_label(operation, lang),
            done: 0,
            total,
            state: OpState::Active,
        }),
        ProgressEvent::Started(operation) => Some(WorkerEvent::Status(lang.pick(
            format!("开始{}...", op_label(operation, lang)),
            format!("Starting {}...", op_label(operation, lang)),
        ))),
        ProgressEvent::Progress {
            operation, size, ..
        } => Some(WorkerEvent::Progress {
            operation: op_label(operation, lang),
            done: size,
            total: None,
            state: OpState::Active,
        }),
        ProgressEvent::Failed(operation) => Some(WorkerEvent::Progress {
            operation: op_label(operation, lang),
            done: 0,
            total: None,
            state: OpState::Failed,
        }),
        ProgressEvent::Finished(operation) => Some(WorkerEvent::Progress {
            operation: op_label(operation, lang),
            done: 0,
            total: None,
            state: OpState::Done,
        }),
        ProgressEvent::DiagnosticMessage { message } => Some(WorkerEvent::Diagnostic(message)),
    }
}

fn op_label(op: ProgressOperation, lang: Lang) -> &'static str {
    match op {
        ProgressOperation::Erase => lang.pick("擦除", "Erase"),
        ProgressOperation::Program => lang.pick("编程", "Program"),
        ProgressOperation::Verify => lang.pick("校验", "Verify"),
        ProgressOperation::Fill => lang.pick("填充", "Fill"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(path: &Path, bytes: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn scan_prefers_release_elf_over_debug_and_noise() {
        let root = std::env::temp_dir().join("probe-rs-ui-test-scan");
        let _ = std::fs::remove_dir_all(&root);
        write(&root.join("target/debug/myapp.elf"), &[0; 4096]);
        write(&root.join("target/release/myapp.elf"), &[0; 8192]);
        write(&root.join("target/debug/deps/dep.elf"), &[0; 4096]);
        write(&root.join("target/debug/build/probe.elf"), &[0; 4096]);
        write(&root.join("src/main.c"), &[]);
        write(&root.join("Objects/app.hex"), &[0; 2048]);

        let (cands, best) = scan_firmware(&root);
        assert_eq!(best, Some(0));
        let first = cands.first().unwrap().path.to_string_lossy().to_lowercase();
        assert!(
            first.contains("release") && first.ends_with("myapp.elf"),
            "expected release myapp.elf, got {first}"
        );
        assert!(
            cands.iter().all(|c| !c.path.to_string_lossy().contains("deps")),
            "deps dir must be skipped"
        );
        assert!(
            cands
                .iter()
                .all(|c| !c.path.to_string_lossy().to_lowercase().contains("\\build\\")),
            "cargo build dir must be skipped"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_finds_cmake_build_outputs() {
        let root = std::env::temp_dir().join("probe-rs-ui-test-cmake");
        let _ = std::fs::remove_dir_all(&root);
        write(&root.join("build/Debug/firmware.elf"), &[0; 4096]);
        write(&root.join("build/Release/firmware.elf"), &[0; 8192]);

        let (cands, best) = scan_firmware(&root);
        assert_eq!(best, Some(0));
        let first = cands.first().unwrap().path.to_string_lossy().to_lowercase();
        assert!(first.contains("release"), "expected release build, got {first}");
        assert_eq!(cands.len(), 2, "build dir must be scanned outside cargo target");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_detects_extensionless_elf() {
        let root = std::env::temp_dir().join("probe-rs-ui-test-elf");
        let _ = std::fs::remove_dir_all(&root);
        let mut bytes = vec![0x7F, b'E', b'L', b'F'];
        bytes.extend_from_slice(&[0u8; 4096]);
        write(
            &root.join("target/thumbv7em-none-eabihf/release/myapp"),
            &bytes,
        );
        write(&root.join("target/thumbv7em-none-eabihf/debug/myapp"), &bytes);
        write(&root.join("src/main.rs"), &[]);

        let (cands, best) = scan_firmware(&root);
        assert_eq!(best, Some(0));
        assert!(!cands.is_empty(), "extensionless ELF must be detected");
        let first = cands.first().unwrap();
        assert_eq!(first.kind, "ELF");
        let p = first.path.to_string_lossy().to_lowercase();
        assert!(
            p.contains("release") && p.ends_with("myapp"),
            "expected release extensionless ELF, got {p}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_empty_folder_returns_none() {
        let root = std::env::temp_dir().join("probe-rs-ui-test-empty");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let (cands, best) = scan_firmware(&root);
        assert!(cands.is_empty());
        assert_eq!(best, None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn brand_grouping_covers_all_families() {
        let families = builtin_chip_families();
        let brands = group_brands(&families);
        let total: usize = brands.iter().map(|b| b.families.len()).sum();
        assert_eq!(total, families.len(), "every family must be assigned to a brand");

        let mut others: Vec<&str> = brands
            .iter()
            .filter(|b| b.name == "Other")
            .flat_map(|b| b.families.iter())
            .map(|&i| families[i].name.as_str())
            .collect();
        others.sort();
        assert_eq!(others, vec!["CIU32F0"], "unexpected unknown brand(s)");

        let brand_of = |name: &str| {
            families
                .iter()
                .position(|f| f.name == name)
                .and_then(|i| {
                    brands.iter().find(|b| b.families.contains(&i))
                })
                .map(|b| b.name.as_str())
        };
        assert_eq!(brand_of("STM32F1"), Some("ST"));
        assert_eq!(brand_of("nRF52"), Some("Nordic"));
        assert_eq!(brand_of("RP235x"), Some("Raspberry Pi"));
        assert_eq!(brand_of("MAX32660"), Some("Maxim"));
        assert_eq!(brand_of("psoc6_01"), Some("Infineon"));
        assert_eq!(brand_of("GD32F1x0"), Some("GigaDevice"));
        assert_eq!(brand_of("SAM3U"), Some("Microchip"));
        assert_eq!(brand_of("Generic ARMv8-M"), Some("ARM"));
    }
}
