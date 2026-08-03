//! 后台线程主循环：接收 UI 命令并分发到各操作模块。
//!
//! `run` 只做命令分发；worker 线程共享状态聚合在 [`Ctx`] 中，
//! 多行命令分支提取为 `handle_xxx` 自由函数，保持主循环薄而清晰。

use std::path::Path;
use std::sync::mpsc;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

use probe_rs::config::Registry;
use probe_rs::probe::DebugProbeInfo;
use probe_rs::Session;

use crate::i18n::{Lang, Msg};
use crate::rtt;
use crate::t;

use super::{
    flash, memory, probe, BootMode, ChipFileInfo, TargetSummary, WorkerCommand, WorkerEvent,
};

/// RTT 轮询间隔。
const RTT_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// worker 线程共享状态（命令 handler 之间传递的可变借用集合）。
struct Ctx<'a> {
    session: &'a mut Option<Session>,
    probes: &'a mut Vec<DebugProbeInfo>,
    rtt: &'a mut Option<rtt::Handle>,
    registry: &'a mut Registry,
    events: &'a mpsc::Sender<WorkerEvent>,
}

/// 烧录命令参数（聚合多字段，避免过长的参数列表）。
struct FlashRequest {
    path: std::path::PathBuf,
    do_chip_erase: bool,
    verify: bool,
    keep_unwritten_bytes: bool,
    reset_after: bool,
    bin_base: u64,
}

