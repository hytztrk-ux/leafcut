//! `leafcut-gui`: native desktop front end for leafcut-core.

// Do not open a console window next to the GUI on Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod i18n;
mod worker;

use eframe::egui;

fn main() -> eframe::Result {
    // PDFs passed on the command line (or via "Open with…") start in the list.
    let initial_files: Vec<std::path::PathBuf> = std::env::args_os()
        .skip(1)
        .map(std::path::PathBuf::from)
        .collect();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("leafcut")
            .with_app_id("leafcut")
            .with_inner_size([960.0, 620.0])
            .with_min_inner_size([720.0, 460.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "leafcut",
        options,
        Box::new(|cc| Ok(Box::new(app::LeafcutApp::new(cc, initial_files)))),
    )
}
