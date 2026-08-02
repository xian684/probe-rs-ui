use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

use crate::firmware::{is_elf, scan_firmware, FirmwareCandidate};
use crate::i18n::Lang;

use probe_rs::config::{MemoryRegion, TargetSelector};
use probe_rs::flashing::{
    erase_all, BinLoader, BinOptions, DownloadOptions, ElfLoader, ElfOptions, FlashProgress,
    HexLoader, ProgressEvent, ProgressOperation, Uf2Loader,
};
use probe_rs::probe::{list::Lister, DebugProbeInfo};
use probe_rs::{Permissions, Session};

use crate::rtt;

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

/// 进度条状态。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OpState {
    Active,
    Done,
    Failed,
}

/// 连接方式（对应 STM32 BOOT0/BOOT1 启动配置场景）。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BootMode {
    /// 正常连接，直接从主 Flash 启动（BOOT0 = 0）。
    Normal,
    /// 复位期间连接，通过探针保持目标复位直至协议初始化完成，
    /// 常用于目标程序干扰 SWD 或 BOOT0 拉高从系统存储器启动的场景。
    UnderReset,
}

/// 发送给后台工作线程的命令。
pub enum WorkerCommand {
    Scan,
    ConnectAuto {
        probe: usize,
        boot_mode: BootMode,
    },
    ConnectManual {
        probe: usize,
        target: String,
        boot_mode: BootMode,
    },
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
    ScanFirmware {
        root: PathBuf,
    },
    SetLang(Lang),
    RttStart,
    RttStop,
    RttWrite {
        data: Vec<u8>,
    },
}

/// RTT 轮询间隔。
const RTT_POLL_INTERVAL: Duration = Duration::from_millis(10);

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
    RttData {
        channel: usize,
        data: Vec<u8>,
    },
    RttStarted {
        up_channels: usize,
        down_channels: usize,
    },
    RttStopped,
}

pub struct Worker {
    pub sender: mpsc::Sender<WorkerCommand>,
    pub receiver: mpsc::Receiver<WorkerEvent>,
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
    let mut rtt: Option<rtt::Handle> = None;

    loop {
        match rx.recv_timeout(RTT_POLL_INTERVAL) {
            Err(RecvTimeoutError::Timeout) => {
                if rtt.is_some() {
                    rtt::poll(&mut rtt, &mut session, &events, lang);
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
            Ok(cmd) => match cmd {
                WorkerCommand::Shutdown => break,
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
                WorkerCommand::ConnectAuto { probe, boot_mode } => {
                    rtt::stop(
                        &mut rtt,
                        &events,
                        lang,
                        "重新连接前已停止 RTT",
                        "RTT stopped before reconnecting",
                    );
                    match connect(&probes, probe, None, boot_mode, lang) {
                        Ok((s, summary)) => {
                            session = Some(s);
                            let _ = events.send(WorkerEvent::Connected(Ok(summary)));
                        }
                        Err(e) => {
                            let _ = events.send(WorkerEvent::Connected(Err(e)));
                        }
                    }
                }
                WorkerCommand::ConnectManual {
                    probe,
                    target,
                    boot_mode,
                } => {
                    rtt::stop(
                        &mut rtt,
                        &events,
                        lang,
                        "重新连接前已停止 RTT",
                        "RTT stopped before reconnecting",
                    );
                    match connect(&probes, probe, Some(target), boot_mode, lang) {
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
                    rtt::stop(
                        &mut rtt,
                        &events,
                        lang,
                        "烧录期间已停止 RTT",
                        "RTT stopped during flashing",
                    );
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
                    let result = result.and_then(|()| {
                        if reset_after {
                            reset(
                                session.as_mut().expect("session must exist after flash"),
                                lang,
                            )?;
                        }
                        Ok(())
                    });
                    let _ = events.send(WorkerEvent::OperationDone(result));
                }
                WorkerCommand::EraseAll => {
                    rtt::stop(
                        &mut rtt,
                        &events,
                        lang,
                        "擦除期间已停止 RTT",
                        "RTT stopped during erase",
                    );
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
                    rtt::stop(&mut rtt, &events, lang, "", "");
                    session = None;
                    let _ = events.send(WorkerEvent::Status(
                        lang.pick("已断开连接".to_owned(), "Disconnected".to_owned()),
                    ));
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
                WorkerCommand::RttStart => {
                    rtt = rtt::start(&mut session, &events, lang);
                }
                WorkerCommand::RttStop => {
                    rtt::stop(&mut rtt, &events, lang, "RTT 已停止", "RTT stopped");
                }
                WorkerCommand::RttWrite { data } => {
                    rtt::write(&mut rtt, &mut session, &data, &events, lang);
                }
            },
        }
    }
}

fn scan() -> Result<Vec<DebugProbeInfo>, String> {
    let lister = Lister::new();
    let probes = lister.list_all();
    Ok(probes)
}

fn connect(
    probes: &[DebugProbeInfo],
    index: usize,
    target: Option<String>,
    boot_mode: BootMode,
    lang: Lang,
) -> Result<(Session, TargetSummary), String> {
    let info = probes.get(index).cloned().ok_or_else(|| {
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

    let attach_err = |e: probe_rs::Error, name: &str| {
        lang.pick(
            format!("连接目标 {name} 失败: {e}"),
            format!("Failed to connect to target {name}: {e}"),
        )
    };

    let session = match (target, boot_mode) {
        (Some(name), BootMode::Normal) => {
            let probe = info.open().map_err(open_err)?;
            probe
                .attach(TargetSelector::Unspecified(name.clone()), permissions)
                .map_err(|e| attach_err(e, &name))?
        }
        (Some(name), BootMode::UnderReset) => {
            let probe = info.open().map_err(open_err)?;
            probe
                .attach_under_reset(TargetSelector::Unspecified(name.clone()), permissions)
                .map_err(|e| attach_err(e, &name))?
        }
        (None, BootMode::Normal) => {
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
        (None, BootMode::UnderReset) => {
            let probe = info.open().map_err(open_err)?;
            match probe.attach_under_reset(TargetSelector::Auto, permissions.clone()) {
                Ok(s) => s,
                Err(first) => {
                    let probe2 = info.open().map_err(open_err)?;
                    match probe2.attach(TargetSelector::Auto, permissions) {
                        Ok(s) => s,
                        Err(_) => {
                            return Err(lang.pick(
                                format!(
                                    "复位期间连接目标失败: {first}。该探针可能不支持自动识别芯片（如 DAPLink/CMSIS-DAP），请在左侧『手动指定目标芯片』中搜索并选择芯片型号后重试"
                                ),
                                format!(
                                    "Failed to attach under reset: {first}. The probe may not support auto-identification (e.g. DAPLink/CMSIS-DAP). Please search and select the chip model under 'Manual Target Selection' on the left and retry"
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
            format!("不支持的文件格式: .{ext}，请选择 .elf / .hex / .bin / .uf2 文件"),
            format!("Unsupported file format: .{ext}. Choose a .elf / .hex / .bin / .uf2 file"),
        ));
    };

    result.map_err(|e| lang.pick(format!("烧录失败: {e}"), format!("Flashing failed: {e}")))
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
    let mut core = session.core(0).map_err(|e| {
        lang.pick(
            format!("获取核心失败: {e}"),
            format!("Failed to get core: {e}"),
        )
    })?;
    core.reset()
        .map_err(|e| lang.pick(format!("复位失败: {e}"), format!("Reset failed: {e}")))
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
