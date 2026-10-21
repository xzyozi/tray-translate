mod app;
mod clipboard;
mod fonts;
mod hotkey;
mod models_view;
mod tray;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([640.0, 560.0])
            .with_min_inner_size([420.0, 360.0]),
        ..Default::default()
    };

    eframe::run_native(
        "tray-translate",
        options,
        Box::new(|cc| {
            let app = app::GuiApp::new(cc)?;
            Ok(Box::new(app))
        }),
    )
}
