#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod app;
mod brush_details;
mod brush_stroke;
mod brush_widgets;
mod brushes;
mod calibration;
mod canvas;
mod chrome;
mod curve_editor;
mod diagnostics;
mod display;
mod document_session;
mod document_tabs;
mod documents;
mod editing;
mod folder_ui;
mod history_palette;
mod layer_commands;
mod layer_controls;
mod layer_details;
mod layer_mask;
mod layer_organize;
mod layers;
mod mask_area;
mod memory_budget;
mod menus;
#[cfg(test)]
mod multi_document_tests;
mod panels;
mod pixel_selection;
mod preferences;
mod project_loader;
mod project_paths;
mod project_state;
mod raster_conversion;
mod recovery_runtime;
mod rulers;
#[cfg(test)]
mod session_target_tests;
mod shortcuts;
mod source_saves;
mod theme;
mod widgets;
mod window_render_cache;
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
    let mut dir = args
        .windows(2)
        .find(|p| p[0] == "--data-dir")
        .map(|p| PathBuf::from(&p[1]))
        .unwrap_or_else(|| {
            std::env::var_os("LOCALAPPDATA")
                .or_else(|| std::env::var_os("XDG_DATA_HOME"))
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                        .join(".local/share")
                })
                .join("Parfait")
        });
    let mut app = loop {
        match app::Parfait::open(dir.clone()) {
            Ok(app) => break app,
            Err(error) => {
                let retry=rfd::MessageDialog::new().set_title("Recovery folder unavailable")
                    .set_description(format!("{error:#}\n\nThe existing files are preserved. Choose another recovery folder to open originals manually?"))
                    .set_buttons(rfd::MessageButtons::YesNo).set_level(rfd::MessageLevel::Error).show();
                if retry != rfd::MessageDialogResult::Yes {
                    return Err(error);
                }
                let Some(next) = rfd::FileDialog::new()
                    .set_title("Choose a different recovery folder")
                    .pick_folder()
                else {
                    return Err(error);
                };
                dir = next;
            }
        }
    };
    app.capture = args
        .windows(2)
        .find(|p| p[0] == "--screenshot")
        .map(|p| diagnostics::Capture::new(PathBuf::from(&p[1])));
    if args.iter().any(|arg| arg == "--calibrate-pressure") {
        app.calibration_dialog =
            Some(calibration::CalibrationDialog::new(app.calibration.clone()));
    }
    if args.iter().any(|arg| arg == "--brush-details") {
        app.brush_details.open = true;
    }
    if args.iter().any(|arg| arg == "--stroke-settings") {
        app.brush_details.open_stroke_settings();
    }
    if args.iter().any(|arg| arg == "--brush-dynamics") {
        app.brush_details.open_pressure_dynamics();
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
