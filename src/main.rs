#![windows_subsystem = "windows"]

mod app;
mod worker;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 760.0])
            .with_min_inner_size([820.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Probe-rs 烧录工具",
        options,
        Box::new(|cc| {
            app::ProbeUiApp::setup(&cc.egui_ctx);
            Ok(Box::new(app::ProbeUiApp::new()) as Box<dyn eframe::App>)
        }),
    )
}
