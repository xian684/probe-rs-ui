use std::path::PathBuf;

use eframe::egui;

/// 加载平台上的 CJK 字体，以便正确显示中文界面。
pub fn setup(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for path in cjk_font_candidates() {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        fonts
            .font_data
            .insert("cjk".to_owned(), egui::FontData::from_owned(bytes).into());
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            if let Some(names) = fonts.families.get_mut(&family) {
                names.push("cjk".to_owned());
            }
        }
        break;
    }
    ctx.set_fonts(fonts);
}

fn cjk_font_candidates() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "windows")]
    extend_paths(
        &mut paths,
        &[
            r"C:\Windows\Fonts\msyh.ttc",
            r"C:\Windows\Fonts\msyhbd.ttc",
            r"C:\Windows\Fonts\simhei.ttf",
            r"C:\Windows\Fonts\simsun.ttc",
        ],
    );

    #[cfg(target_os = "macos")]
    {
        extend_paths(
            &mut paths,
            &[
                "/System/Library/Fonts/PingFang.ttc",
                "/System/Library/Fonts/Hiragino Sans GB.ttc",
                "/System/Library/Fonts/STHeiti Light.ttc",
                "/Library/Fonts/Arial Unicode.ttf",
            ],
        );
        if let Some(home) = std::env::var_os("HOME") {
            paths.extend([
                PathBuf::from(&home).join("Library/Fonts/PingFang.ttc"),
                PathBuf::from(&home).join("Library/Fonts/NotoSansCJKsc-Regular.otf"),
            ]);
        }
    }

    #[cfg(target_os = "linux")]
    {
        extend_paths(
            &mut paths,
            &[
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/opentype/noto/NotoSansCJKSC-Regular.otf",
                "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
                "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
                "/usr/share/fonts/truetype/arphic/ukai.ttc",
                "/usr/share/fonts/truetype/arphic/uming.ttc",
            ],
        );
        if let Some(home) = std::env::var_os("HOME") {
            paths.extend([
                PathBuf::from(&home).join(".local/share/fonts/NotoSansCJK-Regular.ttc"),
                PathBuf::from(&home).join(".local/share/fonts/NotoSansCJKSC-Regular.otf"),
                PathBuf::from(&home).join(".fonts/NotoSansCJK-Regular.ttc"),
            ]);
        }
    }

    paths
}

fn extend_paths(paths: &mut Vec<PathBuf>, candidates: &[&str]) {
    paths.extend(candidates.iter().map(PathBuf::from));
}
