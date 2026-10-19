use super::{Category, Chord, Command, Keymap};
use crate::{app::Parfait, theme::ACCENT, widgets};
use eframe::egui::{self, Key};

pub struct ShortcutEditor {
    pub draft: Keymap,
    pub query: String,
    pub category: Option<Category>,
    pub selected: Command,
    pub slot: usize,
    capture: Option<Capture>,
    message: Option<String>,
}
struct Capture {
    old: Option<Chord>,
    candidate: Option<Chord>,
}
enum Outcome {
    Keep,
    Apply,
    Cancel,
}

impl ShortcutEditor {
    pub fn new(keymap: &Keymap) -> Self {
        Self {
            draft: keymap.clone(),
            query: String::new(),
            category: None,
            selected: Command::Undo,
            slot: 0,
            capture: None,
            message: None,
        }
    }
    fn begin_capture(&mut self, old: Option<Chord>) {
        self.capture = Some(Capture {
            old,
            candidate: None,
        });
        self.message = None;
    }
    fn assign_candidate(&mut self, replace: bool) {
        let Some(Capture {
            old,
            candidate: Some(candidate),
        }) = self.capture
        else {
            return;
        };
        match self.draft.assign(self.selected, old, candidate, replace) {
            Ok(()) => {
                self.slot = self
                    .draft
                    .chords(self.selected)
                    .position(|chord| chord == candidate)
                    .unwrap_or(0);
                self.capture = None;
                self.message = None;
            }
            Err(error) => self.message = Some(error.label().into()),
        }
    }
    fn ui(&mut self, ctx: &egui::Context, active: &Keymap, enabled: bool) -> Outcome {
        let mut outcome = Outcome::Keep;
        if enabled {
            if ctx.input(|input| input.key_pressed(Key::Escape)) {
                if self.capture.take().is_none() {
                    return Outcome::Cancel;
                }
                self.message = None;
            } else if self.capture.is_some() {
                if let Some(chord) = ctx.input(|input| {
                    input.events.iter().find_map(|event| {
                        if let egui::Event::Key {
                            key,
                            pressed: true,
                            repeat: false,
                            modifiers,
                            ..
                        } = event
                            && !matches!(key, Key::Enter | Key::Escape)
                        {
                            Some(Chord::from_event(*key, *modifiers))
                        } else {
                            None
                        }
                    })
                }) {
                    self.capture.as_mut().unwrap().candidate = Some(chord);
                    self.message = None;
                }
                if ctx.input(|input| input.key_pressed(Key::Enter))
                    && self
                        .capture
                        .as_ref()
                        .and_then(|capture| capture.candidate)
                        .is_some_and(|chord| !self.draft.conflicts(self.selected, chord))
                {
                    self.assign_candidate(false);
                }
            }
        }
        let mut open = true;
        egui::Window::new("Shortcut settings")
            .open(&mut open).collapsible(false).default_width(650.).min_width(560.)
            .default_pos(ctx.content_rect().center() - egui::vec2(325., 260.))
            .show(ctx, |ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    ui.add_enabled_ui(self.capture.is_none(), |ui| {
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt("shortcut_category")
                                .width(125.).selected_text(self.category.map_or("All commands", Category::label))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.category, None, "All commands");
                                    for &category in Category::ALL {
                                        ui.selectable_value(&mut self.category, Some(category), category.label());
                                    }
                                });
                            ui.add(egui::TextEdit::singleline(&mut self.query).hint_text("Search commands").char_limit(128).desired_width(210.));
                            ui.label(if self.draft == *active { "Saved bindings" } else { "Unsaved draft" });
                        });
                        ui.separator();
                        let query = self.query.trim().to_ascii_lowercase();
                        let width = (ui.available_width() - 162.).max(240.);
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(width);
                                egui::ScrollArea::vertical().id_salt("shortcut_commands")
                                    .min_scrolled_height(80.).max_height((ctx.content_rect().height() - 400.).clamp(80., 240.))
                                    .auto_shrink([false, false]).show(ui, |ui| {
                                        for &command in Command::ALL {
                                            if self.category.is_some_and(|category| category != command.category())
                                                || (!query.is_empty() && !command.label().to_ascii_lowercase().contains(&query)) { continue }
                                            let response = ui.add_sized([ui.available_width(), 25.], egui::Button::selectable(self.selected == command, command.label())
                                                .shortcut_text(self.draft.summary(command)).truncate());
                                            #[cfg(test)]
                                            ui.data_mut(|data| data.insert_temp(egui::Id::new(("shortcut_command", command)), response.rect));
                                            if response.clicked() { self.selected = command; self.slot = 0; self.message = None; }
                                        }
                                    });
                            });
                            ui.vertical(|ui| {
                                ui.set_width(150.);
                                ui.label(egui::RichText::new(self.selected.label()).color(ACCENT));
                                let chords: Vec<_> = self.draft.chords(self.selected).collect();
                                self.slot = self.slot.min(chords.len().saturating_sub(1));
                                if chords.is_empty() { ui.weak("Unassigned"); }
                                for (index, chord) in chords.iter().enumerate() {
                                    let label = chord.label();
                                    if ui.add_sized([150., 22.], egui::Button::selectable(self.slot == index, &label).truncate()).on_hover_text(label).clicked() { self.slot = index; }
                                }
                                ui.separator();
                                let selected = chords.get(self.slot).copied();
                                ui.add_enabled_ui(selected.is_some(), |ui| {
                                    if widgets::action_button(ui, "Edit shortcut", 150.).clicked() { self.begin_capture(selected); }
                                });
                                ui.add_enabled_ui(chords.len() < 4, |ui| {
                                    if widgets::action_button(ui, "Add shortcut", 150.).clicked() { self.begin_capture(None); }
                                });
                                ui.add_enabled_ui(selected.is_some(), |ui| {
                                    if widgets::action_button(ui, "Remove shortcut", 150.).clicked()
                                        && let Some(chord) = selected {
                                        self.draft.remove(self.selected, chord); self.message = None;
                                    }
                                });
                            });
                        });
                    });
                    ui.separator();
                    if let Some(capture) = &self.capture {
                        ui.label("Press a shortcut. Enter assigns a free key; Escape cancels capture.");
                        let candidate = capture.candidate;
                        ui.label(egui::RichText::new(candidate.map_or_else(|| "Waiting for a key…".into(), Chord::label)).size(16.).color(ACCENT));
                        let conflict = candidate.is_some_and(|chord| self.draft.conflicts(self.selected, chord));
                        if let Some(chord) = candidate {
                            let owners = self.draft.owners(chord).filter(|owner| *owner != self.selected).map(Command::label).collect::<Vec<_>>().join(", ");
                            if !chord.validate() { ui.colored_label(egui::Color32::LIGHT_RED, "Reserved key. Choose another combination."); }
                            else if conflict { ui.colored_label(egui::Color32::LIGHT_RED, format!("Already used by {owners}. Replacing removes that assignment.")); }
                            else if !owners.is_empty() { ui.label(format!("Shared tool key: {owners}. Repeated presses cycle tools.")); }
                        }
                        widgets::action_row(ui, |ui| {
                            if widgets::action_button(ui, "Cancel capture", 110.).clicked() { self.capture = None; self.message = None; }
                            ui.add_enabled_ui(candidate.is_some_and(Chord::validate), |ui| {
                                if widgets::action_button(ui, if conflict { "Replace assignment" } else { "Assign" }, 150.).clicked() { self.assign_candidate(conflict); }
                            });
                        });
                    } else {
                        ui.label("Select a command, then edit, add or remove an assignment. Up to four keys per command.");
                        ui.weak("Space gestures, Escape and Enter stay reserved. OS shortcuts may be intercepted. Tool keys may be shared to cycle tools.");
                    }
                    if let Some(message) = &self.message { ui.colored_label(egui::Color32::LIGHT_RED, message); }
                    ui.separator();
                    widgets::action_row(ui, |ui| {
                        ui.add_enabled_ui(self.capture.is_none() && self.draft.validate(), |ui| {
                            if widgets::action_button(ui, "Apply & close", 110.).clicked() { outcome = Outcome::Apply; }
                        });
                        if widgets::action_button(ui, "Cancel", 70.).clicked() { outcome = Outcome::Cancel; }
                        ui.add_enabled_ui(self.capture.is_none(), |ui| {
                            if widgets::action_button(ui, "Restore all defaults", 140.).clicked() { self.draft = Default::default(); self.slot = 0; self.message = None; }
                        });
                    });
                });
            });
        if self.capture.is_some() {
            ctx.memory_mut(|memory| {
                if let Some(focused) = memory.focused() {
                    memory.surrender_focus(focused);
                }
            });
        }
        if !open { Outcome::Cancel } else { outcome }
    }
}

