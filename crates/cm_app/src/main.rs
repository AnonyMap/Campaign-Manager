//! Campaign Manager — desktop entry point.

// No console window behind the app in Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod image_loader;
mod rich_text;
mod settings;
mod theme_ui;
mod themes;
mod ui_util;
mod views;

use std::path::PathBuf;

fn icon() -> Option<egui::IconData> {
    let bytes = include_bytes!("../assets/icon.png");
    let img = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width: w,
        height: h,
    })
}

/// Write panics to `crash.log` in the settings folder so testers can send it in.
fn install_crash_log() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(dir) = settings::config_dir() {
            let _ = std::fs::create_dir_all(&dir);
            let msg = format!(
                "[{}] Campaign Manager {} crashed: {info}\n{}\n\n",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                cm_core::APP_VERSION,
                std::backtrace::Backtrace::force_capture()
            );
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("crash.log")) {
                let _ = f.write_all(msg.as_bytes());
            }
        }
        default(info);
    }));
}

fn main() -> eframe::Result<()> {
    install_crash_log();
    // Optional argument: a campaign folder (or campaign.json) to open.
    let open_path = std::env::args().nth(1).map(PathBuf::from).filter(|p| p.exists());

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(cm_core::APP_NAME)
        .with_app_id("campaign-manager")
        .with_inner_size([1360.0, 860.0])
        .with_min_inner_size([800.0, 520.0]);
    if let Some(i) = icon() {
        viewport = viewport.with_icon(i);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(cm_core::APP_NAME, options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, open_path)))))
}
