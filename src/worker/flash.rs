//! 烧录、全片擦除与读取 Flash。

use std::io::Write;
use std::path::Path;
use std::sync::mpsc;

use probe_rs::flashing::{
    erase_all, BinLoader, BinOptions, DownloadOptions, ElfLoader, ElfOptions, FlashProgress,
    HexLoader, ProgressEvent, Uf2Loader,
};
use probe_rs::{MemoryInterface, Session};

use crate::firmware::is_elf;
use crate::i18n::{Lang, Msg};
use crate::t;

use super::run::not_connected;
use super::{progress, WorkerEvent};

/// 烧录选项（由 UI 勾选的烧录参数打包而来）。
pub(super) struct FlashOptions {
    pub do_chip_erase: bool,
    pub verify: bool,
    pub keep_unwritten_bytes: bool,
    pub bin_base: u64,
}

/// 烧录固件到目标。按扩展名选择加载器，无扩展名时按 ELF 魔数识别。
pub(super) fn flash(
    session: &mut Option<Session>,
    path: &Path,
    opts: &FlashOptions,
    events: &mpsc::Sender<WorkerEvent>,
    lang: Lang,
) -> Result<(), String> {
    let Some(session) = session.as_mut() else {
        return Err(not_connected(lang));
    };
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let events2 = events.clone();
    let progress = FlashProgress::new(move |event: ProgressEvent| {
        if let Some(ev) = progress::map(event, lang) {
            let _ = events2.send(ev);
        }
    });

    let mut dl = DownloadOptions::new();
    dl.progress = progress;
    dl.do_chip_erase = opts.do_chip_erase;
    dl.verify = opts.verify;
    dl.keep_unwritten_bytes = opts.keep_unwritten_bytes;

    let is_elf_file = matches!(ext.as_str(), "elf" | "axf") || (ext.is_empty() && is_elf(path));

    let result = if is_elf_file {
        probe_rs::flashing::download_file_with_options(
            session,
            path,
            ElfLoader(ElfOptions::default()),
            dl,
        )
    } else if ext == "hex" {
        probe_rs::flashing::download_file_with_options(session, path, HexLoader, dl)
    } else if ext == "bin" {
        probe_rs::flashing::download_file_with_options(
            session,
            path,
            BinLoader(BinOptions {
                base_address: Some(opts.bin_base),
                ..Default::default()
            }),
            dl,
        )
    } else if ext == "uf2" {
        probe_rs::flashing::download_file_with_options(session, path, Uf2Loader, dl)
    } else {
        return Err(t!(lang, Msg::UnsupportedFileFormat, ext));
    };

    result.map_err(|e| t!(lang, Msg::FlashFailed, e))
}

/// 全片擦除。
pub(super) fn erase(
    session: &mut Option<Session>,
    events: &mpsc::Sender<WorkerEvent>,
    lang: Lang,
) -> Result<(), String> {
    let Some(session) = session.as_mut() else {
        return Err(not_connected(lang));
    };
    let events2 = events.clone();
    let mut progress = FlashProgress::new(move |event: ProgressEvent| {
        if let Some(ev) = progress::map(event, lang) {
            let _ = events2.send(ev);
        }
    });
    erase_all(session, &mut progress, false).map_err(|e| t!(lang, Msg::EraseFailed, e))
}

/// 按地址范围读取 Flash，导出为 .bin 文件。
pub(super) fn read(
    session: &mut Option<Session>,
    path: &Path,
    start: u64,
    end: u64,
    events: &mpsc::Sender<WorkerEvent>,
    lang: Lang,
) -> Result<(), String> {
    let Some(session) = session.as_mut() else {
        return Err(not_connected(lang));
    };
    const CHUNK: usize = 4096;

    let total = end.saturating_sub(start);
    let mut file = std::fs::File::create(path).map_err(|e| t!(lang, Msg::CreateFileFailed, e))?;

    let _ = events.send(WorkerEvent::Status(t!(
        lang,
        Msg::ReadingFirmware,
        format!("{start:X}"),
        format!("{end:X}"),
        total / 1024
    )));

    let events2 = events.clone();
    let op = lang.tr(Msg::ReadOp);
    let _ = events2.send(WorkerEvent::Progress {
        operation: op,
        done: 0,
        total: Some(total),
        state: super::OpState::Active,
    });

    let mut addr = start;
    let mut remaining = total;
    let mut core = session
        .core(0)
        .map_err(|e| t!(lang, Msg::GetCoreFailed, e))?;
    while remaining > 0 {
        let n = remaining.min(CHUNK as u64) as usize;
        let mut buf = vec![0u8; n];
        core.read_8(addr, &mut buf)
            .map_err(|e| t!(lang, Msg::ReadFlashFailed, addr, e))?;
        file.write_all(&buf)
            .map_err(|e| t!(lang, Msg::WriteFileFailed, e))?;
        addr += n as u64;
        remaining -= n as u64;
        let _ = events2.send(WorkerEvent::Progress {
            operation: op,
            done: n as u64,
            total: Some(total),
            state: super::OpState::Active,
        });
    }

    let _ = events2.send(WorkerEvent::Progress {
        operation: op,
        done: 0,
        total: None,
        state: super::OpState::Done,
    });
    Ok(())
}
