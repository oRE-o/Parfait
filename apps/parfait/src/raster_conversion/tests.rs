use super::*;
use crate::shortcuts::Chord;
use eframe::egui;
use parfait_core::{Brush, Document, LayerMask, MaskCoverage, Point};
use std::sync::Arc;

fn key(pressed: bool, repeat: bool) -> egui::Event {
    egui::Event::Key {
        key: egui::Key::F6,
        physical_key: None,
        pressed,
        repeat,
        modifiers: Default::default(),
    }
}

fn frame(app: &mut Parfait, context: &egui::Context, events: Vec<egui::Event>, focus: bool) {
    let mut output = context.run_ui(
        egui::RawInput {
            events,
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
fn conversions_are_active_only_undoable_and_durable_with_masks() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.doc = Document::new(8, 8);
    app.doc.layers[1].pixels = Arc::new([128, 128, 128, 128].repeat(64));
    app.doc.layers[1].mask = Some(LayerMask {
        enabled: false,
        coverage: MaskCoverage::Dense(Arc::new(vec![100; 64])),
    });
    app.set_layer_selection(3, 1);
    app.brush.color = [35, 180, 90];
    app.brush.opacity = 0.;
    app.brush.erase = true;
    let context = egui::Context::default();
    let before = app.doc.clone();
    app.execute_command(Command::BrightnessToOpacity, &context);
    assert_eq!(
        app.doc.layers[1].pixels.as_slice(),
        [0, 0, 0, 64].repeat(64)
    );
    assert_eq!(app.doc.layers[0], before.layers[0]);
    assert_eq!(app.doc.layers[1].mask, before.layers[1].mask);
    assert_eq!(app.selected_layer_mask(), 3);
    assert_eq!(app.history.rows().count(), 2);
    let extracted = app.doc.clone();
    app.undo();
    assert!(app.doc.same_edit_state(&before));
    app.redo();
    assert!(app.doc.same_edit_state(&extracted));
    app.doc.layers[1].alpha_locked = true;
    app.execute_command(Command::ConvertToDrawingColor, &context);
    assert_eq!(
        app.doc.layers[1].pixels.as_slice(),
        [35, 180, 90, 64].repeat(64)
    );
    assert_eq!(app.doc.layers[1].mask, before.layers[1].mask);
    let converted = app.doc.clone();
    app.force_save();
    drop(app);
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    assert_eq!(app.doc, converted);
    assert!(
        app.command_disabled_reason(Command::BrightnessToOpacity)
            .is_some()
    );
    assert!(
        app.command_disabled_reason(Command::ConvertToDrawingColor)
            .is_none()
    );
    app.execute_command(Command::ConvertToDrawingColor, &context);
    assert!(app.doc.layers[1].alpha_locked);
}

#[test]
fn guards_refuse_mask_and_locked_targets_or_owned_gestures_without_edits() {
    for command in [Command::BrightnessToOpacity, Command::ConvertToDrawingColor] {
        for blocker in 0..6 {
            let directory = tempfile::tempdir().unwrap();
            let mut app = Parfait::open(directory.path().to_owned()).unwrap();
            app.doc = Document::new(32, 32);
            app.doc.layers[1].pixels = Arc::new([160, 120, 80, 255].repeat(1024));
            app.doc.create_layer_mask(1, 127);
            app.confirm_new = blocker == 0;
            app.workspace.resizing = blocker == 1;
            app.doc.layers[1].locked = blocker == 2;
            if blocker == 3 {
                app.select_layer_target(1, true);
            }
            if blocker == 4 {
                app.brush = Brush {
                    size: 8.,
                    pressure: false,
                    ..Default::default()
                };
                assert!(app.begin_canvas_stroke(Point {
                    x: 16.,
                    y: 16.,
                    pressure: 1.
                }));
            }
            if blocker == 5 {
                app.doc.layers[1].alpha_locked = command == Command::BrightnessToOpacity;
                app.doc.layers[1].locked = command == Command::ConvertToDrawingColor;
            }
            let before = app.doc.clone();
            let rows = app.history.rows().count();
            let context = egui::Context::default();
            assert!(app.command_disabled_reason(command).is_some());
            app.execute_command(command, &context);
            assert_eq!(app.doc, before);
            assert!(Arc::ptr_eq(
                &app.doc.layers[1].pixels,
                &before.layers[1].pixels
            ));
            assert_eq!(app.history.rows().count(), rows);
        }
    }
}

#[test]
fn conversion_shortcut_ignores_repeat_and_text_focus_and_noop_keeps_redo() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.doc = Document::new(8, 8);
    app.doc.layers[1].pixels = Arc::new([60, 50, 40, 200].repeat(64));
    app.keymap
        .assign(
            Command::ConvertToDrawingColor,
            None,
            Chord::new(egui::Key::F6, false, false, false),
            false,
        )
        .unwrap();
    let context = egui::Context::default();
    let before = app.doc.clone();
    frame(&mut app, &context, vec![key(true, false)], true);
    assert_eq!(app.doc, before);
    frame(&mut app, &context, vec![key(false, false)], false);
    context.memory_mut(|memory| {
        if let Some(focus) = memory.focused() {
            memory.surrender_focus(focus);
        }
    });
    frame(&mut app, &context, vec![key(true, false)], false);
    let after = app.doc.clone();
    assert!(!after.same_edit_state(&before));
    app.brush.color = [200, 180, 20];
    frame(&mut app, &context, vec![key(true, true)], false);
    assert_eq!(app.doc, after);
    frame(&mut app, &context, vec![key(false, false)], false);
    app.undo();
    assert!(app.history.can_redo());
    app.brush.color = [60, 50, 40];
    let unchanged = app.doc.clone();
    frame(&mut app, &context, vec![key(true, false)], false);
    assert!(app.history.can_redo());
    assert_eq!(app.doc, unchanged);
    assert!(Arc::ptr_eq(
        &app.doc.layers[1].pixels,
        &unchanged.layers[1].pixels
    ));
}

#[test]
fn version_seventeen_keys_upgrade_and_new_conversion_binding_reopens() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.keymap
        .assign(
            Command::LayerVisibility,
            None,
            Chord::new(egui::Key::F6, false, false, false),
            false,
        )
        .unwrap();
    app.persist_preferences(true);
    let mut legacy: serde_json::Value =
        serde_json::from_str(&app.saved_preferences_json()).unwrap();
    assert_eq!(legacy["version"], 21);
    legacy["version"] = 17.into();
    parfait_storage::save_preferences(
        &directory.path().join("preferences.sqlite"),
        &legacy.to_string(),
    )
    .unwrap();
    drop(app);
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    assert!(!app.preferences_read_failed);
    assert_eq!(
        app.keymap
            .resolve(Chord::new(egui::Key::F6, false, false, false), None),
        Some(Command::LayerVisibility)
    );
    assert!(app.keymap.hint(Command::ConvertToDrawingColor).is_empty());
    assert!(app.keymap.hint(Command::BrightnessToOpacity).is_empty());
    app.keymap
        .assign(
            Command::BrightnessToOpacity,
            None,
            Chord::new(egui::Key::F7, false, false, false),
            false,
        )
        .unwrap();
    app.persist_preferences(true);
    drop(app);
    let app = Parfait::open(directory.path().to_owned()).unwrap();
    assert_eq!(app.keymap.hint(Command::BrightnessToOpacity), "F7");
    assert_eq!(app.preferences_saved.as_ref().unwrap().version, 21);
}
