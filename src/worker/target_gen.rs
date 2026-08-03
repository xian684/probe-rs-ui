//! target-gen 集成：从 CMSIS Pack（文件或解压目录）生成 target YAML 定义文件。

use std::path::Path;

use probe_rs::config::Registry;

use crate::i18n::{Lang, Msg};
use crate::t;

use super::{TargetGenFamilyInfo, TargetGenResult};

/// 生成 target 定义文件。
///
/// `input` 可以是 .pack / .pdsc / .zip 文件，也可以是包含 .pdsc 文件的解压目录。
/// 生成结果写入 `output_dir`（自动创建），文件名形如 `<family_name>.yaml`。
pub(super) fn generate_targets(
    input: &Path,
    output_dir: &Path,
    only_supported: bool,
    lang: Lang,
) -> Result<TargetGenResult, String> {
    if !input.exists() {
        return Err(t!(lang, Msg::TgInputMissing, input.display()));
    }

    let mut families: Vec<probe_rs::config::ChipFamily> = Vec::new();

    if input.is_file() {
        target_gen::generate::visit_file(input, &mut families)
            .map_err(|e| t!(lang, Msg::PackGenFailed, e))?;
    } else {
        target_gen::generate::visit_dirs(input, &mut families)
            .map_err(|e| t!(lang, Msg::PackGenFailed, e))?;
        if families.is_empty() {
            return Err(lang.tr(Msg::PackNoChips).to_owned());
        }
    }

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

    if !output_dir.exists() {
        std::fs::create_dir_all(output_dir)
            .map_err(|e| t!(lang, Msg::CreateFileFailed, e))?;
    }

    let mut out = TargetGenResult {
        families: Vec::with_capacity(families.len()),
    };

    for family in &families {
        let yaml = target_gen::commands::elf::serialize_to_yaml_string(family)
            .map_err(|e| t!(lang, Msg::TgSerializeFailed, family.name, e))?;
        let file_name = format!("{}.yaml", family.name.replace(' ', "_"));
        let path = output_dir.join(&file_name);
        std::fs::write(&path, yaml).map_err(|e| t!(lang, Msg::WriteFileFailed, e))?;
        out.families.push(TargetGenFamilyInfo {
            name: family.name.clone(),
            variant_count: family.variants.len(),
            flash_algo_count: family.flash_algorithms.len(),
            output_file: path.display().to_string(),
        });
    }

    Ok(out)
}
