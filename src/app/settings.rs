//! 配置持久化：把已保存的 `config.toml` 应用到界面状态，或收集当前状态供保存。

use std::path::PathBuf;

use eframe::egui;

use crate::config::AppConfig;
use crate::i18n::{Lang, Msg};
use crate::t;
use crate::worker::{BootMode, WorkerCommand};

use super::{CentralTab, ProbeUiApp, ThemeMode};

impl ProbeUiApp {
    /// 将已保存的配置应用到界面状态。
    pub(crate) fn apply_config(&mut self, cfg: AppConfig) {
        self.lang = if cfg.lang == "en" { Lang::En } else { Lang::Zh };
        self.theme_mode = match cfg.theme.as_str() {
            "light" => ThemeMode::Light,
            "dark" => ThemeMode::Dark,
            _ => ThemeMode::System,
        };
        self.theme_applied = None;
        self.boot_mode = if cfg.boot_mode == "under_reset" {
            BootMode::UnderReset
        } else {
            BootMode::Normal
        };
        self.manual_target = cfg.manual_target;
        if !self.manual_target.trim().is_empty()
            && let Some(family_idx) = self
                .chip_families
                .iter()
                .position(|f| f.chips.iter().any(|c| c == &self.manual_target))
            {
                self.selected_family = Some(family_idx);
                self.selected_brand = self
                    .chip_brands
                    .iter()
                    .position(|b| b.families.contains(&family_idx));
            }
        self.file_path = cfg.file_path;
        self.firmware_root = cfg.firmware_root;
        self.chip_erase = cfg.chip_erase;
        self.verify = cfg.verify;
        self.keep_unwritten = cfg.keep_unwritten;
        self.reset_after = cfg.reset_after;
        self.bin_base = cfg.bin_base;
        self.rtt_view_channel = cfg.rtt_view_channel;
        self.rtt_send_channel = cfg.rtt_send_channel;
        self.rtt_autoscroll = cfg.rtt_autoscroll;
        self.central_tab = match cfg.central_tab.as_str() {
            "memory" => CentralTab::Memory,
            "rtt" => CentralTab::Rtt,
            "arm" => CentralTab::ArmIndex,
            _ => CentralTab::Flash,
        };
        self.mem_start = cfg.mem_start;
        self.mem_len = cfg.mem_len;
        self.mem_write_start = cfg.mem_write_start;
        self.tg_input = cfg.tg_input;
        self.tg_output_dir = cfg.tg_output_dir;
        // 恢复历史导入过的外部芯片包来源：逐个重新加载到 registry 并合并进选型列表。
        self.external_sources = cfg.external_sources;
        self.external_removed = cfg.external_removed;
        for src in &self.external_sources {
            if !src.trim().is_empty() {
                self.send(WorkerCommand::RestoreExternal {
                    path: PathBuf::from(src),
                });
            }
        }
        if !self.external_sources.is_empty() {
            self.log_info(t!(
                self.lang,
                Msg::RestoringExternalPacks,
                self.external_sources.len()
            ));
        }
        self.send(WorkerCommand::SetLang(self.lang));
        if !self.firmware_root.trim().is_empty() {
            self.firmware_scanning = true;
            self.send(WorkerCommand::ScanFirmware {
                root: PathBuf::from(self.firmware_root.clone()),
            });
        }
    }

    /// 收集当前界面状态（含窗口尺寸/位置）用于保存。
    pub(crate) fn collect_config(&mut self, ctx: &egui::Context) -> AppConfig {
        let (size, pos) = ctx.input(|i| {
            let rect = i.viewport().outer_rect;
            let size = rect.map(|r| [r.width(), r.height()]);
            let pos = rect.map(|r| [r.min.x, r.min.y]);
            (size, pos)
        });
        if let Some(s) = size {
            self.win_size = Some(s);
        }
        if let Some(p) = pos {
            self.win_pos = Some(p);
        }
        AppConfig {
            lang: if self.lang.is_en() { "en" } else { "zh" }.into(),
            theme: match self.theme_mode {
                ThemeMode::System => "system",
                ThemeMode::Light => "light",
                ThemeMode::Dark => "dark",
            }
            .into(),
            boot_mode: match self.boot_mode {
                BootMode::Normal => "normal",
                BootMode::UnderReset => "under_reset",
            }
            .into(),
            manual_target: self.manual_target.clone(),
            file_path: self.file_path.clone(),
            firmware_root: self.firmware_root.clone(),
            chip_erase: self.chip_erase,
            verify: self.verify,
            keep_unwritten: self.keep_unwritten,
            reset_after: self.reset_after,
            bin_base: self.bin_base,
            rtt_view_channel: self.rtt_view_channel,
            rtt_send_channel: self.rtt_send_channel,
            rtt_autoscroll: self.rtt_autoscroll,
            central_tab: match self.central_tab {
                CentralTab::Flash => "flash",
                CentralTab::Memory => "memory",
                CentralTab::Rtt => "rtt",
                CentralTab::ArmIndex => "arm",
            }
            .into(),
            mem_start: self.mem_start,
            mem_len: self.mem_len,
            mem_write_start: self.mem_write_start,
            tg_input: self.tg_input.clone(),
            tg_output_dir: self.tg_output_dir.clone(),
            external_sources: self.external_sources.clone(),
            external_removed: self.external_removed.clone(),
            window_size: self.win_size,
            window_pos: self.win_pos,
        }
    }
}
