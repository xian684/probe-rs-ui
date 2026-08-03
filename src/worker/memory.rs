//! 目标内存读写（用于内存查看器视图）。

use probe_rs::{MemoryInterface, Session};

use crate::i18n::{Lang, Msg};
use crate::t;

use super::run::not_connected;

/// 读取目标内存。
pub(super) fn read(
    session: &mut Option<Session>,
    start: u64,
    len: usize,
    lang: Lang,
) -> Result<Vec<u8>, String> {
    let Some(session) = session.as_mut() else {
        return Err(not_connected(lang));
    };
    let mut core = session
        .core(0)
        .map_err(|e| t!(lang, Msg::GetCoreFailed, e))?;
    let mut buf = vec![0u8; len];
    core.read_8(start, &mut buf)
        .map_err(|e| t!(lang, Msg::ReadMemoryFailed, start, e))?;
    Ok(buf)
}

/// 写入目标内存。
pub(super) fn write(
    session: &mut Option<Session>,
    start: u64,
    data: &[u8],
    lang: Lang,
) -> Result<(), String> {
    let Some(session) = session.as_mut() else {
        return Err(not_connected(lang));
    };
    let mut core = session
        .core(0)
        .map_err(|e| t!(lang, Msg::GetCoreFailed, e))?;
    core.write_8(start, data)
        .map_err(|e| t!(lang, Msg::WriteMemoryFailed, start, e))?;
    Ok(())
}
