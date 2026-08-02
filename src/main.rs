#![windows_subsystem = "windows"]

mod app;
mod chips;
mod config;
mod firmware;
mod fonts;
mod i18n;
mod panels;
mod rtt;
mod worker;

/// 程序生成的 64x64 窗口图标（芯片样式）。
fn app_icon() -> eframe::egui::IconData {
    const S: usize = 64;
    let mut rgba = vec![0u8; S * S * 4];
    let mut set = |x: usize, y: usize, r: u8, g: u8, b: u8| {
        let i = (y * S + x) * 4;
        rgba[i] = r;
        rgba[i + 1] = g;
        rgba[i + 2] = b;
        rgba[i + 3] = 255;
    };
    for y in 18..=45 {
        for x in 18..=45 {
            set(x, y, 0x1f, 0x6f, 0xc3);
        }
    }
    for y in 12..=17 {
        for x in 23..=41 {
            if (x - 23) % 5 < 3 {
                set(x, y, 0xd4, 0xaf, 0x37);
            }
        }
    }
    for y in 46..=51 {
        for x in 23..=41 {
            if (x - 23) % 5 < 3 {
                set(x, y, 0xd4, 0xaf, 0x37);
            }
        }
    }
    for x in 12..=17 {
        for y in 23..=41 {
            if (y - 23) % 5 < 3 {
                set(x, y, 0xd4, 0xaf, 0x37);
            }
        }
    }
    for x in 46..=51 {
        for y in 23..=41 {
            if (y - 23) % 5 < 3 {
                set(x, y, 0xd4, 0xaf, 0x37);
            }
        }
    }
    for y in 30..=33 {
        for x in 30..=33 {
            set(x, y, 0x2e, 0xa0, 0x43);
        }
    }
    eframe::egui::IconData {
        rgba,
        width: S as u32,
        height: S as u32,
    }
}

fn main() -> Result<(), eframe::Error> {
    let saved = config::load();
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 760.0])
        .with_min_inner_size([960.0, 600.0])
        .with_icon(app_icon());
    if let Some(pos) = saved.window_pos {
        // 仅当坐标在合理范围内才恢复窗口位置，避免显示器变更后窗口落到屏幕外。
        if pos[0] >= 0.0 && pos[0] <= 20000.0 && pos[1] >= 0.0 && pos[1] <= 20000.0 {
            viewport = viewport.with_position(eframe::egui::pos2(pos[0], pos[1]));
        }
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Probe-rs 烧录工具",
        options,
        Box::new(|cc| {
            fonts::setup(&cc.egui_ctx);
            Ok(Box::new(app::ProbeUiApp::new()) as Box<dyn eframe::App>)
        }),
    )
}
