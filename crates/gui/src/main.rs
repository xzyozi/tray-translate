#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod clipboard;
mod fonts;
// OS 依存の機能は feature で切り離せる。無効のときは同じ API のスタブに差し替える（ADR-007）
#[cfg(feature = "hotkey")]
mod hotkey;
#[cfg(not(feature = "hotkey"))]
#[path = "hotkey_stub.rs"]
mod hotkey;
mod icons;
mod lifecycle;
mod models_view;
mod theme;
#[cfg(feature = "tray")]
mod tray;
#[cfg(not(feature = "tray"))]
#[path = "tray_stub.rs"]
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