impl Parfait {
    pub fn open_shortcut_settings(&mut self) {
        self.show_help = false;
        self.shortcut_editor = Some(ShortcutEditor::new(&self.keymap));
    }
    pub fn shortcut_settings_ui(&mut self, ctx: &egui::Context) {
        let Some(mut editor) = self.shortcut_editor.take() else {
            return;
        };
        match editor.ui(ctx, &self.keymap, self.error.is_none()) {
            Outcome::Cancel => {}
            Outcome::Keep => self.shortcut_editor = Some(editor),
            Outcome::Apply => {
                if self.preferences_read_failed {
                    editor.message = Some("The existing preferences file is protected after a load error. No bindings were applied.".into());
                    self.shortcut_editor = Some(editor);
                    return;
                }
                let previous = std::mem::replace(&mut self.keymap, editor.draft.clone());
                self.persist_preferences(true);
                if !self
                    .preferences_saved
                    .as_ref()
                    .is_some_and(|saved| saved.matches(self))
                {
                    self.keymap = previous;
                    editor.message = Some("Saving failed. Previous bindings remain active; the draft is retained for retry.".into());
                    self.shortcut_editor = Some(editor);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_frame(app: &mut Parfait, ctx: &egui::Context, events: Vec<egui::Event>) {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(960., 640.),
                )),
                events,
                ..Default::default()
            },
            |ui| app.shortcut_settings_ui(ui.ctx()),
        );
        output.textures_delta.clear();
    }
    fn action(app: &mut Parfait, ctx: &egui::Context, label: &str) {
        app_frame(app, ctx, vec![]);
        app_frame(app, ctx, vec![]);
        let rect = ctx
            .data(|data| data.get_temp::<egui::Rect>(egui::Id::new(("action_button", label))))
            .unwrap();
        assert!(ctx.content_rect().contains_rect(rect));
        let event = |pressed| egui::Event::PointerButton {
            pos: rect.center(),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        app_frame(
            app,
            ctx,
            vec![egui::Event::PointerMoved(rect.center()), event(true)],
        );
        app_frame(app, ctx, vec![event(false)]);
    }
    #[test]
    fn apply_cancel_and_failed_save_keep_durable_keymap_and_artwork() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.persist_preferences(true);
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let artwork = app.doc.clone();
        let old = app.keymap.clone();
        let chord = Chord::new(Key::F8, false, false, false);
        let draft = |app: &mut Parfait| {
            app.open_shortcut_settings();
            app.shortcut_editor
                .as_mut()
                .unwrap()
                .draft
                .assign(Command::Undo, None, chord, false)
                .unwrap();
        };
        draft(&mut app);
        action(&mut app, &ctx, "Cancel");
        assert!(app.shortcut_editor.is_none());
        assert!(app.keymap == old);
        draft(&mut app);
        let path = dir.path().join("preferences.sqlite");
        let backup = dir.path().join("preferences.backup");
        std::fs::rename(&path, &backup).unwrap();
        std::fs::create_dir(&path).unwrap();
        action(&mut app, &ctx, "Apply & close");
        assert!(app.error.take().is_some());
        assert!(app.shortcut_editor.is_some());
        assert!(app.keymap == old);
        std::fs::remove_dir(&path).unwrap();
        std::fs::rename(&backup, &path).unwrap();
        action(&mut app, &ctx, "Apply & close");
        assert!(app.error.is_none());
        assert!(app.shortcut_editor.is_none());
        assert_eq!(app.keymap.resolve(chord, None), Some(Command::Undo));
        assert_eq!(app.doc, artwork);
        drop(app);
        let app = Parfait::open(dir.path().to_owned()).unwrap();
        assert_eq!(app.keymap.resolve(chord, None), Some(Command::Undo));
    }
    #[test]
    fn command_list_and_capture_footer_do_not_expand_each_frame() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.open_shortcut_settings();
        let editor = app.shortcut_editor.as_mut().unwrap();
        editor.begin_capture(Some(Chord::new(Key::Z, true, false, false)));
        editor.capture.as_mut().unwrap().candidate = Some(Chord::new(Key::Y, true, false, false));
        let footer = || {
            ctx.data(|data| {
                data.get_temp::<egui::Rect>(egui::Id::new(("action_button", "Apply & close")))
            })
            .unwrap()
        };
        for _ in 0..10 {
            app_frame(&mut app, &ctx, vec![]);
        }
        let first = footer();
        for _ in 0..40 {
            app_frame(&mut app, &ctx, vec![]);
        }
        let last = footer();
        assert!(
            (first.min - last.min).length() < 0.1,
            "{first:?} -> {last:?}"
        );
        assert!(ctx.content_rect().contains_rect(last));
    }

    fn frame(
        editor: &mut ShortcutEditor,
        ctx: &egui::Context,
        key: Key,
        modifiers: egui::Modifiers,
    ) -> Outcome {
        let mut outcome = Outcome::Keep;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(960., 640.),
                )),
                events: vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                }],
                ..Default::default()
            },
            |ui| {
                outcome = editor.ui(ui.ctx(), &Keymap::default(), true);
            },
        );
        output.textures_delta.clear();
        outcome
    }

    #[test]
    fn capture_enter_never_replaces_conflicts_and_escape_cancels_child_first() {
        let ctx = egui::Context::default();
        let active = Keymap::default();
        let mut editor = ShortcutEditor::new(&active);
        editor.begin_capture(Some(Chord::new(Key::Z, true, false, false)));
        let primary = egui::Modifiers {
            command: true,
            ..Default::default()
        };
        frame(&mut editor, &ctx, Key::Y, primary);
        frame(&mut editor, &ctx, Key::Enter, Default::default());
        assert!(editor.capture.is_some());
        assert!(editor.draft == active);
        assert!(matches!(
            frame(&mut editor, &ctx, Key::Escape, Default::default()),
            Outcome::Keep
        ));
        assert!(editor.capture.is_none());
        assert!(matches!(
            frame(&mut editor, &ctx, Key::Escape, Default::default()),
            Outcome::Cancel
        ));
        editor.begin_capture(Some(Chord::new(Key::Z, true, false, false)));
        frame(&mut editor, &ctx, Key::F8, Default::default());
        frame(&mut editor, &ctx, Key::Enter, Default::default());
        assert!(editor.capture.is_none());
        assert_eq!(
            editor
                .draft
                .resolve(Chord::new(Key::F8, false, false, false), None),
            Some(Command::Undo)
        );
        assert!(active != editor.draft);
    }
}