pub(super) fn run(
    rx: mpsc::Receiver<WorkerCommand>,
    events: mpsc::Sender<WorkerEvent>,
    mut lang: Lang,
) {
    let mut session: Option<Session> = None;
    let mut probes: Vec<DebugProbeInfo> = Vec::new();
    let mut rtt: Option<rtt::Handle> = None;
    let mut registry = Registry::from_builtin_families();

    loop {
        match rx.recv_timeout(RTT_POLL_INTERVAL) {
            Err(RecvTimeoutError::Timeout) => {
                if rtt.is_some() {
                    rtt::poll(&mut rtt, &mut session, &events, lang);
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
            Ok(cmd) => {
                let mut ctx = Ctx {
                    session: &mut session,
                    probes: &mut probes,
                    rtt: &mut rtt,
                    registry: &mut registry,
                    events: &events,
                };
                match cmd {
                    WorkerCommand::Shutdown => break,
                    WorkerCommand::Scan => probe::scan(ctx.probes, ctx.events),
                    WorkerCommand::ConnectAuto { probe, boot_mode } => {
                        handle_connect(&mut ctx, probe, None, boot_mode, lang);
                    }
                    WorkerCommand::ConnectManual {
                        probe,
                        target,
                        boot_mode,
                    } => handle_connect(&mut ctx, probe, Some(target), boot_mode, lang),
                    WorkerCommand::Flash {
                        path,
                        do_chip_erase,
                        verify,
                        keep_unwritten_bytes,
                        reset_after,
                        bin_base,
                    } => handle_flash(
                        &mut ctx,
                        FlashRequest {
                            path,
                            do_chip_erase,
                            verify,
                            keep_unwritten_bytes,
                            reset_after,
                            bin_base,
                        },
                        lang,
                    ),
                    WorkerCommand::EraseAll => handle_erase(&mut ctx, lang),
                    WorkerCommand::ReadFlash { path, start, end } => {
                        handle_read_flash(&mut ctx, &path, start, end, lang);
                    }
                    WorkerCommand::MemoryRead { start, len } => {
                        handle_memory_read(&mut ctx, start, len, lang);
                    }
                    WorkerCommand::MemoryWrite { start, data } => {
                        handle_memory_write(&mut ctx, start, &data, lang);
                    }
                    WorkerCommand::Reset => handle_reset(&mut ctx, lang),
                    WorkerCommand::Disconnect => handle_disconnect(&mut ctx, lang),
                    WorkerCommand::ScanFirmware { root } => {
                        handle_scan_firmware(&mut ctx, &root);
                    }
                    WorkerCommand::LoadChipFile { path } => {
                        handle_load_chip_file(&mut ctx, &path, lang);
                    }
                    WorkerCommand::GeneratePack { path } => {
                        handle_generate_pack(&mut ctx, &path, lang);
                    }
                    WorkerCommand::ArmSearch { keyword } => {
                        handle_arm_search(&mut ctx, &keyword, lang);
                    }
                    WorkerCommand::ArmGenerate {
                        filter,
                        output_dir,
                        only_supported,
                        auto_load,
                    } => handle_arm_generate(
                        &mut ctx,
                        &filter,
                        &output_dir,
                        only_supported,
                        auto_load,
                        lang,
                    ),
                    WorkerCommand::ArmDownload { url, output_dir } => {
                        handle_arm_download(&mut ctx, &url, &output_dir, lang);
                    }
                    WorkerCommand::TargetGenGenerate {
                        input,
                        output_dir,
                        only_supported,
                        auto_load,
                    } => handle_target_gen(
                        &mut ctx,
                        &input,
                        &output_dir,
                        only_supported,
                        auto_load,
                        lang,
                    ),
                    WorkerCommand::SetLang(l) => lang = l,
                    WorkerCommand::RttStart => {
                        let started = rtt::start(ctx.session, ctx.events, lang);
                        *ctx.rtt = started;
                    }
                    WorkerCommand::RttStop => {
                        rtt::stop(ctx.rtt, ctx.events, lang, Some(Msg::RttStoppedManual));
                    }
                    WorkerCommand::RttWrite { channel, data } => {
                        rtt::write(ctx.rtt, ctx.session, channel, &data, ctx.events, lang);
                    }
                }
            }
        }
    }
}

/// 连接目标（自动识别或按型号），成功后持有新会话并回写界面。
fn handle_connect(
    ctx: &mut Ctx,
    probe: usize,
    target: Option<String>,
    boot_mode: BootMode,
    lang: Lang,
) {
    rtt::stop(ctx.rtt, ctx.events, lang, Some(Msg::RttStoppedReconnect));
    connect_reply(
        ctx.session,
        probe::connect(ctx.probes, probe, target, boot_mode, lang, ctx.registry),
        ctx.events,
    );
}

/// 烧录固件（可选烧录后复位）。
fn handle_flash(ctx: &mut Ctx, req: FlashRequest, lang: Lang) {
    rtt::stop(ctx.rtt, ctx.events, lang, Some(Msg::RttStoppedFlash));
    let result = flash::flash(
        ctx.session,
        &req.path,
        &flash::FlashOptions {
            do_chip_erase: req.do_chip_erase,
            verify: req.verify,
            keep_unwritten_bytes: req.keep_unwritten_bytes,
            bin_base: req.bin_base,
        },
        ctx.events,
        lang,
    )
    .and_then(|()| {
        if req.reset_after {
            probe::reset(ctx.session, lang)?;
        }
        Ok(())
    });
    let _ = ctx.events.send(WorkerEvent::OperationDone(result));
}

/// 全片擦除。
fn handle_erase(ctx: &mut Ctx, lang: Lang) {
    rtt::stop(ctx.rtt, ctx.events, lang, Some(Msg::RttStoppedErase));
    let result = flash::erase(ctx.session, ctx.events, lang);
    let _ = ctx.events.send(WorkerEvent::OperationDone(result));
}

/// 读取固件到文件。
fn handle_read_flash(ctx: &mut Ctx, path: &Path, start: u64, end: u64, lang: Lang) {
    rtt::stop(ctx.rtt, ctx.events, lang, Some(Msg::RttStoppedRead));
    let result = flash::read(ctx.session, path, start, end, ctx.events, lang);
    let _ = ctx.events.send(WorkerEvent::OperationDone(result));
}

/// 内存读取。
fn handle_memory_read(ctx: &mut Ctx, start: u64, len: usize, lang: Lang) {
    let result = memory::read(ctx.session, start, len, lang);
    let _ = ctx.events.send(WorkerEvent::MemoryRead(result));
}

/// 内存写入。
fn handle_memory_write(ctx: &mut Ctx, start: u64, data: &[u8], lang: Lang) {
    let result = memory::write(ctx.session, start, data, lang);
    let _ = ctx.events.send(WorkerEvent::MemoryWrite(result));
}

/// 复位目标。
fn handle_reset(ctx: &mut Ctx, lang: Lang) {
    let result = probe::reset(ctx.session, lang);
    let _ = ctx.events.send(WorkerEvent::OperationDone(result));
}

/// 断开连接并释放会话。
fn handle_disconnect(ctx: &mut Ctx, lang: Lang) {
    rtt::stop(ctx.rtt, ctx.events, lang, None);
    *ctx.session = None;
    let _ = ctx.events.send(WorkerEvent::Status(lang.tr(Msg::Disconnected).to_owned()));
}

/// 扫描项目目录定位固件。
fn handle_scan_firmware(ctx: &mut Ctx, root: &Path) {
    let (candidates, best) = crate::firmware::scan_firmware(root);
    let _ = ctx.events.send(WorkerEvent::FirmwareScanned {
        root: root.display().to_string(),
        candidates,
        best,
    });
}

/// 加载外部芯片描述文件（YAML target）。
fn handle_load_chip_file(ctx: &mut Ctx, path: &Path, lang: Lang) {
    let result = load_chip_file(ctx.registry, path, lang);
    let _ = ctx.events.send(WorkerEvent::ChipFileLoaded(result));
}

/// 从本地 CMSIS 包（.pack / .pdsc）批量生成芯片族。
fn handle_generate_pack(ctx: &mut Ctx, path: &Path, lang: Lang) {
    let result = generate_from_pack(ctx.registry, path, lang);
    let _ = ctx.events.send(WorkerEvent::PackGenerated(result));
}

/// 搜索 ARM 在线索引（Keil.pidx）。
fn handle_arm_search(ctx: &mut Ctx, keyword: &str, lang: Lang) {
    let result = super::arm::search_packs(keyword, lang);
    let _ = ctx.events.send(WorkerEvent::ArmSearchDone(result));
}

/// ARM 在线下载 + 生成（可选注册 / 可选落盘）。
fn handle_arm_generate(
    ctx: &mut Ctx,
    filter: &str,
    output_dir: &Path,
    only_supported: bool,
    auto_load: bool,
    lang: Lang,
) {
    let result = super::arm::generate_from_arm(
        ctx.registry,
        filter,
        output_dir,
        only_supported,
        auto_load,
        lang,
    );
    let _ = ctx.events.send(WorkerEvent::ArmGenerateDone(result));
}

/// ARM 仅下载 .pack 文件。
fn handle_arm_download(ctx: &mut Ctx, url: &str, output_dir: &Path, lang: Lang) {
    let result = super::arm::download_pack(url, output_dir, lang);
    let _ = ctx.events.send(WorkerEvent::ArmDownloadDone(result));
}

/// target-gen：从本地 CMSIS 包生成 target 定义（可选注册 / 可选落盘）。
fn handle_target_gen(
    ctx: &mut Ctx,
    input: &Path,
    output_dir: &Path,
    only_supported: bool,
    auto_load: bool,
    lang: Lang,
) {
    let result = super::target_gen::generate_targets(
        ctx.registry,
        input,
        output_dir,
        only_supported,
        auto_load,
        lang,
    );
    let _ = ctx.events.send(WorkerEvent::TargetGenDone(result));
}

/// 连接结果回写：成功则持有新会话并通知界面，失败仅通知界面。
fn connect_reply(
    session: &mut Option<Session>,
    result: Result<(Session, TargetSummary), String>,
    events: &mpsc::Sender<WorkerEvent>,
) {
    match result {
        Ok((s, summary)) => {
            *session = Some(s);
            let _ = events.send(WorkerEvent::Connected(Ok(summary)));
        }
        Err(e) => {
            let _ = events.send(WorkerEvent::Connected(Err(e)));
        }
    }
}

/// 加载外部芯片描述文件（CMSIS 包经 target-gen 生成的 YAML target），注册到 registry。
fn load_chip_file(
    registry: &mut Registry,
    path: &std::path::Path,
    lang: Lang,
) -> Result<ChipFileInfo, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| t!(lang, Msg::ReadFileFailed, path.display(), e))?;
    let family_name = registry
        .add_target_family_from_yaml(&content)
        .map_err(|e| format!("{e}"))?;
    let chips = registry
        .get_targets_by_family_name(&family_name)
        .map_err(|e| format!("{e}"))?;
    Ok(ChipFileInfo { family_name, chips })
}

/// 直接从 CMSIS 包（.pack / .pdsc）解析芯片族并注册到 registry（集成 target-gen 库）。
fn generate_from_pack(
    registry: &mut Registry,
    path: &std::path::Path,
    lang: Lang,
) -> Result<Vec<ChipFileInfo>, String> {
    let mut families: Vec<probe_rs::config::ChipFamily> = Vec::new();
    target_gen::generate::visit_file(path, &mut families)
        .map_err(|e| t!(lang, Msg::PackGenFailed, e))?;
    if families.is_empty() {
        return Err(lang.tr(Msg::PackNoChips).to_owned());
    }

    let mut infos = Vec::with_capacity(families.len());
    for family in families {
        let family_name = family.name.clone();
        registry
            .add_target_family(family)
            .map_err(|e| format!("{e}"))?;
        let chips = registry
            .get_targets_by_family_name(&family_name)
            .map_err(|e| format!("{e}"))?;
        infos.push(ChipFileInfo { family_name, chips });
    }
    Ok(infos)
}

/// 统一的『尚未连接目标』错误文案（各操作模块共用）。
pub(super) fn not_connected(lang: Lang) -> String {
    lang.tr(Msg::NotConnectedErr).to_owned()
}
