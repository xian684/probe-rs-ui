//! 后台线程主循环：接收 UI 命令并分发到各操作模块。

use std::sync::mpsc;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

use probe_rs::config::Registry;
use probe_rs::probe::DebugProbeInfo;
use probe_rs::Session;

use crate::i18n::{Lang, Msg};
use crate::rtt;
use crate::t;

use super::{flash, memory, probe, ChipFileInfo, TargetSummary, WorkerCommand, WorkerEvent};

/// RTT 轮询间隔。
const RTT_POLL_INTERVAL: Duration = Duration::from_millis(10);

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
            Ok(cmd) => match cmd {
                WorkerCommand::Shutdown => break,
                WorkerCommand::Scan => probe::scan(&mut probes, &events),
                WorkerCommand::ConnectAuto { probe, boot_mode } => {
                    rtt::stop(&mut rtt, &events, lang, Some(Msg::RttStoppedReconnect));
                    connect_reply(
                        &mut session,
                        probe::connect(&probes, probe, None, boot_mode, lang, &registry),
                        &events,
                    );
                }
                WorkerCommand::ConnectManual {
                    probe,
                    target,
                    boot_mode,
                } => {
                    rtt::stop(&mut rtt, &events, lang, Some(Msg::RttStoppedReconnect));
                    connect_reply(
                        &mut session,
                        probe::connect(&probes, probe, Some(target), boot_mode, lang, &registry),
                        &events,
                    );
                }
                WorkerCommand::Flash {
                    path,
                    do_chip_erase,
                    verify,
                    keep_unwritten_bytes,
                    reset_after,
                    bin_base,
                } => {
                    rtt::stop(&mut rtt, &events, lang, Some(Msg::RttStoppedFlash));
                    let result = flash::flash(
                        &mut session,
                        &path,
                        &flash::FlashOptions {
                            do_chip_erase,
                            verify,
                            keep_unwritten_bytes,
                            bin_base,
                        },
                        &events,
                        lang,
                    )
                    .and_then(|()| {
                        if reset_after {
                            probe::reset(&mut session, lang)?;
                        }
                        Ok(())
                    });
                    let _ = events.send(WorkerEvent::OperationDone(result));
                }
                WorkerCommand::EraseAll => {
                    rtt::stop(&mut rtt, &events, lang, Some(Msg::RttStoppedErase));
                    let result = flash::erase(&mut session, &events, lang);
                    let _ = events.send(WorkerEvent::OperationDone(result));
                }
                WorkerCommand::ReadFlash { path, start, end } => {
                    rtt::stop(&mut rtt, &events, lang, Some(Msg::RttStoppedRead));
                    let result = flash::read(&mut session, &path, start, end, &events, lang);
                    let _ = events.send(WorkerEvent::OperationDone(result));
                }
                WorkerCommand::MemoryRead { start, len } => {
                    let result = memory::read(&mut session, start, len, lang);
                    let _ = events.send(WorkerEvent::MemoryRead(result));
                }
                WorkerCommand::MemoryWrite { start, data } => {
                    let result = memory::write(&mut session, start, &data, lang);
                    let _ = events.send(WorkerEvent::MemoryWrite(result));
                }
                WorkerCommand::Reset => {
                    let result = probe::reset(&mut session, lang);
                    let _ = events.send(WorkerEvent::OperationDone(result));
                }
                WorkerCommand::Disconnect => {
                    rtt::stop(&mut rtt, &events, lang, None);
                    session = None;
                    let _ = events.send(WorkerEvent::Status(lang.tr(Msg::Disconnected).to_owned()));
                }
                WorkerCommand::ScanFirmware { root } => {
                    let (candidates, best) = crate::firmware::scan_firmware(&root);
                    let _ = events.send(WorkerEvent::FirmwareScanned {
                        root: root.display().to_string(),
                        candidates,
                        best,
                    });
                }
                WorkerCommand::LoadChipFile { path } => {
                    let result = load_chip_file(&mut registry, &path, lang);
                    let _ = events.send(WorkerEvent::ChipFileLoaded(result));
                }
                WorkerCommand::GeneratePack { path } => {
                    let result = generate_from_pack(&mut registry, &path, lang);
                    let _ = events.send(WorkerEvent::PackGenerated(result));
                }
                WorkerCommand::TargetGenGenerate {
                    input,
                    output_dir,
                    only_supported,
                    auto_load,
                } => {
                    let result = super::target_gen::generate_targets(
                        &mut registry,
                        &input,
                        &output_dir,
                        only_supported,
                        auto_load,
                        lang,
                    );
                    let _ = events.send(WorkerEvent::TargetGenDone(result));
                }
                WorkerCommand::SetLang(l) => lang = l,
                WorkerCommand::RttStart => rtt = rtt::start(&mut session, &events, lang),
                WorkerCommand::RttStop => {
                    rtt::stop(&mut rtt, &events, lang, Some(Msg::RttStoppedManual));
                }
                WorkerCommand::RttWrite { channel, data } => {
                    rtt::write(&mut rtt, &mut session, channel, &data, &events, lang);
                }
            },
        }
    }
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
