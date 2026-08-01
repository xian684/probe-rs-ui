use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::UNIX_EPOCH;

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

/// 芯片系列及其下的具体型号（用于双列选择器）。
#[derive(Clone)]
pub struct ChipFamilyInfo {
    pub name: String,
    pub chips: Vec<String>,
}

/// 枚举 probe-rs 内置芯片，按系列分组（按名称排序）。
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
                chips,
            }
        })
        .collect();
    families.sort_by(|a, b| a.name.cmp(&b.name));
    families
}

pub fn spawn() -> Worker {
    let (tx, rx) = mpsc::channel::<WorkerCommand>();
    let (etx, erx) = mpsc::channel::<WorkerEvent>();
    std::thread::Builder::new()
        .name("probe-rs-worker".to_owned())
        .spawn(move || run(rx, etx))
        .expect("无法创建后台工作线程");
    Worker {
        sender: tx,
        receiver: erx,
    }
}

fn run(rx: mpsc::Receiver<WorkerCommand>, events: mpsc::Sender<WorkerEvent>) {
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
            WorkerCommand::ConnectAuto { probe } => match connect(&probes, probe, None) {
                Ok((s, summary)) => {
                    session = Some(s);
                    let _ = events.send(WorkerEvent::Connected(Ok(summary)));
                }
                Err(e) => {
                    let _ = events.send(WorkerEvent::Connected(Err(e)));
                }
            },
            WorkerCommand::ConnectManual { probe, target } => {
                match connect(&probes, probe, Some(target)) {
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
                    ),
                    None => Err("尚未连接到目标芯片，请先自动识别目标".to_owned()),
                };
                match result {
                    Ok(()) => {
                        if reset_after {
                            let _ = reset(session.as_mut().expect("session 必须存在"));
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
                    Some(sess) => erase_flash(sess, &events),
                    None => Err("尚未连接到目标芯片，请先自动识别目标".to_owned()),
                };
                let _ = events.send(WorkerEvent::OperationDone(result));
            }
            WorkerCommand::Reset => {
                let result = match &mut session {
                    Some(sess) => reset(sess),
                    None => Err("尚未连接到目标芯片".to_owned()),
                };
                let _ = events.send(WorkerEvent::OperationDone(result));
            }
            WorkerCommand::Disconnect => {
                session = None;
                let _ = events.send(WorkerEvent::Status("已断开连接".to_owned()));
            }
            WorkerCommand::ScanFirmware { root } => {
                let (candidates, best) = scan_firmware(&root);
                let _ = events.send(WorkerEvent::FirmwareScanned {
                    root: root.display().to_string(),
                    candidates,
                    best,
                });
            }
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
    let ext = path.extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "elf" | "axf" => Some("ELF"),
        "hex" => Some("HEX"),
        "bin" => Some("BIN"),
        "uf2" => Some("UF2"),
        _ => None,
    }
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
) -> Result<(Session, TargetSummary), String> {
    let info = probes
        .get(index)
        .cloned()
        .ok_or_else(|| format!("未找到编号为 {index} 的调试探针"))?;

    let permissions = Permissions::new().allow_erase_all();

    let session = match target {
        Some(name) => {
            let probe = info
                .open()
                .map_err(|e| format!("打开探针失败: {e}"))?;
            probe
                .attach(TargetSelector::Unspecified(name.clone()), permissions)
                .map_err(|e| format!("连接目标 {} 失败: {e}", name))?
        }
        None => {
            let probe = info
                .open()
                .map_err(|e| format!("打开探针失败: {e}"))?;
            match probe.attach(TargetSelector::Auto, permissions.clone()) {
                Ok(s) => s,
                Err(first) => {
                    let probe2 = info
                        .open()
                        .map_err(|e| format!("重新打开探针失败: {e}"))?;
                    match probe2.attach_under_reset(TargetSelector::Auto, permissions) {
                        Ok(s) => s,
                        Err(_) => {
                            return Err(format!(
                                "自动识别目标失败: {first}。该探针可能不支持自动识别芯片（如 DAPLink/CMSIS-DAP），请在左侧『手动指定目标芯片』中搜索并选择芯片型号后重试"
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
) -> Result<(), String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let events2 = events.clone();
    let progress = FlashProgress::new(move |event: ProgressEvent| {
        if let Some(ev) = map_progress(event) {
            let _ = events2.send(ev);
        }
    });

    let mut options = DownloadOptions::new();
    options.progress = progress;
    options.do_chip_erase = do_chip_erase;
    options.verify = verify;
    options.keep_unwritten_bytes = keep_unwritten_bytes;

    let result = match ext.as_str() {
        "elf" | "axf" => {
            probe_rs::flashing::download_file_with_options(
                session,
                path,
                ElfLoader(ElfOptions::default()),
                options,
            )
        }
        "hex" => {
            probe_rs::flashing::download_file_with_options(session, path, HexLoader, options)
        }
        "bin" => {
            probe_rs::flashing::download_file_with_options(
                session,
                path,
                BinLoader(BinOptions::default()),
                options,
            )
        }
        "uf2" => {
            probe_rs::flashing::download_file_with_options(session, path, Uf2Loader, options)
        }
        _ => {
            return Err(format!(
                "不支持的文件格式: .{ext}，请选择 .elf / .hex / .bin / .uf2 文件"
            ))
        }
    };

    result.map_err(|e| format!("烧录失败: {e}"))
}

fn erase_flash(
    session: &mut Session,
    events: &mpsc::Sender<WorkerEvent>,
) -> Result<(), String> {
    let events2 = events.clone();
    let mut progress = FlashProgress::new(move |event: ProgressEvent| {
        if let Some(ev) = map_progress(event) {
            let _ = events2.send(ev);
        }
    });
    erase_all(session, &mut progress, false).map_err(|e| format!("全片擦除失败: {e}"))
}

fn reset(session: &mut Session) -> Result<(), String> {
    let mut core = session.core(0).map_err(|e| format!("获取核心失败: {e}"))?;
    core.reset().map_err(|e| format!("复位失败: {e}"))
}

fn map_progress(event: ProgressEvent) -> Option<WorkerEvent> {
    match event {
        ProgressEvent::FlashLayoutReady { .. } => {
            Some(WorkerEvent::Status("已解析固件布局，准备烧录...".to_owned()))
        }
        ProgressEvent::AddProgressBar { operation, total } => Some(WorkerEvent::Progress {
            operation: op_label(operation),
            done: 0,
            total,
            state: OpState::Active,
        }),
        ProgressEvent::Started(operation) => Some(WorkerEvent::Status(format!(
            "开始{}...",
            op_label(operation)
        ))),
        ProgressEvent::Progress {
            operation, size, ..
        } => Some(WorkerEvent::Progress {
            operation: op_label(operation),
            done: size,
            total: None,
            state: OpState::Active,
        }),
        ProgressEvent::Failed(operation) => Some(WorkerEvent::Progress {
            operation: op_label(operation),
            done: 0,
            total: None,
            state: OpState::Failed,
        }),
        ProgressEvent::Finished(operation) => Some(WorkerEvent::Progress {
            operation: op_label(operation),
            done: 0,
            total: None,
            state: OpState::Done,
        }),
        ProgressEvent::DiagnosticMessage { message } => Some(WorkerEvent::Diagnostic(message)),
    }
}

fn op_label(op: ProgressOperation) -> &'static str {
    match op {
        ProgressOperation::Erase => "擦除",
        ProgressOperation::Program => "编程",
        ProgressOperation::Verify => "校验",
        ProgressOperation::Fill => "填充",
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
    fn scan_empty_folder_returns_none() {
        let root = std::env::temp_dir().join("probe-rs-ui-test-empty");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let (cands, best) = scan_firmware(&root);
        assert!(cands.is_empty());
        assert_eq!(best, None);
        let _ = std::fs::remove_dir_all(&root);
    }
}
