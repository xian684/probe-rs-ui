//! 后台事件分发：把 worker 线程回传的 [`WorkerEvent`] 应用到界面状态。
//!
//! `handle_event` 只做分发，各事件域的处理拆分为独立的 `on_xxx` 方法，
//! 保持每个函数短小、按域内聚。

use crate::firmware::FirmwareCandidate;
use crate::i18n::Msg;
use crate::t;
use crate::worker::{OpState, ProbeInfo, TargetSummary, WorkerEvent};

use super::{ChipMergeResult, OpBar, ProbeUiApp};

impl ProbeUiApp {
    /// 事件入口：按事件域分发到对应的处理函数。
    pub(crate) fn handle_event(&mut self, ev: WorkerEvent) {
        match ev {
            WorkerEvent::Probes(r) => self.on_probes(r),
            WorkerEvent::Connected(r) => self.on_connected(r),
            WorkerEvent::Status(s) => self.log_info(s),
            WorkerEvent::Diagnostic(s) => self.log_info(s),
            WorkerEvent::Progress {
                operation,
                done,
                total,
                state,
            } => self.on_progress(operation, done, total, state),
            WorkerEvent::OperationDone(r) => self.on_operation_done(r),
            WorkerEvent::FirmwareScanned {
                root,
                candidates,
                best,
            } => self.on_firmware_scanned(root, candidates, best),
            WorkerEvent::ChipFileLoaded(r) => self.on_chip_file_loaded(r),
            WorkerEvent::PackGenerated(r) => self.on_pack_generated(r),
            WorkerEvent::RestoreExternalDone(r) => self.on_restore_external_done(r),
            WorkerEvent::TargetGenDone(r) => self.on_target_gen_done(r),
            WorkerEvent::ArmSearchDone(r) => self.on_arm_search_done(r),
            WorkerEvent::ArmGenerateDone(r) => self.on_arm_generate_done(r),
            WorkerEvent::ArmDownloadDone(r) => self.on_arm_download_done(r),
            WorkerEvent::RttData { channel, data } => self.on_rtt_data(channel, &data),
            WorkerEvent::RttStarted {
                up_channels,
                down_channels,
            } => self.on_rtt_started(up_channels, down_channels),
            WorkerEvent::RttStopped => self.on_rtt_stopped(),
            WorkerEvent::MemoryRead(r) => self.on_memory_read(r),
            WorkerEvent::MemoryWrite(r) => self.on_memory_write(r),
        }
    }

