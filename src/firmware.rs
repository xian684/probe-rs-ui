use std::path::Path;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

/// 在项目文件夹中扫描到的固件候选文件。
#[derive(Clone)]
pub struct FirmwareCandidate {
    pub path: PathBuf,
    pub kind: &'static str,
    pub size_kb: u64,
    pub modified: u64,
}

/// 在项目文件夹中递归查找编译产物（.elf/.hex/.bin/.uf2），按可烧录性排序返回。
pub fn scan_firmware(root: &Path) -> (Vec<FirmwareCandidate>, Option<usize>) {
    let mut candidates: Vec<FirmwareCandidate> = Vec::new();
    let mut stack: Vec<(PathBuf, usize, bool)> = vec![(root.to_path_buf(), 0, false)];
    while let Some((dir, depth, in_target)) = stack.pop() {
        if depth > 10 {
            continue;
        }
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let ft = match entry.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };
            if ft.is_dir() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if is_ignored_dir(&name, in_target) {
                    continue;
                }
                let child_in_target = in_target || name == "target";
                stack.push((path, depth + 1, child_in_target));
            } else if ft.is_file() {
                if let Some(kind) = firmware_kind(&path) {
                    if let Ok(meta) = entry.metadata() {
                        if meta.len() == 0 {
                            continue;
                        }
                        let modified = meta
                            .modified()
                            .ok()
                            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        candidates.push(FirmwareCandidate {
                            size_kb: meta.len() / 1024,
                            modified,
                            path,
                            kind,
                        });
                    }
                }
            }
        }
    }
    candidates.sort_by(|a, b| {
        fw_score(b)
            .cmp(&fw_score(a))
            .then(b.modified.cmp(&a.modified))
            .then(a.path.cmp(&b.path))
    });
    let best = if candidates.is_empty() { None } else { Some(0) };
    (candidates, best)
}

fn is_ignored_dir(name: &str, in_target: bool) -> bool {
    if name.starts_with('.') {
        return true;
    }
    match name {
        "node_modules" | "tmp" | "doc" | "deps" | "incremental" | "examples" | ".fingerprint"
        | "package" | "crates" => true,
        "build" => in_target,
        _ => false,
    }
}

fn firmware_kind(path: &Path) -> Option<&'static str> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase());
    match ext.as_deref() {
        Some("elf") | Some("axf") => Some("ELF"),
        Some("hex") => Some("HEX"),
        Some("bin") => Some("BIN"),
        Some("uf2") => Some("UF2"),
        _ => {
            // Rust 编译产物通常没有扩展名，但仍是 ELF 文件：按魔数识别。
            if ext.is_none() && is_elf(path) {
                Some("ELF")
            } else {
                None
            }
        }
    }
}

/// 判断文件是否为 ELF 二进制（通过魔数 0x7F 'E' 'L' 'F' 识别）。
pub fn is_elf(path: &Path) -> bool {
    use std::io::Read;
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut magic = [0u8; 4];
    f.read_exact(&mut magic).is_ok() && magic == [0x7F, b'E', b'L', b'F']
}

/// 依据文件类型与所在目录对候选固件打分，得分高者优先。
fn fw_score(c: &FirmwareCandidate) -> i64 {
    let mut s: i64 = match c.kind {
        "ELF" | "HEX" => 4,
        "BIN" => 2,
        "UF2" => 1,
        _ => 0,
    };
    let p = c.path.to_string_lossy().to_lowercase();
    if p.contains("\\release\\") {
        s += 20;
    } else if p.contains("\\debug\\") {
        s += 15;
    }
    if p.contains("\\build\\") || p.contains("\\out\\") || p.contains("\\output\\") {
        s += 8;
    }
    if p.contains("\\objects\\") {
        s += 6;
    }
    if p.contains("\\bin\\") {
        s += 3;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(path: &Path, bytes: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn scan_prefers_release_elf_over_debug_and_noise() {
        let root = std::env::temp_dir().join("probe-rs-ui-test-scan");
        let _ = std::fs::remove_dir_all(&root);
        write(&root.join("target/debug/myapp.elf"), &[0; 4096]);
        write(&root.join("target/release/myapp.elf"), &[0; 8192]);
        write(&root.join("target/debug/deps/dep.elf"), &[0; 4096]);
        write(&root.join("target/debug/build/probe.elf"), &[0; 4096]);
        write(&root.join("src/main.c"), &[]);
        write(&root.join("Objects/app.hex"), &[0; 2048]);

        let (cands, best) = scan_firmware(&root);
        assert_eq!(best, Some(0));
        let first = cands.first().unwrap().path.to_string_lossy().to_lowercase();
        assert!(
            first.contains("release") && first.ends_with("myapp.elf"),
            "expected release myapp.elf, got {first}"
        );
        assert!(
            cands
                .iter()
                .all(|c| !c.path.to_string_lossy().contains("deps")),
            "deps dir must be skipped"
        );
        assert!(
            cands.iter().all(|c| !c
                .path
                .to_string_lossy()
                .to_lowercase()
                .contains("\\build\\")),
            "cargo build dir must be skipped"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_finds_cmake_build_outputs() {
        let root = std::env::temp_dir().join("probe-rs-ui-test-cmake");
        let _ = std::fs::remove_dir_all(&root);
        write(&root.join("build/Debug/firmware.elf"), &[0; 4096]);
        write(&root.join("build/Release/firmware.elf"), &[0; 8192]);

        let (cands, best) = scan_firmware(&root);
        assert_eq!(best, Some(0));
        let first = cands.first().unwrap().path.to_string_lossy().to_lowercase();
        assert!(
            first.contains("release"),
            "expected release build, got {first}"
        );
        assert_eq!(
            cands.len(),
            2,
            "build dir must be scanned outside cargo target"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_detects_extensionless_elf() {
        let root = std::env::temp_dir().join("probe-rs-ui-test-elf");
        let _ = std::fs::remove_dir_all(&root);
        let mut bytes = vec![0x7F, b'E', b'L', b'F'];
        bytes.extend_from_slice(&[0u8; 4096]);
        write(
            &root.join("target/thumbv7em-none-eabihf/release/myapp"),
            &bytes,
        );
        write(
            &root.join("target/thumbv7em-none-eabihf/debug/myapp"),
            &bytes,
        );
        write(&root.join("src/main.rs"), &[]);

        let (cands, best) = scan_firmware(&root);
        assert_eq!(best, Some(0));
        assert!(!cands.is_empty(), "extensionless ELF must be detected");
        let first = cands.first().unwrap();
        assert_eq!(first.kind, "ELF");
        let p = first.path.to_string_lossy().to_lowercase();
        assert!(
            p.contains("release") && p.ends_with("myapp"),
            "expected release extensionless ELF, got {p}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_empty_folder_returns_none() {
        let root = std::env::temp_dir().join("probe-rs-ui-test-empty");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let (cands, best) = scan_firmware(&root);
        assert!(cands.is_empty());
        assert_eq!(best, None);
        let _ = std::fs::remove_dir_all(&root);
    }
}
