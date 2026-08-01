//! RTT session lifecycle and I/O for the background worker.

use std::sync::mpsc;
use std::time::Duration;

use probe_rs::rtt::{try_attach_to_rtt, Rtt, ScanRegion};
use probe_rs::Session;

use crate::i18n::Lang;
use crate::worker::WorkerEvent;

pub type Handle = Rtt;

const READ_BUF_SIZE: usize = 512;

pub fn start(
    session: &mut Option<Session>,
    events: &mpsc::Sender<WorkerEvent>,
    lang: Lang,
) -> Option<Handle> {
    let Some(session) = session.as_mut() else {
        status(
            events,
            lang,
            "尚未连接目标，无法启动 RTT",
            "Not connected to a target. Cannot start RTT",
        );
        return None;
    };
    let mut core = match session.core(0) {
        Ok(core) => core,
        Err(e) => {
            status(
                events,
                lang,
                &format!("获取核心失败，无法启动 RTT: {e}"),
                &format!("Failed to get core. Cannot start RTT: {e}"),
            );
            return None;
        }
    };
    match try_attach_to_rtt(&mut core, Duration::from_secs(3), &ScanRegion::Ram) {
        Ok(rtt) => {
            let _ = events.send(WorkerEvent::RttStarted {
                up_channels: rtt.up_channels.len(),
                down_channels: rtt.down_channels.len(),
            });
            status(
                events,
                lang,
                &format!(
                    "RTT 已启动，检测到 {} 个上行、{} 个下行通道",
                    rtt.up_channels.len(),
                    rtt.down_channels.len()
                ),
                &format!(
                    "RTT started: {} up, {} down channel(s)",
                    rtt.up_channels.len(),
                    rtt.down_channels.len()
                ),
            );
            Some(rtt)
        }
        Err(e) => {
            status(events, lang, &format!("启动 RTT 失败: {e}。请确认固件已初始化 RTT 且目标程序正在运行"), &format!("Failed to start RTT: {e}. Make sure the firmware has initialized RTT and is running"));
            let _ = events.send(WorkerEvent::RttStopped);
            None
        }
    }
}

pub fn stop(
    rtt: &mut Option<Handle>,
    events: &mpsc::Sender<WorkerEvent>,
    lang: Lang,
    zh_msg: &str,
    en_msg: &str,
) {
    if rtt.take().is_some() {
        let _ = events.send(WorkerEvent::RttStopped);
        if !zh_msg.is_empty() {
            status(events, lang, zh_msg, en_msg);
        }
    }
}

pub fn poll(
    rtt: &mut Option<Handle>,
    session: &mut Option<Session>,
    events: &mpsc::Sender<WorkerEvent>,
    lang: Lang,
) {
    let Some(session) = session.as_mut() else {
        return;
    };
    let mut core = match session.core(0) {
        Ok(core) => core,
        Err(e) => {
            status(
                events,
                lang,
                &format!("获取核心失败，RTT 已停止: {e}"),
                &format!("Failed to get core; RTT stopped: {e}"),
            );
            *rtt = None;
            let _ = events.send(WorkerEvent::RttStopped);
            return;
        }
    };
    let error = {
        let Some(handle) = rtt.as_mut() else { return };
        let mut error = None;
        for channel in 0..handle.up_channels.len() {
            let Some(up) = handle.up_channel(channel) else {
                continue;
            };
            let mut buf = [0; READ_BUF_SIZE];
            match up.read(&mut core, &mut buf) {
                Ok(0) => {}
                Ok(n) => {
                    let _ = events.send(WorkerEvent::RttData {
                        channel,
                        data: buf[..n].to_vec(),
                    });
                }
                Err(e) => {
                    error = Some(e.to_string());
                    break;
                }
            }
        }
        error
    };
    if let Some(error) = error {
        status(
            events,
            lang,
            &format!("RTT 读取失败，已停止: {error}"),
            &format!("RTT read failed, stopped: {error}"),
        );
        *rtt = None;
        let _ = events.send(WorkerEvent::RttStopped);
    }
}

pub fn write(
    rtt: &mut Option<Handle>,
    session: &mut Option<Session>,
    data: &[u8],
    events: &mpsc::Sender<WorkerEvent>,
    lang: Lang,
) {
    let (Some(rtt), Some(session)) = (rtt.as_mut(), session.as_mut()) else {
        status(
            events,
            lang,
            "RTT 未启动或未连接目标",
            "RTT not started or not connected",
        );
        return;
    };
    let mut core = match session.core(0) {
        Ok(core) => core,
        Err(e) => {
            status(
                events,
                lang,
                &format!("获取核心失败: {e}"),
                &format!("Failed to get core: {e}"),
            );
            return;
        }
    };
    match rtt.down_channel(0) {
        Some(channel) => match channel.write(&mut core, data) {
            Ok(n) if n < data.len() => status(
                events,
                lang,
                "目标下行缓冲区已满，部分数据未发送",
                "Target down buffer full, some data was not sent",
            ),
            Ok(_) => {}
            Err(e) => status(
                events,
                lang,
                &format!("RTT 下行写入失败: {e}"),
                &format!("RTT down write failed: {e}"),
            ),
        },
        None => status(
            events,
            lang,
            "目标未配置 RTT 下行通道",
            "Target has no RTT down channel",
        ),
    }
}

fn status(events: &mpsc::Sender<WorkerEvent>, lang: Lang, zh: &str, en: &str) {
    let _ = events.send(WorkerEvent::Status(lang.pick(zh.to_owned(), en.to_owned())));
}
