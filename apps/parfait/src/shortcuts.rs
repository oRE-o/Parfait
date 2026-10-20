use crate::app::Parfait;
use eframe::egui::{self, Key};
mod commands;
mod editor;
mod keymap;
pub use commands::{Category, Command};
pub use editor::ShortcutEditor;
pub use keymap::{Chord, Keymap};
pub const HELP: &[&str] = &[
    "Fixed pointer gestures (not key bindings):",
    "Space + drag    Pan canvas",
    "Shift + Space + drag    Rotate view",
    "Ctrl + Space + drag    Zoom view",
    "Mouse wheel / Ctrl + wheel    Zoom at cursor",
    "Alt + click    Pick color",
    "Escape    Close guide / settings (cancel shortcut draft)",
];
impl Parfait {
    pub fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.shortcut_editor.is_some() {
            return;
        }
        if self.show_help || self.show_licenses || self.show_display_settings {
            if ctx.input(|input| input.key_pressed(Key::Escape)) {
                self.show_help = false;
                self.show_licenses = false;
                self.show_display_settings = false;
            } else if self.show_help
                && ctx.input(|input| {
                    input.events.iter().any(|event| {
                        if let egui::Event::Key {
                            key,
                            pressed: true,
                            repeat: false,
                            modifiers,
                            ..
                        } = event
                        {
                            self.keymap.resolve(
                                Chord::from_event(*key, *modifiers),
                                None,
                            ) == Some(Command::ShortcutGuide)
                        } else {
                            false
                        }
                    })
                })
            {
                self.show_help = false;
            }
            return;
        }
        if self.dialog_open()
            || ctx.egui_wants_keyboard_input()
            || self.documents.session.editing.stroke.is_some()
            || self.documents.session.layer_ui.list.owns_pointer()
        {
            return;
        }
        let keys: Vec<_> = ctx.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| {
                    if let egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        repeat,
                        ..
                    } = event
                    {
                        Some((*key, *modifiers, *repeat))
                    } else {
                        None
                    }
                })
                .collect()
        });
        for (key, modifiers, repeat) in keys {
            if let Some(command) = self.keymap.resolve(
                Chord::from_event(key, modifiers),
                self.active_tool_command(),
            ) && (!repeat || command.repeatable())
            {
                self.execute_command(command, ctx);
            }
            if self.dialog_open() {
                break;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_keys_respect_text_focus_and_preserve_artwork() {
        let directory = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(directory.path().to_owned()).unwrap();
        app.documents.session.document = parfait_core::Document::new(8, 8);
        let original = app.documents.session.document.layers.clone();
        let context = egui::Context::default();
        let primary =
            egui::Modifiers { ctrl: true, command: true, ..Default::default() };
        key_frame(&mut app, &context, Key::A, primary, false, false);
        assert_eq!(
            app.documents.session.document.selection,
            Some(parfait_core::MaskCoverage::Uniform(255))
        );
        key_frame(&mut app, &context, Key::D, primary, false, false);
        assert!(app.documents.session.document.selection.is_none());
        app.documents
            .session
            .document
            .set_selection(Some(parfait_core::MaskCoverage::Uniform(64)));
        key_frame(
            &mut app,
            &context,
            Key::I,
            egui::Modifiers { shift: true, ..primary },
            false,
            false,
        );
        assert_eq!(
            app.documents.session.document.selection,
            Some(parfait_core::MaskCoverage::Uniform(191))
        );
        key_frame(&mut app, &context, Key::D, primary, false, true);
        assert!(context.egui_wants_keyboard_input());
        assert_eq!(
            app.documents.session.document.selection,
            Some(parfait_core::MaskCoverage::Uniform(191))
        );
        assert_eq!(app.documents.session.document.layers, original);
    }

    fn key_frame(
        app: &mut Parfait,
        ctx: &egui::Context,
        key: Key,
        modifiers: egui::Modifiers,
        repeat: bool,
        focus: bool,
    ) {
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![
                    egui::Event::ModifiersChanged(modifiers),
                    egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat,
                        modifiers,
                    },
                ],
                ..Default::default()
            },
            |ui| {
                if focus {
                    ui.text_edit_singleline(&mut String::new()).request_focus();
                }
                app.shortcuts(ui.ctx());
            },
        );
        output.textures_delta.clear();
    }

    #[test]
    fn view_shortcuts_respect_repeat_text_stroke_and_modal_ownership() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        let ctx = egui::Context::default();
        let shift = egui::Modifiers { shift: true, ..Default::default() };
        let control =
            egui::Modifiers { ctrl: true, command: true, ..Default::default() };
        key_frame(&mut app, &ctx, Key::H, shift, false, false);
        assert!(app.documents.session.view.flip_horizontal);
        assert_eq!(app.navigation_tool, 0);
        key_frame(&mut app, &ctx, Key::H, shift, true, false);
        assert!(app.documents.session.view.flip_horizontal);
        key_frame(&mut app, &ctx, Key::V, shift, false, false);
        assert!(app.documents.session.view.flip_vertical);
        app.display.rotation_step = 15.;
        key_frame(&mut app, &ctx, Key::ArrowRight, shift, false, false);
        assert!(
            (app.documents.session.view.angle.to_degrees() - 15.).abs() < 0.001
        );
        key_frame(&mut app, &ctx, Key::R, control, false, false);
        assert!(!app.display.rulers);
        key_frame(&mut app, &ctx, Key::R, control, true, false);
        assert!(!app.display.rulers);
        key_frame(
            &mut app,
            &ctx,
            Key::G,
            egui::Modifiers { shift: true, ..control },
            false,
            false,
        );
        assert!(app.display.pixel_grid);
        key_frame(&mut app, &ctx, Key::K, control, false, false);
        assert!(app.show_display_settings);
        key_frame(&mut app, &ctx, Key::H, shift, false, false);
        assert!(app.documents.session.view.flip_horizontal);
        key_frame(
            &mut app,
            &ctx,
            Key::Escape,
            Default::default(),
            false,
            false,
        );
        assert!(!app.show_display_settings);
        for modal in 0..4 {
            app.show_help = modal == 0;
            app.show_licenses = modal == 1;
            app.confirm_new = modal == 2;
            app.error = (modal == 3).then(|| "test error".into());
            key_frame(&mut app, &ctx, Key::V, shift, false, false);
            assert!(app.documents.session.view.flip_vertical);
        }
        app.error = None;
        app.show_help = false;
        app.show_licenses = false;
        app.confirm_new = false;
        app.documents.session.editing.stroke =
            Some(parfait_core::Stroke::begin(
                &mut app.documents.session.document,
                app.brush.clone(),
                parfait_core::Point { x: 12., y: 12., pressure: 1. },
            ));
        key_frame(&mut app, &ctx, Key::H, shift, false, false);
        assert!(app.documents.session.view.flip_horizontal);
        app.documents.session.editing.stroke = None;
        key_frame(&mut app, &ctx, Key::H, shift, false, true);
        assert!(app.documents.session.view.flip_horizontal);
        assert!(ctx.egui_wants_keyboard_input());
    }
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
        let layers = app.documents.session.document.layers.len();
        frame(&mut app, Key::N, true);
        assert_eq!(app.documents.session.document.layers.len(), layers + 1);
        frame(&mut app, Key::Z, false);
        assert_eq!(app.documents.session.document.layers.len(), layers);
    }
}