    /// 探针扫描结果：更新探针列表与选中项。
    fn on_probes(&mut self, result: Result<Vec<ProbeInfo>, String>) {
        match result {
            Ok(list) => {
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
            Err(e) => {
                self.probing = false;
                self.log_err(e);
            }
        }
    }

    /// 连接结果：成功时记录目标并同步内存映射默认值，失败时提示手动选型。
    fn on_connected(&mut self, result: Result<TargetSummary, String>) {
        match result {
            Ok(summary) => {
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
            Err(e) => {
                self.connecting = false;
                self.busy = false;
                self.show_manual = true;
                self.log_err(e);
            }
        }
    }

    /// 后台操作进度：更新或新建进度条。
    fn on_progress(&mut self, operation: &'static str, done: u64, total: Option<u64>, state: OpState) {
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

    /// 烧录 / 擦除等整体操作完成。
    fn on_operation_done(&mut self, result: Result<(), String>) {
        match result {
            Ok(()) => {
                self.busy = false;
                self.log_ok(self.t(Msg::OperationCompleted));
            }
            Err(e) => {
                self.busy = false;
                self.log_err(e);
            }
        }
    }

    /// 固件扫描完成：自动选中最佳固件。
    fn on_firmware_scanned(
        &mut self,
        root: String,
        candidates: Vec<FirmwareCandidate>,
        best: Option<usize>,
    ) {
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

    /// 加载 YAML 芯片描述文件完成（带去重反馈）。
    fn on_chip_file_loaded(&mut self, result: Result<crate::worker::ChipFileInfo, String>) {
        match result {
            Ok(info) => {
                let n = info.chips.len();
                let name = info.family_name.clone();
                match self.merge_chip_file(info, false) {
                    ChipMergeResult::Skipped => {
                        self.log_warn(t!(self.lang, Msg::ChipFamilySkipped, name));
                    }
                    ChipMergeResult::Removed => {}
                    _ => {
                        self.log_ok(t!(self.lang, Msg::ChipFileLoaded, name, n));
                    }
                }
            }
            Err(e) => self.log_err(e),
        }
    }

    /// 启动恢复外部芯片包来源完成：恢复被删除的家族跳过，其余合并。
    fn on_restore_external_done(
        &mut self,
        result: Result<Vec<crate::worker::ChipFileInfo>, String>,
    ) {
        match result {
            Ok(infos) => {
                let mut merged = 0;
                let mut skipped_removed = 0;
                for info in infos {
                    match self.merge_chip_file(info, true) {
                        ChipMergeResult::Removed => skipped_removed += 1,
                        ChipMergeResult::Skipped => {}
                        _ => merged += 1,
                    }
                }
                if merged > 0 {
                    self.log_info(t!(self.lang, Msg::ExternalRestored, merged));
                }
                if skipped_removed > 0 {
                    self.log_info(t!(self.lang, Msg::ExternalRestoreSkipped, skipped_removed));
                }
            }
            Err(e) => self.log_warn(e),
        }
    }

    /// 从本地 CMSIS 包批量生成芯片族完成（带去重计数）。
    fn on_pack_generated(&mut self, result: Result<Vec<crate::worker::ChipFileInfo>, String>) {
        match result {
            Ok(infos) => {
                let n = infos.len();
                let mut skipped = 0;
                for info in infos {
                    if self.merge_chip_file(info, false) == ChipMergeResult::Skipped {
                        skipped += 1;
                    }
                }
                self.log_ok(t!(self.lang, Msg::PackGenerated, n));
                if skipped > 0 {
                    self.log_info(t!(self.lang, Msg::PackGeneratedSkipped, skipped));
                }
            }
            Err(e) => self.log_err(e),
        }
    }

    /// target-gen 生成完成：合并进选型列表并逐文件记录。
    fn on_target_gen_done(&mut self, result: Result<crate::worker::TargetGenResult, String>) {
        match result {
            Ok(mut result) => {
                self.tg_busy = false;
                let n = result.families.len();
                let loaded_n = result.loaded.len();
                // 先保存结果供左侧面板展示，再移动 loaded 合并进选型列表。
                self.tg_result = Some(result.clone());
                let mut skipped = 0;
                for info in result.loaded.drain(..) {
                    if self.merge_chip_file(info, false) == ChipMergeResult::Skipped {
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
            Err(e) => {
                self.tg_busy = false;
                self.log_err(e);
            }
        }
    }

    /// ARM 索引搜索完成：更新结果列表。
    fn on_arm_search_done(&mut self, result: Result<Vec<crate::worker::ArmPackInfo>, String>) {
        match result {
            Ok(list) => {
                self.arm_busy = false;
                self.arm_packs = list;
                self.arm_selected = None;
                self.log_info(t!(
                    self.lang,
                    Msg::ArmSearchResult,
                    self.arm_packs.len()
                ));
            }
            Err(e) => {
                self.arm_busy = false;
                self.arm_packs.clear();
                self.log_err(e);
            }
        }
    }

    /// ARM 在线生成完成：合并进选型列表并逐文件记录。
    fn on_arm_generate_done(&mut self, result: Result<crate::worker::TargetGenResult, String>) {
        match result {
            Ok(mut result) => {
                self.arm_busy = false;
                let n = result.families.len();
                let loaded_n = result.loaded.len();
                self.log_ok(t!(self.lang, Msg::ArmGenerated, n));
                for family in &result.families {
                    if !family.output_file.is_empty() {
                        // 落盘的 YAML 可作下次启动的恢复来源。
                        self.record_external_source(std::path::Path::new(&family.output_file));
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
                    if self.merge_chip_file(info, false) == ChipMergeResult::Skipped {
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
            Err(e) => {
                self.arm_busy = false;
                self.log_err(e);
            }
        }
    }

    /// ARM 仅下载完成：记录落盘路径。
    fn on_arm_download_done(&mut self, result: Result<String, String>) {
        match result {
            Ok(path) => {
                self.arm_busy = false;
                self.log_ok(t!(self.lang, Msg::ArmDownloaded, path));
            }
            Err(e) => {
                self.arm_busy = false;
                self.log_err(e);
            }
        }
    }

    /// RTT 数据到达：写入缓冲（带上限截断）。
    fn on_rtt_data(&mut self, channel: usize, data: &[u8]) {
        let text = String::from_utf8_lossy(data);
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

    /// RTT 启动：记录通道数量并校正选中通道。
    fn on_rtt_started(&mut self, up_channels: usize, down_channels: usize) {
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

    /// RTT 停止。
    fn on_rtt_stopped(&mut self) {
        self.rtt_on = false;
    }

    /// 内存读取完成。
    fn on_memory_read(&mut self, result: Result<Vec<u8>, String>) {
        match result {
            Ok(data) => {
                self.mem_busy = false;
                self.mem_read_addr = self.mem_start;
                self.mem_data = data;
                self.log_ok(t!(self.lang, Msg::MemoryReadDone, self.mem_data.len()));
            }
            Err(e) => {
                self.mem_busy = false;
                self.mem_data.clear();
                self.log_err(e);
            }
        }
    }

    /// 内存写入完成。
    fn on_memory_write(&mut self, result: Result<(), String>) {
        self.mem_busy = false;
        match result {
            Ok(()) => self.log_ok(self.t(Msg::MemoryWritten)),
            Err(e) => self.log_err(e),
        }
    }
}
