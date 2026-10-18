use crate::app::Parfait;
use eframe::egui::{self, Key};
pub const HELP: &[&str] = &[
    "P    Pen       K    Pencil       B    Paint / marker",
    "A    Airbrush       E    Eraser       H    Hand       I    Eyedropper",
    "[ / ]    Smaller / larger brush",
    "Space + drag    Pan canvas",
    "Shift + Space + drag    Rotate view",
    "Ctrl + Space + drag    Zoom view",
    "Mouse wheel / Ctrl + wheel    Zoom at cursor",
    "Ctrl + + / -    Zoom canvas (Ctrl + = also zooms in)",
    "Alt + click    Pick color",
    "Ctrl + 0    Fit canvas / reset rotation",
    "Ctrl + Z    Undo       Ctrl + Y    Redo",
    "Ctrl + Shift + Z    Redo",
    "Ctrl + Shift + N    New layer",
    "Ctrl + N    New drawing",
    "Ctrl + O    Open project",
    "Ctrl + S    Save project       Ctrl + Shift + S    Save as",
    "Ctrl + Shift + E    Export PNG",
    "Tab    Hide side panels",
    "F1    This guide",
];
impl Parfait {
    pub fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.calibration_dialog.is_some()
            || self.brush_details.dialog_open()
            || ctx.egui_wants_keyboard_input()
            || self.stroke.is_some()
            || self.confirm_new
            || self.error.is_some()
        {
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            else {
                continue;
            };
            let command = (modifiers.command || modifiers.ctrl) && !modifiers.alt;
            // Only registered combinations take precedence. An unused modifier falls
            // through to the ordinary key action instead of disabling all tool keys.
            match (key, command, modifiers.shift) {
                (Key::Z, true, false) => self.undo(),
                (Key::Z, true, true) | (Key::Y, true, false) => self.redo(),
                (Key::S, true, shift) => self.save_project(shift),
                (Key::O, true, false) => self.open_project(),
                (Key::N, true, true) => self.edit(|d| d.add_layer()),
                (Key::N, true, false) => self.confirm_new = true,
                (Key::E, true, true) => self.export(),
                (Key::Num0, true, false) => self.view = Default::default(),
                (Key::P, _, _) => self.select_brush_group(crate::brushes::BrushGroup::Ink),
                (Key::B, _, _) => self.select_brush_group(crate::brushes::BrushGroup::Paint),
                (Key::K, _, _) => self.select_brush_group(crate::brushes::BrushGroup::Sketch),
                (Key::A, _, _) => self.select_brush_group(crate::brushes::BrushGroup::Airbrush),
                (Key::E, _, _) => self.select_brush_group(crate::brushes::BrushGroup::Eraser),
                (Key::H, _, _) => self.navigation_tool = 1,
                (Key::I, _, _) => self.navigation_tool = 2,
                (Key::OpenBracket, _, _) => self.brush.size = (self.brush.size / 1.2).max(1.),
                (Key::CloseBracket, _, _) => self.brush.size = (self.brush.size * 1.2).min(512.),
                (Key::Tab, _, _) => self.hide_panels = !self.hide_panels,
                (Key::F1, _, _) => self.show_help = !self.show_help,
                _ => {}
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unused_control_combinations_fall_through_and_registered_ones_win() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
        let ctx = egui::Context::default();
        let frame = |app: &mut Parfait, key, shift| {
            let modifiers = egui::Modifiers {
                ctrl: true,
                command: true,
                shift,
                ..Default::default()
            };
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![
                        egui::Event::ModifiersChanged(modifiers),
                        egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers,
                        },
                    ],
                    ..Default::default()
                },
                |ui| app.shortcuts(ui.ctx()),
            );
            output.textures_delta.clear();
        };
        frame(&mut app, Key::E, false);
        assert!(app.brush.erase);
        frame(&mut app, Key::B, false);
        assert!(!app.brush.erase);
        let size = app.brush.size;
        frame(&mut app, Key::CloseBracket, false);
        assert!((app.brush.size - size * 1.2).abs() < 1e-5);
        frame(&mut app, Key::Tab, false);
        assert!(app.hide_panels);
        let layers = app.doc.layers.len();
        frame(&mut app, Key::N, true);
        assert_eq!(app.doc.layers.len(), layers + 1);
        frame(&mut app, Key::Z, false);
        assert_eq!(app.doc.layers.len(), layers);
    }
}
