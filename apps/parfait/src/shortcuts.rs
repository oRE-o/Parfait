use crate::app::Parfait;
use eframe::egui::{self, Key};
pub const HELP: &[&str] = &[
    "B / P    Brush       E    Eraser",
    "[ / ]    Smaller / larger brush",
    "Space + drag    Pan canvas",
    "Shift + Space + drag    Rotate view",
    "Ctrl + Space + drag    Zoom view",
    "Mouse wheel    Zoom at cursor",
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
        if ctx.egui_wants_keyboard_input()
            || self.stroke.is_some()
            || self.confirm_new
            || self.error.is_some()
        {
            return;
        }
        let i = ctx.input(|i| i.clone());
        let ctrl = i.modifiers.command;
        let shift = i.modifiers.shift;
        if ctrl {
            if i.key_pressed(Key::Z) {
                if shift { self.redo() } else { self.undo() }
            }
            if i.key_pressed(Key::Y) {
                self.redo()
            }
            if i.key_pressed(Key::S) {
                self.save_project(shift)
            }
            if i.key_pressed(Key::O) {
                self.open_project()
            }
            if i.key_pressed(Key::N) {
                if shift {
                    self.edit(|d| d.add_layer())
                } else {
                    self.confirm_new = true
                }
            }
            if shift && i.key_pressed(Key::E) {
                self.export()
            }
            if i.key_pressed(Key::Num0) {
                self.view = Default::default();
            }
        } else {
            if i.key_pressed(Key::B) || i.key_pressed(Key::P) {
                self.brush.erase = false;
                self.navigation_tool = 0;
            }
            if i.key_pressed(Key::E) {
                self.brush.erase = true;
                self.navigation_tool = 0;
            }
            if i.key_pressed(Key::OpenBracket) {
                self.brush.size = (self.brush.size / 1.2).max(1.);
            }
            if i.key_pressed(Key::CloseBracket) {
                self.brush.size = (self.brush.size * 1.2).min(512.);
            }
            if i.key_pressed(Key::Tab) {
                self.hide_panels = !self.hide_panels;
            }
        }
        if i.key_pressed(Key::F1) {
            self.show_help = !self.show_help;
        }
    }
}
