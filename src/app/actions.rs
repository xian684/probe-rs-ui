//! 操作入口与工具函数：固件烧录、内存读写与文件格式识别。

use std::path::PathBuf;

use crate::i18n::Msg;
use crate::t;
use crate::worker::WorkerCommand;

use super::ProbeUiApp;

impl ProbeUiApp {
    pub(crate) fn detected_format(&self) -> Option<&'static str> {
        self.detected_format_of(std::path::Path::new(&self.file_path))
    }

    pub(crate) fn detected_format_of(&self, path: &std::path::Path) -> Option<&'static str> {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                "elf" | "axf" => return Some("ELF"),
                "hex" => return Some("Intel HEX"),
                "bin" => return Some("Binary"),
                "uf2" => return Some("UF2"),
                _ => {}
            }
        }
        // 无扩展名的 Rust 编译产物：按 ELF 魔数识别。
        if crate::firmware::is_elf(path) {
            Some("ELF")
        } else {
            None
        }
    }

    pub(crate) fn start_flash(&mut self) {
        self.flash_file(PathBuf::from(self.file_path.clone()), self.bin_base);
    }

    pub(crate) fn flash_file(&mut self, path: PathBuf, bin_base: u64) {
        if self.detected_format_of(&path).is_none() {
            self.log_err(self.t(Msg::UnsupportedFormat));
            return;
        }
        self.busy = true;
        self.op_bars.clear();
        self.log_info(t!(self.lang, Msg::FlashingPath, path.display()));
        self.send(WorkerCommand::Flash {
            path,
            do_chip_erase: self.chip_erase,
            verify: self.verify,
            keep_unwritten_bytes: self.keep_unwritten,
            reset_after: self.reset_after,
            bin_base,
        });
    }

    pub(crate) fn read_memory(&mut self) {
        if self.connected.is_none() {
            self.log_warn(self.t(Msg::ConnectFirst));
            return;
        }
        let len = self.mem_len.clamp(1, 256 * 1024);
        if len != self.mem_len {
            self.log_warn(t!(self.lang, Msg::ReadLenClamped, len));
            self.mem_len = len;
        }
        let start = self.mem_start;
        self.mem_busy = true;
        self.log_info(t!(self.lang, Msg::ReadingMemory, start, len));
        self.send(WorkerCommand::MemoryRead { start, len });
    }

    pub(crate) fn write_memory(&mut self) {
        let Some(bytes) = parse_hex_bytes(&self.mem_write_input) else {
            self.log_err(self.t(Msg::InvalidHexData));
            return;
        };
        if bytes.is_empty() {
            return;
        }
        if self.connected.is_none() {
            self.log_warn(self.t(Msg::ConnectFirst));
            return;
        }
        let start = self.mem_write_start;
        self.mem_busy = true;
        self.log_info(t!(
            self.lang,
            Msg::WritingMemory,
            format!("{start:X}"),
            bytes.len()
        ));
        self.send(WorkerCommand::MemoryWrite { start, data: bytes });
    }
}

/// 将形如 "DE AD BE EF" 或 "DEADBEEF" 的十六进制字符串解析为字节序列。
fn parse_hex_bytes(s: &str) -> Option<Vec<u8>> {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.is_empty() || !compact.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(compact.len() / 2);
    for i in (0..compact.len()).step_by(2) {
        out.push(u8::from_str_radix(&compact[i..i + 2], 16).ok()?);
    }
    Some(out)
}
