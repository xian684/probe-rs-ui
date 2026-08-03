//! ARM 在线索引（Keil.pidx）集成：搜索可用 Pack，按关键字下载并生成 target 定义。

use std::path::Path;

use probe_rs::config::Registry;

use crate::i18n::{Lang, Msg};
use crate::t;

use super::{ArmPackInfo, ChipFileInfo, TargetGenFamilyInfo, TargetGenResult};

/// 拉取 ARM 索引并按关键字过滤，返回 Pack 摘要列表。
pub(super) fn search_packs(keyword: &str, lang: Lang) -> Result<Vec<ArmPackInfo>, String> {
    let kw = keyword.trim().to_lowercase();
    let idx = crate::arm_runtime_block_on(target_gen::fetch::get_vidx())
        .map_err(|e| t!(lang, Msg::ArmIndexFailed, e))?
        .map_err(|e| t!(lang, Msg::ArmIndexFailed, e))?;

    let mut out: Vec<ArmPackInfo> = idx
        .pdsc_index
        .into_iter()
        .filter(|p| kw.is_empty() || p.name.to_lowercase().contains(&kw))
        .map(|p| ArmPackInfo {
            url: format!(
                "{}/{}.{}.{}.pack",
                p.url.trim_end_matches('/'),
                p.vendor,
                p.name,
                p.version
            ),
            vendor: p.vendor,
            name: p.name,
            version: p.version,
            deprecated: p.deprecated.is_some(),
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// 仅下载 .pack 文件到输出目录，返回落盘路径。
pub(super) fn download_pack(
    url: &str,
    output_dir: &Path,
    lang: Lang,
) -> Result<String, String> {
    if !output_dir.exists() {
        std::fs::create_dir_all(output_dir)
            .map_err(|e| t!(lang, Msg::CreateFileFailed, e))?;
    }
    // 从 URL 提取文件名（{vendor}.{name}.{version}.pack）。
    let file_name = url
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("download.pack");
    let path = output_dir.join(file_name);

    let bytes = reqwest::blocking::get(url)
        .map_err(|e| t!(lang, Msg::ArmDownloadFailed, e))?
        .bytes()
        .map_err(|e| t!(lang, Msg::ArmDownloadFailed, e))?;

    std::fs::write(&path, bytes).map_err(|e| t!(lang, Msg::WriteFileFailed, e))?;
    Ok(path.display().to_string())
}

/// 从 ARM 索引下载匹配关键字的 Pack，生成 target 定义并（可选）注册。
///
/// `filter` 为空时行为与 target-gen `arm` 子命令一致：仅下载 probe-rs 已支持的芯片族；
/// 非空时下载所有名字含关键字的 Pack。
pub(super) fn generate_from_arm(
    registry: &mut Registry,
    filter: &str,
    output_dir: &Path,
    only_supported: bool,
    auto_load: bool,
    lang: Lang,
) -> Result<TargetGenResult, String> {
    let filter_opt = if filter.trim().is_empty() {
        None
    } else {
        Some(filter.trim().to_owned())
    };

    let mut families: Vec<probe_rs::config::ChipFamily> = Vec::new();
    crate::arm_runtime_block_on(target_gen::generate::visit_arm_files(
        &mut families,
        filter_opt.clone(),
    ))
    .map_err(|e| t!(lang, Msg::ArmGenerateFailed, e))?
    .map_err(|e| t!(lang, Msg::ArmGenerateFailed, e))?;

    if families.is_empty() {
        return Err(lang.tr(Msg::PackNoChips).to_owned());
    }

    // 仅保留 probe-rs 已内置支持的芯片族（与 target-gen `arm` 子命令行为一致）。
    if only_supported {
        let builtin = Registry::from_builtin_families();
        let supported: Vec<&str> = builtin
            .families()
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        families.retain(|f| supported.contains(&f.name.as_str()));
        if families.is_empty() {
            return Err(lang.tr(Msg::TgNoSupportedFamily).to_owned());
        }
    }

    // 可选：将 YAML 定义落盘到输出目录。
    if !output_dir.as_os_str().is_empty() {
        if !output_dir.exists() {
            std::fs::create_dir_all(output_dir)
                .map_err(|e| t!(lang, Msg::CreateFileFailed, e))?;
        }
        for family in &families {
            let yaml = target_gen::commands::elf::serialize_to_yaml_string(family)
                .map_err(|e| t!(lang, Msg::TgSerializeFailed, family.name, e))?;
            let file_name = format!("{}.yaml", family.name.replace(' ', "_"));
            let path = output_dir.join(&file_name);
            std::fs::write(&path, yaml).map_err(|e| t!(lang, Msg::WriteFileFailed, e))?;
        }
    }

    let mut out = TargetGenResult {
        families: Vec::with_capacity(families.len()),
        loaded: Vec::new(),
    };

    for family in &families {
        out.families.push(TargetGenFamilyInfo {
            name: family.name.clone(),
            variant_count: family.variants.len(),
            output_file: if output_dir.as_os_str().is_empty() {
                String::new()
            } else {
                output_dir
                    .join(format!("{}.yaml", family.name.replace(' ', "_")))
                    .display()
                    .to_string()
            },
        });

        if auto_load {
            let family_name = family.name.clone();
            registry
                .add_target_family(family.clone())
                .map_err(|e| t!(lang, Msg::PackGenFailed, e))?;
            let chips = registry
                .get_targets_by_family_name(&family_name)
                .map_err(|e| t!(lang, Msg::PackGenFailed, e))?;
            out.loaded.push(ChipFileInfo { family_name, chips });
        }
    }

    Ok(out)
}
