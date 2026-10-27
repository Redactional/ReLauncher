//! ReLauncher: installs, updates and launches Re programs. The list of programs
//! comes from `programs.json` in the ReLauncher repository; each program's
//! latest version comes from its own GitHub releases.

#![cfg_attr(not(test), windows_subsystem = "windows")]

mod app;
mod catalog;
mod github;
mod http;
mod install;
mod settings;
mod theme;

fn main() -> eframe::Result {
    let icon = image::load_from_memory(include_bytes!(
        "../assets/icons/linux/ReLauncher_256x256.png"
    ))
    .map(|i| i.into_rgba8())
    .ok()
    .map(|img| egui::IconData {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
    });
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("ReLauncher")
        .with_inner_size([1100.0, 700.0])
        .with_min_inner_size([720.0, 460.0]);
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    eframe::run_native(
        "ReLauncher",
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
