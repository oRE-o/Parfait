#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod app;
mod brush_details;
mod brushes;
mod calibration;
mod canvas;
mod chrome;
mod curve_editor;
mod diagnostics;
mod panels;
mod preferences;
mod shortcuts;
mod theme;
mod widgets;
mod workspace;
use std::path::PathBuf;

fn main() {
    if let Err(err) = run() {
        eprintln!("Parfait: {err:#}");
        rfd::MessageDialog::new()
            .set_title("Parfait could not open")
            .set_description(format!("{err:#}"))
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}
fn run() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let dir = args
        .windows(2)
        .find(|p| p[0] == "--data-dir")
        .map(|p| PathBuf::from(&p[1]))
        .unwrap_or_else(|| {
            std::env::var_os("LOCALAPPDATA")
                .or_else(|| std::env::var_os("XDG_DATA_HOME"))
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share")
                })
                .join("Parfait")
        });
    let mut app = app::Parfait::open(dir)?;
    app.capture = args
        .windows(2)
        .find(|p| p[0] == "--screenshot")
        .map(|p| diagnostics::Capture::new(PathBuf::from(&p[1])));
    if args.iter().any(|arg| arg == "--calibrate-pressure") {
        app.calibration_dialog = Some(calibration::CalibrationDialog::new(app.calibration.clone()));
    }
    if args.iter().any(|arg| arg == "--brush-details") {
        app.brush_details.open = true;
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_inner_size([1440., 920.])
            .with_min_inner_size([960., 640.]),
        ..Default::default()
    };
    eframe::run_native(
        "Parfait",
        options,
        Box::new(move |cc| {
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))
}
