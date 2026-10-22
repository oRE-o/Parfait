//! Placement contract shared by canvas editing windows. Each tool keeps its own
//! movable position in egui state; opening another tool cannot reuse its ID.
use crate::shortcuts::Command;
use eframe::egui;
pub(crate) fn window(
    ctx: &egui::Context,
    command: Command,
    width: f32,
) -> egui::Window<'static> {
    let screen = ctx.content_rect();
    let height = match command {
        Command::EditPreferences => (screen.height() - 160.).clamp(350., 580.),
        Command::EditModifierSettings => 260.,
        Command::EditFontSettings => 250.,
        Command::EditSystemServices => 230.,
        Command::EditPrivacySettings => 220.,
        command if command.is_settings_command() => {
            (screen.height() - 100.).clamp(250., 700.)
        }
        Command::EditHsl => 240.,
        Command::EditBrightnessContrast
        | Command::EditPosterize
        | Command::EditThreshold => 205.,
        Command::EditLevels => 335.,
        Command::EditToneCurve => 500.,
        Command::EditGradientMap => 360.,
        Command::EditAutoShade => 460.,
        Command::NewTextLayer | Command::NewVectorLayer => 500.,
        _ => 355.,
    };
    let pos = if command.is_raster_transform() {
        egui::pos2(
            (screen.right() - width - 24.).max(screen.left() + 6.),
            screen.top() + 100.,
        )
    } else {
        egui::pos2(
            (screen.center().x - width / 2.).max(screen.left() + 8.),
            (screen.center().y - height / 2.).max(screen.top() + 90.),
        )
    };
    egui::Window::new(command.label())
        .id(egui::Id::new(("canvas_edit_window", command)))
        .default_pos(pos)
        .default_width(width)
        .resizable(false)
        .collapsible(false)
        .constrain_to(screen.shrink(6.))
}
