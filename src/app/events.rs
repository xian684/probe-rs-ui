//! 后台事件分发：把 worker 线程回传的 [`WorkerEvent`] 应用到界面状态。

use crate::i18n::Msg;
use crate::t;
use crate::worker::{OpState, WorkerEvent};

use super::{ChipMergeResult, OpBar, ProbeUiApp};

impl ProbeUiApp {
    pub(crate) fn handle_event(&mut self, ev: WorkerEvent) {
        match ev {
            WorkerEvent::Probes(Ok(list)) => {
                self.probing = false;
                self.probes = list;
                if self.selected_probe >= self.probes.len() {
                    self.selected_probe = 0;
                }
                if self.probes.is_empty() {
                    self.log_warn(self.t(Msg::NoProbes));
                } else {
                    self.log_ok(t!(self.lang, Msg::DetectedProbes, self.probes.len()));
                }
            }
            WorkerEvent::Probes(Err(e)) => {
                self.probing = false;
                self.log_err(e);
            }
            WorkerEvent::Connected(Ok(summary)) => {
                self.connecting = false;
                self.busy = false;
                self.show_manual = false;
                self.log_ok(t!(self.lang, Msg::ConnectedTo, summary.name));
                self.connected = Some(summary);
                if let Some(flash) = self
                    .connected
                    .as_ref()
                    .and_then(|s| s.memory.iter().find(|m| m.kind == "FLASH"))
                {
                    self.read_start = flash.start;
                    self.read_end = flash.end;
                    self.bin_base = flash.start;
                }
                if let Some(ram) = self
                    .connected
                    .as_ref()
                    .and_then(|s| s.memory.iter().find(|m| m.kind == "RAM"))
                {
                    self.mem_start = ram.start;
                    self.mem_write_start = ram.start;
                }
            }
            WorkerEvent::Connected(Err(e)) => {
                self.connecting = false;
                self.busy = false;
                self.show_manual = true;
                self.log_err(e);
            }
            WorkerEvent::Status(s) => self.log_info(s),
            WorkerEvent::Diagnostic(s) => self.log_info(s),
            WorkerEvent::Progress {
                operation,
                done,
                total,
                state,
            } => {
                if let Some(bar) = self.op_bars.iter_mut().find(|b| b.label == operation) {
                    if let Some(t) = total {
                        bar.total = Some(t);
                    }
                    match state {
                        OpState::Active => bar.done += done,
                        OpState::Done => {
                            bar.state = OpState::Done;
                            bar.done = bar.total.unwrap_or(bar.done);
                        }
                        OpState::Failed => bar.state = OpState::Failed,
                    }
                } else {
                    self.op_bars.push(OpBar {
                        label: operation.to_owned(),
                        done,
                        total,
                        state,
                    });
                }
            }
            WorkerEvent::OperationDone(Ok(())) => {
                self.busy = false;
                self.log_ok(self.t(Msg::OperationCompleted));
            }
            WorkerEvent::OperationDone(Err(e)) => {
                self.busy = false;
                self.log_err(e);
            }
            WorkerEvent::FirmwareScanned {
                root,
                candidates,
                best,
            } => {
                self.firmware_scanning = false;
                self.firmware_root = root.clone();
                self.firmware_candidates = candidates;
                if self.firmware_candidates.is_empty() {
                    self.log_warn(t!(self.lang, Msg::NoFirmwareFound, root));
                } else if let Some(i) = best {
                    let path = self.firmware_candidates[i].path.display().to_string();
                    self.file_path = path.clone();
                    self.log_ok(t!(
                        self.lang,
                        Msg::AutoDetectedFirmware,
                        path,
                        self.firmware_candidates.len()
                    ));
                    if self.firmware_candidates.len() > 1 {
                        self.log_info(self.t(Msg::UseOtherFirmware));
                    }
                }
            }
            WorkerEvent::ChipFileLoaded(Ok(info)) => {
                let n = info.chips.len();
                let name = info.family_name.clone();
                match self.merge_chip_file(info) {
                    ChipMergeResult::Skipped => {
                        self.log_warn(t!(self.lang, Msg::ChipFamilySkipped, name));
                    }
                    _ => {
                        self.log_ok(t!(self.lang, Msg::ChipFileLoaded, name, n));
                    }
                }
            }
            WorkerEvent::ChipFileLoaded(Err(e)) => {
                self.log_err(e);
            }
            WorkerEvent::PackGenerated(Ok(infos)) => {
                let n = infos.len();
                let mut skipped = 0;
                for info in infos {
                    if self.merge_chip_file(info) == ChipMergeResult::Skipped {
                        skipped += 1;
                    }
                }
                self.log_ok(t!(self.lang, Msg::PackGenerated, n));
                if skipped > 0 {
                    self.log_info(t!(self.lang, Msg::PackGeneratedSkipped, skipped));
                }
            }
            WorkerEvent::PackGenerated(Err(e)) => {
                self.log_err(e);
            }
            WorkerEvent::TargetGenDone(Ok(mut result)) => {
                self.tg_busy = false;
                let n = result.families.len();
                let loaded_n = result.loaded.len();
                // 先保存结果供左侧面板展示，再移动 loaded 合并进选型列表。
                self.tg_result = Some(result.clone());
                let mut skipped = 0;
                for info in result.loaded.drain(..) {
                    if self.merge_chip_file(info) == ChipMergeResult::Skipped {
                        skipped += 1;
                    }
                }
                self.log_ok(t!(self.lang, Msg::TargetsGenerated, n));
                for family in &result.families {
                    if !family.output_file.is_empty() {
                        self.log_info(t!(
                            self.lang,
                            Msg::TargetFileWritten,
                            family.output_file,
                            family.variant_count
                        ));
                    }
                }
                if loaded_n > 0 {
                    let merged = loaded_n - skipped;
                    if merged > 0 {
                        self.log_info(t!(self.lang, Msg::TgLoadedToSelection, merged));
                    }
                    if skipped > 0 {
                        self.log_info(t!(self.lang, Msg::PackGeneratedSkipped, skipped));
                    }
                }
            }
            WorkerEvent::TargetGenDone(Err(e)) => {
                self.tg_busy = false;
                self.log_err(e);
            }
            WorkerEvent::ArmSearchDone(Ok(list)) => {
                self.arm_busy = false;
                self.arm_packs = list;
                self.arm_selected = None;
                self.log_info(t!(
                    self.lang,
                    Msg::ArmSearchResult,
                    self.arm_packs.len()
                ));
            }
            WorkerEvent::ArmSearchDone(Err(e)) => {
                self.arm_busy = false;
                self.arm_packs.clear();
                self.log_err(e);
            }
            WorkerEvent::ArmGenerateDone(Ok(mut result)) => {
                self.arm_busy = false;
                let n = result.families.len();
                let loaded_n = result.loaded.len();
                self.log_ok(t!(self.lang, Msg::ArmGenerated, n));
                for family in &result.families {
                    if !family.output_file.is_empty() {
                        self.log_info(t!(
                            self.lang,
                            Msg::TargetFileWritten,
                            family.output_file,
                            family.variant_count
                        ));
                    }
                }
                let mut skipped = 0;
                for info in result.loaded.drain(..) {
                    if self.merge_chip_file(info) == ChipMergeResult::Skipped {
                        skipped += 1;
                    }
                }
                if loaded_n > 0 {
                    let merged = loaded_n - skipped;
                    if merged > 0 {
                        self.log_info(t!(self.lang, Msg::TgLoadedToSelection, merged));
                    }
                    if skipped > 0 {
                        self.log_info(t!(self.lang, Msg::PackGeneratedSkipped, skipped));
                    }
                }
            }
            WorkerEvent::ArmGenerateDone(Err(e)) => {
                self.arm_busy = false;
                self.log_err(e);
            }
            WorkerEvent::RttData { channel, data } => {
                let text = String::from_utf8_lossy(&data);
                match self.rtt_view_channel {
                    Some(view) if view != channel => {}
                    Some(_) => self.rtt_buf.push_str(&text),
                    None => {
                        self.rtt_buf.push_str(&format!("[CH{}] ", channel));
                        self.rtt_buf.push_str(&text);
                    }
                }
                const RTT_BUF_CAP: usize = 128 * 1024;
                if self.rtt_buf.len() > RTT_BUF_CAP {
                    let overflow = self.rtt_buf.len() - RTT_BUF_CAP;
                    let cut = self.rtt_buf.floor_char_boundary(overflow);
                    self.rtt_buf.drain(..cut);
                }
            }
            WorkerEvent::RttStarted {
                up_channels,
                down_channels,
            } => {
                self.rtt_on = true;
                self.rtt_up_channels = up_channels;
                self.rtt_down_channels = down_channels;
                if let Some(v) = self.rtt_view_channel
                    && v >= up_channels {
                        self.rtt_view_channel = None;
                    }
                if self.rtt_send_channel >= down_channels.max(1) {
                    self.rtt_send_channel = 0;
                }
                self.log_ok(t!(
                    self.lang,
                    Msg::RttStartedSummary,
                    up_channels,
                    down_channels
                ));
            }
            WorkerEvent::RttStopped => {
                self.rtt_on = false;
            }
            WorkerEvent::MemoryRead(Ok(data)) => {
                self.mem_busy = false;
                self.mem_read_addr = self.mem_start;
                self.mem_data = data;
                self.log_ok(t!(self.lang, Msg::MemoryReadDone, self.mem_data.len()));
            }
            WorkerEvent::MemoryRead(Err(e)) => {
                self.mem_busy = false;
                self.mem_data.clear();
                self.log_err(e);
            }
            WorkerEvent::MemoryWrite(result) => {
                self.mem_busy = false;
                match result {
                    Ok(()) => self.log_ok(self.t(Msg::MemoryWritten)),
                    Err(e) => self.log_err(e),
                }
            }
        }
    }
}
