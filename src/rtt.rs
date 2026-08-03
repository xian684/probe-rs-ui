//! RTT 会话生命周期与读写（供后台工作线程使用）。

use std::sync::mpsc;
use std::time::Duration;

use probe_rs::rtt::{try_attach_to_rtt, Rtt, ScanRegion};
use probe_rs::Session;

use crate::i18n::{Lang, Msg};
use crate::t;
use crate::worker::WorkerEvent;

pub type Handle = Rtt;

const READ_BUF_SIZE: usize = 512;

pub fn start(
    session: &mut Option<Session>,
    events: &mpsc::Sender<WorkerEvent>,
    lang: Lang,
) -> Option<Handle> {
    let Some(session) = session.as_mut() else {
        status(events, lang.tr(Msg::RttNotConnected).to_owned());
        return None;
    };
    let mut core = match session.core(0) {
        Ok(core) => core,
        Err(e) => {
            status(events, t!(lang, Msg::RttCoreFailedStart, e));
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
                t!(
                    lang,
                    Msg::RttStartedDetected,
                    rtt.up_channels.len(),
                    rtt.down_channels.len()
                ),
            );
            Some(rtt)
        }
        Err(e) => {
            status(events, t!(lang, Msg::RttStartFailed, e));
            let _ = events.send(WorkerEvent::RttStopped);
            None
        }
    }
}

pub fn stop(
    rtt: &mut Option<Handle>,
    events: &mpsc::Sender<WorkerEvent>,
    lang: Lang,
    msg: Option<Msg>,
) {
    if rtt.take().is_some() {
        let _ = events.send(WorkerEvent::RttStopped);
        if let Some(msg) = msg {
            status(events, lang.tr(msg).to_owned());
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
            status(events, t!(lang, Msg::RttCoreFailedStopped, e));
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
        status(events, t!(lang, Msg::RttReadFailed, error));
        *rtt = None;
        let _ = events.send(WorkerEvent::RttStopped);
    }
}

pub fn write(
    rtt: &mut Option<Handle>,
    session: &mut Option<Session>,
    channel: usize,
    data: &[u8],
    events: &mpsc::Sender<WorkerEvent>,
    lang: Lang,
) {
    let (Some(rtt), Some(session)) = (rtt.as_mut(), session.as_mut()) else {
        status(events, lang.tr(Msg::RttNotStarted).to_owned());
        return;
    };
    let mut core = match session.core(0) {
        Ok(core) => core,
        Err(e) => {
            status(events, t!(lang, Msg::RttCoreFailed, e));
            return;
        }
    };
    match rtt.down_channel(channel) {
        Some(channel) => match channel.write(&mut core, data) {
            Ok(n) if n < data.len() => status(events, lang.tr(Msg::RttDownBufferFull).to_owned()),
            Ok(_) => {}
            Err(e) => status(events, t!(lang, Msg::RttDownWriteFailed, e)),
        },
        None => status(events, lang.tr(Msg::RttNoDownChannel).to_owned()),
    }
}

fn status(events: &mpsc::Sender<WorkerEvent>, text: String) {
    let _ = events.send(WorkerEvent::Status(text));
}
