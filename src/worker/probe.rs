//! 探针扫描、目标连接与复位。

use std::sync::mpsc;

use probe_rs::config::{MemoryRegion, Registry, TargetSelector};
use probe_rs::probe::list::Lister;
use probe_rs::probe::DebugProbeInfo;
use probe_rs::{Permissions, Session};

use crate::i18n::{Lang, Msg};
use crate::t;

use super::run::not_connected;
use super::{BootMode, MemRegionInfo, ProbeInfo, TargetSummary, WorkerEvent};

/// 扫描已连接的调试探针，更新缓存并通知界面。
pub(super) fn scan(probes: &mut Vec<DebugProbeInfo>, events: &mpsc::Sender<WorkerEvent>) {
    let list = Lister::new().list_all();
    *probes = list.clone();
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

/// 连接目标芯片。`target` 为 `None` 时自动识别，否则按型号手动连接。
/// `registry` 含内置芯片与外部加载的芯片描述文件，供型号解析使用。
pub(super) fn connect(
    probes: &[DebugProbeInfo],
    index: usize,
    target: Option<String>,
    boot_mode: BootMode,
    lang: Lang,
    registry: &Registry,
) -> Result<(Session, TargetSummary), String> {
    let info = probes
        .get(index)
        .cloned()
        .ok_or_else(|| t!(lang, Msg::NoProbeIndex, index))?;

    let permissions = Permissions::new().allow_erase_all();

    let open_err = |e| t!(lang, Msg::OpenProbeFailed, e);

    let attach_err = |e: probe_rs::Error, name: &str| t!(lang, Msg::ConnectTargetFailed, name, e);

    let session = match (target, boot_mode) {
        (Some(name), BootMode::Normal) => {
            let probe = info.open().map_err(open_err)?;
            probe
                .attach_with_registry(
                    TargetSelector::Unspecified(name.clone()),
                    permissions,
                    registry,
                )
                .map_err(|e| attach_err(e, &name))?
        }
        (Some(name), BootMode::UnderReset) => {
            let probe = info.open().map_err(open_err)?;
            probe
                .attach_under_reset_with_registry(
                    TargetSelector::Unspecified(name.clone()),
                    permissions,
                    registry,
                )
                .map_err(|e| attach_err(e, &name))?
        }
        (None, BootMode::Normal) => {
            let probe = info.open().map_err(open_err)?;
            match probe.attach_with_registry(TargetSelector::Auto, permissions.clone(), registry) {
                Ok(s) => s,
                Err(first) => {
                    let probe2 = info.open().map_err(open_err)?;
                    match probe2.attach_under_reset_with_registry(
                        TargetSelector::Auto,
                        permissions,
                        registry,
                    ) {
                        Ok(s) => s,
                        Err(_) => return Err(t!(lang, Msg::AutoDetectFailed, first)),
                    }
                }
            }
        }
        (None, BootMode::UnderReset) => {
            let probe = info.open().map_err(open_err)?;
            match probe.attach_under_reset_with_registry(
                TargetSelector::Auto,
                permissions.clone(),
                registry,
            ) {
                Ok(s) => s,
                Err(first) => {
                    let probe2 = info.open().map_err(open_err)?;
                    match probe2.attach_with_registry(TargetSelector::Auto, permissions, registry) {
                        Ok(s) => s,
                        Err(_) => return Err(t!(lang, Msg::UnderResetFailed, first)),
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

/// 将 probe-rs 内存区域映射为界面摘要。
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

/// 复位目标核心。
pub(super) fn reset(session: &mut Option<Session>, lang: Lang) -> Result<(), String> {
    let Some(session) = session.as_mut() else {
        return Err(not_connected(lang));
    };
    let mut core = session
        .core(0)
        .map_err(|e| t!(lang, Msg::GetCoreFailed, e))?;
    core.reset().map_err(|e| t!(lang, Msg::ResetFailed, e))
}
